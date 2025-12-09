# Modern Format Boost 工具集 | Modern Format Boost Toolkit

[English](#english) | [中文](#中文)

---

## 中文

高质量媒体格式升级工具集，将传统格式转换为现代高效格式，同时保留完整元数据。

### 工具概览

| 工具 | 输入类型 | 输出格式 | 主要用途 |
|------|---------|---------|---------|
| **imgquality** | 图像/动图 | JXL / AV1 MP4 | 图像质量分析与格式升级 |
| **vidquality** | 视频 | AV1 MP4 | 视频质量分析与 AV1 压缩 |
| **vidquality-hevc** | 视频 | HEVC MP4 | 视频质量分析与 HEVC 压缩 |

### 共享模块

所有功能集中在 `shared_utils` 库中：

| 模块 | 功能 |
|------|------|
| `metadata` | 完整元数据保留（EXIF/IPTC/xattr/时间戳/ACL） |
| `progress` | 进度条与 ETA 估算 |
| `safety` | 危险目录检测 |
| `batch` | 批量文件处理 |
| `report` | 汇总报告 |
| `ffprobe` | FFprobe 视频分析封装 |
| `tools` | 外部工具检测 |
| `codecs` | 编解码器信息 |

### 核心特性

#### 🎯 智能质量匹配 (`--match-quality`)

所有工具都支持 `--match-quality` 参数，自动分析输入文件质量并计算匹配的输出参数：

- **imgquality**: 根据 JPEG 质量或 bytes-per-pixel 计算 JXL distance
- **vidquality**: 根据 bits-per-pixel 计算 AV1 CRF (18-35)
- **vidquality-hevc**: 根据 bits-per-pixel 计算 HEVC CRF (18-32)

#### 🔄 智能回退机制

**imgquality** 具有智能回退功能：
- 如果转换后文件变大，自动删除输出并跳过
- 避免小型 PNG 或已高度优化图片转换后体积增大的问题
- 输出清晰消息：`⏭️ Rollback: JXL larger than original`

#### 📊 完整元数据保留

通过 `shared_utils::metadata` 模块，所有工具都能完整保留：
- EXIF/IPTC 元数据（通过 ExifTool）
- ICC 颜色配置文件（通过 `--icc` 参数）
- macOS 扩展属性（xattr）
- 文件时间戳（创建时间、修改时间）
- 文件系统标志和 ACL

#### 📈 进度条与批处理

- 带 ETA 估算的可视化进度条 `[████░░] 67%`
- 详细的批量处理汇总报告
- 危险目录安全检查（防止误操作系统目录）
- 并行处理支持（rayon）

### 安装依赖

```bash
# macOS
brew install jpeg-xl ffmpeg exiftool

# 验证安装
cjxl --version
ffmpeg -version
exiftool -ver
```

### 快速开始

```bash
# 编译所有工具
cd tools/modern_format_boost
cargo build --release

# 图像转换（智能回退：变大则跳过）
./target/release/imgquality auto image.jpg --match-quality

# 视频转换 (AV1)
./target/release/vidquality auto video.mp4 --match-quality

# 视频转换 (HEVC)
./target/release/vidquality-hevc auto video.mp4 --match-quality
```

### 详细文档

- [imgquality 文档](imgquality_API/README.md) - 图像质量分析与转换
- [vidquality 文档](vidquality_API/README.md) - AV1 视频转换
- [vidquality-hevc 文档](vidquality_hevc/README.md) - HEVC 视频转换
- [shared_utils 文档](shared_utils/README.md) - 共享工具库

### 项目结构

```
modern_format_boost/
├── imgquality_API/      # 图像工具
├── vidquality_API/      # AV1 视频工具
├── vidquality_hevc/     # HEVC 视频工具
└── shared_utils/        # 共享工具库
    ├── metadata/        # 元数据保留模块
    │   ├── mod.rs       # 主入口
    │   ├── exif.rs      # ExifTool 封装
    │   ├── macos.rs     # macOS 原生 API
    │   ├── linux.rs     # Linux ACL
    │   ├── windows.rs   # Windows 属性
    │   └── network.rs   # 网络元数据验证
    ├── progress.rs      # 进度条与 ETA
    ├── safety.rs        # 危险目录检测
    ├── batch.rs         # 批量文件处理
    ├── report.rs        # 汇总报告
    ├── ffprobe.rs       # FFprobe 视频分析
    ├── tools.rs         # 外部工具检测
    └── codecs.rs        # 编解码器信息
```

### 许可证

MIT License

---

## English

High-quality media format upgrade toolkit that converts traditional formats to modern efficient formats while preserving complete metadata.

### Tool Overview

| Tool | Input Type | Output Format | Main Purpose |
|------|-----------|---------------|--------------|
| **imgquality** | Images/Animations | JXL / AV1 MP4 | Image quality analysis and format upgrade |
| **vidquality** | Videos | AV1 MP4 | Video quality analysis and AV1 compression |
| **vidquality-hevc** | Videos | HEVC MP4 | Video quality analysis and HEVC compression |

### Shared Modules

All functionality is centralized in the `shared_utils` library:

| Module | Function |
|--------|----------|
| `metadata` | Complete metadata preservation (EXIF/IPTC/xattr/timestamps/ACL) |
| `progress` | Progress bar & ETA estimation |
| `safety` | Dangerous directory detection |
| `batch` | Batch file processing |
| `report` | Summary reports |
| `ffprobe` | FFprobe video analysis wrapper |
| `tools` | External tool detection |
| `codecs` | Codec information |

### Core Features

#### 🎯 Smart Quality Matching (`--match-quality`)

All tools support the `--match-quality` parameter, automatically analyzing input file quality and calculating matching output parameters:

- **imgquality**: Calculates JXL distance based on JPEG quality or bytes-per-pixel
- **vidquality**: Calculates AV1 CRF (18-35) based on bits-per-pixel
- **vidquality-hevc**: Calculates HEVC CRF (18-32) based on bits-per-pixel

#### 🔄 Smart Rollback Mechanism

**imgquality** features smart rollback:
- Automatically deletes output and skips if converted file is larger
- Avoids size increase issues with small PNGs or highly optimized images
- Clear output message: `⏭️ Rollback: JXL larger than original`

#### 📊 Complete Metadata Preservation

Through the `shared_utils::metadata` module, all tools preserve:
- EXIF/IPTC metadata (via ExifTool)
- ICC color profiles (via `--icc` parameter)
- macOS extended attributes (xattr)
- File timestamps (creation time, modification time)
- File system flags and ACL

#### 📈 Progress Bar & Batch Processing

- Visual progress bar with ETA estimation `[████░░] 67%`
- Detailed batch processing summary reports
- Dangerous directory safety checks
- Parallel processing support (rayon)

### Install Dependencies

```bash
# macOS
brew install jpeg-xl ffmpeg exiftool

# Verify installation
cjxl --version
ffmpeg -version
exiftool -ver
```

### Quick Start

```bash
# Build all tools
cd tools/modern_format_boost
cargo build --release

# Image conversion (smart rollback: skip if larger)
./target/release/imgquality auto image.jpg --match-quality

# Video conversion (AV1)
./target/release/vidquality auto video.mp4 --match-quality

# Video conversion (HEVC)
./target/release/vidquality-hevc auto video.mp4 --match-quality
```

### Detailed Documentation

- [imgquality Documentation](imgquality_API/README.md) - Image quality analysis and conversion
- [vidquality Documentation](vidquality_API/README.md) - AV1 video conversion
- [vidquality-hevc Documentation](vidquality_hevc/README.md) - HEVC video conversion
- [shared_utils Documentation](shared_utils/README.md) - Shared utility library

### Project Structure

```
modern_format_boost/
├── imgquality_API/      # Image tool
├── vidquality_API/      # AV1 video tool
├── vidquality_hevc/     # HEVC video tool
└── shared_utils/        # Shared utility library
    ├── metadata/        # Metadata preservation module
    │   ├── mod.rs       # Main entry
    │   ├── exif.rs      # ExifTool wrapper
    │   ├── macos.rs     # macOS native API
    │   ├── linux.rs     # Linux ACL
    │   ├── windows.rs   # Windows attributes
    │   └── network.rs   # Network metadata verification
    ├── progress.rs      # Progress bar & ETA
    ├── safety.rs        # Dangerous directory detection
    ├── batch.rs         # Batch file processing
    ├── report.rs        # Summary reports
    ├── ffprobe.rs       # FFprobe video analysis
    ├── tools.rs         # External tool detection
    └── codecs.rs        # Codec information
```

### License

MIT License
