# static2jxl

High-performance C tool for batch static image → JXL conversion.

[English](#english) | [中文](#中文)

---

## 中文

### 功能

批量将静态图像转换为 JXL 格式，支持完整元数据保留。

**支持格式:**
| 格式 | 模式 | 说明 |
|------|------|------|
| JPEG | 可逆转码 | 保留 DCT 系数，可完美还原 |
| PNG/BMP/TGA | 无损 (d=0) | >2MB 才转换 |
| TIFF | 无损 (d=0) | 非 JPEG 压缩的 TIFF |

**不处理:** RAW 格式、有损 TIFF、<2MB 的无损格式

### 使用

```bash
# 编译
make

# 基本用法
./static2jxl /path/to/images

# 原地替换
./static2jxl --in-place /path/to/images

# 8 线程
./static2jxl -j 8 /path/to/images
```

### 依赖

```bash
brew install jpeg-xl exiftool
```

---

## English

### Features

Batch convert static images to JXL with complete metadata preservation.

**Supported Formats:**
| Format | Mode | Notes |
|--------|------|-------|
| JPEG | Reversible | Preserves DCT, perfectly reversible |
| PNG/BMP/TGA | Lossless (d=0) | Only >2MB files |
| TIFF | Lossless (d=0) | Non-JPEG compressed TIFF |

**Not processed:** RAW formats, lossy TIFF, <2MB lossless files

### Usage

```bash
# Build
make

# Basic usage
./static2jxl /path/to/images

# In-place replacement
./static2jxl --in-place /path/to/images

# 8 threads
./static2jxl -j 8 /path/to/images
```

### Dependencies

```bash
brew install jpeg-xl exiftool
```

---

MIT License
