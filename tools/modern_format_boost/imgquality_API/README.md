# imgquality - 图像质量分析与格式升级工具

高精度图像质量分析工具，支持 JPEG 质量检测（精度 ±1）和智能格式升级。

## 功能特性

- 🔍 **JPEG 质量检测**: 通过量化表分析，精度达到 ±1
- 📊 **图像特征分析**: 熵值、压缩比、色彩空间等
- 🔄 **智能格式转换**: 自动选择最佳转换策略
- 🎯 **质量匹配模式**: 自动计算匹配输入质量的输出参数
- 📦 **元数据保留**: 完整保留 EXIF/IPTC 和文件属性

## 命令概览

```bash
imgquality <COMMAND>

Commands:
  analyze   分析图像质量参数
  convert   转换为指定格式
  auto      智能自动转换（推荐）
  verify    验证转换质量（PSNR/SSIM）
```

## Auto 模式转换逻辑

Auto 模式根据输入格式和特性智能选择转换策略：

| 输入类型 | 条件 | 输出 | 说明 |
|---------|------|------|------|
| JPEG | 默认 | JXL (无损转码) | 保留 DCT 系数，零质量损失 |
| JPEG | `--match-quality` | JXL (有损) | 匹配原始质量，更好压缩 |
| PNG/TIFF/BMP (无损) | - | JXL (d=0) | 数学无损 |
| WebP/AVIF/HEIC (无损) | - | JXL (d=0) | 数学无损 |
| WebP/AVIF/HEIC (有损) | - | 跳过 | 避免代际损失 |
| 动图 (无损) | ≥3秒 | AV1 MP4 | CRF 0 或匹配质量 |
| 动图 (有损) | ≥3秒 + `--match-quality` | AV1 MP4 | 匹配质量 CRF |
| 动图 | <3秒 | 跳过 | 短动画不转换 |

## --match-quality 算法

### 静态图像 (JPEG)

直接使用检测到的 JPEG 质量值计算 JXL distance：

```
distance = (100 - jpeg_quality) / 10

示例:
Q100 → d=0.0 (无损)
Q90  → d=1.0
Q85  → d=1.5
Q80  → d=2.0
```

### 静态图像 (非 JPEG)

基于 bytes-per-pixel 估算质量：

```
estimated_quality = 70 + 15 * log2(effective_bpp * 5)
distance = (100 - estimated_quality) / 10

考虑因素:
- 格式效率 (WebP=0.8, AVIF=0.7, PNG=1.5)
- 色彩深度 (8-bit=1.0, 16-bit=2.0)
- Alpha 通道 (有=1.33, 无=1.0)
```

### 动图 → AV1 MP4

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

## 使用示例

```bash
# 分析图像质量
imgquality analyze image.jpg -r

# 智能转换（默认无损）
imgquality auto image.jpg

# 智能转换（匹配质量，更好压缩）
imgquality auto image.jpg --match-quality

# 批量转换目录
imgquality auto ./photos/ -r --match-quality

# 转换后删除原文件
imgquality auto image.jpg --delete-original

# 验证转换质量
imgquality verify original.jpg converted.jxl
```

## 输出示例

### 分析输出
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

### 转换输出
```
🔄 JPEG→JXL (MATCH QUALITY): photo.jpg
   📊 Quality Analysis (JPEG):
      JPEG Quality: Q85
      Confidence: 98.5%
      Calculated JXL distance: 1.50
   🎯 Matched JXL distance: 1.50
✅ Quality-matched JXL (d=1.50): size reduced 25.3%
```

## 依赖工具

- `cjxl` (libjxl) - JXL 编码
- `djxl` (libjxl) - JXL 解码（验证用）
- `ffmpeg` - 动图转视频
- `exiftool` - 元数据处理
