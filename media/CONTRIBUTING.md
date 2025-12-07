# 本项目开发准则

欢迎为本项目贡献代码！为了确保所有脚本的质量、可靠性和安全性，任何处理用户文件的脚本都必须严格遵守以下五条核心准则。

---

### 准则一：最完整的元数据保留 (Complete Metadata Preservation)

文件不仅仅包含可见的数据，更携带着描述其自身的宝贵信息（元数据）。我们的目标是，在任何转换或处理过程中，都不能丢失这些信息。

- **实现方式**:
    - **内部元数据**: 对于图片、视频等文件，必须使用如 `exiftool` 这样的专业工具，在处理完成后，通过 `-tagsfromfile` 命令从源文件向目标文件进行一次完整的元数据迁移。这包括但不限于：
        - **图片**: EXIF, IPTC, XMP, GPS, ICC Profile, ColorSpace
        - **视频/动画**: Duration, Frame Count, FPS, Encoder, Comment
    - **系统元数据**: 必须使用 `touch -r <源文件> <目标文件>` 命令，将源文件的文件系统时间戳（特别是修改日期）精确地复制到目标文件。
    - **动画帧保留**: 对于 MP4→WebP/GIF 等动画转换，必须确保 100% 帧数保留，使用 `-r $fps` 和 `-vsync cfr` 参数保证帧率一致性。

### 准则二：简易安全功能与响亮报错 (Robust Safety & Loud Errors)

用户的系统安全是第一位的。绝不能因为脚本的误用而导致灾难性后果。

- **实现方式**:
    - **危险目录检查**: 脚本必须内置一个“危险目录”列表（如 `/`, `/System`, `/usr`, 用户主目录 `~` 等）。
    - **触发条件**: 当且仅当一个具有破坏性的操作（见准则四）被激活时，此安全检查才会被触发。
    - **响亮报错**: 如果目标目录位于危险目录之内，脚本**必须**立即退出，并打印出非常醒目、清晰、易于理解的错误信息，明确告知用户为何操作被禁止。

### 准则三：批量处理能力 (Batch Processing Capability)

所有脚本都应该被设计为能高效处理大量文件，而非一次只能处理一个。

- **实现方式**:
    - 脚本的主要输入参数应该是一个目录路径。
    - 必须使用 `find` 命令并配合 `-print0` 和 `while read -r -d $'\0'` 循环来安全、递归地遍历目录下的所有目标文件。这种方式可以完美处理文件名中包含空格或特殊字符的情况。

### 准则四：明确的原地替换功能 (Explicit In-Place Replacement)

直接修改或删除用户的原始文件是危险操作，绝不能作为默认行为。用户必须明确表示他们希望这样做。

- **实现方式**:
    - **可选标志**: 任何原地替换、删除源文件或其它破坏性操作，都**必须**隐藏在一个可选的命令行标志之后（例如 `--in-place` 或 `--delete-source`）。
    - **默认行为**: 在不提供任何标志的情况下，脚本的默认行为应该是安全的、非破坏性的（例如，在旁边创建新文件）。

### 准则五：多次验证后的“安全删除” (Verified Safe Deletes)

只有在确认所有步骤都成功完成后，才能执行删除或替换原始文件的操作。

- **实现方式**:
    - **检查退出码**: 在执行核心操作（如 `ffmpeg`, `cjxl` 转换）后，必须立刻检查其退出码 (`$?`)。如果退出码非零，则必须中止后续操作，并报告错误。
    - **操作顺序**: 对于原地替换，正确的、经过验证的顺序是：
        1.  转换到**临时文件**。
        2.  验证转换是否成功（检查退出码）。
        3.  将元数据和时间戳从原始文件复制到临时文件。
        4.  **全部成功后**，才可删除原始文件，并将临时文件重命名。

---
所有新的贡献都将被依据以上准则进行评估。



# Script Hub

Welcome to my Script Hub! This repository collects various utility scripts designed to enhance efficiency.

**Core Design Principles**:
- **Complete Metadata Preservation**: All scripts strive to preserve both internal (EXIF, XMP, ICC Profile) and system metadata (timestamps) during any conversion or processing. And the most complete media information (e.g., FPS and frame count)
  - **Internal Metadata**: EXIF, XMP, IPTC, ICC Profile using `exiftool`
  - **File System Metadata**: Modification time, access time, creation time using `touch -r`
  - **Media Information**: Frame count, FPS, duration for animations/videos (100% preserved)
- **Health Check Validation**: All conversion scripts validate output files to ensure they are viewable/playable before deleting originals.
- **Whitelist-Only Processing**: Scripts only process specific file formats (whitelist), ignoring all other files for safety.
- **English Output with Emoji**: All scripts MUST output in English with emoji indicators for better readability and international compatibility. No Chinese or other language output is allowed in script execution.

**User Experience Enhancements**:
- **Visual Progress Bar**: All scripts now feature a visual progress bar `[████░░] 67%` with ETA estimation.
- **Real-time Feedback**: Video conversion scripts display real-time ffmpeg stats (frame, fps, speed) to prevent "frozen" state perception.
- **Detailed Reporting**: Comprehensive summary reports at the end of each execution.
- **Safety First**: Destructive operations (like deleting or overwriting original files) must only be enabled via explicit flags (e.g., `--in-place` or `--delete-source`).
- **Robust Safety & Loud Errors**: Scripts include a "dangerous directory" check. If a destructive operation is attempted on a protected system directory, the script will loudly abort with a clear error message.
- **Batch Processing Capability**: Scripts are designed for efficient batch processing of files within a specified directory.
- **Verified Safe Deletes**: Original files are only deleted or replaced after confirming successful conversion/processing, health validation, and proper metadata transfer.

---

## Script List

1.  [Batch JPEG to JXL (jpeg_to_jxl.sh)](#batch-jpeg-to-jxl-jpeg_to_jxlsh)
2.  [Batch PNG to Lossless JXL (png_to_jxl.sh)](#batch-png-to-lossless-jxl-png_to_jxlsh)
3.  [Batch HEIC to Lossless PNG (heic_to_png.sh)](#batch-heic-to-lossless-png-heic_to_pngsh)
4.  [Animated Image to H.266/VVC Video (imganim_to_vvc.sh)](#animated-image-to-h266vvc-video-imganim_to_vvcsh)
5.  [Video to High-Quality GIF (video_to_hq_gif.sh)](#video-to-high-quality-gif-video_to_hq_gifsh)
6.  [Batch Merge XMP Metadata (merge_xmp.sh)](#batch-merge-xmp-metadata-merge_xmpsh)
7.  [Archive Script (archive_and_upload.sh)](#archive-script-archive_and_uploadsh)
8.  [Incompatible Media Converter (convert_incompatible_media.sh)](#incompatible-media-converter-convert_incompatible_mediash)

---

### Batch JPEG to JXL (jpeg_to_jxl.sh)

#### Functionality
Batch converts JPEG images (`.jpg`, `.jpeg`) within a specified folder to high-quality, high-compression JXL format.

- **Metadata**: Preserves full system file timestamps.
- **In-place Conversion**: Supports `--in-place` mode, replacing original images with `.jxl` files upon successful conversion.

#### Dependencies
- **`jpeg-xl`**: Install via Homebrew on macOS: `brew install jpeg-xl`

#### Usage
```bash
# Grant execute permission
chmod +x jpeg_to_jxl.sh

# Standard mode (creates new .jxl files alongside originals)
./jpeg_to_jxl.sh /path/to/images

# In-place conversion mode
./jpeg_to_jxl.sh --in-place /path/to/images
```

---

### Batch PNG to Lossless JXL (png_to_jxl.sh)

#### Functionality
Batch converts PNG images (`.png`) within a specified folder to **mathematically lossless** JXL format, achieving extreme lossless compression.

- **Metadata**: Preserves full system file timestamps.
- **In-place Conversion**: Supports `--in-place` mode, replacing original images with `.jxl` files upon successful conversion.

#### Dependencies
- **`jpeg-xl`**: Install via Homebrew on macOS: `brew install jpeg-xl`

#### Usage
```bash
# Grant execute permission
chmod +x png_to_jxl.sh

# Standard mode (creates new .jxl files alongside originals)
./png_to_jxl.sh /path/to/images

# In-place conversion mode
./png_to_jxl.sh --in-place /path/to/images
```

---

### Batch HEIC to Lossless PNG (heic_to_png.sh)

#### Functionality
Batch converts HEIC/HEIF images (`.heic`, `.heif`) commonly used on Apple devices to a more compatible **lossless PNG** format.

- **Metadata**: Uses `exiftool` to ensure the most complete transfer of internal metadata (EXIF, GPS, XMP, etc.) and preserves system file timestamps.
- **In-place Conversion**: Supports `--in-place` mode, replacing original images with `.png` files upon successful conversion.

#### Dependencies
- **`libheif`**: Install via Homebrew on macOS: `brew install libheif`
- **`exiftool`**: Install via Homebrew on macOS: `brew install exiftool`

#### Usage
```bash
# Grant execute permission
chmod +x heic_to_png.sh

# Standard mode (creates new .png files alongside originals)
./heic_to_png.sh /path/to/images

# In-place conversion mode
./heic_to_png.sh --in-place /path/to/images
```

---

### Animated Image to H.266/VVC Video (imganim_to_vvc.sh)

#### Functionality
Intelligently identifies (by MIME type, not just extension) and batch converts animated images (GIF, Animated WebP, APNG) within a directory to the modern, efficient H.266 (VVC) video format (`.mp4`).

- **Metadata**: Strives to preserve internal metadata and fully retains system file timestamps.
- **In-place Conversion**: Supports `--in-place` mode, replacing original animated images with `.mp4` videos upon successful conversion.

#### Dependencies
- **`ffmpeg`**: Requires compilation with `libvvenc` support. Homebrew's `ffmpeg` might not include this by default; users may need to compile manually or use a different source.
- **`exiftool`**: Install via Homebrew on macOS: `brew install exiftool`

#### Usage
```bash
# Grant execute permission
chmod +x imganim_to_vvc.sh

# Standard mode (creates new .mp4 files alongside originals)
./imganim_to_vvc.sh /path/to/images

# In-place conversion mode
./imganim_to_vvc.sh --in-place /path/to/images
```

---

### Video to High-Quality GIF (video_to_hq_gif.sh)

#### Functionality
Batch converts common video files (`.mp4`, `.mov`, etc.) into visually stunning, high-quality GIFs. The script employs a two-pass `ffmpeg` method (video analysis -> optimal color palette generation -> conversion) for best results, utilizing advanced dithering algorithms for smooth color transitions.

- **Metadata**: Attempts to migrate internal video metadata and fully retains system file timestamps.
- **Source File Cleanup**: Supports `--delete-source` mode, deleting the original video file after successful GIF generation.
- **Customization**: Allows setting custom framerates and output widths.

#### Dependencies
- **`ffmpeg`**: Install via Homebrew on macOS: `brew install ffmpeg`

#### Usage
```bash
# Grant execute permission
chmod +x video_to_hq_gif.sh

# Standard mode (retains original video)
./video_to_hq_gif.sh /path/to/videos

# Cleanup mode (deletes original video after successful conversion)
./video_to_hq_gif.sh --delete-source /path/to/videos

# Custom framerate and width (e.g., 24 FPS, 720px width)
./video_to_hq_gif.sh -r 24 -s 720 /path/to/videos
```

---

### Batch Merge XMP Metadata (merge_xmp.sh)

#### Functionality
Fully merges `.xmp` sidecar metadata files, typically generated in professional photo/video workflows, back into their corresponding main media files.

- **Safety Measures**: `exiftool` automatically creates backups of original files (suffixed with `_original`) before modification.
- **Source File Cleanup**: Supports `--delete-xmp` mode, deleting the `.xmp` file after successful metadata merge.

#### Dependencies
- **`ExifTool`**: Install via Homebrew on macOS: `brew install exiftool`

#### Usage
```bash
# Grant execute permission
chmod +x merge_xmp.sh

# Standard mode (retains .xmp files)
./merge_xmp.sh /path/to/media

# Cleanup mode (deletes .xmp files after successful merge)
./merge_xmp.sh --delete-xmp /path/to/media
```

---

### Archive Script (archive_and_upload.sh)

#### Functionality
Automatically splits and archives all files within a directory into `.zip` compressed chunks (approx. 500MB each). Ideal for archiving large projects or datasets.

#### Usage
```bash
# Grant execute permission
chmod +x archive_and_upload.sh

# Run the script
./archive_and_upload.sh ./source_folder
```

**Note**: Archives are created in the current directory as `archive_part_1.zip`, `archive_part_2.zip`, etc.

---

### Incompatible Media Converter (convert_incompatible_media.sh)

#### Functionality
Batch converts incompatible media formats to universally compatible formats with **complete metadata preservation**, **health validation**, and **optimized performance**:
- 📷 **HEIC/HEIF → PNG**: Lossless conversion using macOS native `sips` or `heif-convert`
- 🎬 **MP4 → WebP** (default): **Optimized lossless conversion** (3-5x faster), preserves ALL frames, smaller than GIF
- 🎬 **MP4 → GIF** (optional): Lossless conversion, larger file size

#### Key Features

**⚡ Performance Optimizations**
- **Optimized WebP Conversion**: Single-step direct conversion (MP4 → WebP) without intermediate files
- **3-5x Faster**: Eliminated PNG frame extraction step, dramatically reduced disk I/O
- **Minimal Temporary Files**: Reduces disk space usage from hundreds of MB to just a few MB
- **Efficient Processing**: Processes files sequentially with minimal memory overhead

**🏥 Health Check Validation**
- Validates file signatures (PNG magic bytes, GIF87a/GIF89a, RIFF/WEBP)
- Verifies media structure using `ffprobe` (dimensions, codec, frame count)
- Performs decode test using `ffmpeg` to ensure playability
- Reports health statistics with pass/fail/warning counts

**📋 Maximum Metadata Preservation**
- **Image Metadata**: EXIF, XMP, IPTC, ICC Profile, ColorSpace
- **Animation Metadata**: Frame count, FPS, duration (100% preserved)
- **System Metadata**: Creation time, modification time, access time
- **Verification**: Reports metadata preservation rate (≥70% = GOOD)

**🔒 Safety & Reliability**
- **Whitelist Mode**: Only processes specified formats (HEIC/HEIF/MP4)
- **Atomic Operations**: Temp file → Verify → Health Check → Replace
- **Automatic Backup**: Original files backed up before any modification
- **Protected Directories**: Blocks operations on system directories
- **Converted File Protection**: Tracks newly converted files and protects them from accidental deletion in `--keep-only-incompatible` mode

#### Dependencies
- **`sips`** (macOS native) or **`libheif`**: `brew install libheif`
- **`exiftool`**: `brew install exiftool`
- **`ffmpeg`** & **`ffprobe`**: `brew install ffmpeg`

#### Usage
```bash
# Grant execute permission
chmod +x convert_incompatible_media.sh

# Standard mode (with health check and metadata verification)
./convert_incompatible_media.sh /path/to/media

# Verbose mode (shows detailed metadata info)
./convert_incompatible_media.sh --verbose /path/to/media

# Dry-run mode (preview without executing)
./convert_incompatible_media.sh --dry-run /path/to/media

# Skip health check (not recommended)
./convert_incompatible_media.sh --skip-health-check /path/to/media

# WebP format (high-quality lossy, smaller file)
./convert_incompatible_media.sh --format webp /path/to/media

# Keep-only-incompatible mode (⚠️ DESTRUCTIVE: deletes all compatible files)
# IMPORTANT: Creates a copy first for safety!
cp -R /path/to/media /path/to/media_copy
./convert_incompatible_media.sh --keep-only-incompatible /path/to/media_copy
```

**Best Practices**:
1. **Use Copy Mode**: Always operate on a copy of your data, not the original
2. **Verify First**: Run with `--dry-run` to preview changes before execution
3. **Keep Backups**: The script creates automatic backups, but external backups are recommended
4. **Check Results**: Verify converted files before deleting the original directory

**Keep-Only-Incompatible Mode**:
This special mode converts incompatible media (HEIC/HEIF/MP4) AND deletes all other compatible files (JPG, PNG, GIF, WebP, etc.). Only the converted files remain. Use with extreme caution!

**Example Output (Verbose Mode)**:
```
📷 Converting HEIC → PNG: photo.heic
📋 Original file info:
    Image Width: 2851
    Image Height: 4093
🔄 Step 1/4: Converting image format...
📋 Step 2/4: Migrating metadata (EXIF, XMP, ICC)...
⏰ Step 3/4: Preserving timestamps...
🏥 Step 4/4: Health validation...
🏥 ✅ Passed: photo.png (4645308 bytes)
📋 Verifying metadata preservation...
    📊 Original tags: 42
    📊 Converted tags: 31
    ✅ Metadata preservation: GOOD (≥70%)
✅ Done: photo.heic → photo.png

🎬 Converting MP4 → GIF: video.mp4
📋 Original file info:
    📹 codec_name=h264
    📹 width=1280, height=720
    🎞️  FPS: 30/1
    🖼️  Frames: 302
    ⏱️  Duration: 10.224000s
🏥 ✅ Passed: video.gif (61038323 bytes)
📋 Verifying metadata preservation...
    🖼️  Original frames: 302
    🖼️  Converted frames: 302
    ✅ Frame count: PRESERVED
```

**Health Report**:
```
╔══════════════════════════════════════════════╗
║        🏥 Media Health Report                ║
╠══════════════════════════════════════════════╣
║  ✅ Passed:                             4  ║
║  ❌ Failed:                             0  ║
║  ⚠️  Warnings:                          0  ║
║  📊 Health Rate:                    100%  ║
╚══════════════════════════════════════════════╝
```

---

### Media Health Check Module (media_health_check.sh)

#### Functionality
Standalone utility to validate media file integrity and playability. Can be used independently or sourced by other scripts.

#### Features
- 🔍 **Format Detection**: Validates file signatures for PNG, GIF, WebP, JXL, JPEG, MP4
- 📊 **Structure Validation**: Uses `ffprobe` to verify dimensions, codec, frame count
- 🎬 **Decode Test**: Attempts to decode first frame to ensure playability
- 📋 **Batch Processing**: Can scan entire directories recursively

#### Usage
```bash
# Check single file
./media_health_check.sh image.png

# Check directory
./media_health_check.sh /path/to/media/

# Check multiple files
./media_health_check.sh *.gif *.png
```

---

### Substore Scripts

A collection of advanced JavaScript rule files designed for the [Sub-Store](https://github.com/sub-store-org/Sub-Store) subscription management tool. These scripts automatically optimize proxy nodes from subscription links for enhanced performance, security, and privacy. Rules are tailored for different proxy clients (e.g., Clash, Sing-box, Surge, Shadowrocket) and node configurations (e.g., relay, entrance).
