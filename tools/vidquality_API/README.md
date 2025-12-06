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

🚀 **Intelligent `auto` & `simple` Conversion Engines**
- **Smart `auto` Strategy**: Automatically determines the best conversion path:
  - **Modern Codecs (AV1/H.265/VP9 etc.)**: **Skip**. Detected modern formats are skipped to avoid generational loss.
  - **`Lossless` Source (FFV1/ProRes etc.) → AV1 Lossless**: Converts bulky lossless masters to **mathematically lossless** AV1 (CRF 0 + Lossless), significantly reducing size while maintaining bit-perfect quality.
  - **`Lossy` Source (H.264/MPEG etc.) → AV1 (CRF 0)**: Compresses using visually lossless CRF 0 settings for high quality.
- **Simple Mode**: Enforces **AV1 Mathematical Lossless** mode by default for absolute quality preservation.
- **Archival-Grade Parameters**: Uses CRF 0 for visually lossless results on lossy sources.
- **Lossless Audio Handling**: Automatically converts audio to **FLAC** or high-quality AAC.
- **`--explore` Mode**: For the `auto` command, starts from CRF 0 and finds the optimal size.
- **Most Comprehensive Metadata Preservation**: default **Mandatory** use of `exiftool` (if installed) and system APIs:

⭐ **New: Mathematical Lossless AV1 Mode**
- **`--lossless` Flag**: A powerful new option for `auto` and `simple` commands. It forces the conversion to use **mathematically lossless AV1**. This is useful for creating archival masters from sources where FFV1 is not desired.
  - **⚠️ It is extremely slow.**
  - **⚠️ It can result in very large files, sometimes larger than the original.**

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

### 2. `strategy`: Preview the Conversion Plan

Performs a "dry run" to show what the `auto` command will do, without executing.

```bash
vidquality strategy "youtube_dl.mkv"
```

### 3. `auto`: Smart Automatic Conversion (Recommended)

The `auto` command is the main function, intelligently converting a video based on the analysis.

#### Archival Example
Converts a high-quality source to a robust FFV1/MKV archival master.
```bash
vidquality auto "ProRes_Master.mov" --output ./archive/
```

#### Compression with Size Exploration
Converts a lossy source to AV1, finding the best size/quality trade-off.
```bash
vidquality auto "youtube_dl.mkv" --output ./compressed/ --explore
```

#### Mathematical Lossless AV1 Archival
Overrides the default to create a lossless AV1 archive instead of FFV1.
```bash
vidquality auto "ProRes_Master.mov" --output ./archive/ --lossless
```
*Log:*
```
🎬 Auto Mode Conversion
   ⚠️  Mathematical lossless AV1: ENABLED (VERY SLOW!)
...
```

### 4. `simple`: Convert Everything to High-Quality AV1

A direct mode to convert any input video to AV1/MP4.

#### Visually Lossless (Default)
Uses `CRF 0` for visually lossless results.
```bash
vidquality simple "screencast.mov" --output ./videos/
```

#### Mathematically Lossless
Uses the `--lossless` flag for true lossless conversion.
```bash
vidquality simple "screencast.mov" --output ./videos/ --lossless
```
*Log:*
```
🎬 Simple Mode Conversion
   ⚠️  ALL videos → AV1 MP4 (MATHEMATICAL LOSSLESS - VERY SLOW!)
...
```

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

🚀 **智能 `auto` & `simple` 转换引擎**
- **智能 `auto` 策略**: 自动确定最佳转换路径：
  - **现代编码 (AV1/H.265/VP9等)**: **自动跳过**。源文件已是高效格式，避免无效重编码和代际损耗。
  - **无损源文件 (FFV1/ProRes等) → AV1 Lossless**: 将庞大的无损母版转换为**数学无损**的 AV1 (CRF 0 + Lossless)，在保持逐比特一致的同时显著减小体积。
  - **有损源文件 (H.264/MPEG等) → AV1 (CRF 0)**: 使用视觉无损的 CRF 0 参数进行高质量压缩。
- **Simple 模式**: 默认强制使用 **AV1 数学无损** 模式，确保绝对的质量保留。
- **归档级参数**: 针对有损转换使用 CRF 0 确保视觉无损。
- **无损音频处理**: 自动将音频转换为 **FLAC** 或高码率 AAC。
- **`--explore` 模式**: 在 `auto` 命令中，从 CRF 0 开始尝试，直到找到比源文件更小的体积。
- **最全面元数据保留**: 默认**强制**使用 `exiftool`（如已安装）和系统 API 进行最大程度的元数据迁移：
  - **完整 Exif/IPTC/XMP**: 无损复制所有标签。
  - **系统时间戳**: 完美复刻文件创建时间 (CreationDate/Btime) 和修改时间。
  - **文件权限**: 保持原始文件的读写权限属性。

⭐ **新功能: 数学无损 AV1 模式**
- **`--lossless` 标志**: `auto` 和 `simple` 命令的一个强大的新选项。它会强制转换使用**数学无损的 AV1**。这对于从不希望使用 FFV1 的源创建归档母版非常有用。
  - **⚠️ 速度极慢。**
  - **⚠️ 生成的文件可能非常大，有时甚至比原文件还大。**

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

### 2. `strategy`: 预览转换计划

执行一次“空运行”，显示 `auto` 命令将执行的操作，而不实际转换。

```bash
vidquality strategy "youtube_dl.mkv"
```

### 3. `auto`: 智能自动转换 (推荐)

`auto` 是核心命令，它会根据分析结果智能地转换视频。

#### 归档示例
将高质量源文件转换为健壮的 FFV1/MKV 归档母版。
```bash
vidquality auto "ProRes_Master.mov" --output ./archive/
```

#### 使用尺寸探索进行压缩
将有损源文件转换为 AV1，并找到最佳的体积/质量平衡点。
```bash
vidquality auto "youtube_dl.mkv" --output ./compressed/ --explore
```

#### 数学无损 AV1 归档
覆盖默认行为，创建一个无损的 AV1 归档文件而不是 FFV1。
```bash
vidquality auto "ProRes_Master.mov" --output ./archive/ --lossless
```
*日志:*
```
🎬 Auto 模式转换
   ⚠️  数学无损 AV1: 已启用 (速度极慢!)
...
```

### 4. `simple`: 将所有文件转换为高质量 AV1

一个直接的模式，将任何输入视频都转换为 AV1/MP4。

#### 视觉无损 (默认)
使用 `CRF 0` 以获得视觉无损的结果。
```bash
vidquality simple "screencast.mov" --output ./videos/
```

#### 数学无损
使用 `--lossless` 标志进行真正的无损转换。
```bash
vidquality simple "screencast.mov" --output ./videos/ --lossless
```
*日志:*
```
🎬 Simple 模式转换
   ⚠️  所有视频 → AV1 MP4 (数学无损 - 速度极慢!)
...
```
