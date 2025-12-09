# Modern Format Boost 工具集 | Modern Format Boost Toolkit

[English](#english) | [中文](#中文)

---

## 中文

高质量媒体格式升级工具集，将传统格式转换为现代高效格式，同时保留完整元数据。

本工具建议用于重要图集升级用途使用

### 🎯 设计理念

1. **质量优先**: 默认使用最高质量设置（CRF 0 / 数学无损），避免代际损失
2. **智能决策**: 自动检测输入格式和质量，选择最佳转换策略
3. **元数据完整**: 完整保留 EXIF/IPTC/xattr/时间戳/ACL
4. **安全可靠**: 危险目录检测、智能回退、响亮报错
5. **性能优化**: 并行处理、线程限制、进度可视化

### 工具概览

| 工具 | 输入类型 | 输出格式 | 视频编码器 | 主要用途 |
|------|---------|---------|-----------|---------|
| **imgquality** | 图像/动图 | JXL / AV1 MP4 | SVT-AV1 | 图像质量分析与格式升级 |
| **imgquality-hevc** | 图像/动图 | JXL / HEVC MP4 | x265 | 图像质量分析与 HEVC 动图转换 |
| **vidquality** | 视频 | AV1 MP4 | SVT-AV1 | 视频质量分析与 AV1 压缩 |
| **vidquality-hevc** | 视频 | HEVC MP4 | x265 | 视频质量分析与 HEVC 压缩 |

### 编码器对比

| 编码器 | 压缩效率 | 编码速度 | 兼容性 | 专利 |
|--------|---------|---------|--------|------|
| **SVT-AV1** | ⭐⭐⭐⭐⭐ 最佳 | ⭐⭐⭐ 中等 | ⭐⭐⭐⭐ 良好 | 免费 |
| **x265 (HEVC)** | ⭐⭐⭐⭐ 优秀 | ⭐⭐⭐⭐ 快速 | ⭐⭐⭐⭐⭐ 极佳 | 需授权 |

**选择建议**:
- 追求最佳压缩率 → AV1 (imgquality / vidquality)
- 需要 Apple 设备兼容 → HEVC (imgquality-hevc / vidquality-hevc)
- 需要快速编码 → HEVC

### 共享模块 (shared_utils)

所有功能集中在 `shared_utils` 库中，避免代码重复：

| 模块 | 功能 |
|------|------|
| `metadata` | 完整元数据保留（EXIF/IPTC/xattr/时间戳/ACL） |
| `conversion` | 转换通用功能（ConversionResult/ConvertOptions/防重复） |
| `progress` | 进度条与 ETA 估算 |
| `safety` | 危险目录检测 |
| `batch` | 批量文件处理 |
| `report` | 汇总报告 |
| `ffprobe` | FFprobe 视频分析封装 |
| `tools` | 外部工具检测 |
| `codecs` | 编解码器信息 |
| `video` | 视频处理通用功能（偶数分辨率填充等） |

### 核心特性

#### 🎯 智能质量匹配 (`--match-quality`)

所有工具都支持 `--match-quality` 参数，自动分析输入文件质量并计算匹配的输出参数：

- **imgquality/imgquality-hevc**: 根据 JPEG 质量或 bytes-per-pixel 计算 JXL distance 或视频 CRF
- **vidquality**: 根据 bits-per-pixel 计算 AV1 CRF (18-35)
- **vidquality-hevc**: 根据 bits-per-pixel 计算 HEVC CRF (18-32)

#### 🔄 智能回退机制

**imgquality/imgquality-hevc** 具有智能回退功能：
- 如果转换后文件变大，自动删除输出并跳过
- 避免小型 PNG 或已高度优化图片转换后体积增大的问题
- 输出清晰消息：`⏭️ Rollback: JXL larger than original`

#### 📊 完整元数据保留

通过 `shared_utils::metadata` 模块，所有工具都能完整保留：
- EXIF/IPTC 元数据（通过 ExifTool）
- ICC 颜色配置文件
- macOS 扩展属性（xattr）
- 文件时间戳（创建时间、修改时间）
- 文件系统标志和 ACL

#### 📈 进度条与批处理

- 带 ETA 估算的可视化进度条 `[████░░] 67%`
- 详细的批量处理汇总报告
- 危险目录安全检查（防止误操作系统目录）
- 并行处理支持（rayon，限制线程数避免系统卡顿）

#### 🎬 偶数分辨率自动填充

所有视频转换工具自动处理奇数分辨率问题：
- AV1/HEVC 编码器要求宽高为偶数
- 自动添加 1 像素填充（黑色边框）
- 避免 "Picture height must be an integer multiple of the specified chroma subsampling" 错误

### 安装依赖

```bash
# macOS
brew install jpeg-xl ffmpeg exiftool

# 验证安装
cjxl --version
ffmpeg -version
exiftool -ver

# 验证 SVT-AV1 支持
ffmpeg -encoders | grep svt
```

### 快速开始

```bash
# 编译所有工具
cd tools/modern_format_boost
cargo build --release

# 图像转换 (AV1 动图)
./target/release/imgquality auto image.jpg --match-quality

# 图像转换 (HEVC 动图，Apple 兼容)
./target/release/imgquality-hevc auto image.jpg --match-quality

# 视频转换 (AV1)
./target/release/vidquality auto video.mp4 --match-quality

# 视频转换 (HEVC)
./target/release/vidquality-hevc auto video.mp4 --match-quality
```

### 转换策略对照表

#### 图像工具 (imgquality / imgquality-hevc)

| 输入类型 | 条件 | imgquality 输出 | imgquality-hevc 输出 |
|---------|------|----------------|---------------------|
| JPEG | 默认 | JXL (无损转码) | JXL (无损转码) |
| JPEG | `--match-quality` | JXL (有损匹配) | JXL (有损匹配) |
| PNG/TIFF/BMP (无损) | - | JXL (d=0) | JXL (d=0) |
| WebP/AVIF/HEIC (无损) | - | JXL (d=0) | JXL (d=0) |
| WebP/AVIF/HEIC (有损) | - | 跳过 | 跳过 |
| 动图 (无损) | ≥3秒 | **AV1 MP4** (CRF 0) | **HEVC MP4** (CRF 0) |
| 动图 (无损) | ≥3秒 + `--match-quality` | **AV1 MP4** (CRF 匹配) | **HEVC MP4** (CRF 匹配) |
| 动图 | <3秒 | 跳过 | 跳过 |

#### 视频工具 (vidquality / vidquality-hevc)

| 输入编码 | 压缩类型 | vidquality 输出 | vidquality-hevc 输出 |
|---------|---------|----------------|---------------------|
| H.265/AV1/VP9/VVC | 任意 | 跳过 | 跳过 |
| FFV1/其他无损 | 无损 | AV1 无损 | HEVC 无损 MKV |
| ProRes/DNxHD | 视觉无损 | AV1 CRF 0 | HEVC CRF 18 |
| H.264/其他 | 有损 | AV1 CRF 0 | HEVC CRF 20 |
| H.264/其他 | 有损 + `--match-quality` | AV1 CRF 18-35 | HEVC CRF 18-32 |

### 详细文档

- [imgquality 文档](imgquality_API/README.md) - 图像质量分析与 AV1 动图转换
- [imgquality-hevc 文档](imgquality_hevc/README.md) - 图像质量分析与 HEVC 动图转换
- [vidquality 文档](vidquality_API/README.md) - AV1 视频转换
- [vidquality-hevc 文档](vidquality_hevc/README.md) - HEVC 视频转换
- [shared_utils 文档](shared_utils/README.md) - 共享工具库

### 项目结构

```
modern_format_boost/
├── imgquality_API/      # 图像工具 (AV1 动图)
├── imgquality_hevc/     # 图像工具 (HEVC 动图)
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
    ├── conversion.rs    # 转换通用功能
    ├── progress.rs      # 进度条与 ETA
    ├── safety.rs        # 危险目录检测
    ├── batch.rs         # 批量文件处理
    ├── report.rs        # 汇总报告
    ├── ffprobe.rs       # FFprobe 视频分析
    ├── tools.rs         # 外部工具检测
    ├── codecs.rs        # 编解码器信息
    └── video.rs         # 视频处理通用功能
```

### 许可证

MIT License

---

## English

High-quality media format upgrade toolkit that converts traditional formats to modern efficient formats while preserving complete metadata.

This tool is recommended for use in critical album upgrades.

### 🎯 Design Philosophy

1. **Quality First**: Default to highest quality settings (CRF 0 / mathematical lossless), avoid generational loss
2. **Smart Decisions**: Auto-detect input format and quality, select optimal conversion strategy
3. **Complete Metadata**: Full preservation of EXIF/IPTC/xattr/timestamps/ACL
4. **Safe & Reliable**: Dangerous directory detection, smart rollback, loud errors
5. **Performance Optimized**: Parallel processing, thread limiting, progress visualization

### Tool Overview

| Tool | Input Type | Output Format | Video Encoder | Main Purpose |
|------|-----------|---------------|---------------|--------------|
| **imgquality** | Images/Animations | JXL / AV1 MP4 | SVT-AV1 | Image quality analysis and format upgrade |
| **imgquality-hevc** | Images/Animations | JXL / HEVC MP4 | x265 | Image quality analysis and HEVC animation conversion |
| **vidquality** | Videos | AV1 MP4 | SVT-AV1 | Video quality analysis and AV1 compression |
| **vidquality-hevc** | Videos | HEVC MP4 | x265 | Video quality analysis and HEVC compression |

### Encoder Comparison

| Encoder | Compression | Speed | Compatibility | Patents |
|---------|-------------|-------|---------------|---------|
| **SVT-AV1** | ⭐⭐⭐⭐⭐ Best | ⭐⭐⭐ Medium | ⭐⭐⭐⭐ Good | Royalty-free |
| **x265 (HEVC)** | ⭐⭐⭐⭐ Excellent | ⭐⭐⭐⭐ Fast | ⭐⭐⭐⭐⭐ Excellent | Licensed |

**Recommendations**:
- Want best compression ratio → AV1 (imgquality / vidquality)
- Need Apple device compatibility → HEVC (imgquality-hevc / vidquality-hevc)
- Need fast encoding → HEVC

### Shared Modules (shared_utils)

All functionality is centralized in the `shared_utils` library to avoid code duplication:

| Module | Function |
|--------|----------|
| `metadata` | Complete metadata preservation (EXIF/IPTC/xattr/timestamps/ACL) |
| `conversion` | Conversion utilities (ConversionResult/ConvertOptions/anti-duplicate) |
| `progress` | Progress bar & ETA estimation |
| `safety` | Dangerous directory detection |
| `batch` | Batch file processing |
| `report` | Summary reports |
| `ffprobe` | FFprobe video analysis wrapper |
| `tools` | External tool detection |
| `codecs` | Codec information |
| `video` | Video processing utilities (even dimension padding, etc.) |

### Core Features

#### 🎯 Smart Quality Matching (`--match-quality`)

All tools support the `--match-quality` parameter, automatically analyzing input file quality and calculating matching output parameters:

- **imgquality/imgquality-hevc**: Calculates JXL distance or video CRF based on JPEG quality or bytes-per-pixel
- **vidquality**: Calculates AV1 CRF (18-35) based on bits-per-pixel
- **vidquality-hevc**: Calculates HEVC CRF (18-32) based on bits-per-pixel

#### 🔄 Smart Rollback Mechanism

**imgquality/imgquality-hevc** features smart rollback:
- Automatically deletes output and skips if converted file is larger
- Avoids size increase issues with small PNGs or highly optimized images
- Clear output message: `⏭️ Rollback: JXL larger than original`

#### 📊 Complete Metadata Preservation

Through the `shared_utils::metadata` module, all tools preserve:
- EXIF/IPTC metadata (via ExifTool)
- ICC color profiles
- macOS extended attributes (xattr)
- File timestamps (creation time, modification time)
- File system flags and ACL

#### 📈 Progress Bar & Batch Processing

- Visual progress bar with ETA estimation `[████░░] 67%`
- Detailed batch processing summary reports
- Dangerous directory safety checks
- Parallel processing support (rayon, with thread limiting to avoid system slowdown)

#### 🎬 Auto Even Dimension Padding

All video conversion tools automatically handle odd resolution issues:
- AV1/HEVC encoders require even width and height
- Automatically adds 1 pixel padding (black border)
- Avoids "Picture height must be an integer multiple of the specified chroma subsampling" error

### Install Dependencies

```bash
# macOS
brew install jpeg-xl ffmpeg exiftool

# Verify installation
cjxl --version
ffmpeg -version
exiftool -ver

# Verify SVT-AV1 support
ffmpeg -encoders | grep svt
```

### Quick Start

```bash
# Build all tools
cd tools/modern_format_boost
cargo build --release

# Image conversion (AV1 animation)
./target/release/imgquality auto image.jpg --match-quality

# Image conversion (HEVC animation, Apple compatible)
./target/release/imgquality-hevc auto image.jpg --match-quality

# Video conversion (AV1)
./target/release/vidquality auto video.mp4 --match-quality

# Video conversion (HEVC)
./target/release/vidquality-hevc auto video.mp4 --match-quality
```

### Conversion Strategy Reference

#### Image Tools (imgquality / imgquality-hevc)

| Input Type | Condition | imgquality Output | imgquality-hevc Output |
|------------|-----------|-------------------|------------------------|
| JPEG | Default | JXL (lossless transcode) | JXL (lossless transcode) |
| JPEG | `--match-quality` | JXL (lossy matched) | JXL (lossy matched) |
| PNG/TIFF/BMP (lossless) | - | JXL (d=0) | JXL (d=0) |
| WebP/AVIF/HEIC (lossless) | - | JXL (d=0) | JXL (d=0) |
| WebP/AVIF/HEIC (lossy) | - | Skip | Skip |
| Animation (lossless) | ≥3s | **AV1 MP4** (CRF 0) | **HEVC MP4** (CRF 0) |
| Animation (lossless) | ≥3s + `--match-quality` | **AV1 MP4** (CRF matched) | **HEVC MP4** (CRF matched) |
| Animation | <3s | Skip | Skip |

#### Video Tools (vidquality / vidquality-hevc)

| Input Codec | Compression | vidquality Output | vidquality-hevc Output |
|-------------|-------------|-------------------|------------------------|
| H.265/AV1/VP9/VVC | Any | Skip | Skip |
| FFV1/Other lossless | Lossless | AV1 Lossless | HEVC Lossless MKV |
| ProRes/DNxHD | Visually Lossless | AV1 CRF 0 | HEVC CRF 18 |
| H.264/Other | Lossy | AV1 CRF 0 | HEVC CRF 20 |
| H.264/Other | Lossy + `--match-quality` | AV1 CRF 18-35 | HEVC CRF 18-32 |

### Detailed Documentation

- [imgquality Documentation](imgquality_API/README.md) - Image quality analysis and AV1 animation conversion
- [imgquality-hevc Documentation](imgquality_hevc/README.md) - Image quality analysis and HEVC animation conversion
- [vidquality Documentation](vidquality_API/README.md) - AV1 video conversion
- [vidquality-hevc Documentation](vidquality_hevc/README.md) - HEVC video conversion
- [shared_utils Documentation](shared_utils/README.md) - Shared utility library

### Project Structure

```
modern_format_boost/
├── imgquality_API/      # Image tool (AV1 animation)
├── imgquality_hevc/     # Image tool (HEVC animation)
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
    ├── conversion.rs    # Conversion utilities
    ├── progress.rs      # Progress bar & ETA
    ├── safety.rs        # Dangerous directory detection
    ├── batch.rs         # Batch file processing
    ├── report.rs        # Summary reports
    ├── ffprobe.rs       # FFprobe video analysis
    ├── tools.rs         # External tool detection
    ├── codecs.rs        # Codec information
    └── video.rs         # Video processing utilities
```

### License

MIT License
