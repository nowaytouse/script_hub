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

// ===================================================================================
// Legacy / Precision Logic (Restored from previous implementation)
// Used to explicitly set Creation Time (btime) and Added Time if copyfile somehow misses it
// or for granular control.
// ===================================================================================

#[repr(C)]
struct attrlist {
    bitmapcount: u16,
    reserved: u16,
    commonattr: u32,
    volattr: u32,
    dirattr: u32,
    fileattr: u32,
    forkattr: u32,
}

const ATTR_CMN_CRTIME: u32 = 0x00000200;
const ATTR_CMN_ADDEDTIME: u32 = 0x10000000;
const ATTR_BIT_MAP_COUNT: u16 = 5;

#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

#[repr(C)]
struct AttrBufAddedTime {
    length: u32,
    added_time: Timespec,
}

pub fn set_creation_time(path: &Path, time: std::time::SystemTime) -> io::Result<()> {
    set_time_attr(path, time, ATTR_CMN_CRTIME)
}

pub fn set_added_time(path: &Path, time: std::time::SystemTime) -> io::Result<()> {
    set_time_attr(path, time, ATTR_CMN_ADDEDTIME)
}

pub fn get_added_time(path: &Path) -> io::Result<std::time::SystemTime> {
    extern "C" {
        fn getattrlist(
            path: *const i8,
            attrList: *mut attrlist,
            attrBuf: *mut std::ffi::c_void,
            attrBufSize: usize,
            options: u32,
        ) -> i32;
    }

    let c_path = CString::new(path.as_os_str().as_bytes())?;
    let mut attr_list = attrlist {
        bitmapcount: ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: ATTR_CMN_ADDEDTIME,
        volattr: 0,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    let mut buf = AttrBufAddedTime {
        length: 0,
        added_time: Timespec { tv_sec: 0, tv_nsec: 0 },
    };

    let ret = unsafe {
        getattrlist(
            c_path.as_ptr(),
            &mut attr_list,
            &mut buf as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<AttrBufAddedTime>(),
            0,
        )
    };

    if ret != 0 {
        return Err(io::Error::last_os_error());
    }

    let duration = std::time::Duration::new(
        buf.added_time.tv_sec as u64,
        buf.added_time.tv_nsec as u32,
    );
    Ok(std::time::SystemTime::UNIX_EPOCH + duration)
}

fn set_time_attr(path: &Path, time: std::time::SystemTime, attr: u32) -> io::Result<()> {
    extern "C" {
        fn setattrlist(
            path: *const i8,
            attrList: *mut attrlist,
            attrBuf: *mut std::ffi::c_void,
            attrBufSize: usize,
            options: u32,
        ) -> i32;
    }

    let c_path = CString::new(path.as_os_str().as_bytes())?;
    let mut attr_list = attrlist {
        bitmapcount: ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: attr,
        volattr: 0,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };

    let duration = time.duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let mut buf = Timespec {
        tv_sec: duration.as_secs() as i64,
        tv_nsec: duration.subsec_nanos() as i64,
    };

    let ret = unsafe {
        setattrlist(
            c_path.as_ptr(),
            &mut attr_list,
            &mut buf as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<Timespec>(),
            0,
        )
    };

    if ret != 0 {
         return Err(io::Error::last_os_error());
    }
    Ok(())
}
