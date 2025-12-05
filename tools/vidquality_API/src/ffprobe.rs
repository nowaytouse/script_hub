//! FFprobe wrapper module
//!
//! Calls ffprobe to extract video metadata

use crate::{VidQualityError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// FFprobe analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FFprobeResult {
    pub format_name: String,
    pub duration: f64,
    pub size: u64,
    pub bit_rate: u64,
    pub video_codec: String,
    pub video_codec_long: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub frame_count: u64,
    pub pix_fmt: String,
    pub color_space: Option<String>,
    pub color_transfer: Option<String>,
    pub bit_depth: u8,
    pub has_audio: bool,
    pub audio_codec: Option<String>,
}

/// Probe video file using ffprobe
pub fn probe_video(path: &Path) -> Result<FFprobeResult> {
    // Check if ffprobe exists
    if Command::new("ffprobe").arg("-version").output().is_err() {
        return Err(VidQualityError::ToolNotFound("ffprobe".to_string()));
    }
    
    let output = Command::new("ffprobe")
        .args(&[
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            path.to_str().unwrap_or(""),
        ])
        .output()?;
    
    if !output.status.success() {
        return Err(VidQualityError::FFprobeError(
            String::from_utf8_lossy(&output.stderr).to_string()
        ));
    }
    
    let json_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| VidQualityError::FFprobeError(e.to_string()))?;
    
    // Extract format info
    let format = &json["format"];
    let format_name = format["format_name"].as_str().unwrap_or("unknown").to_string();
    let duration = format["duration"].as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let size = format["size"].as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let bit_rate = format["bit_rate"].as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    
    // Find video stream
    let streams = json["streams"].as_array()
        .ok_or_else(|| VidQualityError::FFprobeError("No streams found".to_string()))?;
    
    let video_stream = streams.iter()
        .find(|s| s["codec_type"].as_str() == Some("video"))
        .ok_or_else(|| VidQualityError::FFprobeError("No video stream found".to_string()))?;
    
    let video_codec = video_stream["codec_name"].as_str().unwrap_or("unknown").to_string();
    let video_codec_long = video_stream["codec_long_name"].as_str().unwrap_or("").to_string();
    let width = video_stream["width"].as_u64().unwrap_or(0) as u32;
    let height = video_stream["height"].as_u64().unwrap_or(0) as u32;
    
    // Parse frame rate (e.g., "30/1" or "29.97")
    let frame_rate = parse_frame_rate(
        video_stream["r_frame_rate"].as_str().unwrap_or("0/1")
    );
    
    // Get frame count
    let frame_count = video_stream["nb_frames"].as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| (duration * frame_rate) as u64);
    
    let pix_fmt = video_stream["pix_fmt"].as_str().unwrap_or("unknown").to_string();
    let color_space = video_stream["color_space"].as_str().map(|s| s.to_string());
    let color_transfer = video_stream["color_transfer"].as_str().map(|s| s.to_string());
    
    // Determine bit depth from pixel format
    let bit_depth = detect_bit_depth(&pix_fmt);
    
    // Check for audio
    let has_audio = streams.iter().any(|s| s["codec_type"].as_str() == Some("audio"));
    let audio_codec = streams.iter()
        .find(|s| s["codec_type"].as_str() == Some("audio"))
        .and_then(|s| s["codec_name"].as_str())
        .map(|s| s.to_string());
    
    Ok(FFprobeResult {
        format_name,
        duration,
        size,
        bit_rate,
        video_codec,
        video_codec_long,
        width,
        height,
        frame_rate,
        frame_count,
        pix_fmt,
        color_space,
        color_transfer,
        bit_depth,
        has_audio,
        audio_codec,
    })
}

/// Parse frame rate string (e.g., "30/1" or "29.97")
fn parse_frame_rate(s: &str) -> f64 {
    if s.contains('/') {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() == 2 {
            let num = parts[0].parse::<f64>().unwrap_or(0.0);
            let den = parts[1].parse::<f64>().unwrap_or(1.0);
            if den > 0.0 {
                return num / den;
            }
        }
    }
    s.parse::<f64>().unwrap_or(0.0)
}

/// Detect bit depth from pixel format
fn detect_bit_depth(pix_fmt: &str) -> u8 {
    if pix_fmt.contains("10le") || pix_fmt.contains("10be") || pix_fmt.contains("p010") {
        10
    } else if pix_fmt.contains("12le") || pix_fmt.contains("12be") {
        12
    } else if pix_fmt.contains("16le") || pix_fmt.contains("16be") {
        16
    } else {
        8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_frame_rate() {
        assert!((parse_frame_rate("30/1") - 30.0).abs() < 0.001);
        assert!((parse_frame_rate("30000/1001") - 29.97).abs() < 0.01);
        assert!((parse_frame_rate("24") - 24.0).abs() < 0.001);
    }
    
    #[test]
    fn test_detect_bit_depth() {
        assert_eq!(detect_bit_depth("yuv420p"), 8);
        assert_eq!(detect_bit_depth("yuv420p10le"), 10);
        assert_eq!(detect_bit_depth("yuv422p12le"), 12);
    }
}
