# vidquality-hevc - 视频质量分析与 HEVC 转换工具

高质量视频分析工具，支持智能 HEVC/H.265 压缩和质量匹配。

## 功能特性

- 🔍 **视频质量检测**: 编码器、比特率、帧率、分辨率等
- 📊 **压缩类型识别**: 无损/视觉无损/有损
- 🔄 **智能 HEVC 转换**: 使用 libx265 编码器
- 🎯 **质量匹配模式**: 自动计算匹配输入质量的 CRF
- 📦 **元数据保留**: 完整保留文件属性和时间戳
- 🍎 **Apple 兼容**: 使用 hvc1 标签确保 Apple 设备兼容

## 命令概览

```bash
vidquality-hevc <COMMAND>

Commands:
  analyze   分析视频属性
  auto      智能自动转换（推荐）
  simple    简单模式（全部转 HEVC CRF 18）
  strategy  显示推荐策略（不转换）
```

## Auto 模式转换逻辑

| 输入编码 | 压缩类型 | 输出 | 说明 |
|---------|---------|------|------|
| H.265/AV1/VP9/VVC | 任意 | 跳过 | 现代编码，避免代际损失 |
| FFV1/其他无损 | 无损 | HEVC 无损 MKV | x265 lossless 模式 |
| ProRes/DNxHD | 视觉无损 | HEVC CRF 18 | 高质量压缩 |
| H.264/其他 | 有损 | HEVC CRF 20 | 默认高质量 |
| H.264/其他 | 有损 + `--match-quality` | HEVC CRF 18-32 | 匹配输入质量 |

## --match-quality 算法

基于 bits-per-pixel (bpp) 计算匹配的 CRF：

```
CRF = 51 - 10 * log2(effective_bpp * 100)
范围: [18, 32]

考虑因素:
- 编码器效率 (H.264=1.0, H.265=0.6, VP9=0.65, AV1=0.5, ProRes=1.5, MJPEG=2.0)
- B 帧 (有=1.1, 无=1.0)
- 分辨率 (4K+=0.85, 1080p=0.9, 720p=0.95, SD=1.0)
```

### CRF 对应关系

| 输入 bpp | 计算 CRF | 质量等级 |
|---------|---------|---------|
| 1.0 | ~18 | 极高质量 |
| 0.3 | ~23 | 高质量 |
| 0.1 | ~28 | 中等质量 |
| 0.03 | ~32 | 较低质量 |

## 使用示例

```bash
# 分析视频
vidquality-hevc analyze video.mp4

# 智能转换（默认策略）
vidquality-hevc auto video.mp4

# 智能转换（匹配质量）
vidquality-hevc auto video.mp4 --match-quality

# 批量转换目录
vidquality-hevc auto ./videos/ --match-quality

# 探索更小文件（逐步提高 CRF 直到输出小于输入）
vidquality-hevc auto video.mp4 --explore

# 强制无损模式
vidquality-hevc auto video.mp4 --lossless

# 转换后删除原文件
vidquality-hevc auto video.mp4 --delete-original

# 查看推荐策略
vidquality-hevc strategy video.mp4
```

## 输出示例

### 分析输出
```
📊 Video Analysis Report (HEVC)
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
🎬 Auto Mode Conversion (HEVC/H.265)
   Lossy sources → HEVC MP4 (CRF auto-matched to input quality)
   🎯 Match Quality: ENABLED

🎬 Auto Mode: video.mp4 → HEVC MP4 (High Quality)
   Reason: Source is H.264 (Lossy) - compressing with HEVC CRF 20
   🎯 Match Quality Mode: using CRF 23 to match input quality
   📊 Quality Analysis:
      Raw bpp: 0.1286
      Codec factor: 1.00 (H.264)
      B-frames: true (factor: 1.10)
      Resolution: 1920x1080 (factor: 0.90)
      Effective bpp: 0.1273
      Calculated CRF: 23
   ✅ Complete: 52.1% of original
```

## HEVC vs AV1 对比

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

## 依赖工具

- `ffmpeg` (带 libx265) - 视频编码
- `ffprobe` - 视频分析
- `exiftool` - 元数据处理
