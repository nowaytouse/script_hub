//! Metadata Preservation Module
//! 
//! Complete metadata preservation across all layers:
//! - Internal: EXIF/IPTC/XMP via ExifTool
//! - Network: WhereFroms, User Tags
//! - System: ACL, Flags, Xattr, Timestamps

use std::path::Path;
use std::io;

mod exif;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;
mod network;

pub use exif::preserve_internal_metadata;

/// Nuclear Preservation: The Ultimate Metadata Strategy
pub fn preserve_pro(src: &Path, dst: &Path) -> io::Result<()> {
    // Step 1: Internal Metadata (Exif, MakerNotes, ICC)
    if let Err(e) = exif::preserve_internal_metadata(src, dst) {
        eprintln!("⚠️ [metadata] Internal metadata failed: {}", e);
    }

    // Step 2: Network & User Context (Verification)
    let _ = network::verify_network_metadata(src, dst);

    // Step 3: System Layer
    #[cfg(target_os = "macos")]
    {
        if let Ok(metadata) = std::fs::metadata(src) {
            if let Ok(created) = metadata.created() {
                let _ = macos::set_creation_time(dst, created);
            }
            if let Ok(added) = macos::get_added_time(src) {
                let _ = macos::set_added_time(dst, added);
            }
        }
        if let Err(e) = macos::copy_native_metadata(src, dst) {
            eprintln!("⚠️ [metadata] macOS native copy failed: {}", e);
        } else {
            return Ok(());
        }
    }

    #[cfg(target_os = "linux")]
    { let _ = linux::preserve_linux_attributes(src, dst); }

    #[cfg(target_os = "windows")]
    { let _ = windows::preserve_windows_attributes(src, dst); }

    // Fallback: xattrs + timestamps
    #[cfg(not(target_os = "macos"))]
    copy_xattrs_manual(src, dst);

    if let Ok(metadata) = std::fs::metadata(src) {
        let atime = filetime::FileTime::from_last_access_time(&metadata);
        let mtime = filetime::FileTime::from_last_modification_time(&metadata);
        let _ = filetime::set_file_times(dst, atime, mtime);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode();
            let _ = std::fs::set_permissions(dst, std::fs::Permissions::from_mode(mode));
        }
    }
    Ok(())
}

/// Alias for preserve_pro
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
