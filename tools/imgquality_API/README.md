# imgquality - Intelligent Image Analysis & Conversion

**A high-performance, parallel-processing CLI tool for deep image analysis and smart, quality-preserving format upgrades.** It provides technically detailed analysis and opinionated conversion strategies to ensure the highest quality results and prevent common mistakes like re-compressing lossy files.

## Core Philosophy

1.  **Preserve Quality**: Never degrade quality by re-compressing a lossy file (e.g., lossy WebP or JPEG) into another lossy format.
2.  **Maximize Efficiency**: Use the best modern codecs for the job: JPEG XL (JXL) for both lossless and lossy images, and AVIF for specific lossy cases.
3.  **Provide Clarity**: Offer detailed analysis so the user understands *why* a certain action is or is not recommended.

## Features

✨ **Comprehensive Quality Analysis**
- **Deep JPEG Analysis**: Estimates JPEG quality (`Q` score), identifies quantization tables (standard vs. custom), and detects encoder signatures.
- **HEIC/AVIF Aware**: Correctly identifies HEIC and AVIF as modern formats and advises against unnecessary conversion.
- **Lossless/Lossy Detection**: Accurately identifies compression type for WebP, PNG, etc.
- **Rich Metadata**: Extracts color depth, color space, dimensions, alpha, and animation status.
- **Image Complexity**: Calculates entropy and compression ratio to measure image complexity.

🚀 **Intelligent `auto` Conversion Engine**
- **Smart Strategy**: Automatically chooses the best conversion path based on source properties.
  - **JPEG → JXL**: **True lossless transcode** (`--lossless_jpeg=1`) that preserves original DCT coefficients, reducing size by ~20% with zero quality loss.
  - **PNG/Lossless WebP/TIFF → JXL**: Mathematical lossless compression (`-d 0.0`), reducing size by 30-60%.
  - **Animated (Lossless) → AV1 MP4**: Converts lossless animations (e.g., GIF) to a highly efficient video format.
  - **Static Lossy (non-JPEG) → AVIF**: Converts other static lossy files to AVIF for better compression.
- **Safe by Default**: Automatically **skips** converting lossy WebP and animated lossy files to prevent quality degradation.
- **Parallel Processing**: Uses all available CPU cores to process large directories in parallel.
- **Anti-Duplicate**: Remembers which files have been successfully processed to avoid redundant work on subsequent runs (can be overridden with `--force`).

💡 **CLI & API Modes**
- **Interactive CLI**: Rich, human-readable output, including detailed reasons for recommendations.
- **JSON API**: Provides structured `json` output for easy integration with scripts and other tools.

## Installation

### Prerequisites

```bash
# Required: Install JPEG XL toolkit
brew install jpeg-xl

# Required: Install AVIF toolkit (libavif) for AVIF conversion
brew install libavif

# Required: Install FFmpeg for animated conversions
brew install ffmpeg

# Optional: For metadata preservation during conversion
brew install exiftool
```

### Build & Install

```bash
# Navigate to the project directory
cd /path/to/imgquality_API

# Build the release binary
cargo build --release

# The binary will be at: ./target/release/imgquality

# Optional: Install to your system path
cargo install --path .
```

## Command Usage

### 1. `analyze`: Deep Image Analysis

Provides a detailed report and a clear recommendation.

```bash
imgquality analyze photo.png --recommend
```
*Example Output:*
```
📊 Image Quality Analysis Report
...
💡 JXL Format Recommendation
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✅ PNG → JXL
📝 **Reason**: Lossless source can be losslessly compressed to JXL for significant space savings.
🎯 **Quality**: Mathematically Lossless
💾 **Expected Reduction**: 45.1%
⚙️  **Command**: cjxl 'photo.png' '{output}.jxl' -d 0.0 -e 8
```

### 2. `auto`: Smart Automatic Conversion (Recommended)

The `auto` command intelligently analyzes each image and converts it to the optimal format using parallel processing. **This is the recommended command for batch processing.**

```bash
# Analyze and convert a directory, saving to a new location
imgquality auto ./input_dir --output ./output_dir

# Convert recursively, and delete original files after success
# Use with caution!
imgquality auto ./media --recursive --delete-original
```

*Example Log:*
```
📂 Found 152 files to process (parallel mode)
🔄 JPEG→JXL lossless transcode: ./media/IMG_001.JPG
✅ Converted successfully (reduced 22.5%)
🔄 Lossless→JXL: ./media/screenshot.png
✅ Converted successfully (reduced 58.1%)
🔄 Animated lossless→AV1 MP4: ./media/animation.gif
✅ Converted successfully (reduced 85.3%)
⏭️ Skipping lossy WebP (to avoid quality loss): ./media/image.webp
⏭️ Skipping modern format (already efficient): ./media/icon.avif
⏭️ Skipped: Already processed: ./media/processed_before.png
...
✅ Auto-conversion complete: 140 succeeded, 10 skipped, 2 failed
```

### 3. `convert`: Manual Conversion

Manually convert images to a *specific* format. This command is less intelligent than `auto` and may result in quality loss if used improperly (e.g., converting JPEG to lossy JXL).

```bash
imgquality convert image.png --to jxl --output ./converted/
```

### 4. `verify`: Verify Conversion Quality

Compares two images and calculates perceptual quality metrics. This performs a **full calculation**, which is more accurate than the *estimation* provided by the `analyze` command.

```bash
imgquality verify original.png converted.jxl
```
*Example Output:*
```
...
📏 Quality Metrics:
   PSNR: ∞ dB (Identical - mathematically lossless)
   SSIM: 1.000000 (Identical)
   MS-SSIM: 1.000000 (Identical)

✅ Verification complete: Conversion is mathematically lossless.
```

---

# imgquality - 智能图像分析与转换工具

**一款高性能、并行处理的命令行工具，用于深度图像质量分析和智能、保质量的格式升级。** 它提供技术上详尽的分析和带有明确观点的转换策略，以确保最高质量的转换结果，并防止诸如重复压缩有损文件等常见错误。

## 核心理念

1.  **保证质量**: 绝不通过将有损文件（如 JPEG 或有损 WebP）重新压缩为另一种有损格式而降低其质量。
2.  **极致效率**: 使用最优秀的现代编码器：JPEG XL (JXL) 用于无损和有损图像，AVIF 用于特定的有损场景。
3.  **清晰明确**: 提供详尽的分析，让用户理解*为什么*推荐或不推荐某个操作。

## 功能特性

✨ **全面的质量分析**
- **深度 JPEG 分析**: 估算 JPEG 质量值（`Q` 分数），识别量化表（标准 vs. 自定义），并检测编码器签名。
- **HEIC/AVIF 感知**: 能正确识别 HEIC 和 AVIF 为现代格式，并建议避免不必要的转换。
- **无损/有损检测**: 精准识别 WebP、PNG 等格式的压缩类型。
- **丰富的元数据**: 提取色深、色彩空间、尺寸、Alpha 通道和动画状态。
- **图像复杂度**: 通过计算熵和压缩率来衡量图像的复杂程度。

🚀 **智能 `auto` 转换引擎**
- **智能策略**: 根据源文件属性自动选择最佳转换路径。
  - **JPEG → JXL**: **真正的无损转码** (`--lossless_jpeg=1`)，它会保留原始的 DCT 系数，在完全不损失质量的前提下将体积减少约 20%。
  - **PNG/无损 WebP/TIFF → JXL**: 数学无损压缩 (`-d 0.0`)，可减少 30-60% 的体积。
  - **无损动画 → AV1 MP4**: 将无损动画（如 GIF）转换为高效的视频格式。
  - **静态有损 (非 JPEG) → AVIF**: 将其他静态有损文件转换为 AVIF 以获得更高的压缩率。
- **默认安全**: 自动**跳过**对有损 WebP 和有损动画的转换，以防止质量下降。
- **并行处理**: 使用所有可用的 CPU 核心并行处理大批量图像。
- **防止重复**: 会记录已成功处理的文件，在后续运行时自动跳过，避免重复工作（可通过 `--force` 覆盖）。

💡 **CLI 与 API 双模式**
- **交互式 CLI**: 提供信息丰富、人类可读的输出，包含详尽的推荐理由。
- **JSON API**: 提供结构化的 `json` 输出，便于与脚本和其他工具集成。

## 安装

### 前置依赖

```bash
# 必需：安装 JPEG XL 工具包
brew install jpeg-xl

# 必需：安装 AVIF 工具包 (libavif) 用于 AVIF 转换
brew install libavif

# 必需：安装 FFmpeg 用于动画转换
brew install ffmpeg

# 可选：用于在转换中保留元数据
brew install exiftool
```

### 编译与安装

```bash
# 导航至项目目录
cd /path/to/imgquality_API

# 编译 Release 版本
cargo build --release

# 二进制文件位于 ./target/release/imgquality

# 可选：将程序安装到系统路径
cargo install --path .
```

## 命令用法

### 1. `analyze`: 深度图像分析

提供详细的报告和清晰的建议。

```bash
imgquality analyze photo.png --recommend
```
*输出示例:*
```
📊 图像质量分析报告
...
💡 JXL 格式推荐
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✅ PNG → JXL
📝 **原因**: 无损源文件可以无损压缩为 JXL，以节省大量空间。
🎯 **质量**: 数学无损
💾 **预期减少**: 45.1%
⚙️  **命令**: cjxl 'photo.png' '{output}.jxl' -d 0.0 -e 8
```

### 2. `auto`: 智能自动转换 (推荐)

`auto` 命令会智能分析每个图像，并使用并行处理将其转换为最优格式。**这是进行批量处理时最推荐的命令。**

```bash
# 分析并转换目录，保存到新位置
imgquality auto ./input_dir --output ./output_dir

# 递归转换，并在成功后删除原文件
# 请谨慎使用！
imgquality auto ./media --recursive --delete-original
```

*日志示例:*
```
📂 发现 152 个文件待处理 (并行模式)
🔄 JPEG→JXL 无损转码: ./media/IMG_001.JPG
✅ 转换成功 (体积减少 22.5%)
🔄 无损→JXL: ./media/screenshot.png
✅ 转换成功 (体积减少 58.1%)
🔄 无损动画→AV1 MP4: ./media/animation.gif
✅ 转换成功 (体积减少 85.3%)
⏭️ 跳过有损 WebP (避免质量损失): ./media/image.webp
⏭️ 跳过现代格式 (已足够高效): ./media/icon.avif
⏭️ 已跳过: 文件之前处理过: ./media/processed_before.png
...
✅ 自动转换完成: 140 成功, 10 跳过, 2 失败
```

### 3. `convert`: 手动转换

手动将图像转换为*特定*格式。此命令不如 `auto` 智能，如果使用不当（例如，将 JPEG 转换为有损 JXL）可能会导致质量损失。

```bash
imgquality convert image.png --to jxl --output ./converted/
```

### 4. `verify`: 验证转换质量

比较两个图像并计算感知质量指标。此命令执行的是**完全计算**，比 `analyze` 命令提供的*估算值*更精确。

```bash
imgquality verify original.png converted.jxl
```
*输出示例:*
```
...
📏 质量度量:
   PSNR: ∞ dB (完全相同 - 数学无损)
   SSIM: 1.000000 (完全相同)
   MS-SSIM: 1.000000 (完全相同)

✅ 验证完成: 转换是数学无损的。
```
