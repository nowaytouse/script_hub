use std::path::Path;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::io;

/// Native macOS `copyfile` flag constants
/// Corresponds to <copyfile.h>
/// COPYFILE_ACL | COPYFILE_STAT | COPYFILE_XATTR | COPYFILE_NOFOLLOW
const COPYFILE_FLAGS: u32 = (1<<0) | (1<<1) | (1<<2) | (1<<3);

/// Uses macOS native `copyfile` API to clone ALL metadata relative to security and fs context.
/// This includes:
/// - ACLs (Access Control Lists)
/// - BSD File Flags (uchg, hidden, etc.)
/// - Extended Attributes (All xattrs including resource forks)
/// - Creation Date / Added Date / Modification Date / Access Date
/// - Mode / Permissions
pub fn copy_native_metadata(src: &Path, dst: &Path) -> io::Result<()> {
    extern "C" {
        fn copyfile(from: *const i8, to: *const i8, state: *mut std::ffi::c_void, flags: u32) -> i32;
    }

    let src_c = CString::new(src.as_os_str().as_bytes())?;
    let dst_c = CString::new(dst.as_os_str().as_bytes())?;

    // COPYFILE_METADATA wrapper
    let ret = unsafe {
        copyfile(src_c.as_ptr(), dst_c.as_ptr(), std::ptr::null_mut(), COPYFILE_FLAGS)
    };

    if ret < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
