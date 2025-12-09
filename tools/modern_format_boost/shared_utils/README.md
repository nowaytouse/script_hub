# Shared Utilities

Shared utility library for modern_format_boost tools (imgquality, vidquality, vidquality-hevc).

## Modules

### 1. Progress Bar (`progress.rs`)
Visual progress feedback with ETA estimation.

```rust
use shared_utils::{create_progress_bar, BatchProgress, format_bytes, format_duration};

let pb = create_progress_bar(100, "Converting");
pb.inc(1);
pb.finish_with_message("Done!");
```

### 2. Safety Checks (`safety.rs`)
Prevent accidental damage to system directories.

```rust
use shared_utils::check_dangerous_directory;
check_dangerous_directory(Path::new("/Users/me/Documents/photos"))?;
```

### 3. Batch Processing (`batch.rs`)
Utilities for batch file processing.

```rust
use shared_utils::{collect_files, IMAGE_EXTENSIONS, BatchResult};
let files = collect_files(Path::new("./photos"), IMAGE_EXTENSIONS, true);
```

### 4. Summary Reports (`report.rs`)
Detailed reporting after batch operations.

```rust
use shared_utils::{print_summary_report, print_health_report};
print_summary_report(&result, duration, input_bytes, output_bytes, "Conversion");
```

### 5. FFprobe Wrapper (`ffprobe.rs`) 🆕
Video/animation analysis using ffprobe.

```rust
use shared_utils::{probe_video, get_duration, get_frame_count, FFprobeResult};

// Full video analysis
let result = probe_video(Path::new("video.mp4"))?;
println!("Codec: {}, Duration: {}s", result.video_codec, result.duration);

// Quick duration check
let duration = get_duration(Path::new("animation.gif"));
```

### 6. External Tools (`tools.rs`) 🆕
Check for required external tools.

```rust
use shared_utils::{check_image_tools, check_video_tools, require_tools, print_tool_report};

// Check all image processing tools
let tools = check_image_tools();
print_tool_report(&tools);

// Require specific tools (exits with error if missing)
require_tools(&["ffmpeg", "cjxl", "exiftool"])?;
```

### 7. Codec Information (`codecs.rs`) 🆕
Video codec detection and information.

```rust
use shared_utils::{DetectedCodec, get_codec_info, CodecCategory};

let codec = DetectedCodec::from_ffprobe("h264");
println!("Codec: {}, Modern: {}", codec.as_str(), codec.is_modern());

let info = get_codec_info("av1");
println!("Category: {:?}", info.category);
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
shared_utils = { path = "../shared_utils" }
```

## Design Principles

Following media/CONTRIBUTING.md:
- **Visual Progress Bar**: `[████░░] 67%` with ETA estimation
- **Robust Safety & Loud Errors**: Dangerous directory detection
- **Batch Processing Capability**: Efficient file collection
- **Detailed Reporting**: Comprehensive summary reports
- **Code Reuse**: Eliminate duplicate code across tools
