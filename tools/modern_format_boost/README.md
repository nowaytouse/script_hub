# Modern Format Boost 工具集

高质量媒体格式升级工具集，将传统格式转换为现代高效格式，同时保留完整元数据。

## 工具概览

| 工具 | 输入类型 | 输出格式 | 主要用途 |
|------|---------|---------|---------|
| **imgquality** | 图像/动图 | JXL / AV1 MP4 | 图像质量分析与格式升级 |
| **vidquality** | 视频 | AV1 MP4 | 视频质量分析与 AV1 压缩 |
| **vidquality-hevc** | 视频 | HEVC MP4 | 视频质量分析与 HEVC 压缩 |

## 核心特性

### 🎯 智能质量匹配 (`--match-quality`)

所有工具都支持 `--match-quality` 参数，自动分析输入文件质量并计算匹配的输出参数：

- **imgquality**: 根据 JPEG 质量或 bytes-per-pixel 计算 JXL distance
- **vidquality**: 根据 bits-per-pixel 计算 AV1 CRF (18-35)
- **vidquality-hevc**: 根据 bits-per-pixel 计算 HEVC CRF (18-32)

### 📊 完整元数据保留

通过 `metadata_keeper` 模块，所有工具都能完整保留：
- EXIF/IPTC 元数据（通过 ExifTool）
- macOS 扩展属性（xattr）
- 文件时间戳（创建时间、修改时间）
- 文件系统标志和 ACL

### 🔄 智能转换策略

**Auto 模式**会根据输入格式智能选择转换策略：
- 现代格式（HEVC/AV1/VP9）→ 跳过（避免代际损失）
- 无损源 → 无损输出
- 有损源 → 质量匹配的有损输出

## 安装依赖

```bash
# macOS
brew install jpeg-xl ffmpeg exiftool

# 验证安装
cjxl --version
ffmpeg -version
exiftool -ver
```

## 快速开始

```bash
# 编译所有工具
cd tools/modern_format_boost
cargo build --release

# 图像转换
./target/release/imgquality auto image.jpg --match-quality

# 视频转换 (AV1)
./target/release/vidquality auto video.mp4 --match-quality

# 视频转换 (HEVC)
./target/release/vidquality-hevc auto video.mp4 --match-quality
```

## 详细文档

- [imgquality 文档](imgquality_API/README.md) - 图像质量分析与转换
- [vidquality 文档](vidquality_API/README.md) - AV1 视频转换
- [vidquality-hevc 文档](vidquality_hevc/README.md) - HEVC 视频转换

## 项目结构

```
modern_format_boost/
├── imgquality_API/      # 图像工具
├── vidquality_API/      # AV1 视频工具
├── vidquality_hevc/     # HEVC 视频工具
└── metadata_keeper/     # 共享元数据保留模块
```

## 许可证

MIT License
