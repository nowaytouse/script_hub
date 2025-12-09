# imgquality-hevc - 图像质量分析与 HEVC 格式升级工具

[English](#english) | [中文](#中文)

---

## 中文

高精度图像质量分析工具，支持 JPEG 质量检测（精度 ±1）和智能 HEVC 格式升级。

### 与 imgquality (AV1) 的区别

| 特性 | imgquality (AV1) | imgquality-hevc |
|------|-----------------|-----------------|
| 静态图输出 | JXL | JXL |
| 动图输出 | AV1 MP4 | **HEVC MP4** |
| 编码器 | SVT-AV1 | **libx265** |
| 默认 CRF | 0 | **0** |
| 兼容性 | 较好 | **极佳 (Apple/硬件)** |
| 编码速度 | 中等 | **快** |

**选择建议**:
- 需要 Apple 设备兼容 → **imgquality-hevc**
- 追求最佳压缩率 → imgquality (AV1)
- 需要快速编码 → **imgquality-hevc**

### 功能特性

- 🔍 **JPEG 质量检测**: 通过量化表分析，精度达到 ±1
- 📊 **图像特征分析**: 熵值、压缩比、色彩空间等
- 🔄 **智能格式转换**: 静态图→JXL，动图→HEVC MP4
- 🎯 **质量匹配模式**: 自动计算匹配输入质量的输出参数
- 📦 **元数据保留**: 完整保留 EXIF/IPTC、ICC 颜色配置文件和文件属性
- ⏭️ **智能回退**: 转换后变大则自动回退跳过
- 📈 **进度条**: 带 ETA 估算的可视化进度条
- 🛡️ **安全检查**: 危险目录检测，防止误操作
- 🍎 **Apple 兼容**: 使用 hvc1 标签确保 Apple 设备兼容

### 命令概览

```bash
imgquality-hevc <COMMAND>

Commands:
  analyze   分析图像质量参数
  auto      智能自动转换（推荐）
  verify    验证转换质量（PSNR/SSIM）
```

### Auto 模式转换逻辑

| 输入类型 | 条件 | 输出 | 说明 |
|---------|------|------|------|
| JPEG | 默认 | JXL (无损转码) | 保留 DCT 系数，零质量损失 |
| JPEG | `--match-quality` | JXL (有损) | 匹配原始质量，更好压缩 |
| PNG/TIFF/BMP (无损) | - | JXL (d=0) | 数学无损 |
| WebP/AVIF/HEIC (无损) | - | JXL (d=0) | 数学无损 |
| WebP/AVIF/HEIC (有损) | - | 跳过 | 避免代际损失 |
| 动图 (无损) | ≥3秒 | **HEVC MP4 CRF 0** | 视觉无损 |
| 动图 (有损) | ≥3秒 + `--match-quality` | **HEVC MP4 CRF 0-32** | 匹配质量 |
| 动图 | <3秒 | 跳过 | 短动画不转换 |
| 动图 | `--lossless` | **HEVC MKV 无损** | x265 lossless 模式 |

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

#### 动图 → HEVC MP4

基于 bytes-per-pixel-per-second 计算 CRF：

```
CRF = 63 - 8 * log2(effective_bpps * 1000)
范围: [0, 32]

考虑因素:
- 格式效率 (GIF=2.5, APNG=1.5, WebP=1.0)
- 色彩深度 (8-bit=1.3, 其他=1.0)
- 分辨率 (>2MP=0.8, >0.5MP=0.9, 其他=1.0)
- Alpha 通道 (有=0.9, 无=1.0)
```

### 使用示例

```bash
# 分析图像质量
imgquality-hevc analyze image.jpg -r

# 智能转换（默认无损）
imgquality-hevc auto image.jpg

# 智能转换（匹配质量，更好压缩）
imgquality-hevc auto image.jpg --match-quality

# 批量转换目录（带进度条）
imgquality-hevc auto ./photos/ -r --match-quality

# 转换后删除原文件
imgquality-hevc auto image.jpg --delete-original

# 强制数学无损（动图→HEVC MKV）
imgquality-hevc auto animation.gif --lossless

# 验证转换质量
imgquality-hevc verify original.jpg converted.jxl
```

### 性能优化

- **并发限制**: 使用 CPU 核心数的一半（最少 1，最多 4）
- **线程限制**: FFmpeg 添加 `-threads` 参数，x265 添加 `pools=N` 参数
- **避免系统卡顿**: 留出资源给系统和编码器内部线程

### 依赖

#### 外部工具
- `cjxl` (libjxl) - JXL 编码
- `djxl` (libjxl) - JXL 解码（验证用）
- `ffmpeg` (带 libx265) - 动图转 HEVC 视频
- `exiftool` - 元数据处理

#### Rust 依赖
- `shared_utils` - 共享工具库（元数据、进度条、安全检查、视频处理）

---

## English

High-precision image quality analysis tool with JPEG quality detection (±1 accuracy) and smart HEVC format upgrade.

### Difference from imgquality (AV1)

| Feature | imgquality (AV1) | imgquality-hevc |
|---------|-----------------|-----------------|
| Static Output | JXL | JXL |
| Animation Output | AV1 MP4 | **HEVC MP4** |
| Encoder | SVT-AV1 | **libx265** |
| Default CRF | 0 | **0** |
| Compatibility | Good | **Excellent (Apple/Hardware)** |
| Encoding Speed | Medium | **Fast** |

**Recommendations**:
- Need Apple device compatibility → **imgquality-hevc**
- Want best compression ratio → imgquality (AV1)
- Need fast encoding → **imgquality-hevc**

### Features

- 🔍 **JPEG Quality Detection**: Quantization table analysis with ±1 accuracy
- 📊 **Image Feature Analysis**: Entropy, compression ratio, color space, etc.
- 🔄 **Smart Format Conversion**: Static→JXL, Animation→HEVC MP4
- 🎯 **Quality Matching Mode**: Auto-calculate output parameters matching input quality
- 📦 **Metadata Preservation**: Complete EXIF/IPTC, ICC color profile, and file attribute preservation
- ⏭️ **Smart Rollback**: Auto rollback and skip if converted file is larger
- 📈 **Progress Bar**: Visual progress bar with ETA estimation
- 🛡️ **Safety Checks**: Dangerous directory detection to prevent accidents
- 🍎 **Apple Compatible**: Uses hvc1 tag for Apple device compatibility

### Command Overview

```bash
imgquality-hevc <COMMAND>

Commands:
  analyze   Analyze image quality parameters
  auto      Smart auto conversion (recommended)
  verify    Verify conversion quality (PSNR/SSIM)
```

### Auto Mode Conversion Logic

| Input Type | Condition | Output | Description |
|------------|-----------|--------|-------------|
| JPEG | Default | JXL (lossless transcode) | Preserves DCT coefficients, zero quality loss |
| JPEG | `--match-quality` | JXL (lossy) | Match original quality, better compression |
| PNG/TIFF/BMP (lossless) | - | JXL (d=0) | Mathematical lossless |
| WebP/AVIF/HEIC (lossless) | - | JXL (d=0) | Mathematical lossless |
| WebP/AVIF/HEIC (lossy) | - | Skip | Avoid generational loss |
| Animation (lossless) | ≥3s | **HEVC MP4 CRF 0** | Visually lossless |
| Animation (lossy) | ≥3s + `--match-quality` | **HEVC MP4 CRF 0-32** | Quality matched |
| Animation | <3s | Skip | Short animations not converted |
| Animation | `--lossless` | **HEVC MKV Lossless** | x265 lossless mode |

### Usage Examples

```bash
# Analyze image quality
imgquality-hevc analyze image.jpg -r

# Smart conversion (default lossless)
imgquality-hevc auto image.jpg

# Smart conversion (match quality, better compression)
imgquality-hevc auto image.jpg --match-quality

# Batch convert directory (with progress bar)
imgquality-hevc auto ./photos/ -r --match-quality

# Delete original after conversion
imgquality-hevc auto image.jpg --delete-original

# Force mathematical lossless (animation→HEVC MKV)
imgquality-hevc auto animation.gif --lossless

# Verify conversion quality
imgquality-hevc verify original.jpg converted.jxl
```

### Performance Optimization

- **Concurrency Limit**: Uses half of CPU cores (min 1, max 4)
- **Thread Limit**: FFmpeg with `-threads`, x265 with `pools=N`
- **Avoid System Lag**: Reserves resources for system and encoder internal threads

### Dependencies

#### External Tools
- `cjxl` (libjxl) - JXL encoding
- `djxl` (libjxl) - JXL decoding (for verification)
- `ffmpeg` (with libx265) - Animation to HEVC video
- `exiftool` - Metadata processing

#### Rust Dependencies
- `shared_utils` - Shared utility library (metadata, progress, safety, video processing)
