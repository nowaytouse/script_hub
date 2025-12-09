# imgquality - 图像质量分析与格式升级工具 | Image Quality Analyzer

[English](#english) | [中文](#中文)

---

## 中文

高精度图像质量分析工具，支持 JPEG 质量检测（精度 ±1）和智能格式升级。

### 与 imgquality-hevc 的区别

| 特性 | imgquality (本工具) | imgquality-hevc |
|------|-------------------|-----------------|
| 静态图输出 | JXL | JXL |
| 动图输出 | **AV1 MP4** | HEVC MP4 |
| 编码器 | **SVT-AV1** | libx265 |
| 默认 CRF | 0 | 0 |
| 压缩效率 | **最佳** | 较好 |
| 兼容性 | 较好 | 极佳 (Apple/硬件) |
| 编码速度 | 中等 (SVT-AV1 比 libaom 快 10-20 倍) | 快 |

**选择建议**:
- 追求最佳压缩率 → **imgquality (AV1)**
- 需要 Apple 设备兼容 → imgquality-hevc
- 需要快速编码 → imgquality-hevc

### 功能特性

- 🔍 **JPEG 质量检测**: 通过量化表分析，精度达到 ±1
- 📊 **图像特征分析**: 熵值、压缩比、色彩空间等
- 🔄 **智能格式转换**: 静态图→JXL，动图→AV1 MP4
- 🎯 **质量匹配模式**: 自动计算匹配输入质量的输出参数
- 📦 **元数据保留**: 完整保留 EXIF/IPTC、ICC 颜色配置文件和文件属性
- ⏭️ **智能回退**: 转换后变大则自动回退跳过
- 📈 **进度条**: 带 ETA 估算的可视化进度条
- 🛡️ **安全检查**: 危险目录检测，防止误操作
- 🚀 **SVT-AV1 编码器**: 比 libaom-av1 快 10-20 倍

### 架构说明

本工具使用 `shared_utils` 共享库提供以下功能：
- **元数据保留** (`shared_utils::metadata`): ExifTool 封装 + 跨平台原生 API
- **进度条** (`shared_utils::progress`): 带 ETA 估算的可视化进度
- **安全检查** (`shared_utils::safety`): 危险目录检测
- **批量处理** (`shared_utils::batch`): 统一的批量处理报告
- **视频处理** (`shared_utils::video`): 偶数尺寸修正、滤镜链生成

### 命令概览

```bash
imgquality <COMMAND>

Commands:
  analyze   分析图像质量参数
  auto      智能自动转换（推荐）
  verify    验证转换质量（PSNR/SSIM）
```

### Auto 模式转换逻辑

Auto 模式根据输入格式和特性智能选择转换策略：

| 输入类型 | 条件 | 输出 | 说明 |
|---------|------|------|------|
| JPEG | 默认 | JXL (无损转码) | 保留 DCT 系数，零质量损失 |
| JPEG | `--match-quality` | JXL (有损) | 匹配原始质量，更好压缩 |
| PNG/TIFF/BMP (无损) | - | JXL (d=0) | 数学无损 |
| WebP/AVIF/HEIC (无损) | - | JXL (d=0) | 数学无损 |
| WebP/AVIF/HEIC (有损) | - | 跳过 | 避免代际损失 |
| 动图 (无损) | ≥3秒 | **AV1 MP4 CRF 0** | 视觉无损 (SVT-AV1) |
| 动图 (有损) | ≥3秒 + `--match-quality` | **AV1 MP4 CRF 18-35** | 匹配质量 |
| 动图 | <3秒 | 跳过 | 短动画不转换 |
| 动图 | `--lossless` | **AV1 MKV 无损** | SVT-AV1 lossless 模式 |

### 智能回退机制

当转换后文件体积变大时，工具会自动：
1. 删除输出文件
2. 跳过该文件
3. 输出清晰消息：`⏭️ Rollback: JXL larger than original`

这对于小型 PNG 或已高度优化的图片非常有用，避免转换后体积反而增大。

### --match-quality 算法

#### 静态图像 (JPEG)

直接使用检测到的 JPEG 质量值计算 JXL distance：

```
distance = (100 - jpeg_quality) / 10

示例:
Q100 → d=0.0 (无损)
Q90  → d=1.0
Q85  → d=1.5
Q80  → d=2.0
```

#### 静态图像 (非 JPEG)

基于 bytes-per-pixel 估算质量：

```
estimated_quality = 70 + 15 * log2(effective_bpp * 5)
distance = (100 - estimated_quality) / 10

考虑因素:
- 格式效率 (WebP=0.8, AVIF=0.7, PNG=1.5)
- 色彩深度 (8-bit=1.0, 16-bit=2.0)
- Alpha 通道 (有=1.33, 无=1.0)
```

#### 动图 → AV1 MP4 (SVT-AV1)

基于 bytes-per-pixel-per-second 计算 CRF：

```
CRF = 63 - 8 * log2(effective_bpps * 1000)
范围: [18, 35]

考虑因素:
- 格式效率 (GIF=2.5, APNG=1.5, WebP=1.0)
- 色彩深度 (8-bit=1.3, 其他=1.0)
- 分辨率 (>2MP=0.8, >0.5MP=0.9, 其他=1.0)
- Alpha 通道 (有=0.9, 无=1.0)
```

### 使用示例

```bash
# 分析图像质量
imgquality analyze image.jpg -r

# 智能转换（默认无损）
imgquality auto image.jpg

# 智能转换（匹配质量，更好压缩）
imgquality auto image.jpg --match-quality

# 批量转换目录（带进度条）
imgquality auto ./photos/ -r --match-quality

# 转换后删除原文件
imgquality auto image.jpg --delete-original

# 强制数学无损（动图→AV1 MKV）
imgquality auto animation.gif --lossless

# 验证转换质量
imgquality verify original.jpg converted.jxl
```

### 性能优化

- **并发限制**: 使用 CPU 核心数的一半（最少 1，最多 4）
- **线程限制**: cjxl 添加 `-j` 参数，FFmpeg 添加 `-threads` 参数
- **SVT-AV1**: 添加 `lp=N` 参数限制逻辑处理器数
- **避免系统卡顿**: 留出资源给系统和编码器内部线程

### 输出示例

#### 分析输出
```
📊 Image Quality Analysis Report
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 File: photo.jpg
📷 Format: JPEG (Lossy)
📐 Dimensions: 4000x3000
💾 Size: 2,456,789 bytes (2.34 MB)

🎯 JPEG Quality Analysis (精度: ±1)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 Estimated quality: Q=85 (High Quality)
🎯 Confidence: 98.5%
```

#### 转换输出（智能回退）
```
🔄 Legacy Lossless→JXL: small_icon.png
   ⏭️  Rollback: JXL larger than original (1024 → 1536 bytes, +50.0%)
✅ Skipped: JXL would be larger (+50.0%)
```

#### 转换输出（成功）
```
🔄 JPEG→JXL (MATCH QUALITY): photo.jpg
   📊 Quality Analysis (JPEG):
      JPEG Quality: Q85
      Confidence: 98.5%
      Calculated JXL distance: 1.50
   🎯 Matched JXL distance: 1.50
✅ Quality-matched JXL (d=1.50): size reduced 25.3%
```

### 依赖

#### 外部工具
- `cjxl` (libjxl) - JXL 编码
- `djxl` (libjxl) - JXL 解码（验证用）
- `ffmpeg` (带 libsvtav1) - 动图转 AV1 视频
- `exiftool` - 元数据处理

#### Rust 依赖
- `shared_utils` - 共享工具库（元数据、进度条、安全检查、视频处理）

---

## English

High-precision image quality analysis tool with JPEG quality detection (±1 accuracy) and smart format upgrade.

### Difference from imgquality-hevc

| Feature | imgquality (this tool) | imgquality-hevc |
|---------|----------------------|-----------------|
| Static Output | JXL | JXL |
| Animation Output | **AV1 MP4** | HEVC MP4 |
| Encoder | **SVT-AV1** | libx265 |
| Default CRF | 0 | 0 |
| Compression | **Best** | Good |
| Compatibility | Good | Excellent (Apple/Hardware) |
| Encoding Speed | Medium (SVT-AV1 is 10-20x faster than libaom) | Fast |

**Recommendations**:
- Want best compression ratio → **imgquality (AV1)**
- Need Apple device compatibility → imgquality-hevc
- Need fast encoding → imgquality-hevc

### Features

- 🔍 **JPEG Quality Detection**: Quantization table analysis with ±1 accuracy
- 📊 **Image Feature Analysis**: Entropy, compression ratio, color space, etc.
- 🔄 **Smart Format Conversion**: Static→JXL, Animation→AV1 MP4
- 🎯 **Quality Matching Mode**: Auto-calculate output parameters matching input quality
- 📦 **Metadata Preservation**: Complete EXIF/IPTC, ICC color profile, and file attribute preservation
- ⏭️ **Smart Rollback**: Auto rollback and skip if converted file is larger
- 📈 **Progress Bar**: Visual progress bar with ETA estimation
- 🛡️ **Safety Checks**: Dangerous directory detection to prevent accidents
- 🚀 **SVT-AV1 Encoder**: 10-20x faster than libaom-av1

### Architecture

This tool uses the `shared_utils` shared library for:
- **Metadata Preservation** (`shared_utils::metadata`): ExifTool wrapper + cross-platform native APIs
- **Progress Bar** (`shared_utils::progress`): Visual progress with ETA estimation
- **Safety Checks** (`shared_utils::safety`): Dangerous directory detection
- **Batch Processing** (`shared_utils::batch`): Unified batch processing reports
- **Video Processing** (`shared_utils::video`): Even dimension correction, filter chain generation

### Command Overview

```bash
imgquality <COMMAND>

Commands:
  analyze   Analyze image quality parameters
  auto      Smart auto conversion (recommended)
  verify    Verify conversion quality (PSNR/SSIM)
```

### Auto Mode Conversion Logic

Auto mode intelligently selects conversion strategy based on input format and characteristics:

| Input Type | Condition | Output | Description |
|------------|-----------|--------|-------------|
| JPEG | Default | JXL (lossless transcode) | Preserves DCT coefficients, zero quality loss |
| JPEG | `--match-quality` | JXL (lossy) | Match original quality, better compression |
| PNG/TIFF/BMP (lossless) | - | JXL (d=0) | Mathematical lossless |
| WebP/AVIF/HEIC (lossless) | - | JXL (d=0) | Mathematical lossless |
| WebP/AVIF/HEIC (lossy) | - | Skip | Avoid generational loss |
| Animation (lossless) | ≥3s | **AV1 MP4 CRF 0** | Visually lossless (SVT-AV1) |
| Animation (lossy) | ≥3s + `--match-quality` | **AV1 MP4 CRF 18-35** | Quality matched |
| Animation | <3s | Skip | Short animations not converted |
| Animation | `--lossless` | **AV1 MKV Lossless** | SVT-AV1 lossless mode |

### Smart Rollback Mechanism

When converted file is larger than original, the tool automatically:
1. Deletes the output file
2. Skips the file
3. Outputs clear message: `⏭️ Rollback: JXL larger than original`

This is useful for small PNGs or highly optimized images to avoid size increase after conversion.

### --match-quality Algorithm

#### Static Images (JPEG)

Directly uses detected JPEG quality to calculate JXL distance:

```
distance = (100 - jpeg_quality) / 10

Examples:
Q100 → d=0.0 (lossless)
Q90  → d=1.0
Q85  → d=1.5
Q80  → d=2.0
```

#### Static Images (Non-JPEG)

Estimates quality based on bytes-per-pixel:

```
estimated_quality = 70 + 15 * log2(effective_bpp * 5)
distance = (100 - estimated_quality) / 10

Factors considered:
- Format efficiency (WebP=0.8, AVIF=0.7, PNG=1.5)
- Color depth (8-bit=1.0, 16-bit=2.0)
- Alpha channel (yes=1.33, no=1.0)
```

#### Animation → AV1 MP4 (SVT-AV1)

Calculates CRF based on bytes-per-pixel-per-second:

```
CRF = 63 - 8 * log2(effective_bpps * 1000)
Range: [18, 35]

Factors considered:
- Format efficiency (GIF=2.5, APNG=1.5, WebP=1.0)
- Color depth (8-bit=1.3, other=1.0)
- Resolution (>2MP=0.8, >0.5MP=0.9, other=1.0)
- Alpha channel (yes=0.9, no=1.0)
```

### Usage Examples

```bash
# Analyze image quality
imgquality analyze image.jpg -r

# Smart conversion (default lossless)
imgquality auto image.jpg

# Smart conversion (match quality, better compression)
imgquality auto image.jpg --match-quality

# Batch convert directory (with progress bar)
imgquality auto ./photos/ -r --match-quality

# Delete original after conversion
imgquality auto image.jpg --delete-original

# Force mathematical lossless (animation→AV1 MKV)
imgquality auto animation.gif --lossless

# Verify conversion quality
imgquality verify original.jpg converted.jxl
```

### Performance Optimization

- **Concurrency Limit**: Uses half of CPU cores (min 1, max 4)
- **Thread Limit**: cjxl with `-j`, FFmpeg with `-threads`
- **SVT-AV1**: Uses `lp=N` to limit logical processors
- **Avoid System Lag**: Reserves resources for system and encoder internal threads

### Output Examples

#### Analysis Output
```
📊 Image Quality Analysis Report
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 File: photo.jpg
📷 Format: JPEG (Lossy)
📐 Dimensions: 4000x3000
💾 Size: 2,456,789 bytes (2.34 MB)

🎯 JPEG Quality Analysis (Accuracy: ±1)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 Estimated quality: Q=85 (High Quality)
🎯 Confidence: 98.5%
```

#### Conversion Output (Smart Rollback)
```
🔄 Legacy Lossless→JXL: small_icon.png
   ⏭️  Rollback: JXL larger than original (1024 → 1536 bytes, +50.0%)
✅ Skipped: JXL would be larger (+50.0%)
```

#### Conversion Output (Success)
```
🔄 JPEG→JXL (MATCH QUALITY): photo.jpg
   📊 Quality Analysis (JPEG):
      JPEG Quality: Q85
      Confidence: 98.5%
      Calculated JXL distance: 1.50
   🎯 Matched JXL distance: 1.50
✅ Quality-matched JXL (d=1.50): size reduced 25.3%
```

### Dependencies

#### External Tools
- `cjxl` (libjxl) - JXL encoding
- `djxl` (libjxl) - JXL decoding (for verification)
- `ffmpeg` (with libsvtav1) - Animation to AV1 video
- `exiftool` - Metadata processing

#### Rust Dependencies
- `shared_utils` - Shared utility library (metadata, progress, safety, video processing)
