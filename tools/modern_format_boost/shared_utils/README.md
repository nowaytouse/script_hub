# Shared Utilities - 共享工具库

[English](#english) | [中文](#中文)

---

## 中文

modern_format_boost 工具集（imgquality、vidquality、vidquality-hevc）的共享工具库。

### 模块概览

| 模块 | 功能 |
|------|------|
| `metadata` | 完整元数据保留（EXIF/IPTC/xattr/时间戳/ACL） |
| `progress` | 进度条与 ETA 估算（含 SmartProgressBar） |
| `safety` | 危险目录检测 |
| `batch` | 批量文件处理 |
| `report` | 汇总报告 |
| `ffprobe` | FFprobe 视频分析封装 |
| `tools` | 外部工具检测 |
| `codecs` | 编解码器信息 |
| `conversion` | **转换通用功能**（ConversionResult/ConvertOptions/防重复） |
| `video` | **视频处理工具**（偶数尺寸修正/滤镜链生成/YUV420兼容性） |

### 模块详情

#### 1. 元数据保留 (`metadata/`)

完整的跨平台元数据保留，支持三层架构：

```rust
use shared_utils::preserve_metadata;

// 保留所有元数据
preserve_metadata(src_path, dst_path)?;
```

**支持的元数据类型**：
- **内部层**: EXIF/IPTC/XMP（通过 ExifTool）
- **网络层**: WhereFroms、用户标签
- **系统层**: ACL、文件标志、xattr、时间戳

**平台支持**：
- macOS: 使用原生 `copyfile` API
- Linux: 使用 `getfacl`/`setfacl`
- Windows: 使用 PowerShell ACL 命令

#### 2. 进度条 (`progress.rs`)

带 ETA 估算的可视化进度反馈。

```rust
use shared_utils::{create_progress_bar, format_bytes, format_duration};

let pb = create_progress_bar(100, "Converting");
pb.inc(1);
pb.finish_with_message("Done!");
```

#### 3. 安全检查 (`safety.rs`)

防止意外损坏系统目录。

```rust
use shared_utils::check_dangerous_directory;

check_dangerous_directory(Path::new("/Users/me/Documents/photos"))?;
```

#### 4. 批量处理 (`batch.rs`)

批量文件处理工具。

```rust
use shared_utils::{collect_files, IMAGE_EXTENSIONS, BatchResult};

let files = collect_files(Path::new("./photos"), IMAGE_EXTENSIONS, true);
```

#### 5. 汇总报告 (`report.rs`)

详细的批量操作报告。

```rust
use shared_utils::print_summary_report;

print_summary_report(&result, duration, input_bytes, output_bytes, "Conversion");
```

#### 6. FFprobe 封装 (`ffprobe.rs`)

视频/动画分析。

```rust
use shared_utils::{probe_video, get_duration, get_frame_count};

let result = probe_video(Path::new("video.mp4"))?;
println!("Codec: {}, Duration: {}s", result.video_codec, result.duration);
```

#### 7. 外部工具检测 (`tools.rs`)

检查所需外部工具。

```rust
use shared_utils::{check_image_tools, require_tools};

let tools = check_image_tools();
require_tools(&["ffmpeg", "cjxl", "exiftool"])?;
```

#### 8. 编解码器信息 (`codecs.rs`)

视频编解码器检测和信息。

```rust
use shared_utils::{DetectedCodec, get_codec_info};

let codec = DetectedCodec::from_ffprobe("h264");
println!("Modern: {}", codec.is_modern());
```

#### 9. 视频处理工具 (`video.rs`)

视频尺寸修正和滤镜链生成，解决 YUV420 色度子采样兼容性问题。

```rust
use shared_utils::video::{
    ensure_even_dimensions,
    get_dimension_correction_filter,
    build_video_filter_chain,
    get_ffmpeg_dimension_args,
    is_yuv420_compatible,
};

// 检查并修正奇数尺寸
let (width, height, needs_correction) = ensure_even_dimensions(1921, 1081);
// width=1920, height=1080, needs_correction=true

// 生成 FFmpeg 滤镜字符串
let filter = get_dimension_correction_filter(1921, 1081);
// Some("crop=1920:1080:0:0")

// 生成完整滤镜链（含 alpha 通道处理）
let chain = build_video_filter_chain(1921, 1081, true);
// "format=rgba,colorchannelmixer=aa=1.0,format=rgb24,crop=1920:1080:0:0,format=yuv420p"

// 获取 FFmpeg 参数
let args = get_ffmpeg_dimension_args(1921, 1081, false);
// ["-vf", "crop=1920:1080:0:0,format=yuv420p"]

// 检查 YUV420 兼容性
let compatible = is_yuv420_compatible(1920, 1080);
// true
```

**解决的问题**：
- YUV420 色度子采样要求宽度和高度都是偶数
- 常见错误: `Picture height must be an integer multiple of the specified chroma subsampling`
- 自动裁剪到偶数尺寸（比填充黑边更好）
- 处理带 alpha 通道的输入（先移除 alpha）

### 使用方法

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
shared_utils = { path = "../shared_utils" }
```

---

## English

Shared utility library for modern_format_boost tools (imgquality, vidquality, vidquality-hevc).

### Module Overview

| Module | Function |
|--------|----------|
| `metadata` | Complete metadata preservation (EXIF/IPTC/xattr/timestamps/ACL) |
| `progress` | Progress bar & ETA estimation (with SmartProgressBar) |
| `safety` | Dangerous directory detection |
| `batch` | Batch file processing |
| `report` | Summary reports |
| `ffprobe` | FFprobe video analysis wrapper |
| `tools` | External tool detection |
| `codecs` | Codec information |
| `conversion` | **Conversion utilities** (ConversionResult/ConvertOptions/anti-duplicate) |
| `video` | **Video processing** (even dimension correction/filter chain/YUV420 compatibility) |

### Module Details

#### 1. Metadata Preservation (`metadata/`)

Complete cross-platform metadata preservation with three-layer architecture:

```rust
use shared_utils::preserve_metadata;

// Preserve all metadata
preserve_metadata(src_path, dst_path)?;
```

**Supported Metadata Types**:
- **Internal Layer**: EXIF/IPTC/XMP (via ExifTool)
- **Network Layer**: WhereFroms, User Tags
- **System Layer**: ACL, File Flags, xattr, Timestamps

**Platform Support**:
- macOS: Uses native `copyfile` API
- Linux: Uses `getfacl`/`setfacl`
- Windows: Uses PowerShell ACL commands

#### 2. Progress Bar (`progress.rs`)

Visual progress feedback with ETA estimation.

```rust
use shared_utils::{create_progress_bar, format_bytes, format_duration};

let pb = create_progress_bar(100, "Converting");
pb.inc(1);
pb.finish_with_message("Done!");
```

#### 3. Safety Checks (`safety.rs`)

Prevent accidental damage to system directories.

```rust
use shared_utils::check_dangerous_directory;

check_dangerous_directory(Path::new("/Users/me/Documents/photos"))?;
```

#### 4. Batch Processing (`batch.rs`)

Utilities for batch file processing.

```rust
use shared_utils::{collect_files, IMAGE_EXTENSIONS, BatchResult};

let files = collect_files(Path::new("./photos"), IMAGE_EXTENSIONS, true);
```

#### 5. Summary Reports (`report.rs`)

Detailed reporting after batch operations.

```rust
use shared_utils::print_summary_report;

print_summary_report(&result, duration, input_bytes, output_bytes, "Conversion");
```

#### 6. FFprobe Wrapper (`ffprobe.rs`)

Video/animation analysis using ffprobe.

```rust
use shared_utils::{probe_video, get_duration, get_frame_count};

let result = probe_video(Path::new("video.mp4"))?;
println!("Codec: {}, Duration: {}s", result.video_codec, result.duration);
```

#### 7. External Tools (`tools.rs`)

Check for required external tools.

```rust
use shared_utils::{check_image_tools, require_tools};

let tools = check_image_tools();
require_tools(&["ffmpeg", "cjxl", "exiftool"])?;
```

#### 8. Codec Information (`codecs.rs`)

Video codec detection and information.

```rust
use shared_utils::{DetectedCodec, get_codec_info};

let codec = DetectedCodec::from_ffprobe("h264");
println!("Modern: {}", codec.is_modern());
```

#### 9. Video Processing (`video.rs`)

Video dimension correction and filter chain generation for YUV420 chroma subsampling compatibility.

```rust
use shared_utils::video::{
    ensure_even_dimensions,
    get_dimension_correction_filter,
    build_video_filter_chain,
    get_ffmpeg_dimension_args,
    is_yuv420_compatible,
};

// Check and correct odd dimensions
let (width, height, needs_correction) = ensure_even_dimensions(1921, 1081);
// width=1920, height=1080, needs_correction=true

// Generate FFmpeg filter string
let filter = get_dimension_correction_filter(1921, 1081);
// Some("crop=1920:1080:0:0")

// Generate complete filter chain (with alpha channel handling)
let chain = build_video_filter_chain(1921, 1081, true);
// "format=rgba,colorchannelmixer=aa=1.0,format=rgb24,crop=1920:1080:0:0,format=yuv420p"

// Get FFmpeg arguments
let args = get_ffmpeg_dimension_args(1921, 1081, false);
// ["-vf", "crop=1920:1080:0:0,format=yuv420p"]

// Check YUV420 compatibility
let compatible = is_yuv420_compatible(1920, 1080);
// true
```

**Problems Solved**:
- YUV420 chroma subsampling requires even width and height
- Common error: `Picture height must be an integer multiple of the specified chroma subsampling`
- Auto-crop to even dimensions (better than padding with black borders)
- Handle alpha channel inputs (remove alpha first)

### Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
shared_utils = { path = "../shared_utils" }
```
