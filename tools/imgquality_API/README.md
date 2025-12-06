# imgquality - Intelligent Image Analysis & Conversion

**A high-performance, parallel-processing CLI tool for deep image analysis and smart, quality-preserving format upgrades.** It provides technically detailed analysis and opinionated conversion strategies to ensure the highest quality results and prevent common mistakes like re-compressing lossy files.

## Core Philosophy

1.  **Preserve Quality**: Never degrade quality by re-compressing a lossy file (e.g., lossy WebP or JPEG) into another lossy format, unless explicitly creating a mathematical lossless version.
2.  **Maximize Efficiency**: Use the best modern codecs for the job: JPEG XL (JXL) for most cases, and AVIF/AV1 for specific lossy-to-lossless or animated use cases.
3.  **Provide Clarity**: Offer detailed analysis so the user understands *why* a certain action is or is not recommended.

## Features

✨ **Comprehensive Quality Analysis**
- **Deep JPEG Analysis**: Estimates JPEG quality (`Q` score), identifies quantization tables (standard vs. custom), and detects encoder signatures.
- **HEIC/AVIF Aware**: Correctly identifies HEIC and AVIF as modern formats and advises against unnecessary conversion in standard mode.
- **Lossless/Lossy Detection**: Accurately identifies compression type for WebP, PNG, etc.
- **Rich Metadata**: Extracts color depth, color space, dimensions, alpha, and animation status.
- **Image Complexity**: Calculates entropy and compression ratio to measure image complexity.

🚀 **Intelligent `auto` Conversion Engine**
- **Smart Strategy**: Automatically chooses the best conversion path based on source properties.
  - **JPEG → JXL**: **True lossless transcode** (`--lossless_jpeg=1`) that preserves original DCT coefficients, reducing size by ~20% with zero quality loss.
  - **PNG/Lossless WebP/TIFF → JXL**: Mathematical lossless compression (`-d 0.0`), reducing size by 30-60%.
  - **Animated (Lossless) → AV1 MP4**: Converts lossless animations (e.g., GIF) to a highly efficient, visually lossless video.
  - **Static Lossy (non-JPEG) → AVIF**: Converts other static lossy files to AVIF for better compression.
- **Safe by Default**: Automatically **skips** converting lossy WebP and animated lossy files to prevent quality degradation.
- **Parallel Processing**: Uses all available CPU cores to process large directories in parallel.
- **Anti-Duplicate**: Remembers which files have been successfully processed to avoid redundant work on subsequent runs (can be overridden with `--force`).

⭐ **New: Mathematical Lossless Mode**
- **`--lossless` Flag**: A powerful new option for the `auto` command that overrides standard behavior. It will convert images (including lossy sources) into **mathematically lossless AVIF or AV1 files**. This is useful for creating archival masters from sources that are not JXL-compatible, but be aware:
  - **⚠️ It is extremely slow.**
  - **⚠️ It can result in very large files, sometimes larger than the original.**

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

### 2. `auto`: Smart Automatic Conversion (Recommended)

The `auto` command intelligently analyzes each image and converts it to the optimal format.

#### Standard Conversion
```bash
# Analyze and convert a directory, saving to a new location
imgquality auto ./input_dir --output ./output_dir
```

#### Mathematical Lossless Conversion
Use the `--lossless` flag to create archival-grade AVIF/AV1 files from any source.
```bash
# Convert a lossy WebP into a mathematically lossless AVIF
imgquality auto image.webp --output ./archive/ --lossless
```
*Log:*
```
⚠️  Mathematical lossless mode: ENABLED (VERY SLOW!)
📂 Found 1 files to process (parallel mode)
🔄 Lossy→AVIF (MATHEMATICAL LOSSLESS): image.webp
✅ Conversion successful...
```

### 3. `convert`: Manual Conversion

Manually convert images to a *specific* format. This command is less intelligent than `auto`.

```bash
imgquality convert image.png --to jxl --output ./converted/
```

### 4. `verify`: Verify Conversion Quality

Compares two images and calculates perceptual quality metrics. This performs a **full calculation**, which is more accurate than the *estimation* provided by the `analyze` command.

```bash
imgquality verify original.png converted.jxl
```
---

# imgquality - 智能图像分析与转换工具

**一款高性能、并行处理的命令行工具，用于深度图像质量分析和智能、保质量的格式升级。** 它提供技术上详尽的分析和带有明确观点的转换策略，以确保最高质量的转换结果，并防止诸如重复压缩有损文件等常见错误。

## 核心理念

1.  **保证质量**: 绝不通过将有损文件（如 JPEG 或有损 WebP）重新压缩为另一种有损格式而降低其质量，除非是明确创建数学无损版本。
2.  **极致效率**: 使用最优秀的现代编码器：JPEG XL (JXL) 用于大多数场景，AVIF/AV1 用于特定的有损转无损或动画场景。
3.  **清晰明确**: 提供详尽的分析，让用户理解*为什么*推荐或不推荐某个操作。

## 功能特性

✨ **全面的质量分析**
- **深度 JPEG 分析**: 估算 JPEG 质量值（`Q` 分数），识别量化表（标准 vs. 自定义），并检测编码器签名。
- **HEIC/AVIF 感知**: 在标准模式下能正确识别 HEIC 和 AVIF 为现代格式，并建议避免不必要的转换。
- **无损/有损检测**: 精准识别 WebP、PNG 等格式的压缩类型。
- **丰富的元数据**: 提取色深、色彩空间、尺寸、Alpha 通道和动画状态。
- **图像复杂度**: 通过计算熵和压缩率来衡量图像的复杂程度。

🚀 **智能 `auto` 转换引擎**
- **智能策略**: 根据源文件属性自动选择最佳转换路径。
  - **JPEG → JXL**: **真正无损转码** (`--lossless_jpeg=1`)，保留原始 DCT 系数，体积减少 ~20%。
  - **静态有损 (如有损 WebP) → JXL**: **视觉无损升级** (Quality 100)，防止有损转有损的代际损失，提供最佳的编辑和归档灵活性。
  - **PNG/TIFF → JXL**: 数学无损压缩 (`-d 0.0`)，减少 30-60% 体积。
  - **WebP/AVIF (有损)**: **自动跳过**。避免将已经是现代高效格式的有损文件再次转换，防止代际损耗。
  - **WebP/AVIF (无损)**: **转为 JXL 无损**。利用 JXL 更高的压缩效率进行优化。
  - **无损动画 → AV1 MP4**: 将无损动画转换为高效的视觉无损视频。
- **默认安全**: 自动跳过现代有损格式 (WebP/AVIF/HEIC) 以避质量下降。
- **最全面元数据保留**: 默认**强制**保留所有可能的元数据，无需任何参数：
  - **完整 Exif/IPTC/XMP**: 包括厂商私有标记 (MakerNotes)。
  - **时间戳完美复刻**:
    - **创建时间 (Creation Date)**: 尽全力保留 (`-FileCreateDate` / `-CreationDate`)。
    - **修改时间 (Modify Time)**: 完美保留。
    - **访问时间 (Access Time)**: 完美保留。
    > **⚠️ 注意**: `FileInodeChangeDate` (ctime) 是文件元数据变更时间，由操作系统内核强制更新，**无法保留原始值**。这是系统级安全特性，而非工具缺陷。
  - **文件权限**: 保持只读/读写属性。
- **并行处理**: 利用多核并行处理大批量图像。
- **防止重复**: 会记录已成功处理的文件，在后续运行时自动跳过，避免重复工作（可通过 `--force` 覆盖）。

⭐ **新功能: 数学无损模式**
- **`--lossless` 标志**: `auto` 命令的一个强大的新选项，它会覆盖标准行为。此模式会将图像（包括有损源）转换为**数学无损的 AVIF 或 AV1 文件**。这对于从不兼容 JXL 的源创建归档母版非常有用，但请注意：
  - **⚠️ 速度极慢。**
  - **⚠️ 生成的文件可能非常大，有时甚至比原文件还大。**

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

### 2. `auto`: 智能自动转换 (推荐)

`auto` 命令会智能分析每个图像并将其转换为最优格式。

#### 标准转换
```bash
# 分析并转换目录，保存到新位置
imgquality auto ./input_dir --output ./output_dir
```

#### 数学无损转换
使用 `--lossless` 标志从任何源创建归档级的 AVIF/AV1 文件。
```bash
# 将一个有损的 WebP 文件转换为数学无损的 AVIF
imgquality auto image.webp --output ./archive/ --lossless
```
*日志:*
```
⚠️  数学无损模式: 已启用 (速度极慢!)
📂 发现 1 个文件待处理 (并行模式)
🔄 有损→AVIF (数学无损): image.webp
✅ 转换成功...
```

### 3. `convert`: 手动转换

手动将图像转换为*特定*格式。此命令不如 `auto` 智能。

```bash
imgquality convert image.png --to jxl --output ./converted/
```

### 4. `verify`: 验证转换质量

比较两个图像并计算感知质量指标。此命令执行的是**完全计算**，比 `analyze` 命令提供的*估算值*更精确。

```bash
imgquality verify original.png converted.jxl
```