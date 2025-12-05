use crate::{ImgQualityError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ConversionOptions {
    pub target_format: String,
    pub output_dir: Option<PathBuf>,
    pub in_place: bool,
    pub quality_params: String,
}

/// Convert image to target format using external tools
pub fn convert_image(input: &PathBuf, options: &ConversionOptions) -> Result<PathBuf> {
    // Check if required tools are available
    check_dependencies(&options.target_format)?;

    // Determine output path
    let output_path = determine_output_path(input, &options.target_format, &options.output_dir)?;

    // Execute conversion
    match options.target_format.to_lowercase().as_str() {
        "jxl" => convert_to_jxl(input, &output_path, &options.quality_params)?,
        _ => {
            return Err(ImgQualityError::UnsupportedFormat(format!(
                "Target format not supported: {}",
                options.target_format
            )));
        }
    }

    // If in-place mode, replace original
    if options.in_place {
        std::fs::remove_file(input)?;
    }

    Ok(output_path)
}

fn check_dependencies(target_format: &str) -> Result<()> {
    match target_format.to_lowercase().as_str() {
        "jxl" => {
            // Check if cjxl is available
            if which::which("cjxl").is_err() {
                return Err(ImgQualityError::ToolNotFound(
                    "cjxl not found. Install with: brew install jpeg-xl".to_string(),
                ));
            }
        }
        _ => {}
    }

    // Check for exiftool (for metadata preservation)
    if which::which("exiftool").is_err() {
        eprintln!("⚠️  Warning: exiftool not found. Metadata will not be preserved.");
        eprintln!("   Install with: brew install exiftool");
    }

    Ok(())
}

fn determine_output_path(
    input: &PathBuf,
    target_format: &str,
    output_dir: &Option<PathBuf>,
) -> Result<PathBuf> {
    let file_stem = input
        .file_stem()
        .ok_or_else(|| ImgQualityError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid input filename",
        )))?;

    let output_filename = format!("{}.{}", file_stem.to_string_lossy(), target_format.to_lowercase());

    let output_path = if let Some(dir) = output_dir {
        // Create output directory if it doesn't exist
        std::fs::create_dir_all(dir)?;
        dir.join(output_filename)
    } else {
        // Same directory as input
        input.with_file_name(output_filename)
    };

    Ok(output_path)
}

fn convert_to_jxl(input: &Path, output: &Path, quality_params: &str) -> Result<()> {
    // Create temporary output path
    let temp_output = output.with_extension("jxl.tmp");

    // Parse quality parameters from the command string
    // Example: "cjxl 'input' '{output}.jxl' -d 0.0 --modular -e 8"
    let params: Vec<&str> = quality_params
        .split_whitespace()
        .filter(|s| !s.contains("cjxl") && !s.contains("'") && !s.contains("{output}"))
        .collect();

    // Build cjxl command
    let mut cmd = Command::new("cjxl");
    cmd.arg(input);
    cmd.arg(&temp_output);

    // Add quality parameters
    for param in params {
        if !param.is_empty() && param != input.to_string_lossy() {
            cmd.arg(param);
        }
    }

    // Execute conversion
    let output_result = cmd.output().map_err(|e| {
        ImgQualityError::ConversionError(format!("Failed to execute cjxl: {}", e))
    })?;

    if !output_result.status.success() {
        let stderr = String::from_utf8_lossy(&output_result.stderr);
        return Err(ImgQualityError::ConversionError(format!(
            "cjxl conversion failed: {}",
            stderr
        )));
    }

    // Preserve metadata using exiftool
    if which::which("exiftool").is_ok() {
        let _ = Command::new("exiftool")
            .arg("-tagsfromfile")
            .arg(input)
            .arg("-all:all")
            .arg("-overwrite_original")
            .arg(&temp_output)
            .output(); // Ignore errors, metadata is optional
    }

    // Preserve timestamps
    if let Ok(metadata) = std::fs::metadata(input) {
        if let Ok(modified) = metadata.modified() {
            let _ = filetime::set_file_mtime(&temp_output, filetime::FileTime::from_system_time(modified));
        }
    }

    // Health check: verify the output is valid JXL
    verify_jxl_health(&temp_output)?;

    // Move temp file to final location
    std::fs::rename(&temp_output, output)?;

    Ok(())
}

fn verify_jxl_health(path: &Path) -> Result<()> {
    // Check file signature
    let mut file = std::fs::File::open(path)?;
    let mut sig = [0u8; 2];
    use std::io::Read;
    file.read_exact(&mut sig)?;

    // JXL signature: 0xFF 0x0A (bare JXL) or 0x00 0x00 (ISOBMFF container)
    if sig != [0xFF, 0x0A] && sig != [0x00, 0x00] {
        return Err(ImgQualityError::ConversionError(
            "Invalid JXL file signature".to_string(),
        ));
    }

    // Try to decode with djxl if available
    if which::which("djxl").is_ok() {
        let result = Command::new("djxl")
            .arg(path)
            .arg("/dev/null")
            .output();

        if let Ok(output) = result {
            if !output.status.success() {
                return Err(ImgQualityError::ConversionError(
                    "JXL health check failed: file cannot be decoded".to_string(),
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_output_path() {
        let input = PathBuf::from("/path/to/image.png");
        let output = determine_output_path(&input, "jxl", &None).unwrap();
        assert_eq!(output, PathBuf::from("/path/to/image.jxl"));
    }

    #[test]
    fn test_determine_output_path_with_dir() {
        // Use temp directory for this test since it tries to create the output dir
        let temp_dir = std::env::temp_dir().join("imgquality_test");
        let input = PathBuf::from("/path/to/image.png");
        let output_dir = Some(temp_dir.clone());
        let output = determine_output_path(&input, "jxl", &output_dir).unwrap();
        assert_eq!(output, temp_dir.join("image.jxl"));
        // Cleanup
        let _ = std::fs::remove_dir(&temp_dir);
    }
}

