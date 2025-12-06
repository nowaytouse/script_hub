use std::path::Path;
use std::io;

mod exif;
#[cfg(target_os = "macos")]
mod macos;
mod network;

/// **Nuclear Preservation**: The Ultimate Metadata Strategy
/// 
/// Orchestrates the preservation of metadata across all layers:
/// 1. **Internal Layer**: Exif/IPTC/XMP via `exiftool` (injects data into file content).
/// 2. **Network Layer**: Verifies preservation of `WhereFroms` and identifying tags.
/// 3. **System Layer**: The "Atomic Snapshot". Uses native OS APIs to overwite any
///    timestamp drifts caused by step 1, and clones ACLs, Flags, and Resource Forks.
///
/// This specific order is critical: ExifTool modifies the file (changing mtime/inode).
/// The System Layer (copyfile) must run LAST to restore the original timestamps and
/// file system attributes.
pub fn preserve_pro(src: &Path, dst: &Path) -> io::Result<()> {
    
    // Step 1: Internal Metadata (Exif, MakerNotes, ICC)
    // This modifies the destination file content!
    if let Err(e) = exif::preserve_internal_metadata(src, dst) {
        eprintln!("⚠️ [metadata_keeper] Internal metadata preservation failed: {}", e);
        // Continue? Yes, because we still want system metadata.
    }

    // Step 2: Network & User Context (Verification)
    // Metadata usually carried by xattrs, which Step 3 will handle, but we verify here.
    // (In future, this step could actively inject sidecar data if configured).
    let _ = network::verify_network_metadata(src, dst);

    // Step 3: System Layer "Nuclear Option" (macOS)
    // Transfers ACLs, Flags, Xattrs (including WhereFroms), Resource Forks, and Timestamps.
    // MUST BE LAST.
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = macos::copy_native_metadata(src, dst) {
            eprintln!("⚠️ [metadata_keeper] Failed to copy native macOS metadata: {}", e);
            // Fallback to standard flow
        } else {
            // Done. `copyfile` handles everything on macOS.
            return Ok(());
        }
    }

    // Step 3b: Standard Fallback (Linux/Windows)
    // A. Xattrs
    #[cfg(not(target_os = "macos"))] 
    copy_xattrs_manual(src, dst);

    // B. Timestamps & Permissions
    if let Ok(metadata) = std::fs::metadata(src) {
        let atime = filetime::FileTime::from_last_access_time(&metadata);
        let mtime = filetime::FileTime::from_last_modification_time(&metadata);
        
        if let Err(e) = filetime::set_file_times(dst, atime, mtime) {
            eprintln!("⚠️ [metadata_keeper] Failed to set atime/mtime: {}", e);
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode();
            let _ = std::fs::set_permissions(dst, std::fs::Permissions::from_mode(mode));
        }
    }

    Ok(())
}

// Keep legacy alias for now, or direct to pro
pub fn preserve_metadata(src: &Path, dst: &Path) -> io::Result<()> {
    preserve_pro(src, dst)
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
