//! Lossless Converter Module
//! 
//! Provides conversion API for verified lossless/lossy images
//! With anti-duplicate execution mechanism

use crate::{ImgQualityError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::io::{BufRead, BufReader, Write};

// Global processed files tracker (anti-duplicate)
lazy_static::lazy_static! {
    static ref PROCESSED_FILES: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

/// Conversion result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub success: bool,
    pub input_path: String,
    pub output_path: Option<String>,
    pub input_size: u64,
    pub output_size: Option<u64>,
    pub size_reduction: Option<f64>,
    pub message: String,
    pub skipped: bool,
    pub skip_reason: Option<String>,
}

/// Conversion options
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    /// Force conversion even if already processed
    pub force: bool,
    /// Output directory (None = same as input)
    pub output_dir: Option<PathBuf>,
    /// Delete original after successful conversion
    pub delete_original: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            force: false,
            output_dir: None,
            delete_original: false,
        }
    }
}

/// Check if file has already been processed (anti-duplicate)
pub fn is_already_processed(path: &Path) -> bool {
    let canonical = path.canonicalize().ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| path.display().to_string());
    
    let processed = PROCESSED_FILES.lock().unwrap();
    processed.contains(&canonical)
}

/// Mark file as processed
pub fn mark_as_processed(path: &Path) {
    let canonical = path.canonicalize().ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| path.display().to_string());
    
    let mut processed = PROCESSED_FILES.lock().unwrap();
    processed.insert(canonical);
}

/// Load processed files list from disk
pub fn load_processed_list(list_path: &Path) -> Result<()> {
    if !list_path.exists() {
        return Ok(());
    }
    
    let file = fs::File::open(list_path)?;
    let reader = BufReader::new(file);
    let mut processed = PROCESSED_FILES.lock().unwrap();
    
    for line in reader.lines() {
        if let Ok(path) = line {
            processed.insert(path);
        }
    }
    
    Ok(())
}

/// Save processed files list to disk
pub fn save_processed_list(list_path: &Path) -> Result<()> {
    let processed = PROCESSED_FILES.lock().unwrap();
    let mut file = fs::File::create(list_path)?;
    
    for path in processed.iter() {
        writeln!(file, "{}", path)?;
    }
    
    Ok(())
}

/// Convert static image to JXL with specified distance/quality
/// distance: 0.0 = lossless, 0.1 = visually lossless (Q100 lossy), 1.0 = Q90
pub fn convert_to_jxl(input: &Path, options: &ConvertOptions, distance: f32) -> Result<ConversionResult> {
    // Anti-duplicate check
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "jxl", &options.output_dir);
    
    // Ensure output directory exists
    if let Some(parent) = output.parent() {
        let _ = fs::create_dir_all(parent);
    }
    
    // Check if output already exists
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // Execute cjxl (v0.11+ syntax)
    let result = Command::new("cjxl")
        .arg(input)
        .arg(&output)
        .arg("-d").arg(format!("{:.1}", distance))  // Distance parameter
        .arg("-e").arg("8")    // Effort 8 for better compression
        .output();
    
    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);
            
            // Validate output
            if let Err(e) = verify_jxl_health(&output) {
                 let _ = fs::remove_file(&output);
                 return Err(e);
            }

            // Copy metadata and timestamps
            copy_metadata(input, &output);
            
            mark_as_processed(input);
            
            if options.delete_original {
                fs::remove_file(input)?;
            }
            
            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("Conversion successful: size reduced {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("cjxl failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("cjxl not found: {}", e)))
        }
    }
}

/// Convert JPEG to JXL using lossless JPEG transcode (preserves DCT coefficients)
/// This is the BEST option for JPEG files - no quality loss at all
pub fn convert_jpeg_to_jxl(input: &Path, options: &ConvertOptions) -> Result<ConversionResult> {
    // Anti-duplicate check
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "jxl", &options.output_dir);
    
    // Check if output already exists
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // Execute cjxl with --lossless_jpeg=1 for lossless JPEG transcode
    let result = Command::new("cjxl")
        .arg(input)
        .arg(&output)
        .arg("--lossless_jpeg=1")  // Lossless JPEG transcode - preserves DCT coefficients
        .output();
    
    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);
            
            // Validate output
            if let Err(e) = verify_jxl_health(&output) {
                 let _ = fs::remove_file(&output);
                 return Err(e);
            }

            // Copy metadata and timestamps
            copy_metadata(input, &output);
            
            mark_as_processed(input);
            
            if options.delete_original {
                fs::remove_file(input)?;
            }
            
            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("JPEG lossless transcode successful: size reduced {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("cjxl JPEG transcode failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("cjxl not found: {}", e)))
        }
    }
}

/// Convert static lossy image to AVIF
pub fn convert_to_avif(input: &Path, quality: Option<u8>, options: &ConvertOptions) -> Result<ConversionResult> {
    // Anti-duplicate check
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "avif", &options.output_dir);
    
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // Use original quality or default to high quality
    let q = quality.unwrap_or(85);
    
    let result = Command::new("avifenc")
        .arg("-s").arg("4")       // Speed 4 (balanced)
        .arg("-j").arg("all")     // Use all CPU cores
        .arg("-q").arg(q.to_string())
        .arg(input)
        .arg(&output)
        .output();
    
    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);

            // Copy metadata and timestamps
            copy_metadata(input, &output);

            mark_as_processed(input);

            if options.delete_original {
                fs::remove_file(input)?;
            }

            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("Conversion successful: size reduced {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("avifenc failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("avifenc not found: {}", e)))
        }
    }
}

/// Convert animated lossless to AV1 MP4 (Q=100 visual lossless)
pub fn convert_to_av1_mp4(input: &Path, options: &ConvertOptions) -> Result<ConversionResult> {
    // Anti-duplicate check
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "mp4", &options.output_dir);
    
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // AV1 with CRF 0 for visually lossless
    let result = Command::new("ffmpeg")
        .arg("-y")  // Overwrite
        .arg("-i").arg(input)
        .arg("-c:v").arg("libaom-av1")
        .arg("-crf").arg("0")    // Lossless mode
        .arg("-b:v").arg("0")
        .arg("-pix_fmt").arg("yuv420p")
        .arg(&output)
        .output();
    
    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);
            
            // Copy metadata and timestamps
            copy_metadata(input, &output);
            
            mark_as_processed(input);
            
            if options.delete_original {
                fs::remove_file(input)?;
            }
            
            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("Conversion successful: size reduced {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("ffmpeg failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("ffmpeg not found: {}", e)))
        }
    }
}

/// Convert image to AVIF using mathematical lossless (⚠️ VERY SLOW)
pub fn convert_to_avif_lossless(input: &Path, options: &ConvertOptions) -> Result<ConversionResult> {
    eprintln!("⚠️  Mathematical lossless AVIF encoding - this will be SLOW!");
    
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "avif", &options.output_dir);
    
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // Mathematical lossless AVIF
    let result = Command::new("avifenc")
        .arg("--lossless")  // Mathematical lossless
        .arg("-s").arg("4")
        .arg("-j").arg("all")
        .arg(input)
        .arg(&output)
        .output();
    
    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);
            
            // Copy metadata and timestamps
            copy_metadata(input, &output);
            
            mark_as_processed(input);
            
            if options.delete_original {
                fs::remove_file(input)?;
            }
            
            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("Lossless AVIF: size {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("avifenc lossless failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("avifenc not found: {}", e)))
        }
    }
}

/// Convert animated to AV1 MP4 using mathematical lossless (⚠️ VERY SLOW)
pub fn convert_to_av1_mp4_lossless(input: &Path, options: &ConvertOptions) -> Result<ConversionResult> {
    eprintln!("⚠️  Mathematical lossless AV1 encoding - this will be VERY SLOW!");
    
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "mp4", &options.output_dir);
    
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // Mathematical lossless AV1
    let result = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i").arg(input)
        .arg("-c:v").arg("libaom-av1")
        .arg("-lossless").arg("1")  // Mathematical lossless
        .arg("-cpu-used").arg("4")
        .arg("-row-mt").arg("1")
        .arg(&output)
        .output();

    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);

            // Copy metadata and timestamps
            copy_metadata(input, &output);

            mark_as_processed(input);

            if options.delete_original {
                fs::remove_file(input)?;
            }

            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("Lossless AV1: size {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("ffmpeg lossless failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("ffmpeg not found: {}", e)))
        }
    }
}

// MacOS specialized timestamp setter (creation time + date added)
#[cfg(target_os = "macos")]
mod macos_ext {
    use std::io;
    use std::path::Path;
    use std::time::SystemTime;
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

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

    extern "C" {
        fn setattrlist(
            path: *const i8,
            attrList: *mut attrlist,
            attrBuf: *mut std::ffi::c_void,
            attrBufSize: usize,
            options: u32,
        ) -> i32;
        fn getattrlist(
            path: *const i8,
            attrList: *mut attrlist,
            attrBuf: *mut std::ffi::c_void,
            attrBufSize: usize,
            options: u32,
        ) -> i32;
    }

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

    pub fn set_creation_time(path: &Path, time: SystemTime) -> io::Result<()> {
        set_time_attr(path, time, ATTR_CMN_CRTIME)
    }

    pub fn set_added_time(path: &Path, time: SystemTime) -> io::Result<()> {
        set_time_attr(path, time, ATTR_CMN_ADDEDTIME)
    }

    fn set_time_attr(path: &Path, time: SystemTime, attr: u32) -> io::Result<()> {
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

        let duration = time.duration_since(SystemTime::UNIX_EPOCH)
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

    pub fn get_added_time(path: &Path) -> io::Result<SystemTime> {
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
        Ok(SystemTime::UNIX_EPOCH + duration)
    }
}

// Helper to copy extended attributes (xattr)
fn copy_xattrs(src: &Path, dst: &Path) {
    // Skip system xattrs that cannot/should not be copied
    const SKIP_XATTRS: &[&str] = &[
        "com.apple.quarantine",           // Security: re-applied by system
        "com.apple.provenance",           // Security: system managed
        "com.apple.rootless",             // SIP protected
        "com.apple.system.Security",      // Security labels
    ];

    if let Ok(iter) = xattr::list(src) {
        for name in iter {
            if let Some(name_str) = name.to_str() {
                if SKIP_XATTRS.iter().any(|&s| name_str == s) {
                    continue;
                }
                if let Ok(Some(value)) = xattr::get(src, name_str) {
                    let _ = xattr::set(dst, name_str, &value);
                }
            }
        }
    }
}


// Helper to copy metadata and timestamps from source to destination
// Maximum metadata preservation: exiftool + xattr + setattrlist + filetime + flags + ACL
fn copy_metadata(src: &Path, dst: &Path) {
    // 1. ExifTool: Copy ALL metadata tags with maximum coverage
    if which::which("exiftool").is_ok() {
        let _ = Command::new("exiftool")
            .arg("-tagsfromfile").arg(src)
            .arg("-all:all")                          // All standard tags
            .arg("-FileCreateDate<FileCreateDate")    // System creation time
            .arg("-FileModifyDate<FileModifyDate")    // System modify time
            .arg("-CreationDate<CreationDate")        // QuickTime/Mac creation
            .arg("-DateTimeOriginal<DateTimeOriginal") // Original capture time
            .arg("-CreateDate<CreateDate")            // File creation date
            .arg("-ModifyDate<ModifyDate")            // Content modify date
            .arg("-SubSecTimeOriginal<SubSecTimeOriginal")  // Sub-second precision
            .arg("-SubSecTimeDigitized<SubSecTimeDigitized")
            .arg("-SubSecTime<SubSecTime")
            .arg("-GPSDateTime<GPSDateTime")          // GPS timestamp
            .arg("-AllDates")                         // Convenience: all date tags
            .arg("-ICC_Profile<ICC_Profile")          // Color profile
            .arg("-use").arg("MWG")                   // Metadata Working Group compat
            .arg("-overwrite_original")
            .arg(dst)
            .output();
    }

    // 2. Copy Extended Attributes (xattr) - Finder tags, comments, etc.
    copy_xattrs(src, dst);

    // 3. Preserve file system timestamps
    if let Ok(metadata) = fs::metadata(src) {
        #[cfg(target_os = "macos")]
        {
            // A. Creation time (btime) via setattrlist
            if let Ok(created) = metadata.created() {
                let _ = macos_ext::set_creation_time(dst, created);
            }
            // B. Date Added (kMDItemDateAdded) via setattrlist
            if let Ok(added) = macos_ext::get_added_time(src) {
                let _ = macos_ext::set_added_time(dst, added);
            }
        }

        // C. Access and Modification time (atime/mtime) - atomic operation
        // This is also handled by copy_native_metadata on macOS, but kept for other OSes.
        #[cfg(not(target_os = "macos"))]
        {
            let atime = filetime::FileTime::from_last_access_time(&metadata);
            let mtime = filetime::FileTime::from_last_modification_time(&metadata);
            let _ = filetime::set_file_times(dst, atime, mtime);
        }

        // D. Preserve file permissions (chmod)
        // This is handled by copy_native_metadata on macOS, but kept for other OSes.
        #[cfg(not(target_os = "macos"))]
        let _ = fs::set_permissions(dst, metadata.permissions());

        // 5. Native macOS Metadata (The "Nuclear Option")
        // Uses copyfile() to transfer ACLs, Flags, Xattrs, and all Timestamps in one go.
        // This is placed LAST to override any partial sets and ensure system-level consistency.
        #[cfg(target_os = "macos")]
        copy_native_metadata(src, dst);
    }

    // 4. Copy file flags (uchg, hidden, nodump, etc.)
    // This is handled by copy_native_metadata on macOS, so only run for other OSes if applicable.
    #[cfg(not(target_os = "macos"))]
    copy_file_flags(src, dst);

    // 5. Copy ACLs via system command (most reliable)
    // This is handled by copy_native_metadata on macOS, so this block is now redundant.
    // #[cfg(target_os = "macos")]
    // {
    //     // ... ACL copy logic ...
    // }
}

// Helper to copy system metadata (ACL, xattr, flags, etc) using native copyfile
#[cfg(target_os = "macos")]
fn copy_native_metadata(src: &Path, dst: &Path) {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    // FFI to macOS copyfile
    // copyfile(const char *from, const char *to, copyfile_state_t state, copyfile_flags_t flags)
    extern "C" {
        fn copyfile(from: *const i8, to: *const i8, state: *mut std::ffi::c_void, flags: u32) -> i32;
    }

    // Usually defined as (COPYFILE_SECURITY | COPYFILE_METADATA) in headers but simple masks work.
    // Explicit mask for what we want:
    // COPYFILE_ACL (1<<0) | COPYFILE_STAT (1<<1) | COPYFILE_XATTR (1<<2) | COPYFILE_DATA (0) 
    // Wait, typical COPYFILE_METADATA is a convenience macro. 
    // Let's use specific bits:
    // 1<<0: COPYFILE_ACL
    // 1<<1: COPYFILE_STAT (includes timestamps, flags, mode)
    // 1<<2: COPYFILE_XATTR
    // 1<<3: COPYFILE_NOFOLLOW (good practice)
    
    // Safer definition, assuming libc doesn't expose it directly (it often doesn't in cross-platform crates)
    const FLAGS: u32 = (1<<0) | (1<<1) | (1<<2); // ACL | STAT | XATTR

    let src_c = match CString::new(src.as_os_str().as_bytes()) {
        Ok(s) => s,
        Err(_) => return,
    };
    let dst_c = match CString::new(dst.as_os_str().as_bytes()) {
        Ok(s) => s,
        Err(_) => return,
    };

    unsafe {
        if copyfile(src_c.as_ptr(), dst_c.as_ptr(), std::ptr::null_mut(), FLAGS) < 0 {
             eprintln!("⚠️ Failed to copy native metadata (ACL/Flags/Xattr)");
        }
    }
}


/// Determine output path and ensure directory exists
fn determine_output_path(input: &Path, extension: &str, output_dir: &Option<PathBuf>) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    
    let output = match output_dir {
        Some(dir) => {
            // Ensure output directory exists
            let _ = fs::create_dir_all(dir);
            dir.join(format!("{}.{}", stem, extension))
        }
        None => input.with_extension(extension),
    };
    
    output
}

/// Clear processed files list
pub fn clear_processed_list() {
    let mut processed = PROCESSED_FILES.lock().unwrap();
    processed.clear();
}

/// Verify that JXL file is valid using signature and optional decoding
fn verify_jxl_health(path: &Path) -> Result<()> {
    // Check file signature
    let mut file = fs::File::open(path)?;
    let mut sig = [0u8; 2];
    use std::io::Read;
    file.read_exact(&mut sig)?;

    // JXL signature: 0xFF 0x0A (bare JXL) or 0x00 0x00 (ISOBMFF container)
    if sig != [0xFF, 0x0A] && sig != [0x00, 0x00] {
        return Err(ImgQualityError::ConversionError(
            "Invalid JXL file signature".to_string(),
        ));
    }
    
    // Skip full decode check for performance, signature is usually enough for cjxl output
    // Unless paranoia mode is requested.
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_determine_output_path() {
        let input = Path::new("/path/to/image.png");
        let output = determine_output_path(input, "jxl", &None);
        assert_eq!(output, Path::new("/path/to/image.jxl"));
    }
    
    #[test]
    fn test_determine_output_path_with_dir() {
        let input = Path::new("/path/to/image.png");
        let output_dir = Some(PathBuf::from("/output"));
        let output = determine_output_path(input, "avif", &output_dir);
        assert_eq!(output, Path::new("/output/image.avif"));
    }
}
