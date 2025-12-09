# jpeg2jxl - High-Performance JPEG to JXL Batch Converter

[English](#english) | [中文](#中文)

---

## 中文

高性能 C 语言实现的 JPEG 到 JXL 批量转换工具，专为大规模批量处理设计。

### 与 Shell 脚本版本的对比

| 特性 | jpeg2jxl (C) | jpeg_to_jxl.sh |
|------|-------------|----------------|
| 语言 | C | Bash |
| 多线程 | ✅ 原生支持 | ❌ 单线程 |
| 性能 | **极快** | 较慢 |
| 内存效率 | 高 | 低 |
| 大批量处理 | **优化** | 一般 |
| 启动开销 | 极低 | 高（每个文件启动子进程） |

**性能提升**: 在多核系统上，C 版本比 Shell 脚本快 **5-10 倍**。

### 功能特性

- 🚀 **多线程并行处理**: 充分利用多核 CPU
- 📦 **完整元数据保留**: EXIF, XMP, IPTC (通过 exiftool)
- ⏰ **系统时间戳保留**: 保持原始文件的修改时间
- 🏥 **健康检查验证**: 转换后验证 JXL 文件完整性
- 📊 **进度条与 ETA**: 实时显示转换进度和预计剩余时间
- 🛡️ **安全检查**: 危险目录检测，防止误操作
- 🔄 **原地替换模式**: 可选删除原始文件
- ⚙️ **可配置质量**: 支持无损和有损压缩

### 编译

```bash
cd tools/jpeg2jxl
make
```

编译后的二进制文件位于 `bin/jpeg2jxl`。

### 安装

```bash
make install  # 安装到 /usr/local/bin/
```

### 依赖

- `cjxl` (libjxl) - JXL 编码
- `djxl` (libjxl) - JXL 解码（健康检查用）
- `exiftool` - 元数据迁移

```bash
# macOS
brew install jpeg-xl exiftool

# Ubuntu/Debian
sudo apt install libjxl-tools libimage-exiftool-perl
```

### 使用方法

```bash
# 标准模式（在原文件旁创建 .jxl）
jpeg2jxl /path/to/images

# 原地替换模式（删除原始 JPEG）
jpeg2jxl --in-place /path/to/images

# 使用 8 个线程
jpeg2jxl -j 8 /path/to/images

# 无损压缩
jpeg2jxl -d 0 /path/to/images

# 预览模式（不实际转换）
jpeg2jxl --dry-run /path/to/images

# 详细输出
jpeg2jxl --verbose /path/to/images
```

### 命令行选项

| 选项 | 说明 | 默认值 |
|------|------|--------|
| `--in-place, -i` | 原地替换模式 | 关闭 |
| `--skip-health-check` | 跳过健康检查 | 关闭 |
| `--no-recursive` | 不处理子目录 | 递归 |
| `--verbose, -v` | 详细输出 | 关闭 |
| `--dry-run` | 预览模式 | 关闭 |
| `-j <N>` | 并行线程数 | 4 |
| `-d <distance>` | JXL 距离 (0=无损, 1=高质量) | 1.0 |
| `-e <effort>` | JXL 努力度 (1-9) | 7 |

### 输出示例

```
╔══════════════════════════════════════════════╗
║   📷 jpeg2jxl - High-Performance Converter   ║
╚══════════════════════════════════════════════╝

ℹ️  [INFO] 📁 Target: /path/to/images
ℹ️  [INFO] 📋 Whitelist: .jpg, .jpeg → .jxl
ℹ️  [INFO] 🎯 Quality: distance=1.0, effort=7
ℹ️  [INFO] 🔧 Threads: 4
ℹ️  [INFO] 📁 Found: 1234 files

📊 Progress: [████████████████████░░░░░░░░░░] 67% (827/1234) | ⏱️  ETA: ~2m 15s
   📄 photo_2024_summer_vacation.jpg

╔══════════════════════════════════════════════╗
║   📊 Conversion Complete                     ║
╚══════════════════════════════════════════════╝

📈 Statistics:
   Total files:    1234
   ✅ Success:      1230
   ❌ Failed:       2
   ⏭️  Skipped:      2
   ⏱️  Time:         5m 32s
   💾 Input:        2456.78 MB
   💾 Output:       1234.56 MB
   📉 Reduction:    49.7%

🏥 Health Report:
   ✅ Passed:  1230
   ❌ Failed:  2
   📊 Rate:    99%
```

---

## English

High-performance C implementation of JPEG to JXL batch converter, designed for large-scale batch processing.

### Comparison with Shell Script

| Feature | jpeg2jxl (C) | jpeg_to_jxl.sh |
|---------|-------------|----------------|
| Language | C | Bash |
| Multi-threading | ✅ Native | ❌ Single-threaded |
| Performance | **Blazing fast** | Slow |
| Memory Efficiency | High | Low |
| Large Batch | **Optimized** | Average |
| Startup Overhead | Minimal | High (subprocess per file) |

**Performance Boost**: On multi-core systems, C version is **5-10x faster** than shell script.

### Features

- 🚀 **Multi-threaded Processing**: Fully utilizes multi-core CPUs
- 📦 **Complete Metadata Preservation**: EXIF, XMP, IPTC (via exiftool)
- ⏰ **System Timestamp Preservation**: Maintains original file modification time
- 🏥 **Health Check Validation**: Verifies JXL file integrity after conversion
- 📊 **Progress Bar with ETA**: Real-time progress and estimated time remaining
- 🛡️ **Safety Checks**: Dangerous directory detection to prevent accidents
- 🔄 **In-place Mode**: Optional original file deletion
- ⚙️ **Configurable Quality**: Supports lossless and lossy compression

### Build

```bash
cd tools/jpeg2jxl
make
```

Binary will be at `bin/jpeg2jxl`.

### Install

```bash
make install  # Installs to /usr/local/bin/
```

### Dependencies

- `cjxl` (libjxl) - JXL encoding
- `djxl` (libjxl) - JXL decoding (for health check)
- `exiftool` - Metadata migration

```bash
# macOS
brew install jpeg-xl exiftool

# Ubuntu/Debian
sudo apt install libjxl-tools libimage-exiftool-perl
```

### Usage

```bash
# Standard mode (creates .jxl alongside original)
jpeg2jxl /path/to/images

# In-place mode (deletes original JPEG)
jpeg2jxl --in-place /path/to/images

# Use 8 threads
jpeg2jxl -j 8 /path/to/images

# Lossless compression
jpeg2jxl -d 0 /path/to/images

# Preview mode (no actual conversion)
jpeg2jxl --dry-run /path/to/images

# Verbose output
jpeg2jxl --verbose /path/to/images
```

### Command Line Options

| Option | Description | Default |
|--------|-------------|---------|
| `--in-place, -i` | In-place replacement mode | Off |
| `--skip-health-check` | Skip health validation | Off |
| `--no-recursive` | Don't process subdirectories | Recursive |
| `--verbose, -v` | Verbose output | Off |
| `--dry-run` | Preview mode | Off |
| `-j <N>` | Number of parallel threads | 4 |
| `-d <distance>` | JXL distance (0=lossless, 1=high quality) | 1.0 |
| `-e <effort>` | JXL effort (1-9) | 7 |
