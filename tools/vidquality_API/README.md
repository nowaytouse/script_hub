# vidquality - Intelligent Video Archival & Compression

**A high-performance CLI tool for deep video analysis and intelligent, quality-preserving format conversion, specializing in FFV1 for archival and AV1 for modern compression.**

`vidquality` uses `ffmpeg` and `ffprobe` to analyze video files and determine the best conversion strategy. It is built on a philosophy of robust, high-quality media preservation and distribution.

## Core Philosophy

1.  **Archival First**: Prioritize the preservation of high-quality and lossless master files using the best-in-class archival codec, **FFV1**, with recommended settings for maximum robustness.
2.  **Efficient Compression**: Use the **AV1 codec** for creating high-quality, efficient distribution copies from lossy sources.
3.  **Provide Clarity**: Offer detailed analysis to explain *why* a certain strategy is chosen, based on technical properties of the source file.

## Features

✨ **Deep Video Analysis**
- **Codec & Compression Analysis**: Detects specific codecs (e.g., `ProRes`, `DNxHD`, `H.264`) and classifies them into `Lossless`, `Visually Lossless`, `High Quality`, or `Standard Quality`.
- **Quality Score (0-100)**: Calculates a heuristic quality score based on compression type, adding bonuses for high bit depth (≥10-bit) and resolution (≥4K).
- **Archival Candidate Logic**: Intelligently flags videos suitable for archival. A file is a candidate if it's `Lossless`, `Visually Lossless`, or uses a professional codec like `ProRes`.
- **Rich Metadata**: Extracts format, resolution, FPS, bit depth, color space, duration, and audio information.

🚀 **Intelligent `auto` Conversion Engine**
- **Smart Strategy**: Automatically determines the best conversion path:
  - **`Lossless` or `Visually Lossless` Source → FFV1 MKV**: For perfect, bit-for-bit archival. This applies to masters like ProRes, DNxHD, and other lossless formats.
  - **`High` or `Standard Quality` Source → AV1 MP4**: For efficient, high-quality compression.
- **Archival-Grade Parameters**: Uses community-recommended `ffmpeg` settings for FFV1 (`-level 3`, `-slices 24`, `-slicecrc 1`) to ensure a robust archival master.
- **Lossless Audio Handling**: Automatically converts audio to **FLAC** in FFV1 archives and **AAC 320k** in AV1 files.
- **`--explore` Mode**: For AV1 conversion, this unique feature finds the optimal file size by starting at a high quality (CRF 0) and incrementally lowering it until the output is smaller than the input (capped at CRF 23 for safety).
- **Metadata Preservation**: Automatically carries over metadata and file timestamps using `exiftool` and `touch` (if installed).

## Installation

### Prerequisites

`vidquality` requires a recent version of **FFmpeg** (which includes `ffprobe`) to be installed and available in your system's `PATH`.

```bash
# On macOS using Homebrew
brew install ffmpeg

# For metadata preservation (recommended)
brew install exiftool
```

### Build & Install

```bash
# Navigate to the project directory
cd /path/to/vidquality_API

# Build the release binary
cargo build --release

# The binary will be at: ./target/release/vidquality

# Optional: Install to your system path
cargo install --path .
```

## Command Usage

### 1. `analyze`: Deep Video Analysis

Displays a detailed technical report.

```bash
vidquality analyze "ProRes_Master.mov"
```
*Example Output:*
```
📊 Video Analysis Report
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 File: ProRes_Master.mov
📦 Format: mov,mp4,m4a,3gp,3g2,mj2
🎬 Codec: ProRes (Apple ProRes)
🔍 Compression: Visually Lossless
...
⭐ Quality Score: 98/100 (Base:95 + Depth Bonus:3)
📦 Archival Candidate: ✅ Yes
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### 2. `strategy`: Preview the Conversion Plan

Performs a "dry run" to show what the `auto` command will do, without executing.

```bash
vidquality strategy "youtube_dl.mkv"
```
*Example Output:*
```
🎯 Recommended Strategy (Auto Mode)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 File: youtube_dl.mkv
🎬 Codec: H.264 (Standard Quality)
💡 Target: AV1 MP4 (High Quality)
📝 Reason: Source is H.264 (Standard Quality) - compressing with AV1 CRF 0
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### 3. `auto`: Smart Automatic Conversion (Recommended)

The `auto` command is the main function, intelligently converting a video based on the analysis.

#### Archival Example
```bash
vidquality auto "ProRes_Master.mov" --output ./archive/
```
*Log:*
```
INFO  vidquality::conversion_api: 🎬 Auto Mode: ProRes_Master.mov → FFV1 MKV (Archival)
INFO  vidquality::conversion_api:    Reason: Source is ProRes (Visually Lossless) - preserving with FFV1 MKV
...
INFO  vidquality::conversion_api:    ✅ Complete: 75.3% of original
```

#### Compression with Size Exploration
```bash
vidquality auto "youtube_dl.mkv" --output ./compressed/ --explore
```
*Log:*
```
INFO  vidquality::conversion_api: 🎬 Auto Mode: youtube_dl.mkv → AV1 MP4 (High Quality)
INFO  vidquality::conversion_api:    🔍 Exploring smaller size (input: 50000000 bytes)
INFO  vidquality::conversion_api:    📊 CRF 0: 58000000 bytes (116.0%)
INFO  vidquality::conversion_api:    📊 CRF 1: 54000000 bytes (108.0%)
...
INFO  vidquality::conversion_api:    📊 CRF 5: 48500000 bytes (97.0%)
INFO  vidquality::conversion_api:    ✅ Found smaller output at CRF 5
...
INFO  vidquality::conversion_api: 📊 Conversion Summary:
...
INFO  vidquality::conversion_api:    🔍 Explored 6 CRF values, final: CRF 5
```

### 4. `simple`: Convert Everything to High-Quality AV1

A direct mode that converts any input video to AV1/MP4 using `CRF 0` for visually lossless results. This is for quick compression when archival strategy is not needed.

```bash
vidquality simple "screencast.mov" --output ./videos/
```

### 5. `--lossless`: Mathematical Lossless AV1 (⚠️ SLOW)

Both `auto` and `simple` support the `--lossless` flag for true **mathematical lossless** AV1 encoding. This produces bit-perfect output but is **VERY SLOW** and creates **huge files**.

```bash
# Lossless AV1 in simple mode
vidquality simple "video.mov" --output ./output/ --lossless

# Lossless AV1 in auto mode (only affects lossy sources)
vidquality auto "video.mp4" --output ./output/ --lossless
```

> ⚠️ **Warning**: Mathematical lossless AV1 encoding is extremely slow (10-100x slower than lossy). Use only when bit-perfect quality is essential and file size is not a concern.

---

# vidquality - 智能视频归档与压缩工具

**一款高性能的命令行工具，用于深度视频分析和智能、保质量的格式转换，专注于 FFV1 归档和 AV1 现代压缩。**

`vidquality` 使用 `ffmpeg` 和 `ffprobe` 分析视频文件以确定最佳转换策略，其构建于一套健壮、高质量的媒体保存与分发理念之上。

## 核心理念

1.  **归档优先**: 优先使用行业顶级的归档编码器 **FFV1**，并采用推荐的参数配置，以最稳健的方式保存高质量和无损的母版文件。
2.  **高效压缩**: 使用 **AV1 编码器** 从有损源文件创建高质量、高效率的分发副本。
3.  **清晰明确**: 基于源文件的技术属性，提供详细的分析，以解释*为什么*选择某种特定的转换策略。

## 功能特性

✨ **深度视频分析**
- **编码与压缩分析**: 能检测特定编码器（如 `ProRes`, `DNxHD`, `H.264`），并将其分为 `无损`, `视觉无损`, `高质量` 或 `标准质量`。
- **质量分数 (0-100)**: 基于压缩类型计算启发式分数，并为高位深度 (≥10-bit) 和高分辨率 (≥4K) 提供额外加分。
- **归档候选逻辑**: 智能标记适合归档的视频。如果文件是 `无损`、`视觉无损` 或使用如 `ProRes` 等专业编码器，它就会被视为候选。
- **丰富的元数据**: 提取格式、分辨率、帧率、位深度、色彩空间、时长和音频信息。

🚀 **智能 `auto` 转换引擎**
- **智能策略**: 自动确定最佳转换路径：
  - **`无损` 或 `视觉无损` 源文件 → FFV1 MKV**: 用于完美的、逐比特的数字归档。适用于 ProRes、DNxHD 等母版文件。
  - **`高质量` 或 `标准质量` 源文件 → AV1 MP4**: 用于高效、高质量的压缩。
- **归档级参数**: 为 FFV1 使用社区推荐的 `ffmpeg` 设置 (`-level 3`, `-slices 24`, `-slicecrc 1`)，确保归档母版的稳健性。
- **无损音频处理**: 在 FFV1 归档中自动将音频转换为 **FLAC**（无损音频），在 AV1 文件中转换为 **AAC 320k**。
- **`--explore` 模式**: 在 AV1 转换中，这个独特功能可通过从高质量（CRF 0）开始，逐步降低质量直到输出文件小于输入文件（为安全起见，上限为 CRF 23），来找到最佳文件大小。
- **元数据保留**: 如果安装了 `exiftool` 和 `touch`，会自动迁移元数据和文件时间戳。

## 安装

### 前置依赖

`vidquality` 需要在您的系统 `PATH` 中安装并配置好最新版的 **FFmpeg** (它包含了 `ffprobe`)。

```bash
# 在 macOS 上使用 Homebrew
brew install ffmpeg

# 为了保留元数据（推荐）
brew install exiftool
```

### 编译与安装

```bash
# 导航至项目目录
cd /path/to/vidquality_API

# 编译 Release 版本
cargo build --release

# 二进制文件位于 ./target/release/vidquality

# 可选：将程序安装到系统路径
cargo install --path .
```

## 命令用法

### 1. `analyze`: 深度视频分析

显示详细的技术报告。

```bash
vidquality analyze "ProRes_Master.mov"
```
*输出示例:*
```
📊 视频分析报告
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 文件: ProRes_Master.mov
📦 格式: mov,mp4,m4a,3gp,3g2,mj2
🎬 编码: ProRes (Apple ProRes)
🔍 压缩: 视觉无损
...
⭐ 质量分数: 98/100 (基础:95 + 位深度加分:3)
📦 归档候选: ✅ 是
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### 2. `strategy`: 预览转换计划

执行一次“空运行”，显示 `auto` 命令将执行的操作，而不实际转换。

```bash
vidquality strategy "youtube_dl.mkv"
```
*输出示例:*
```
🎯 推荐策略 (Auto 模式)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 文件: youtube_dl.mkv
🎬 编码: H.264 (标准质量)
💡 目标: AV1 MP4 (高质量)
📝 原因: 源文件是 H.264 (标准质量) - 使用 AV1 CRF 0 进行压缩
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### 3. `auto`: 智能自动转换 (推荐)

`auto` 是核心命令，它会根据分析结果智能地转换视频。

#### 归档示例
```bash
vidquality auto "ProRes_Master.mov" --output ./archive/
```
*日志:*
```
INFO  vidquality::conversion_api: 🎬 Auto 模式: ProRes_Master.mov → FFV1 MKV (归档)
INFO  vidquality::conversion_api:    原因: 源文件是 ProRes (视觉无损) - 使用 FFV1 MKV 进行保存
...
INFO  vidquality::conversion_api:    ✅ 完成: 体积为原始文件的 75.3%
```

#### 使用尺寸探索进行压缩
```bash
vidquality auto "youtube_dl.mkv" --output ./compressed/ --explore
```
*日志:*
```
INFO  vidquality::conversion_api: 🎬 Auto 模式: youtube_dl.mkv → AV1 MP4 (高质量)
INFO  vidquality::conversion_api:    🔍 正在探索更小体积 (输入: 50000000 字节)
INFO  vidquality::conversion_api:    📊 CRF 0: 58000000 字节 (116.0%)
INFO  vidquality::conversion_api:    📊 CRF 1: 54000000 字节 (108.0%)
...
INFO  vidquality::conversion_api:    📊 CRF 5: 48500000 字节 (97.0%)
INFO  vidquality::conversion_api:    ✅ 在 CRF 5 找到更小的输出
...
INFO  vidquality::conversion_api: 📊 转换总结:
...
INFO  vidquality::conversion_api:    🔍 探索了 6 个 CRF 值, 最终使用: CRF 5
```

### 4. `simple`: 将所有文件转换为高质量 AV1

一个直接的模式，将任何输入视频都使用 `CRF 0` 转换为 AV1/MP4 以获得视觉无损的结果。适用于不需要归档策略的快速批量压缩。

```bash
vidquality simple "screencast.mov" --output ./videos/
```

### 5. `--lossless`: 数学无损 AV1 (⚠️ 极慢)

`auto` 和 `simple` 均支持 `--lossless` 标志，用于真正的**数学无损** AV1 编码。这将产生逐比特完美的输出，但**非常慢**且生成**巨大的文件**。

```bash
# 简单模式下的无损 AV1
vidquality simple "video.mov" --output ./output/ --lossless

# 自动模式下的无损 AV1 (仅对有损源生效)
vidquality auto "video.mp4" --output ./output/ --lossless
```

> ⚠️ **警告**: 数学无损 AV1 编码极其缓慢（比有损慢 10-100 倍）。仅在需要逐比特完美质量且文件大小不是问题时使用。