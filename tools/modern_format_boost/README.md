# Modern Format Boost

High-quality media format upgrade toolkit with complete metadata preservation.

[English](#english) | [中文](#中文)

---

## 中文

### 工具概览

| 工具 | 输入 | 输出 | 编码器 | 适用场景 |
|------|------|------|--------|----------|
| **imgquality** | 图像/动图 | JXL / AV1 MP4 | SVT-AV1 | 最佳压缩率 |
| **imgquality-hevc** | 图像/动图 | JXL / HEVC MP4 | x265 | Apple 兼容 |
| **vidquality** | 视频 | AV1 MP4 | SVT-AV1 | 最佳压缩率 |
| **vidquality-hevc** | 视频 | HEVC MP4 | x265 | Apple 兼容 |

### 核心特性

- 🎯 **智能质量匹配** (`--match-quality`): 自动计算最佳输出参数
- 📊 **完整元数据保留**: EXIF/IPTC/XMP/ICC/时间戳/xattr
- 🔄 **智能回退**: 输出大于输入时自动跳过
- 📈 **进度可视化**: 实时进度条和 ETA
- 🛡️ **安全检查**: 危险目录检测

### 快速开始

```bash
# 编译
cargo build --release

# 图像转换 (Apple 兼容)
./target/release/imgquality-hevc auto /path/to/images --match-quality --delete-original

# 视频转换 (Apple 兼容)
./target/release/vidquality-hevc auto /path/to/videos --match-quality --delete-original
```

### 转换策略

**图像工具:**
| 输入 | 输出 |
|------|------|
| JPEG | JXL (无损转码) |
| PNG/BMP/TIFF | JXL (d=0) |
| 动图 ≥3s | AV1/HEVC MP4 |
| 有损格式 | 跳过 |

**视频工具:**
| 输入 | 输出 |
|------|------|
| H.265/AV1/VP9 | 跳过 |
| H.264 | AV1/HEVC |
| 无损 | 无损 AV1/HEVC |

### 依赖

```bash
brew install jpeg-xl ffmpeg exiftool
```

---

## English

### Tool Overview

| Tool | Input | Output | Encoder | Use Case |
|------|-------|--------|---------|----------|
| **imgquality** | Images/GIF | JXL / AV1 MP4 | SVT-AV1 | Best compression |
| **imgquality-hevc** | Images/GIF | JXL / HEVC MP4 | x265 | Apple compatible |
| **vidquality** | Videos | AV1 MP4 | SVT-AV1 | Best compression |
| **vidquality-hevc** | Videos | HEVC MP4 | x265 | Apple compatible |

### Key Features

- 🎯 **Smart Quality Matching** (`--match-quality`): Auto-calculate optimal output params
- 📊 **Complete Metadata**: EXIF/IPTC/XMP/ICC/timestamps/xattr preserved
- 🔄 **Smart Rollback**: Auto-skip if output larger than input
- 📈 **Progress Visualization**: Real-time progress bar with ETA
- 🛡️ **Safety Checks**: Dangerous directory detection

### Quick Start

```bash
# Build
cargo build --release

# Image conversion (Apple compatible)
./target/release/imgquality-hevc auto /path/to/images --match-quality --delete-original

# Video conversion (Apple compatible)
./target/release/vidquality-hevc auto /path/to/videos --match-quality --delete-original
```

### Conversion Strategy

**Image Tools:**
| Input | Output |
|-------|--------|
| JPEG | JXL (lossless transcode) |
| PNG/BMP/TIFF | JXL (d=0) |
| Animation ≥3s | AV1/HEVC MP4 |
| Lossy formats | Skip |

**Video Tools:**
| Input | Output |
|-------|--------|
| H.265/AV1/VP9 | Skip |
| H.264 | AV1/HEVC |
| Lossless | Lossless AV1/HEVC |

### Dependencies

```bash
brew install jpeg-xl ffmpeg exiftool
```

---

MIT License
