# imgquality - Image Quality Analysis and Format Upgrade Tool

High-performance CLI tool for precise image quality analysis with intelligent format upgrade recommendations.

## Features

✨ **Precise Quality Analysis**
- 🔍 Lossless/Lossy detection
- 📊 PSNR/SSIM quality metrics (precise calculation)
- 🎨 Color depth and color space analysis
- 🎬 Animation detection (GIF/WebP/APNG)
- 📐 Complete image metadata extraction

💡 **Smart Format Upgrade**
- PNG → JXL (lossless, 30-60% size reduction)
- JPEG → JXL (lossless transcode with --lossless_jpeg=1)
- GIF/Animated → AV1 MP4 (visually lossless)
- Static lossy (non-JPEG) → AVIF

🚀 **Dual Purpose**
- Standalone CLI tool
- JSON API mode (for frontend/script integration)

⚡ **Performance Optimized**
- Rust-based for excellent performance
- **Parallel batch processing** (rayon-powered)
- Recursive directory processing
- Whitelist-only file processing

## 安装

### 前置依赖

```bash
# 安装 JPEG XL 工具
brew install jpeg-xl

# 安装 exiftool（可选，用于元数据保留）
brew install exiftool
```

### 编译安装

```bash
cd /path/to/imgquality
cargo build --release

# 二进制文件位于：
# ./target/release/imgquality

# 可选：安装到系统路径
cargo install --path .
```

## 使用方法

### 1. 分析图像质量

#### 单个文件（人类可读输出）

```bash
imgquality analyze image.png
```

输出示例：
```
📊 Image Analysis Report
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📁 File: image.png
📷 Format: PNG
📐 Dimensions: 1920x1080
💾 File Size: 1500000 bytes
🎨 Color Depth: 8-bit
🌈 Color Space: sRGB
🔍 Has Alpha: Yes
🎬 Animated: No
🔒 Lossless: Yes ✓
```

#### 单个文件（JSON输出，供API调用）

```bash
imgquality analyze image.png --output json
```

输出示例：
```json
{
  "file_path": "image.png",
  "format": "PNG",
  "width": 1920,
  "height": 1080,
  "is_lossless": true,
  "color_depth": 8,
  "color_space": "sRGB",
  "has_alpha": true,
  "is_animated": false,
  "psnr": null,
  "ssim": null,
  "file_size": 1500000,
  "metadata": {}
}
```

#### 分析并获取升级建议

```bash
imgquality analyze image.png --recommend
```

额外输出：
```
💡 Upgrade Recommendation
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🔄 PNG → JXL
📝 Reason: PNG is already lossless. JXL can provide 30-60% size reduction while maintaining mathematical losslessness
💾 Expected Size Reduction: 45.0%
🎯 Quality: Mathematically Lossless
⚙️  Command: cjxl 'image.png' '{output}.jxl' -d 0.0 --modular -e 8
```

#### 批量分析目录

```bash
# 当前目录所有图像
imgquality analyze ./images

# 递归扫描子目录
imgquality analyze ./images --recursive

# JSON输出（适合脚本处理）
imgquality analyze ./images --recursive --output json --recommend
```

### 2. 转换图像格式

#### 单个文件转换

```bash
# 转换为 JXL（自动选择最佳参数）
imgquality convert image.png --to jxl

# 指定输出目录
imgquality convert image.png --to jxl --output /path/to/output

# 就地替换（删除原文件）
imgquality convert image.png --to jxl --in-place
```

#### 批量转换

```bash
# 转换目录中所有图像
imgquality convert ./images --to jxl --output ./output

# 递归处理子目录
imgquality convert ./images --to jxl --output ./output --recursive

# 就地替换（谨慎使用！）
imgquality convert ./images --to jxl --in-place --recursive
```

### 3. 验证转换质量

```bash
imgquality verify original.png converted.jxl
```

输出示例：
```
🔍 Verifying conversion quality...
   Original: original.png
   Converted: converted.jxl

📊 Comparison:
   Original size:  1500000 bytes
   Converted size: 750000 bytes
   Size reduction: 50.00%

✅ Verification complete (basic checks passed)
```

## 前端集成

### JavaScript/Node.js 示例

```javascript
const { exec } = require('child_process');
const util = require('util');
const execPromise = util.promisify(exec);

async function analyzeImage(imagePath) {
  const { stdout } = await execPromise(
    `imgquality analyze "${imagePath}" --output json --recommend`
  );
  return JSON.parse(stdout);
}

async function convertImage(imagePath, outputDir) {
  const analysis = await analyzeImage(imagePath);
  
  if (analysis.recommendation.recommended_format === 'JXL') {
    await execPromise(
      `imgquality convert "${imagePath}" --to jxl --output "${outputDir}"`
    );
    console.log(`✅ Converted: ${imagePath} → ${outputDir}`);
  }
}

// 使用示例
(async () => {
  const result = await analyzeImage('photo.png');
  console.log('Analysis:', result);
  console.log('Recommendation:', result.recommendation);
  
  await convertImage('photo.png', './output');
})();
```

### Shell 脚本示例

```bash
#!/bin/bash

# 分析并转换所有PNG图像为JXL
for img in *.png; do
  echo "Processing: $img"
  
  # 获取推荐
  recommendation=$(imgquality analyze "$img" --output json --recommend | jq -r '.recommendation.recommended_format')
  
  if [ "$recommendation" = "JXL" ]; then
    imgquality convert "$img" --to jxl --output ./output
    echo "✅ Converted: $img"
  else
    echo "⏭️  Skipped: $img (no upgrade recommended)"
  fi
done
```

## 核心算法

### 无损检测

对于PNG、GIF等格式：
- 检查格式固有特性（PNG/GIF总是无损）
- 对于WebP，检查VP8L块（无损编码）

对于JPEG：
- 总是视为有损

### 质量评估

- **PSNR** (峰值信噪比)：量化图像失真程度
- **SSIM** (结构相似性)：更接近人眼感知的质量评估
- 基于像素级别分析和统计特征

### 格式升级决策

#### PNG → JXL
- 条件：已确认无损
- 参数：`-d 0.0 --modular`（数学无损）
- 收益：30-60% 体积减少

#### JPEG → JXL
- 特殊对待 仅使用JXL转码参数

#### WebP → JXL
- 无损WebP：`-d 0.0 --modular`
- 有损WebP：`-d 1.0`

## 命令参考

### analyze 命令

```
imgquality analyze [OPTIONS] <INPUT>

参数:
  <INPUT>                输入文件或目录

选项:
  -r, --recursive        递归扫描目录
  -o, --output <FORMAT>  输出格式 [human|json] [default: human]
  --recommend            包含升级建议
  -h, --help            显示帮助信息
```

### convert 命令

```
imgquality convert [OPTIONS] <INPUT>

参数:
  <INPUT>               输入文件或目录

选项:
  -t, --to <FORMAT>     目标格式 [default: jxl]
  -o, --output <DIR>    输出目录
  --in-place            就地替换原文件
  -r, --recursive       递归处理目录
  -h, --help           显示帮助信息
```

### verify 命令

```
imgquality verify <ORIGINAL> <CONVERTED>

参数:
  <ORIGINAL>    原始文件
  <CONVERTED>   转换后的文件
```

## 性能优化

- 编译时启用了 LTO（链接时优化）
- Release 模式使用最高优化级别（opt-level = 3）
- 未来版本将支持并行处理大批量文件

## 项目结构

```
imgquality/
├── src/
│   ├── main.rs          # CLI 入口
│   ├── lib.rs           # 库接口和错误类型
│   ├── analyzer.rs      # 图像质量分析
│   ├── recommender.rs   # 格式升级决策
│   ├── converter.rs     # 格式转换执行
│   └── formats.rs       # 格式特定工具
├── examples/            # 使用示例
├── Cargo.toml          # 项目配置
└── README.md           # 本文档
```

## 常见问题

### Q: 为什么主要推荐JXL格式？

A: JPEG XL (JXL) 是下一代图像格式，具有：
- 更好的压缩效率（比PNG/JPEG小30-60%）
- 支持无损和有损压缩
- 保留完整元数据
- 渐进式加载
- 广泛的色彩空间支持

### Q: 转换是否真正无损？

A: 对于PNG→JXL使用`-d 0.0 --modular`参数时，转换是数学上的无损（bit-for-bit可逆）。工具会在转换后进行健康检查验证。

### Q: 可以批量处理大量图像吗？

A: 可以。使用`--recursive`选项递归处理目录。当前版本按顺序处理，未来版本将支持并行处理。

### Q: 元数据会保留吗？

A: 如果安装了exiftool，转换时会自动保留EXIF/XMP/IPTC元数据和文件时间戳。

### Q: JSON API的输出格式稳定吗？

A: JSON输出格式在同一主版本号内保持稳定，适合用于自动化脚本和前端集成。

## 开发路线图

- [x] 核心质量分析
- [x] JXL格式转换
- [x] JSON API模式
- [ ] 并行批处理
- [ ] 更精确的PSNR/SSIM计算
- [ ] 支持更多输出格式（AVIF, WebP2）
- [ ] GUI 界面
- [ ] 云服务API

## 许可证

MIT License

## 作者

创建于 2025 年，作为媒体处理工具集的一部分。
