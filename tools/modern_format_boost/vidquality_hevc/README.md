# vidquality-hevc - HEVC/H.265 Video Archival & Compression

**A high-performance CLI tool for deep video analysis and intelligent, quality-preserving format conversion, specializing in HEVC/H.265 for both archival (lossless) and compression (lossy).**

> 📌 **Note**: This is the HEVC variant of vidquality. For AV1/SVT-AV1 encoding, use the original `vidquality` tool.

`vidquality-hevc` uses `ffmpeg` and `ffprobe` to analyze video files and determine the best conversion strategy using HEVC/H.265 codec.

## Core Philosophy

1.  **HEVC Archival**: Use HEVC lossless mode for preserving high-quality master files with excellent compression.
2.  **Efficient Compression**: Use HEVC with optimized CRF settings for creating high-quality distribution copies.
3.  **Apple Compatibility**: Uses `hvc1` tag for maximum Apple device compatibility.

## Features

✨ **Deep Video Analysis**
- Same analysis capabilities as vidquality
- Codec & Compression Analysis
- Quality Score (0-100)
- Rich Metadata extraction

🚀 **Intelligent Conversion Engines**
- **Smart `auto` Strategy**:
  - **Modern Codecs (AV1/H.265/VP9/VVC)**: **Skip** to avoid generational loss
  - **Lossless Source → HEVC Lossless MKV**: Mathematical lossless preservation
  - **Lossy Source → HEVC MP4 (CRF 18-20)**: High quality compression
- **Simple Mode**: HEVC MP4 with CRF 18 (high quality)
- **Apple Compatible**: Uses `hvc1` tag for iOS/macOS playback

## Installation

### Prerequisites

```bash
# On macOS using Homebrew
brew install ffmpeg

# For metadata preservation (recommended)
brew install exiftool
```

### Build & Install

```bash
cd /path/to/vidquality_hevc
cargo build --release
cargo install --path .
```

## Command Usage

### 1. `analyze`: Deep Video Analysis

```bash
vidquality-hevc analyze "video.mov"
```

### 2. `strategy`: Preview the Conversion Plan

```bash
vidquality-hevc strategy "video.mkv"
```

### 3. `auto`: Smart Automatic Conversion

```bash
# Standard conversion
vidquality-hevc auto "video.mov" --output ./output/

# With lossless mode
vidquality-hevc auto "video.mov" --output ./archive/ --lossless

# With size exploration
vidquality-hevc auto "video.mkv" --output ./compressed/ --explore
```

### 4. `simple`: Convert to HEVC MP4

```bash
vidquality-hevc simple "video.mov" --output ./videos/
```

## Comparison with vidquality (AV1)

| Feature | vidquality (AV1) | vidquality-hevc |
|---------|------------------|-----------------|
| Encoder | SVT-AV1 | libx265 |
| Compression | Better (~30% smaller) | Good |
| Speed | Slower | Faster |
| Compatibility | Modern browsers/devices | Universal |
| Apple Support | Limited | Excellent (hvc1) |

## CRF Settings

| Source Type | CRF | Quality |
|-------------|-----|---------|
| Lossless | 0 (lossless mode) | Mathematical lossless |
| Visually Lossless | 18 | Near-lossless |
| Standard | 20 | High quality |
| Explore Max | 28 | Good quality |

---

# vidquality-hevc - HEVC/H.265 视频归档与压缩工具

**一款高性能的命令行工具，用于深度视频分析和智能、保质量的格式转换，专注于 HEVC/H.265 编码的归档（无损）和压缩（有损）。**

> 📌 **注意**: 这是 vidquality 的 HEVC 变体。如需 AV1/SVT-AV1 编码，请使用原版 `vidquality` 工具。

## 核心理念

1.  **HEVC 归档**: 使用 HEVC 无损模式保存高质量母版文件
2.  **高效压缩**: 使用优化的 CRF 设置创建高质量分发副本
3.  **Apple 兼容**: 使用 `hvc1` 标签确保 Apple 设备最佳兼容性

## 安装

```bash
# 安装依赖
brew install ffmpeg exiftool

# 编译
cd /path/to/vidquality_hevc
cargo build --release
cargo install --path .
```

## 使用方法

```bash
# 分析视频
vidquality-hevc analyze "video.mov"

# 智能转换
vidquality-hevc auto "video.mov" --output ./output/

# 无损模式
vidquality-hevc auto "video.mov" --output ./archive/ --lossless

# 简单模式
vidquality-hevc simple "video.mov" --output ./videos/
```
