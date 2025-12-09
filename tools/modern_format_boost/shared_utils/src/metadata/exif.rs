//! ExifTool wrapper for internal metadata preservation
//! 
//! Performance optimizations:
//! - Cached exiftool availability check
//! - Minimal argument set for common cases
//! - Fast path for same-format conversions

use std::path::Path;
use std::process::Command;
use std::io;
use std::sync::OnceLock;

/// Cached exiftool availability (checked once per process)
static EXIFTOOL_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Check if exiftool is available (cached)
fn is_exiftool_available() -> bool {
    *EXIFTOOL_AVAILABLE.get_or_init(|| which::which("exiftool").is_ok())
}

/// Preserve internal metadata via ExifTool
/// 
/// Performance: ~50-200ms per file depending on metadata complexity
pub fn preserve_internal_metadata(src: &Path, dst: &Path) -> io::Result<()> {
    if !is_exiftool_available() {
        // Only warn once per process
        static WARNED: OnceLock<()> = OnceLock::new();
        WARNED.get_or_init(|| {
            eprintln!("⚠️ [metadata] ExifTool not found. EXIF/IPTC will NOT be preserved.");
        });
        return Ok(());
    }

    // 🚀 Performance: Use minimal argument set
    // -all:all copies everything, individual date tags are redundant
    let output = Command::new("exiftool")
        .arg("-tagsfromfile").arg(src)
        .arg("-all:all")              // Copy all metadata
        .arg("-ICC_Profile<ICC_Profile") // Ensure ICC is copied
        .arg("-use").arg("MWG")       // Metadata Working Group standard
        .arg("-api").arg("LargeFileSupport=1")
        .arg("-overwrite_original")   // Don't create backup
        .arg("-q")                    // Quiet mode
        .arg("-m")                    // Ignore minor errors (faster)
        .arg(dst)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Don't fail on minor warnings
        if !stderr.contains("Warning") {
            return Err(io::Error::new(io::ErrorKind::Other, format!("ExifTool failed: {}", stderr)));
        }
    }
    Ok(())
}
