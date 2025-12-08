# vidquality - 视频质量分析与 AV1 转换工具

高质量视频分析工具，支持智能 AV1 压缩和质量匹配。

## 功能特性

- 🔍 **视频质量检测**: 编码器、比特率、帧率、分辨率等
- 📊 **压缩类型识别**: 无损/视觉无损/有损
- 🔄 **智能 AV1 转换**: 使用 SVT-AV1 编码器（比 libaom 快 10-20 倍）
- 🎯 **质量匹配模式**: 自动计算匹配输入质量的 CRF
- 📦 **元数据保留**: 完整保留文件属性和时间戳

## 命令概览

```bash
vidquality <COMMAND>

Commands:
  analyze   分析视频属性
  auto      智能自动转换（推荐）
  simple    简单模式（全部转 AV1 无损）
  strategy  显示推荐策略（不转换）
```

## Auto 模式转换逻辑

| 输入编码 | 压缩类型 | 输出 | 说明 |
|---------|---------|------|------|
| H.265/AV1/VP9/VVC | 任意 | 跳过 | 现代编码，避免代际损失 |
| FFV1/其他无损 | 无损 | AV1 无损 | 数学无损 AV1 |
| ProRes/DNxHD | 视觉无损 | AV1 CRF 0 | 高质量压缩 |
| H.264/其他 | 有损 | AV1 CRF 0 | 默认高质量 |
| H.264/其他 | 有损 + `--match-quality` | AV1 CRF 18-35 | 匹配输入质量 |

## --match-quality 算法

基于 bits-per-pixel (bpp) 计算匹配的 CRF：

```
CRF = 50 - 8 * log2(effective_bpp * 100)
范围: [18, 35]

考虑因素:
- 编码器效率 (H.264=1.0, H.265=0.7, VP9=0.75, ProRes=1.5, MJPEG=2.0)
- B 帧 (有=1.1, 无=1.0)
- 分辨率 (4K+=0.85, 1080p=0.9, 720p=0.95, SD=1.0)
```

### CRF 对应关系

| 输入 bpp | 计算 CRF | 质量等级 |
|---------|---------|---------|
| 1.0 | ~18 | 极高质量 |
| 0.3 | ~24 | 高质量 |
| 0.1 | ~28 | 中等质量 |
| 0.03 | ~33 | 较低质量 |

## 使用示例

```bash
# 分析视频
vidquality analyze video.mp4

# 智能转换（默认 CRF 0）
vidquality auto video.mp4

# 智能转换（匹配质量）
vidquality auto video.mp4 --match-quality

# 批量转换目录
vidquality auto ./videos/ --match-quality

# 探索更小文件（逐步提高 CRF 直到输出小于输入）
vidquality auto video.mp4 --explore

# 强制数学无损
vidquality auto video.mp4 --lossless

# 转换后删除原文件
vidquality auto video.mp4 --delete-original

# 查看推荐策略
vidquality strategy video.mp4
```

## 输出示例

### 分析输出
```
📊 Video Analysis Report
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 File: video.mp4
📦 Format: mov,mp4,m4a,3gp,3g2,mj2
🎬 Codec: H.264 (H.264 / AVC / MPEG-4 AVC)
🔍 Compression: Lossy

📐 Resolution: 1920x1080
🎞️  Frames: 3600 @ 30.00 fps
⏱️  Duration: 120.00s
💾 File Size: 125,000,000 bytes
📊 Bitrate: 8,333,333 bps

⭐ Quality Score: 75/100
```

### 转换输出
```
🎬 Auto Mode Conversion (AV1)
   🎯 Match Quality: ENABLED

🎬 Auto Mode: video.mp4 → AV1 MP4 (High Quality)
   Reason: Source is H.264 (Lossy) - compressing with AV1 CRF 0
   🎯 Match Quality Mode: using CRF 24 to match input quality
   📊 Quality Analysis:
      Raw bpp: 0.1286
      Codec factor: 1.00 (H.264)
      B-frames: true (factor: 1.10)
      Resolution: 1920x1080 (factor: 0.90)
      Effective bpp: 0.1273
      Calculated CRF: 24
   ✅ Complete: 45.2% of original
```

## 编码器说明

本工具使用 **SVT-AV1** (`libsvtav1`) 编码器：
- 比 libaom-av1 快 10-20 倍
- preset 6（平衡速度和质量）
- 支持多线程

## 依赖工具

- `ffmpeg` (带 libsvtav1) - 视频编码
- `ffprobe` - 视频分析
- `exiftool` - 元数据处理
