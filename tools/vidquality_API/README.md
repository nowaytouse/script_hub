# vidquality - Intelligent Video Archival & Compression

**A high-performance CLI tool for deep video analysis and intelligent, quality-preserving format conversion, specializing in FFV1 for archival and AV1 for modern compression.**

`vidquality` uses `ffmpeg` and `ffprobe` to analyze video files and determine the best conversion strategy. It is designed for two primary use cases:

1.  **Archival**: Converting lossless or high-quality master files into the **FFV1 (v3) codec within an MKV container**, a format recommended by numerous archival institutions for its mathematical losslessness and robustness.
2.  **Compression**: Converting lossy source files into the **AV1 codec within an MP4 container**, providing exceptional compression efficiency while preserving visual quality.

## Features

✨ **Deep Video Analysis**
- **Rich Metadata**: Uses `ffprobe` to extract detailed stream information: codec, format, resolution, FPS, bit depth, color space, duration, and audio tracks.
- **Compression Detection**: Accurately determines if a video is `Lossless`, `High-Quality Lossy`, or `Standard Lossy`.
- **Quality Score**: Calculates a heuristic quality score (0-100) based on bitrate, resolution, and compression to help identify high-quality sources.
- **Archival Candidate Check**: Flags videos that are ideal candidates for archival (e.g., lossless codecs like `prores`, `ffv1`, `huffyuv`).

🚀 **Intelligent `auto` Conversion Engine**
- **Smart Strategy**: Automatically chooses the best conversion path based on source quality.
  - **Lossless/High-Quality Source → FFV1 MKV**: For perfect, bit-for-bit archival.
  - **Lossy Source → AV1 MP4**: For efficient, high-quality distribution copies.
- **`--explore` Mode**: A unique feature that automatically runs multiple conversion passes to find the highest CRF (lowest file size) for AV1 that doesn't exceed the original file's size, ensuring optimal compression.
- **Safety First**: Logs all operations and includes an option to `--delete-original` only after a successful conversion.

💡 **CLI & API Modes**
- **Interactive CLI**: Rich, human-readable output for easy analysis.
- **JSON API**: Provides structured `json` output for seamless integration with media asset management (MAM) scripts or other tools.

## Installation

### Prerequisites

`vidquality` requires a recent version of **FFmpeg** (which includes `ffprobe`) to be installed and available in your system's `PATH`.

```bash
# On macOS using Homebrew
brew install ffmpeg

# On Debian/Ubuntu
sudo apt update && sudo apt install ffmpeg

# On Windows using Chocolatey
choco install ffmpeg
```

### Build & Install

```bash
# Navigate to the project directory
cd /path/to/vidquality_API

# Build the release binary
cargo build --release

# The binary will be at: ./target/release/vidquality

# Optional: Install to your system path for easy access
cargo install --path .
```

## Command Usage

### 1. `analyze`: Deep Video Analysis

Displays a detailed report of a video's technical properties.

```bash
vidquality analyze "My Wedding Video.mov"
```
*Example Output:*
```
📊 Video Analysis Report
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 File: My Wedding Video.mov
📦 Format: mov,mp4,m4a,3gp,3g2,mj2
🎬 Codec: prores (Apple ProRes)
🔍 Compression: Lossless
📐 Resolution: 1920x1080
🎞️  Frames: 5400 @ 30.00 fps
⏱️  Duration: 180.00s
🎨 Bit Depth: 10-bit
🌈 Pixel Format: yuv422p10le
💾 File Size: 23000000000 bytes
📊 Bitrate: 1022222222 bps
🎵 Audio: pcm_s16le
⭐ Quality Score: 98/100
📦 Archival Candidate: ✅ Yes
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

#### JSON Output for Scripting
```bash
vidquality analyze "My Wedding Video.mov" --output json
```

### 2. `strategy`: See the Recommended Strategy

Performs a "dry run" to show what the `auto` command would do without converting.

```bash
vidquality strategy "My Wedding Video.mov"
```
*Example Output:*
```
🎯 Recommended Strategy (Auto Mode)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 File: My Wedding Video.mov
🎬 Codec: prores (Lossless)
💡 Target: FFV1 MKV (Archival)
📝 Reason: Source is lossless, ideal for archival format.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### 3. `auto`: Smart Automatic Conversion (Recommended)

The `auto` command intelligently analyzes and converts the video to the appropriate target format (FFV1 for archival, AV1 for compression). **This is the recommended command.**

#### Basic Usage
```bash
vidquality auto "My Wedding Video.mov" --output ./archive/
```
*Log:*
```
INFO  vidquality::conversion_api: 🎬 Auto Mode Conversion
INFO  vidquality::conversion_api:    Lossless sources → FFV1 MKV (archival)
INFO  vidquality::conversion_api:    Lossy sources → AV1 MP4 (high quality)
...
INFO  vidquality::conversion_api: ✅ Successfully converted
INFO  vidquality::conversion_api:    Output: ./archive/My Wedding Video.mkv
INFO  vidquality::conversion_api:    Size Ratio: 75.3%
```

#### Using `--explore` for Optimal AV1 Compression
When converting a lossy file, use `--explore` to get the smallest possible file size.
```bash
vidquality auto "youtube_rip.mp4" --output ./compressed/ --explore
```
*Log:*
```
INFO  vidquality::conversion_api: 📊 Size exploration: ENABLED
...
INFO  vidquality::conversion_api:    Input size: 50MB, Output: 60MB. Exploring smaller...
INFO  vidquality::conversion_api:    Trying CRF 25...
...
INFO  vidquality::conversion_api: 📊 Conversion Summary:
INFO  vidquality::conversion_api:    Input:  youtube_rip.mp4 (50000000 bytes)
INFO  vidquality::conversion_api:    Output: ./compressed/youtube_rip.mp4 (45000000 bytes)
INFO  vidquality::conversion_api:    Ratio:  90.0%
INFO  vidquality::conversion_api:    🔍 Explored 3 CRF values, final: CRF 28
```

### 4. `simple`: Convert Everything to AV1

A straightforward mode that converts any input video directly to AV1/MP4. Useful for quick, high-quality batch compression without archival considerations.

```bash
vidquality simple "screencast.mov" --output ./videos/
```

---

# vidquality - 智能视频归档与压缩工具

**一款高性能的命令行工具，用于深度视频分析和智能、保质量的格式转换，专注于 FFV1 归档和 AV1 现代压缩。**

`vidquality` 使用 `ffmpeg` 和 `ffprobe` 来分析视频文件，并确定最佳的转换策略。它主要为两个核心场景设计：

1.  **归档 (Archival)**: 将无损或高质量的母版文件转换为 **FFV1 (v3) 编码的 MKV 容器**。该格式因其数学无损和稳健性而被众多档案机构推荐。
2.  **压缩 (Compression)**: 将有损的源文件转换为 **AV1 编码的 MP4 容器**，在保持出色视觉质量的同时，提供卓越的压缩效率。

## 功能特性

✨ **深度视频分析**
- **丰富元数据**: 使用 `ffprobe` 提取详细的流信息：编码器、格式、分辨率、帧率、位深度、色彩空间、时长和音轨。
- **压缩类型检测**: 精准判断视频是 `无损 (Lossless)`、`高质量有损 (High-Quality Lossy)` 还是 `标准有损 (Standard Lossy)`。
- **质量分数**: 基于码率、分辨率和压缩类型计算出一个启发式的质量分数（0-100），以帮助识别高质量源。
- **归档候选检查**: 标记出适合归档的视频（例如，使用 `prores`, `ffv1`, `huffyuv` 等无损编码器）。

🚀 **智能 `auto` 转换引擎**
- **智能策略**: 根据源文件质量自动选择最佳转换路径。
  - **无损/高质量源 → FFV1 MKV**: 用于完美的、逐比特的数字归档。
  - **有损源 → AV1 MP4**: 用于高效、高质量的分发副本。
- **`--explore` 模式**: 一个独特的功能，可自动运行多轮转换，以寻找在不超过原文件大小的前提下、可用的最高 CRF 值（即最小文件大小），确保最优压缩。
- **安全第一**: 记录所有操作，并提供 `--delete-original` 选项，仅在转换成功后删除源文件。

💡 **CLI 与 API 双模式**
- **交互式 CLI**: 提供丰富、人类可读的输出，便于手动分析。
- **JSON API**: 提供结构化的 `json` 输出，可无缝集成到媒体资产管理（MAM）脚本或其他工具中。

## 安装

### 前置依赖

`vidquality` 需要在您的系统 `PATH` 中安装并配置好最新版的 **FFmpeg** (它包含了 `ffprobe`)。

```bash
# 在 macOS 上使用 Homebrew
brew install ffmpeg

# 在 Debian/Ubuntu 上
sudo apt update && sudo apt install ffmpeg

# 在 Windows 上使用 Chocolatey
choco install ffmpeg
```

### 编译与安装

```bash
# 导航至项目目录
cd /path/to/vidquality_API

# 编译 Release 版本
cargo build --release

# 二进制文件位于 ./target/release/vidquality

# 可选：将程序安装到系统路径以便于访问
cargo install --path .
```

## 命令用法

### 1. `analyze`: 深度视频分析

显示视频技术属性的详细报告。

```bash
vidquality analyze "我的婚礼视频.mov"
```
*输出示例:*
```
📊 视频分析报告
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 文件: 我的婚礼视频.mov
📦 格式: mov,mp4,m4a,3gp,3g2,mj2
🎬 编码: prores (Apple ProRes)
🔍 压缩: 无损
📐 分辨率: 1920x1080
🎞️  帧数: 5400 @ 30.00 fps
⏱️  时长: 180.00s
🎨 位深度: 10-bit
🌈 像素格式: yuv422p10le
💾 文件大小: 23000000000 字节
📊 码率: 1022222222 bps
🎵 音频: pcm_s16le
⭐ 质量分数: 98/100
📦 归档候选: ✅ 是
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

#### 用于脚本的 JSON 输出
```bash
vidquality analyze "我的婚礼视频.mov" --output json
```

### 2. `strategy`: 查看推荐策略

执行一次“空运行”（dry run），显示 `auto` 命令将会采取的转换策略，而无需实际执行。

```bash
vidquality strategy "我的婚礼视频.mov"
```
*输出示例:*
```
🎯 推荐策略 (Auto 模式)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 文件: 我的婚礼视频.mov
🎬 编码: prores (无损)
💡 目标: FFV1 MKV (归档)
📝 原因: 源文件是无损的，是理想的归档格式。
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### 3. `auto`: 智能自动转换 (推荐)

`auto` 命令会智能分析并根据场景将视频转换为最合适的格式（FFV1 用于归档，AV1 用于压缩）。**这是最推荐使用的命令。**

#### 基本用法
```bash
vidquality auto "我的婚礼视频.mov" --output ./archive/
```
*日志:*
```
INFO  vidquality::conversion_api: 🎬 Auto 模式转换
INFO  vidquality::conversion_api:    无损源 → FFV1 MKV (归档)
INFO  vidquality::conversion_api:    有损源 → AV1 MP4 (高质量)
...
INFO  vidquality::conversion_api: ✅ 转换成功
INFO  vidquality::conversion_api:    输出: ./archive/我的婚礼视频.mkv
INFO  vidquality::conversion_api:    体积比例: 75.3%
```

#### 使用 `--explore` 实现最优 AV1 压缩
当转换有损文件时，使用 `--explore` 来获取可能的最小文件体积。
```bash
vidquality auto "youtube_rip.mp4" --output ./compressed/ --explore
```
*日志:*
```
INFO  vidquality::conversion_api: 📊 体积探索模式: 已启用
...
INFO  vidquality::conversion_api:    输入体积: 50MB, 输出: 60MB. 正在探索更小体积...
INFO  vidquality::conversion_api:    尝试 CRF 25...
...
INFO  vidquality::conversion_api: 📊 转换总结:
INFO  vidquality::conversion_api:    输入:  youtube_rip.mp4 (50000000 字节)
INFO  vidquality::conversion_api:    输出: ./compressed/youtube_rip.mp4 (45000000 字节)
INFO  vidquality::conversion_api:    比例:  90.0%
INFO  vidquality::conversion_api:    🔍 探索了 3 个 CRF 值, 최종: CRF 28
```

### 4. `simple`: 将所有视频转换为 AV1

一个直接的模式，将任何输入视频都转换为 AV1/MP4。适用于无需考虑归档、仅需进行快速高质量批量压缩的场景。

```bash
vidquality simple "screencast.mov" --output ./videos/
```
