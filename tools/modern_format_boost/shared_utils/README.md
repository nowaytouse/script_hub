# Shared Utilities

Shared utility library for modern_format_boost tools (imgquality, vidquality, vidquality-hevc).

## Features

### 1. Progress Bar (`progress.rs`)
Visual progress feedback with ETA estimation, following media/CONTRIBUTING.md requirements.

```rust
use shared_utils::{create_progress_bar, BatchProgress, format_bytes, format_duration};

// Simple progress bar
let pb = create_progress_bar(100, "Converting");
for i in 0..100 {
    pb.inc(1);
}
pb.finish_with_message("Done!");

// Batch progress with statistics
let mut progress = BatchProgress::new(100, "Processing");
progress.success("file1.png");
progress.fail("file2.png");
progress.skip("file3.png");
progress.finish();
```

### 2. Safety Checks (`safety.rs`)
Prevent accidental damage to system directories.

```rust
use shared_utils::check_dangerous_directory;

// Will error if path is /System, /usr, etc.
check_dangerous_directory(Path::new("/Users/me/Documents/photos"))?;

// For destructive operations (delete, in-place replace)
check_safe_for_destructive(path, "delete")?;
```

### 3. Batch Processing (`batch.rs`)
Utilities for batch file processing.

```rust
use shared_utils::{collect_files, IMAGE_EXTENSIONS, BatchResult};

// Collect all image files recursively
let files = collect_files(Path::new("./photos"), IMAGE_EXTENSIONS, true);

// Track batch results
let mut result = BatchResult::new();
result.success();
result.fail(path, "Error message".to_string());
result.skip();
println!("Success rate: {:.1}%", result.success_rate());
```

### 4. Summary Reports (`report.rs`)
Detailed reporting after batch operations.

```rust
use shared_utils::{print_summary_report, print_health_report};

// Print detailed summary
print_summary_report(&result, duration, input_bytes, output_bytes, "Image Conversion");

// Print health check report
print_health_report(passed, failed, warnings);
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
- **Robust Safety & Loud Errors**: Dangerous directory detection with clear error messages
- **Batch Processing Capability**: Efficient file collection and parallel processing support
- **Detailed Reporting**: Comprehensive summary reports with statistics
