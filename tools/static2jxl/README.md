# static2jxl - 智能静态图像转 JXL 工具

[English](#english) | [中文](#中文)

---

## 中文

### 简介

`static2jxl` 是一个高性能的 C 语言批量图像转换工具，专门用于将静态图像转换为 JXL 格式。与 `modern_format_boost` 不同，本工具**仅处理静态图像**，不涉及视频或动态图片。

### 核心特性

- 🚀 **多线程并行处理** - 充分利用多核 CPU
- 🎯 **智能模式选择** - 根据源格式自动选择有损/无损模式
- 📋 **完整元数据保留** - 5 层元数据保留（见下方详细说明）
- ⏰ **系统时间戳保留** - 保持文件修改时间（在最后设置，避免被覆盖）
- 🏥 **健康检查验证** - 确保输出文件可用
- 🔒 **安全检查** - 防止误操作系统目录
- 🔄 **智能回退** - 如果 JXL 比原文件大，自动跳过
- 📊 **详细统计报告** - 按格式分类、压缩率、跳过原因

### 元数据保留（5 层）

遵循 `media/CONTRIBUTING.md` 的元数据保留要求：

| 层级 | 内容 | 工具 |
|------|------|------|
| **Layer 1** | 内部元数据 (EXIF, IPTC, XMP, ICC Profile) | exiftool |
| **Layer 2** | macOS 扩展属性 (xattr) | xattr |
| **Layer 3** | 系统时间戳 (mtime, atime) | utimes() |
| **Layer 4** | macOS 创建时间 (birthtime) | SetFile |
| **Layer 5** | 元数据验证 (verbose 模式) | exiftool |

**关键**：时间戳必须在最后设置！因为 exiftool 会修改文件，从而更新时间戳。

### 支持的格式

| 格式 | 转换模式 | 条件 | 说明 |
|------|----------|------|------|
| **JPEG** | 🔄 可逆转码 (`--lossless_jpeg=1`) | 无 | **保留 DCT 系数，可完美还原原始 JPEG！** |
| **PNG** | 无损 (-d 0) | >2MB | 真正无损，收益巨大 |
| **BMP** | 无损 (-d 0) | >2MB | 未压缩格式，收益巨大 |
| **TIFF** | 无损 (-d 0) | >2MB + 非JPEG压缩 | 见下方说明 |
| **TGA** | 无损 (-d 0) | >2MB | 游戏/设计常用，压缩差 |
| **PPM/PBM/PGM** | 无损 (-d 0) | >2MB | 简单位图格式 |

### JPEG 可逆转码说明

JPEG 使用 `--lossless_jpeg=1` 参数进行**可逆转码**：

- ✅ **保留 DCT 系数** - 不进行任何重新编码
- ✅ **完美可逆** - 可以用 `djxl` 还原为**完全相同**的原始 JPEG
- ✅ **零质量损失** - 这是 JPEG 转 JXL 的最佳方式
- 📉 **压缩率** - 通常可减少 20-30% 文件大小

### TIFF 特殊处理

TIFF 格式较为复杂，可能包含不同的压缩方式：

| TIFF 压缩类型 | 处理方式 | 原因 |
|---------------|----------|------|
| 未压缩 | ✅ 转换 | JXL 收益巨大 |
| LZW/Deflate | ✅ 转换 | JXL 仍有优势 |
| JPEG 压缩 | ❌ 跳过 | 已经有损，不适合 |

### RAW 格式说明

**不处理** RAW 格式（DNG, CR2, CR3, NEF, ARW, ORF, RW2, RAF 等）：

- RAW 保留传感器原始数据，有后期调整空间
- 转 JXL 会失去 RAW 的灵活性
- 建议：保留 RAW，只对导出的成品转 JXL

### 2MB 阈值说明

对于无损源格式（PNG, BMP, TGA, PPM, TIFF），只有文件大于 2MB 才会转换：

- 小文件转换收益有限
- 避免处理图标、缩略图等小文件
- 可通过 `--force-lossless` 强制转换所有文件

### 安装

```bash
# 编译
cd tools/static2jxl
make

# 安装到系统（可选）
sudo make install
```

### 依赖

```bash
# macOS
brew install jpeg-xl exiftool

# Linux (Ubuntu/Debian)
sudo apt install libjxl-tools libimage-exiftool-perl
```

### 使用方法

```bash
# 基本用法
./static2jxl /path/to/images

# 原地替换模式（删除原文件）
./static2jxl --in-place /path/to/images

# 8 线程并行处理
./static2jxl -j 8 /path/to/images

# 详细输出
./static2jxl --verbose /path/to/images

# 预览模式（不实际转换）
./static2jxl --dry-run /path/to/images

# 强制所有格式使用无损模式
./static2jxl --force-lossless /path/to/images
```

### 命令行选项

| 选项 | 说明 |
|------|------|
| `--in-place, -i` | 原地替换，删除原文件 |
| `--skip-health-check` | 跳过健康检查（不推荐） |
| `--no-recursive` | 不递归处理子目录 |
| `--force-lossless` | 强制所有格式使用无损模式 |
| `--verbose, -v` | 显示详细输出 |
| `--dry-run` | 预览模式，不实际转换 |
| `-j <N>` | 并行线程数（默认 4） |
| `-d <distance>` | 覆盖 JXL distance 参数 |
| `-e <effort>` | JXL effort 1-9（默认 7） |

### 与 modern_format_boost 的区别

| 特性 | static2jxl | modern_format_boost |
|------|------------|---------------------|
| 静态图像 | ✅ | ✅ |
| 动态图像 | ❌ | ✅ |
| 视频处理 | ❌ | ✅ |
| 实现语言 | C | Rust |
| 目标场景 | 大批量静态图像 | 全媒体格式优化 |

---

## English

### Introduction

`static2jxl` is a high-performance C batch image converter for converting static images to JXL format. Unlike `modern_format_boost`, this tool **only handles static images**, not videos or animations.

### Key Features

- 🚀 **Multi-threaded Processing** - Fully utilize multi-core CPUs
- 🎯 **Smart Mode Selection** - Auto-select lossy/lossless based on source format
- 📋 **Complete Metadata Preservation** - 5-layer metadata preservation (see below)
- ⏰ **Timestamp Preservation** - Keep file modification times (set LAST to avoid overwrite)
- 🏥 **Health Check Validation** - Ensure output files are valid
- 🔒 **Safety Checks** - Prevent accidental operations on system directories
- 🔄 **Smart Rollback** - Auto-skip if JXL is larger than original
- 📊 **Detailed Statistics** - By format, compression ratio, skip reasons

### Metadata Preservation (5 Layers)

Following `media/CONTRIBUTING.md` requirements:

| Layer | Content | Tool |
|-------|---------|------|
| **Layer 1** | Internal (EXIF, IPTC, XMP, ICC Profile) | exiftool |
| **Layer 2** | macOS Extended Attributes (xattr) | xattr |
| **Layer 3** | System Timestamps (mtime, atime) | utimes() |
| **Layer 4** | macOS Creation Time (birthtime) | SetFile |
| **Layer 5** | Metadata Verification (verbose mode) | exiftool |

**Critical**: Timestamps MUST be set LAST! exiftool modifies the file, which updates timestamps.

### Supported Formats

| Format | Mode | Condition | Notes |
|--------|------|-----------|-------|
| **JPEG** | 🔄 Reversible (`--lossless_jpeg=1`) | None | **Preserves DCT coefficients, perfectly reversible!** |
| **PNG** | Lossless (-d 0) | >2MB | True lossless, huge benefits |
| **BMP** | Lossless (-d 0) | >2MB | Uncompressed, huge benefits |
| **TIFF** | Lossless (-d 0) | >2MB + non-JPEG | See below |
| **TGA** | Lossless (-d 0) | >2MB | Common in games/design, poor compression |
| **PPM/PBM/PGM** | Lossless (-d 0) | >2MB | Simple bitmap formats |

### JPEG Reversible Transcode

JPEG files use `--lossless_jpeg=1` for **reversible transcoding**:

- ✅ **Preserves DCT coefficients** - No re-encoding
- ✅ **Perfectly reversible** - Can restore to **identical** original JPEG with `djxl`
- ✅ **Zero quality loss** - This is the BEST way to convert JPEG to JXL
- 📉 **Compression** - Typically 20-30% size reduction

### TIFF Handling

TIFF is complex and may contain different compression types:

| TIFF Compression | Action | Reason |
|------------------|--------|--------|
| Uncompressed | ✅ Convert | Huge JXL benefits |
| LZW/Deflate | ✅ Convert | JXL still better |
| JPEG compressed | ❌ Skip | Already lossy |

### RAW Formats

RAW formats (DNG, CR2, CR3, NEF, ARW, ORF, RW2, RAF, etc.) are **NOT processed**:

- RAW preserves sensor data with post-processing flexibility
- Converting to JXL loses RAW flexibility
- Recommendation: Keep RAW, only convert exported finals to JXL

### 2MB Threshold

For lossless source formats (PNG, BMP, TGA, PPM, TIFF), only files >2MB are converted:

- Small files have limited conversion benefits
- Avoids processing icons, thumbnails, etc.
- Use `--force-lossless` to convert all files

### Installation

```bash
# Build
cd tools/static2jxl
make

# Install to system (optional)
sudo make install
```

### Dependencies

```bash
# macOS
brew install jpeg-xl exiftool

# Linux (Ubuntu/Debian)
sudo apt install libjxl-tools libimage-exiftool-perl
```

### Usage

```bash
# Basic usage
./static2jxl /path/to/images

# In-place mode (delete originals)
./static2jxl --in-place /path/to/images

# 8 parallel threads
./static2jxl -j 8 /path/to/images

# Verbose output
./static2jxl --verbose /path/to/images

# Dry-run (preview only)
./static2jxl --dry-run /path/to/images

# Force lossless for all formats
./static2jxl --force-lossless /path/to/images
```

### Command Line Options

| Option | Description |
|--------|-------------|
| `--in-place, -i` | Replace originals |
| `--skip-health-check` | Skip health validation (not recommended) |
| `--no-recursive` | Don't process subdirectories |
| `--force-lossless` | Force lossless mode for all formats |
| `--verbose, -v` | Show detailed output |
| `--dry-run` | Preview mode, no actual conversion |
| `-j <N>` | Parallel threads (default: 4) |
| `-d <distance>` | Override JXL distance |
| `-e <effort>` | JXL effort 1-9 (default: 7) |

### Comparison with modern_format_boost

| Feature | static2jxl | modern_format_boost |
|---------|------------|---------------------|
| Static images | ✅ | ✅ |
| Animated images | ❌ | ✅ |
| Video processing | ❌ | ✅ |
| Language | C | Rust |
| Target use case | Batch static images | Full media optimization |
