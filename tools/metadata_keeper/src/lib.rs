use std::path::Path;
use std::io;

#[cfg(target_os = "macos")]
mod macos;

/// Preserves all possible metadata from source to destination.
/// 
/// This is the "Nuclear Option" that tries to clone:
/// 1. Native system attributes (ACL, Flags, Resource Fork, Creation Date) via platform APIs.
/// 2. Extended attributes (xattr).
/// 3. Standard timestamps (atime/mtime).
/// 
/// Note: Internal metadata (Exif/IPTC) inside the file content should be handled 
/// separately (e.g. via `exiftool`) before calling this, as this function focuses
/// on the file-system and OS level attributes.
pub fn preserve_metadata(src: &Path, dst: &Path) -> io::Result<()> {
    
    // 1. macOS Native "Nuclear" Copy
    // This handles ACL, Xattr, Flags, and Timestamps all in one go.
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = macos::copy_native_metadata(src, dst) {
            eprintln!("⚠️ [metadata_keeper] Failed to copy native macOS metadata: {}", e);
            // Fallthrough to standard methods if native fails? 
            // Usually if this fails, standard methods might also fail, but we can try.
        } else {
            // If native copy succeeded, we are mostly done.
            return Ok(());
        }
    }

    // 2. Standard Fallback / Cross-platform Logic
    // Access and Modification time (atime/mtime)
    if let Ok(metadata) = std::fs::metadata(src) {
        let atime = filetime::FileTime::from_last_access_time(&metadata);
        let mtime = filetime::FileTime::from_last_modification_time(&metadata);
        
        if let Err(e) = filetime::set_file_times(dst, atime, mtime) {
            eprintln!("⚠️ [metadata_keeper] Failed to set atime/mtime: {}", e);
        }

        // Permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _current_perms = std::fs::metadata(dst)?.permissions();
            let mode = metadata.permissions().mode();
            // Only update if changed (optional check)
            if let Err(e) = std::fs::set_permissions(dst, std::fs::Permissions::from_mode(mode)) {
                 eprintln!("⚠️ [metadata_keeper] Failed to set permissions: {}", e);
            }
        }
    }

    // 3. Manual Xattr Copy (Linux / Non-macOS or Fallback)
    #[cfg(not(target_os = "macos"))] 
    copy_xattrs_manual(src, dst);

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn copy_xattrs_manual(src: &Path, dst: &Path) {
    if let Ok(iter) = xattr::list(src) {
        for name in iter {
            if let Some(name_str) = name.to_str() {
                if let Ok(Some(value)) = xattr::get(src, name_str) {
                    let _ = xattr::set(dst, name_str, &value);
                }
            }
        }
    }
}
