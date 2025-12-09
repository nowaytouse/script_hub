# vidquality - 视频质量分析与 AV1 转换工具

[English](#english) | [中文](#中文)

---

## 中文

高质量视频分析工具，支持智能 AV1 压缩和质量匹配。

### 与 vidquality-hevc 的区别

| 特性 | vidquality (本工具) | vidquality-hevc |
|------|-------------------|-----------------|
| 输出编码 | **AV1** | HEVC/H.265 |
| 编码器 | **SVT-AV1** | libx265 |
| 默认 CRF | 0 | 18-20 |
| 压缩效率 | **最佳** | 较好 |
| 兼容性 | 较好 | 极佳 (Apple/硬件) |
| 编码速度 | 中等 (SVT-AV1 比 libaom 快 10-20 倍) | 快 |

**选择建议**:
- 追求最佳压缩率 → **vidquality (AV1)**
- 需要 Apple 设备兼容 → vidquality-hevc
- 需要快速编码 → vidquality-hevc

### 功能特性

- 🔍 **视频质量检测**: 编码器、比特率、帧率、分辨率等
- 📊 **压缩类型识别**: 无损/视觉无损/有损
- 🔄 **智能 AV1 转换**: 使用 SVT-AV1 编码器（比 libaom 快 10-20 倍）
- 🎯 **质量匹配模式**: 自动计算匹配输入质量的 CRF
- 📦 **元数据保留**: 完整保留文件属性和时间戳
- 📈 **尺寸探索模式**: 逐步提高 CRF 直到输出小于输入
- 🛡️ **安全检查**: 危险目录检测，防止误操作
- 🚀 **SVT-AV1 编码器**: 比 libaom-av1 快 10-20 倍

### 架构说明

本工具使用 `shared_utils` 共享库提供以下功能：
- **元数据保留** (`shared_utils::metadata`): ExifTool 封装 + 跨平台原生 API
- **FFprobe 封装** (`shared_utils::ffprobe`): 统一的视频信息获取
- **编解码器检测** (`shared_utils::codecs`): FFmpeg 编解码器可用性检测
- **视频处理** (`shared_utils::video`): 偶数尺寸修正、滤镜链生成
- **安全检查** (`shared_utils::safety`): 危险目录检测
- **批量处理** (`shared_utils::batch`): 统一的批量处理报告

### 命令概览

```bash
vidquality <COMMAND>

Commands:
  analyze   分析视频属性
  auto      智能自动转换（推荐）
  simple    简单模式（全部转 AV1 无损）
  strategy  显示推荐策略（不转换）
```

### Auto 模式转换逻辑

| 输入编码 | 压缩类型 | 输出 | 说明 |
|---------|---------|------|------|
| H.265/AV1/VP9/VVC | 任意 | 跳过 | 现代编码，避免代际损失 |
| FFV1/其他无损 | 无损 | AV1 无损 | 数学无损 AV1 |
| ProRes/DNxHD | 视觉无损 | AV1 CRF 0 | 高质量压缩 |
| H.264/其他 | 有损 | AV1 CRF 0 | 默认高质量 |
| H.264/其他 | 有损 + `--match-quality` | AV1 CRF 18-35 | 匹配输入质量 |

### --match-quality 算法

基于 bits-per-pixel (bpp) 计算匹配的 CRF：

```
CRF = 50 - 8 * log2(effective_bpp * 100)
范围: [18, 35]

考虑因素:
- 编码器效率 (H.264=1.0, H.265=0.7, VP9=0.75, ProRes=1.5, MJPEG=2.0)
- B 帧 (有=1.1, 无=1.0)
- 分辨率 (4K+=0.85, 1080p=0.9, 720p=0.95, SD=1.0)
```

### 使用示例

```bash
# 分析视频
vidquality analyze video.mp4

# 智能转换（默认 CRF 0）
vidquality auto video.mp4

# 智能转换（匹配质量）
vidquality auto video.mp4 --match-quality

# 批量转换目录
vidquality auto ./videos/ --match-quality

# 探索更小文件
vidquality auto video.mp4 --explore

# 强制数学无损
vidquality auto video.mp4 --lossless

# 转换后删除原文件
vidquality auto video.mp4 --delete-original

# 查看推荐策略
vidquality strategy video.mp4
```

### 编码器说明

本工具使用 **SVT-AV1** (`libsvtav1`) 编码器：
- 比 libaom-av1 快 10-20 倍
- preset 6（平衡速度和质量）
- 支持多线程
- 添加 `lp=N` 参数限制逻辑处理器数

### 性能优化

- **并发限制**: 使用 CPU 核心数的一半（最少 1，最多 4）
- **线程限制**: FFmpeg 添加 `-threads` 参数，SVT-AV1 添加 `lp=N` 参数
- **避免系统卡顿**: 留出资源给系统和编码器内部线程

### 依赖

#### 外部工具
- `ffmpeg` (带 libsvtav1) - 视频编码
- `ffprobe` - 视频分析
- `exiftool` - 元数据处理

#### Rust 依赖
- `shared_utils` - 共享工具库（元数据、FFprobe、编解码器检测、视频处理、安全检查）

---

## English

High-quality video analysis tool with smart AV1 compression and quality matching.

### Difference from vidquality-hevc

| Feature | vidquality (this tool) | vidquality-hevc |
|---------|----------------------|-----------------|
| Output Codec | **AV1** | HEVC/H.265 |
| Encoder | **SVT-AV1** | libx265 |
| Default CRF | 0 | 18-20 |
| Compression | **Best** | Good |
| Compatibility | Good | Excellent (Apple/Hardware) |
| Encoding Speed | Medium (SVT-AV1 is 10-20x faster than libaom) | Fast |

**Recommendations**:
- Want best compression ratio → **vidquality (AV1)**
- Need Apple device compatibility → vidquality-hevc
- Need fast encoding → vidquality-hevc

### Features

- 🔍 **Video Quality Detection**: Encoder, bitrate, frame rate, resolution
- 📊 **Compression Type Recognition**: Lossless/Visually Lossless/Lossy
- 🔄 **Smart AV1 Conversion**: Uses SVT-AV1 encoder (10-20x faster than libaom)
- 🎯 **Quality Matching Mode**: Auto-calculate CRF matching input quality
- 📦 **Metadata Preservation**: Complete file attribute and timestamp preservation
- 📈 **Size Exploration Mode**: Gradually increase CRF until output < input
- 🛡️ **Safety Checks**: Dangerous directory detection to prevent accidents
- 🚀 **SVT-AV1 Encoder**: 10-20x faster than libaom-av1

### Architecture

This tool uses the `shared_utils` shared library for:
- **Metadata Preservation** (`shared_utils::metadata`): ExifTool wrapper + cross-platform native APIs
- **FFprobe Wrapper** (`shared_utils::ffprobe`): Unified video information retrieval
- **Codec Detection** (`shared_utils::codecs`): FFmpeg codec availability detection
- **Video Processing** (`shared_utils::video`): Even dimension correction, filter chain generation
- **Safety Checks** (`shared_utils::safety`): Dangerous directory detection
- **Batch Processing** (`shared_utils::batch`): Unified batch processing reports

### Command Overview

```bash
vidquality <COMMAND>

Commands:
  analyze   Analyze video properties
  auto      Smart auto conversion (recommended)
  simple    Simple mode (all to AV1 lossless)
  strategy  Show recommended strategy (no conversion)
```

### Auto Mode Conversion Logic

| Input Codec | Compression | Output | Description |
|-------------|-------------|--------|-------------|
| H.265/AV1/VP9/VVC | Any | Skip | Modern codec, avoid generational loss |
| FFV1/Other lossless | Lossless | AV1 Lossless | Mathematical lossless AV1 |
| ProRes/DNxHD | Visually Lossless | AV1 CRF 0 | High quality compression |
| H.264/Other | Lossy | AV1 CRF 0 | Default high quality |
| H.264/Other | Lossy + `--match-quality` | AV1 CRF 18-35 | Match input quality |

### --match-quality Algorithm

Calculates matching CRF based on bits-per-pixel (bpp):

```
CRF = 50 - 8 * log2(effective_bpp * 100)
Range: [18, 35]

Factors:
- Encoder efficiency (H.264=1.0, H.265=0.7, VP9=0.75, ProRes=1.5, MJPEG=2.0)
- B-frames (yes=1.1, no=1.0)
- Resolution (4K+=0.85, 1080p=0.9, 720p=0.95, SD=1.0)
```

### Usage Examples

```bash
# Analyze video
vidquality analyze video.mp4

# Smart conversion (default CRF 0)
vidquality auto video.mp4

# Smart conversion (match quality)
vidquality auto video.mp4 --match-quality

# Batch convert directory
vidquality auto ./videos/ --match-quality

# Explore smaller file
vidquality auto video.mp4 --explore

# Force mathematical lossless
vidquality auto video.mp4 --lossless

# Delete original after conversion
vidquality auto video.mp4 --delete-original

# View recommended strategy
vidquality strategy video.mp4
```

### Encoder Notes

This tool uses **SVT-AV1** (`libsvtav1`) encoder:
- 10-20x faster than libaom-av1
- preset 6 (balanced speed and quality)
- Multi-threading support
- Uses `lp=N` to limit logical processors

### Performance Optimization

- **Concurrency Limit**: Uses half of CPU cores (min 1, max 4)
- **Thread Limit**: FFmpeg with `-threads`, SVT-AV1 with `lp=N`
- **Avoid System Lag**: Reserves resources for system and encoder internal threads

### Dependencies

#### External Tools
- `ffmpeg` (with libsvtav1) - Video encoding
- `ffprobe` - Video analysis
- `exiftool` - Metadata processing

#### Rust Dependencies
- `shared_utils` - Shared utility library (metadata, ffprobe, codecs, video, safety)
