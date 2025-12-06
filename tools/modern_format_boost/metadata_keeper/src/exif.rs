use std::path::Path;
use std::process::Command;
use std::io;

/// Wrapper for ExifTool to handle Internal Metadata (Layer 1-2)
/// 
/// This module encapsulates the logic for invoking `exiftool` to clone:
/// - EXIF, IPTC, XMP standard tags
/// - MakerNotes (Canon, Nikon, Sony private data)
/// - ICC Profiles
/// - QuickTime/MP4 specific dates
pub fn preserve_internal_metadata(src: &Path, dst: &Path) -> io::Result<()> {
    if which::which("exiftool").is_err() {
        eprintln!("⚠️ [metadata_keeper] ExifTool not found. Internal metadata (Exif/IPTC) will NOT be preserved.");
        return Ok(()); // Non-fatal, but meaningful
    }

    // Construct the command with the "Ultimate" set of flags derived from imgquality/vidquality
    // plus additional safety checks.
    let output = Command::new("exiftool")
        .arg("-tagsfromfile").arg(src)
        // 1. Basic Transfer
        .arg("-all:all")                          // All standard tags
        
        // 2. System Time Sync (Crucial for Finder continuity)
        // Note: These might be overwritten by file system timestamp preservation later, 
        // but it's good to have them embedded in XMP/Exif too.
        .arg("-FileCreateDate<FileCreateDate")    // System creation time
        .arg("-FileModifyDate<FileModifyDate")    // System modify time
        
        // 3. Apple/QuickTime Specifics
        .arg("-CreationDate<CreationDate")        // QuickTime/Mac creation
        .arg("-MacOS:all")                        // Force macOS specific tags if possible (though -tagsfromfile usually handles it)
        
        // 4. Photo Specifics
        .arg("-DateTimeOriginal<DateTimeOriginal") // Original capture time
        .arg("-CreateDate<CreateDate")            // File creation date
        .arg("-ModifyDate<ModifyDate")            // Content modify date
        .arg("-SubSecTimeOriginal<SubSecTimeOriginal")  // Sub-second precision
        .arg("-SubSecTimeDigitized<SubSecTimeDigitized")
        .arg("-SubSecTime<SubSecTime")
        .arg("-GPSDateTime<GPSDateTime")          // GPS timestamp
        .arg("-ICC_Profile<ICC_Profile")          // Color profile
        
        // 5. Video Specifics (from vidquality)
        .arg("-MediaCreateDate<MediaCreateDate")  // Video track creation
        .arg("-MediaModifyDate<MediaModifyDate")  // Video track modify
        .arg("-TrackCreateDate<TrackCreateDate")  // Track creation
        .arg("-TrackModifyDate<TrackModifyDate")  // Track modify

        // 6. Convenience & Robustness
        .arg("-AllDates")                         // Convenience: all date tags
        .arg("-use").arg("MWG")                   // Metadata Working Group compat
        .arg("-api").arg("LargeFileSupport=1")    // Support > 4GB files
        .arg("-overwrite_original")               // Don't create _original backup
        .arg("-q")                                // Quiet mode (less spam)
        .arg(dst)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::new(io::ErrorKind::Other, format!("ExifTool failed: {}", stderr)));
    }

    Ok(())
}
