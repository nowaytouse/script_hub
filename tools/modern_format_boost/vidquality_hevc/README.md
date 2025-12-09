# vidquality-hevc - 视频质量分析与 HEVC 转换工具

[English](#english) | [中文](#中文)

---

## 中文

高质量视频分析工具，支持智能 HEVC/H.265 压缩和质量匹配。

### 功能特性

- 🔍 **视频质量检测**: 编码器、比特率、帧率、分辨率等
- 📊 **压缩类型识别**: 无损/视觉无损/有损
- 🔄 **智能 HEVC 转换**: 使用 libx265 编码器
- 🎯 **质量匹配模式**: 自动计算匹配输入质量的 CRF
- 📦 **元数据保留**: 完整保留文件属性和时间戳
- 🍎 **Apple 兼容**: 使用 hvc1 标签确保 Apple 设备兼容
- 📈 **尺寸探索模式**: 逐步提高 CRF 直到输出小于输入

### 命令概览

```bash
vidquality-hevc <COMMAND>

Commands:
  analyze   分析视频属性
  auto      智能自动转换（推荐）
  simple    简单模式（全部转 HEVC CRF 18）
  strategy  显示推荐策略（不转换）
```

### Auto 模式转换逻辑

| 输入编码 | 压缩类型 | 输出 | 说明 |
|---------|---------|------|------|
| H.265/AV1/VP9/VVC | 任意 | 跳过 | 现代编码，避免代际损失 |
| FFV1/其他无损 | 无损 | HEVC 无损 MKV | x265 lossless 模式 |
| ProRes/DNxHD | 视觉无损 | HEVC CRF 18 | 高质量压缩 |
| H.264/其他 | 有损 | HEVC CRF 20 | 默认高质量 |
| H.264/其他 | 有损 + `--match-quality` | HEVC CRF 18-32 | 匹配输入质量 |

### --match-quality 算法

基于 bits-per-pixel (bpp) 计算匹配的 CRF：

```
CRF = 51 - 10 * log2(effective_bpp * 100)
范围: [18, 32]

考虑因素:
- 编码器效率 (H.264=1.0, H.265=0.6, VP9=0.65, AV1=0.5, ProRes=1.5, MJPEG=2.0)
- B 帧 (有=1.1, 无=1.0)
- 分辨率 (4K+=0.85, 1080p=0.9, 720p=0.95, SD=1.0)
```

#### CRF 对应关系

| 输入 bpp | 计算 CRF | 质量等级 |
|---------|---------|---------|
| 1.0 | ~18 | 极高质量 |
| 0.3 | ~23 | 高质量 |
| 0.1 | ~28 | 中等质量 |
| 0.03 | ~32 | 较低质量 |

### 使用示例

```bash
# 分析视频
vidquality-hevc analyze video.mp4

# 智能转换（默认策略）
vidquality-hevc auto video.mp4

# 智能转换（匹配质量）
vidquality-hevc auto video.mp4 --match-quality

# 批量转换目录
vidquality-hevc auto ./videos/ --match-quality

# 探索更小文件
vidquality-hevc auto video.mp4 --explore

# 强制无损模式
vidquality-hevc auto video.mp4 --lossless

# 转换后删除原文件
vidquality-hevc auto video.mp4 --delete-original

# 查看推荐策略
vidquality-hevc strategy video.mp4
```

### HEVC vs AV1 对比

| 特性 | HEVC (本工具) | AV1 (vidquality) |
|------|--------------|------------------|
| 压缩效率 | 较好 | 最佳 |
| 编码速度 | 快 | 中等 |
| 兼容性 | 极佳 (Apple/硬件) | 较好 |
| 专利 | 需授权 | 免费 |

**选择建议**:
- 需要 Apple 设备兼容 → HEVC
- 追求最佳压缩率 → AV1
- 需要快速编码 → HEVC

### 依赖工具

- `ffmpeg` (带 libx265) - 视频编码
- `ffprobe` - 视频分析
- `exiftool` - 元数据处理

---

## English

High-quality video analysis tool with smart HEVC/H.265 compression and quality matching.

### Features

- 🔍 **Video Quality Detection**: Encoder, bitrate, frame rate, resolution
- 📊 **Compression Type Recognition**: Lossless/Visually Lossless/Lossy
- 🔄 **Smart HEVC Conversion**: Uses libx265 encoder
- 🎯 **Quality Matching Mode**: Auto-calculate CRF matching input quality
- 📦 **Metadata Preservation**: Complete file attribute and timestamp preservation
- 🍎 **Apple Compatible**: Uses hvc1 tag for Apple device compatibility
- 📈 **Size Exploration Mode**: Gradually increase CRF until output < input

### Command Overview

```bash
vidquality-hevc <COMMAND>

Commands:
  analyze   Analyze video properties
  auto      Smart auto conversion (recommended)
  simple    Simple mode (all to HEVC CRF 18)
  strategy  Show recommended strategy (no conversion)
```

### Auto Mode Conversion Logic

| Input Codec | Compression | Output | Description |
|-------------|-------------|--------|-------------|
| H.265/AV1/VP9/VVC | Any | Skip | Modern codec, avoid generational loss |
| FFV1/Other lossless | Lossless | HEVC Lossless MKV | x265 lossless mode |
| ProRes/DNxHD | Visually Lossless | HEVC CRF 18 | High quality compression |
| H.264/Other | Lossy | HEVC CRF 20 | Default high quality |
| H.264/Other | Lossy + `--match-quality` | HEVC CRF 18-32 | Match input quality |

### --match-quality Algorithm

Calculates matching CRF based on bits-per-pixel (bpp):

```
CRF = 51 - 10 * log2(effective_bpp * 100)
Range: [18, 32]

Factors:
- Encoder efficiency (H.264=1.0, H.265=0.6, VP9=0.65, AV1=0.5, ProRes=1.5, MJPEG=2.0)
- B-frames (yes=1.1, no=1.0)
- Resolution (4K+=0.85, 1080p=0.9, 720p=0.95, SD=1.0)
```

#### CRF Correspondence

| Input bpp | Calculated CRF | Quality Level |
|-----------|---------------|---------------|
| 1.0 | ~18 | Extremely High Quality |
| 0.3 | ~23 | High Quality |
| 0.1 | ~28 | Medium Quality |
| 0.03 | ~32 | Lower Quality |

### Usage Examples

```bash
# Analyze video
vidquality-hevc analyze video.mp4

# Smart conversion (default strategy)
vidquality-hevc auto video.mp4

# Smart conversion (match quality)
vidquality-hevc auto video.mp4 --match-quality

# Batch convert directory
vidquality-hevc auto ./videos/ --match-quality

# Explore smaller file
vidquality-hevc auto video.mp4 --explore

# Force lossless mode
vidquality-hevc auto video.mp4 --lossless

# Delete original after conversion
vidquality-hevc auto video.mp4 --delete-original

# View recommended strategy
vidquality-hevc strategy video.mp4
```

### HEVC vs AV1 Comparison

| Feature | HEVC (this tool) | AV1 (vidquality) |
|---------|-----------------|------------------|
| Compression Efficiency | Good | Best |
| Encoding Speed | Fast | Medium |
| Compatibility | Excellent (Apple/Hardware) | Good |
| Patents | Licensed | Royalty-free |

**Recommendations**:
- Need Apple device compatibility → HEVC
- Want best compression ratio → AV1
- Need fast encoding → HEVC

### Dependencies

- `ffmpeg` (with libx265) - Video encoding
- `ffprobe` - Video analysis
- `exiftool` - Metadata processing
