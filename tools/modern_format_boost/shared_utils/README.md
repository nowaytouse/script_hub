# Shared Utilities - 共享工具库

[English](#english) | [中文](#中文)

---

## 中文

modern_format_boost 工具集（imgquality、vidquality、vidquality-hevc）的共享工具库。

### 模块概览

| 模块 | 功能 |
|------|------|
| `progress.rs` | 进度条与 ETA 估算 |
| `safety.rs` | 危险目录检测 |
| `batch.rs` | 批量文件处理 |
| `report.rs` | 汇总报告 |
| `ffprobe.rs` | FFprobe 视频分析封装 |
| `tools.rs` | 外部工具检测 |
| `codecs.rs` | 编解码器信息 |

### 模块详情

#### 1. 进度条 (`progress.rs`)

带 ETA 估算的可视化进度反馈。

```rust
use shared_utils::{create_progress_bar, BatchProgress, format_bytes, format_duration};

// 创建进度条
let pb = create_progress_bar(100, "Converting");
pb.inc(1);
pb.finish_with_message("Done!");

// 格式化工具
let size = format_bytes(1024 * 1024);  // "1.00 MB"
let time = format_duration(Duration::from_secs(125));  // "2m 5s"
```

#### 2. 安全检查 (`safety.rs`)

防止意外损坏系统目录。

```rust
use shared_utils::check_dangerous_directory;

// 检查目录是否安全
check_dangerous_directory(Path::new("/Users/me/Documents/photos"))?;

// 危险目录会返回错误:
// - /System, /Library, /Applications
// - /usr, /bin, /sbin, /etc
// - 用户根目录 (~)
```

#### 3. 批量处理 (`batch.rs`)

批量文件处理工具。

```rust
use shared_utils::{collect_files, IMAGE_EXTENSIONS, VIDEO_EXTENSIONS, BatchResult};

// 收集图像文件
let files = collect_files(Path::new("./photos"), IMAGE_EXTENSIONS, true);

// 收集视频文件
let files = collect_files(Path::new("./videos"), VIDEO_EXTENSIONS, true);

// 批量结果统计
let mut result = BatchResult::new();
result.succeeded += 1;
result.failed += 1;
result.skipped += 1;
```

#### 4. 汇总报告 (`report.rs`)

详细的批量操作报告。

```rust
use shared_utils::{print_summary_report, print_health_report};

// 打印汇总报告
print_summary_report(&result, duration, input_bytes, output_bytes, "Conversion");

// 输出示例:
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 📊 Conversion Summary Report
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ✅ Succeeded: 10
// ❌ Failed: 2
// ⏭️  Skipped: 3
// ⏱️  Duration: 1m 30s
// 💾 Input: 100.00 MB → Output: 45.00 MB (55.0% reduction)
```

#### 5. FFprobe 封装 (`ffprobe.rs`)

视频/动画分析。

```rust
use shared_utils::{probe_video, get_duration, get_frame_count, FFprobeResult};

// 完整视频分析
let result = probe_video(Path::new("video.mp4"))?;
println!("Codec: {}, Duration: {}s", result.video_codec, result.duration);
println!("Resolution: {}x{}", result.width, result.height);
println!("FPS: {}", result.fps);

// 快速时长检查
let duration = get_duration(Path::new("animation.gif"));

// 获取帧数
let frames = get_frame_count(Path::new("video.mp4"));
```

#### 6. 外部工具检测 (`tools.rs`)

检查所需外部工具。

```rust
use shared_utils::{check_image_tools, check_video_tools, require_tools, print_tool_report};

// 检查所有图像处理工具
let tools = check_image_tools();
print_tool_report(&tools);

// 检查所有视频处理工具
let tools = check_video_tools();

// 要求特定工具（缺失则报错退出）
require_tools(&["ffmpeg", "cjxl", "exiftool"])?;
```

#### 7. 编解码器信息 (`codecs.rs`)

视频编解码器检测和信息。

```rust
use shared_utils::{DetectedCodec, get_codec_info, CodecCategory};

// 从 ffprobe 输出检测编解码器
let codec = DetectedCodec::from_ffprobe("h264");
println!("Codec: {}, Modern: {}", codec.as_str(), codec.is_modern());

// 获取编解码器详细信息
let info = get_codec_info("av1");
println!("Category: {:?}", info.category);

// 编解码器分类
// - Modern: AV1, H.265, VP9, VVC
// - Legacy: H.264, MPEG-4, MPEG-2
// - Lossless: FFV1, HuffYUV
// - Professional: ProRes, DNxHD
```

### 使用方法

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
shared_utils = { path = "../shared_utils" }
```

### 设计原则

遵循 media/CONTRIBUTING.md 规范：
- **可视化进度条**: `[████░░] 67%` 带 ETA 估算
- **健壮安全与响亮错误**: 危险目录检测
- **批量处理能力**: 高效文件收集
- **详细报告**: 全面的汇总报告
- **代码复用**: 消除工具间重复代码

---

## English

Shared utility library for modern_format_boost tools (imgquality, vidquality, vidquality-hevc).

### Module Overview

| Module | Function |
|--------|----------|
| `progress.rs` | Progress bar & ETA estimation |
| `safety.rs` | Dangerous directory detection |
| `batch.rs` | Batch file processing |
| `report.rs` | Summary reports |
| `ffprobe.rs` | FFprobe video analysis wrapper |
| `tools.rs` | External tool detection |
| `codecs.rs` | Codec information |

### Module Details

#### 1. Progress Bar (`progress.rs`)

Visual progress feedback with ETA estimation.

```rust
use shared_utils::{create_progress_bar, BatchProgress, format_bytes, format_duration};

// Create progress bar
let pb = create_progress_bar(100, "Converting");
pb.inc(1);
pb.finish_with_message("Done!");

// Formatting utilities
let size = format_bytes(1024 * 1024);  // "1.00 MB"
let time = format_duration(Duration::from_secs(125));  // "2m 5s"
```

#### 2. Safety Checks (`safety.rs`)

Prevent accidental damage to system directories.

```rust
use shared_utils::check_dangerous_directory;

// Check if directory is safe
check_dangerous_directory(Path::new("/Users/me/Documents/photos"))?;

// Dangerous directories return error:
// - /System, /Library, /Applications
// - /usr, /bin, /sbin, /etc
// - User home directory (~)
```

#### 3. Batch Processing (`batch.rs`)

Utilities for batch file processing.

```rust
use shared_utils::{collect_files, IMAGE_EXTENSIONS, VIDEO_EXTENSIONS, BatchResult};

// Collect image files
let files = collect_files(Path::new("./photos"), IMAGE_EXTENSIONS, true);

// Collect video files
let files = collect_files(Path::new("./videos"), VIDEO_EXTENSIONS, true);

// Batch result statistics
let mut result = BatchResult::new();
result.succeeded += 1;
result.failed += 1;
result.skipped += 1;
```

#### 4. Summary Reports (`report.rs`)

Detailed reporting after batch operations.

```rust
use shared_utils::{print_summary_report, print_health_report};

// Print summary report
print_summary_report(&result, duration, input_bytes, output_bytes, "Conversion");

// Output example:
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// 📊 Conversion Summary Report
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ✅ Succeeded: 10
// ❌ Failed: 2
// ⏭️  Skipped: 3
// ⏱️  Duration: 1m 30s
// 💾 Input: 100.00 MB → Output: 45.00 MB (55.0% reduction)
```

#### 5. FFprobe Wrapper (`ffprobe.rs`)

Video/animation analysis using ffprobe.

```rust
use shared_utils::{probe_video, get_duration, get_frame_count, FFprobeResult};

// Full video analysis
let result = probe_video(Path::new("video.mp4"))?;
println!("Codec: {}, Duration: {}s", result.video_codec, result.duration);
println!("Resolution: {}x{}", result.width, result.height);
println!("FPS: {}", result.fps);

// Quick duration check
let duration = get_duration(Path::new("animation.gif"));

// Get frame count
let frames = get_frame_count(Path::new("video.mp4"));
```

#### 6. External Tools (`tools.rs`)

Check for required external tools.

```rust
use shared_utils::{check_image_tools, check_video_tools, require_tools, print_tool_report};

// Check all image processing tools
let tools = check_image_tools();
print_tool_report(&tools);

// Check all video processing tools
let tools = check_video_tools();

// Require specific tools (exits with error if missing)
require_tools(&["ffmpeg", "cjxl", "exiftool"])?;
```

#### 7. Codec Information (`codecs.rs`)

Video codec detection and information.

```rust
use shared_utils::{DetectedCodec, get_codec_info, CodecCategory};

// Detect codec from ffprobe output
let codec = DetectedCodec::from_ffprobe("h264");
println!("Codec: {}, Modern: {}", codec.as_str(), codec.is_modern());

// Get codec detailed info
let info = get_codec_info("av1");
println!("Category: {:?}", info.category);

// Codec categories
// - Modern: AV1, H.265, VP9, VVC
// - Legacy: H.264, MPEG-4, MPEG-2
// - Lossless: FFV1, HuffYUV
// - Professional: ProRes, DNxHD
```

### Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
shared_utils = { path = "../shared_utils" }
```

### Design Principles

Following media/CONTRIBUTING.md:
- **Visual Progress Bar**: `[████░░] 67%` with ETA estimation
- **Robust Safety & Loud Errors**: Dangerous directory detection
- **Batch Processing Capability**: Efficient file collection
- **Detailed Reporting**: Comprehensive summary reports
- **Code Reuse**: Eliminate duplicate code across tools
