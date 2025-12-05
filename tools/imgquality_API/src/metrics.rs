//! Image Quality Metrics Module
//! 
//! Provides precise PSNR and SSIM calculations between images

use image::{DynamicImage, GenericImageView};

/// Calculate PSNR (Peak Signal-to-Noise Ratio) between two images
/// Returns PSNR in dB. Higher values indicate better quality.
/// PSNR > 40dB: Excellent, PSNR 30-40dB: Good, PSNR < 30dB: Poor
pub fn calculate_psnr(original: &DynamicImage, converted: &DynamicImage) -> Option<f64> {
    let (w1, h1) = original.dimensions();
    let (w2, h2) = converted.dimensions();
    
    // Images must have same dimensions
    if w1 != w2 || h1 != h2 {
        return None;
    }
    
    let orig_rgb = original.to_rgb8();
    let conv_rgb = converted.to_rgb8();
    
    let mut mse_sum: f64 = 0.0;
    let pixel_count = (w1 * h1) as f64;
    
    for (p1, p2) in orig_rgb.pixels().zip(conv_rgb.pixels()) {
        let r1 = p1[0] as f64;
        let g1 = p1[1] as f64;
        let b1 = p1[2] as f64;
        let r2 = p2[0] as f64;
        let g2 = p2[1] as f64;
        let b2 = p2[2] as f64;
        
        mse_sum += (r1 - r2).powi(2) + (g1 - g2).powi(2) + (b1 - b2).powi(2);
    }
    
    let mse = mse_sum / (3.0 * pixel_count);
    
    if mse == 0.0 {
        // Identical images
        return Some(f64::INFINITY);
    }
    
    let max_pixel: f64 = 255.0;
    let psnr = 10.0 * (max_pixel * max_pixel / mse).log10();
    
    Some(psnr)
}

/// Calculate SSIM (Structural Similarity Index) between two images
/// Returns SSIM between 0.0 and 1.0. 1.0 means identical images.
/// SSIM > 0.95: Excellent, SSIM 0.85-0.95: Good, SSIM < 0.85: Poor
pub fn calculate_ssim(original: &DynamicImage, converted: &DynamicImage) -> Option<f64> {
    let (w1, h1) = original.dimensions();
    let (w2, h2) = converted.dimensions();
    
    if w1 != w2 || h1 != h2 {
        return None;
    }
    
    let orig_gray = original.to_luma8();
    let conv_gray = converted.to_luma8();
    
    let pixel_count = (w1 * h1) as f64;
    
    // Calculate means
    let mut mean_x: f64 = 0.0;
    let mut mean_y: f64 = 0.0;
    
    for (p1, p2) in orig_gray.pixels().zip(conv_gray.pixels()) {
        mean_x += p1[0] as f64;
        mean_y += p2[0] as f64;
    }
    
    mean_x /= pixel_count;
    mean_y /= pixel_count;
    
    // Calculate variances and covariance
    let mut var_x: f64 = 0.0;
    let mut var_y: f64 = 0.0;
    let mut cov_xy: f64 = 0.0;
    
    for (p1, p2) in orig_gray.pixels().zip(conv_gray.pixels()) {
        let x = p1[0] as f64;
        let y = p2[0] as f64;
        var_x += (x - mean_x).powi(2);
        var_y += (y - mean_y).powi(2);
        cov_xy += (x - mean_x) * (y - mean_y);
    }
    
    var_x /= pixel_count;
    var_y /= pixel_count;
    cov_xy /= pixel_count;
    
    // SSIM constants (for 8-bit images)
    let c1 = (0.01 * 255.0_f64).powi(2);
    let c2 = (0.03 * 255.0_f64).powi(2);
    
    // SSIM formula
    let numerator = (2.0 * mean_x * mean_y + c1) * (2.0 * cov_xy + c2);
    let denominator = (mean_x.powi(2) + mean_y.powi(2) + c1) * (var_x + var_y + c2);
    
    Some(numerator / denominator)
}

/// Quality assessment description based on PSNR
pub fn psnr_quality_description(psnr: f64) -> &'static str {
    if psnr == f64::INFINITY {
        "Identical (lossless)"
    } else if psnr > 50.0 {
        "Excellent - virtually lossless"
    } else if psnr > 40.0 {
        "Very good - minimal visible difference"
    } else if psnr > 35.0 {
        "Good - acceptable quality"
    } else if psnr > 30.0 {
        "Fair - noticeable degradation"
    } else {
        "Poor - significant quality loss"
    }
}

/// Quality assessment description based on SSIM
pub fn ssim_quality_description(ssim: f64) -> &'static str {
    if ssim >= 0.999 {
        "Identical"
    } else if ssim >= 0.98 {
        "Excellent - virtually lossless"
    } else if ssim >= 0.95 {
        "Very good - minimal visible difference"
    } else if ssim >= 0.90 {
        "Good - acceptable quality"
    } else if ssim >= 0.85 {
        "Fair - noticeable degradation"
    } else {
        "Poor - significant quality loss"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;
    
    #[test]
    fn test_identical_images() {
        let img1 = DynamicImage::ImageRgb8(RgbImage::from_fn(100, 100, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        }));
        let img2 = img1.clone();
        
        let psnr = calculate_psnr(&img1, &img2);
        assert!(psnr.unwrap().is_infinite());
        
        let ssim = calculate_ssim(&img1, &img2);
        assert!((ssim.unwrap() - 1.0).abs() < 0.001);
    }
}
