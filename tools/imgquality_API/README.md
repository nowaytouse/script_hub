# imgquality - Intelligent Image Analysis & Conversion

**High-performance, parallel-processing CLI tool for deep image quality analysis and smart, quality-preserving format upgrades.**

## Features

✨ **Comprehensive Quality Analysis**
- **Lossless/Lossy Detection**: Accurately identifies compression type for PNG, WebP, JPEG, etc.
- **Deep JPEG Analysis**: Estimates JPEG quality (`Q`), identifies quantization tables (standard vs. custom), and detects encoder signatures.
- **HEIC/HEIF Analysis**: Parses and analyzes High-Efficiency Image Format containers.
- **Advanced Quality Metrics**: Precise PSNR, SSIM, and MS-SSIM calculations for verifying conversion quality.
- **Rich Metadata**: Extracts color depth, color space, dimensions, alpha channel, and animation status.

🚀 **Intelligent `auto` Conversion Engine**
- **Smart Strategy**: Automatically chooses the best output format based on source properties.
  - **JPEG → JXL**: Lossless transcoding, reducing size by 20-30% with perfect quality.
  - **PNG/Lossless → JXL**: Mathematical lossless compression, reducing size by 30-60%.
  - **Static Lossy → AVIF**: Converts other lossy formats to AVIF for maximum efficiency.
  - **Animated → AV1 MP4**: Converts animated GIFs/WebPs to high-quality, efficient video.
- **Parallel Processing**: Uses `rayon` to process large directories of images in parallel, maximizing CPU usage.
- **Safety First**: Operations are logged, and original files can be automatically deleted upon successful conversion.

💡 **CLI & API Modes**
- **Interactive CLI**: Rich, human-readable output for easy analysis.
- **JSON API**: Provides structured `json` output for seamless integration with scripts, frontends, or other tools.

## Installation

### Prerequisites

```bash
# Required: Install JPEG XL toolkit
brew install jpeg-xl

# Required: Install AVIF toolkit (libavif)
brew install libavif

# Required: Install FFmpeg for animated conversions
brew install ffmpeg

# Optional: For metadata preservation
brew install exiftool
```

### Build & Install

```bash
# Navigate to the project directory
cd /path/to/imgquality

# Build the release binary
cargo build --release

# The binary will be at: ./target/release/imgquality

# Optional: Install to your system path
cargo install --path .
```

## Command Usage

### 1. `analyze`: Deep Image Analysis

Provides a detailed report on an image's technical properties.

#### Single File (Human-Readable)
```bash
imgquality analyze image.jpg
```
*Example Output:*
```
📊 Image Quality Analysis Report
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 File: image.jpg
📷 Format: JPEG (Lossy)
📐 Dimensions: 4032x3024
💾 Size: 2.50 MB
🎨 Bit depth: 8-bit sRGB
...
📈 Quality Analysis
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🎯 JPEG Quality Analysis (Accuracy: ±2)
📊 Estimated quality: Q=95 (Excellent)
🎯 Confidence:   98.5%
📋 Quantization table:   IJG Standard ✓
🏭 Encoder:   Apple HEIC
✨ Assessment: High quality original
```

#### Get Upgrade Recommendation
Use the `--recommend` flag to get a smart conversion suggestion.
```bash
imgquality analyze photo.png --recommend
```
*Additional Output:*
```
💡 JXL Format Recommendation
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✅ PNG → JXL
📝 Reason: Lossless source can be losslessly compressed to JXL for significant space savings.
🎯 Quality: Mathematically Lossless
💾 Expected Reduction: 45.1%
⚙️ Command: cjxl 'photo.png' '{output}.jxl' -d 0.0 -e 8
```

#### Batch Analysis (JSON Output)
Analyze a directory and output structured JSON, perfect for scripting.
```bash
imgquality analyze ./my_images --recursive --output json --recommend
```

### 2. `auto`: Smart Automatic Conversion (Recommended)

The `auto` command intelligently analyzes each image and converts it to the optimal format using parallel processing. **This is the recommended command for batch processing.**

#### Convert a Directory
Converts all supported images in `./input_dir` and saves them to `./output_dir`.
```bash
imgquality auto ./input_dir --output ./output_dir
```

#### In-Place Conversion with Deletion
Convert recursively, and delete original files after a successful conversion. **Use with caution.**
```bash
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
⏭️ Skipping already-optimal file: ./media/icon.avif
...
✅ Auto-conversion complete: 148 succeeded, 2 skipped, 2 failed
```

### 3. `convert`: Manual Conversion

Manually convert one or more images to a *specific* target format. Less intelligent than `auto`.

```bash
# Convert a single file to JXL
imgquality convert image.png --to jxl

# Convert a directory, replacing original files
imgquality convert ./images --to jxl --in-place --recursive
```

### 4. `verify`: Verify Conversion Quality

Compares an original and converted image, calculating size reduction and perceptual quality metrics.

```bash
imgquality verify original.png converted.jxl
```
*Example Output:*
```
🔍 Verifying conversion quality...
   Original:  original.png
   Converted: converted.jxl

📊 Size Comparison:
   Original size:  1.43 MB
   Converted size: 750.12 KB
   Size reduction: 47.5%

📏 Quality Metrics:
   PSNR: ∞ dB (Identical - mathematically lossless)
   SSIM: 1.000000 (Identical)
   MS-SSIM: 1.000000 (Identical)

✅ Verification complete: Conversion is mathematically lossless.
```

## API for Developers

### Node.js Example

Use the JSON output to integrate `imgquality` into your applications.

```javascript
const { exec } = require('child_process');
const util = require('util');
const execPromise = util.promisify(exec);

async function analyzeAndConvert(imagePath, outputDir) {
  try {
    // The 'auto' command is simple and powerful
    const command = `imgquality auto "${imagePath}" --output "${outputDir}"`;
    const { stdout, stderr } = await execPromise(command);
    
    console.log('Conversion Log:', stdout);
    if (stderr) {
      console.error('Error:', stderr);
    }
  } catch (error) {
    console.error('Failed to execute imgquality:', error);
  }
}

// Example usage
(async () => {
  await analyzeAndConvert('~/Pictures/my_photo.heic', '~/Pictures/output');
})();
```

---

# imgquality - 智能图像分析与转换工具

**一款高性能、并行处理的命令行工具，用于深度图像质量分析和智能、保质量的格式升级。**

## 功能特性

✨ **全面的质量分析**
- **无损/有损检测**: 精准识别 PNG、WebP、JPEG 等格式的压缩类型。
- **深度 JPEG 分析**: 估算 JPEG 质量值（`Q`），识别量化表（标准 vs. 自定义），并检测编码器签名。
- **HEIC/HEIF 分析**: 解析并分析高效率图像格式容器。
- **高级质量度量**: 精确计算 PSNR、SSIM 和 MS-SSIM，用于验证转换质量。
- **丰富的元数据**: 提取色深、色彩空间、尺寸、Alpha 通道和动画状态。

🚀 **智能 `auto` 转换引擎**
- **智能策略**: 根据源文件属性自动选择最佳输出格式。
  - **JPEG → JXL**: 无损转码，体积减少 20-30%，质量完美。
  - **PNG/无损 → JXL**: 数学无损压缩，体积减少 30-60%。
  - **静态有损 → AVIF**: 将其他有损格式转换为 AVIF 以实现最高效率。
  - **动画 → AV1 MP4**: 将 GIF/WebP 动图转换为高质量、高效率的视频。
- **并行处理**: 使用 `rayon` 并行处理大批量图像，充分利用 CPU 性能。
- **安全第一**: 操作均有日志记录，并可在成功转换后自动删除原文件。

💡 **CLI 与 API 双模式**
- **交互式 CLI**: 提供丰富、人类可读的输出，便于手动分析。
- **JSON API**: 提供结构化的 `json` 输出，可无缝集成到脚本、前端或其他工具中。

## 安装

### 前置依赖

```bash
# 必需：安装 JPEG XL 工具包
brew install jpeg-xl

# 必需：安装 AVIF 工具包 (libavif)
brew install libavif

# 必需：安装 FFmpeg 用于动画转换
brew install ffmpeg

# 可选：用于保留元数据
brew install exiftool
```

### 编译与安装

```bash
# 导航至项目目录
cd /path/to/imgquality

# 编译 Release 版本
cargo build --release

# 二进制文件位于 ./target/release/imgquality

# 可选：将程序安装到系统路径
cargo install --path .
```

## 命令用法

### 1. `analyze`: 深度图像分析

提供关于图像技术属性的详细报告。

#### 单文件分析（人类可读）
```bash
imgquality analyze image.jpg
```
*输出示例:*
```
📊 图像质量分析报告
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 文件: image.jpg
📷 格式: JPEG (有损)
📐 尺寸: 4032x3024
💾 体积: 2.50 MB
🎨 位深度: 8-bit sRGB
...
📈 质量分析
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🎯 JPEG 质量分析 (精度: ±2)
📊 估算质量: Q=95 (极佳)
🎯 置信度:   98.5%
📋 量化表:   IJG 标准 ✓
🏭 编码器:   Apple HEIC
✨ 评估: 高质量原图
```

#### 获取升级建议
使用 `--recommend` 标志获取智能转换建议。
```bash
imgquality analyze photo.png --recommend
```
*额外输出:*
```
💡 JXL 格式推荐
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✅ PNG → JXL
📝 原因: 无损源文件可以无损压缩为 JXL，以节省大量空间。
🎯 质量: 数学无损
💾 预期减少: 45.1%
⚙️ 命令: cjxl 'photo.png' '{output}.jxl' -d 0.0 -e 8
```

#### 批量分析 (JSON 输出)
分析目录并输出结构化 JSON，非常适合脚本处理。
```bash
imgquality analyze ./my_images --recursive --output json --recommend
```

### 2. `auto`: 智能自动转换 (推荐)

`auto` 命令会智能分析每个图像，并使用并行处理将其转换为最优格式。**这是进行批量处理时最推荐的命令。**

#### 转换目录
转换 `./input_dir` 中的所有支持的图像，并保存到 `./output_dir`。
```bash
imgquality auto ./input_dir --output ./output_dir
```

#### 就地转换并删除原文件
递归转换，并在成功后删除原文件。**请谨慎使用此选项。**
```bash
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
⏭️ 跳过已最优化的文件: ./media/icon.avif
...
✅ 自动转换完成: 148 成功, 2 跳过, 2 失败
```

### 3. `convert`: 手动转换

手动将一个或多个图像转换为*特定*的目标格式。此命令不如 `auto` 智能。

```bash
# 将单个文件转换为 JXL
imgquality convert image.png --to jxl

# 转换目录，并替换原文件
imgquality convert ./images --to jxl --in-place --recursive
```

### 4. `verify`: 验证转换质量

比较原始图像和转换后图像，计算体积减少率和感知质量指标。

```bash
imgquality verify original.png converted.jxl
```
*输出示例:*
```
🔍 正在验证转换质量...
   原始文件:  original.png
   转换后文件: converted.jxl

📊 体积对比:
   原始体积:  1.43 MB
   转换后体积: 750.12 KB
   体积减少: 47.5%

📏 质量度量:
   PSNR: ∞ dB (完全相同 - 数学无损)
   SSIM: 1.000000 (完全相同)
   MS-SSIM: 1.000000 (完全相同)

✅ 验证完成: 转换是数学无损的。
```

## 开发者 API

### Node.js 示例

使用 JSON 输出将 `imgquality` 集成到您的应用程序中。

```javascript
const { exec } = require('child_process');
const util = require('util');
const execPromise = util.promisify(exec);

async function analyzeAndConvert(imagePath, outputDir) {
  try {
    // 'auto' 命令既简单又强大
    const command = `imgquality auto "${imagePath}" --output "${outputDir}"`;
    const { stdout, stderr } = await execPromise(command);
    
    console.log('转换日志:', stdout);
    if (stderr) {
      console.error('错误:', stderr);
    }
  } catch (error) {
    console.error('执行 imgquality 失败:', error);
  }
}

// 使用示例
(async () => {
  await analyzeAndConvert('~/Pictures/my_photo.heic', '~/Pictures/output');
})();
```