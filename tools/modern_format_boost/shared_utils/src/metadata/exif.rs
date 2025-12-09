//! ExifTool wrapper for internal metadata preservation

use std::path::Path;
use std::process::Command;
use std::io;

/// Preserve internal metadata via ExifTool
pub fn preserve_internal_metadata(src: &Path, dst: &Path) -> io::Result<()> {
    if which::which("exiftool").is_err() {
        eprintln!("⚠️ [metadata] ExifTool not found. EXIF/IPTC will NOT be preserved.");
        return Ok(());
    }

    let output = Command::new("exiftool")
        .arg("-tagsfromfile").arg(src)
        .arg("-all:all")
        .arg("-FileCreateDate<FileCreateDate")
        .arg("-FileModifyDate<FileModifyDate")
        .arg("-CreationDate<CreationDate")
        .arg("-DateTimeOriginal<DateTimeOriginal")
        .arg("-CreateDate<CreateDate")
        .arg("-ModifyDate<ModifyDate")
        .arg("-SubSecTimeOriginal<SubSecTimeOriginal")
        .arg("-SubSecTimeDigitized<SubSecTimeDigitized")
        .arg("-GPSDateTime<GPSDateTime")
        .arg("-ICC_Profile<ICC_Profile")
        .arg("-MediaCreateDate<MediaCreateDate")
        .arg("-MediaModifyDate<MediaModifyDate")
        .arg("-TrackCreateDate<TrackCreateDate")
        .arg("-TrackModifyDate<TrackModifyDate")
        .arg("-AllDates")
        .arg("-use").arg("MWG")
        .arg("-api").arg("LargeFileSupport=1")
        .arg("-overwrite_original")
        .arg("-q")
        .arg(dst)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::new(io::ErrorKind::Other, format!("ExifTool failed: {}", stderr)));
    }
    Ok(())
}
