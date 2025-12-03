# Script Hub

Welcome to my Script Hub! This repository collects various utility scripts designed to enhance efficiency.

**Core Design Principles**:
- **Complete Metadata Preservation**: All scripts strive to preserve both internal (EXIF, XMP) and system metadata (timestamps) during any conversion or processing.
- **Safety First**: Destructive operations (like deleting or overwriting original files) must only be enabled via explicit flags (e.g., `--in-place` or `--delete-source`).
- **Robust Safety & Loud Errors**: Scripts include a "dangerous directory" check. If a destructive operation is attempted on a protected system directory, the script will loudly abort with a clear error message.
- **Batch Processing Capability**: Scripts are designed for efficient batch processing of files within a specified directory.
- **Verified Safe Deletes**: Original files are only deleted or replaced after confirming successful conversion/processing and proper metadata transfer.

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
Batch converts incompatible media formats to universally compatible formats with **complete metadata preservation**:
- **HEIC/HEIF → PNG**: Lossless conversion using macOS native `sips` or `heif-convert`
- **MP4 → High-Quality GIF**: Two-pass conversion with optimized color palette (15 FPS, 540px width for social media compatibility)

#### Key Features
- **Atomic Operations**: Temp file → Verify → Replace (prevents data corruption)
- **Complete Metadata Preservation**:
  - Internal metadata (EXIF, XMP, IPTC, ICC Profile)
  - System metadata (creation time, modification time, access time)
- **Automatic Backup**: Original files backed up before conversion
- **Multi-level Verification**: File existence, size, and integrity checks
- **Safety Checks**: Prevents operations on protected system directories

#### Dependencies
- **`libheif`** (optional): `brew install libheif`
- **`exiftool`**: `brew install exiftool`
- **`ffmpeg`**: `brew install ffmpeg`

#### Usage
```bash
# Grant execute permission
chmod +x convert_incompatible_media.sh

# Standard mode (converts and replaces, with automatic backup)
./convert_incompatible_media.sh /path/to/media

# Dry-run mode (preview without executing)
./convert_incompatible_media.sh --dry-run /path/to/media

# Verbose mode with custom backup directory
./convert_incompatible_media.sh --verbose --backup-dir /path/to/backup /path/to/media
```

---

### Substore Scripts

A collection of advanced JavaScript rule files designed for the [Sub-Store](https://github.com/sub-store-org/Sub-Store) subscription management tool. These scripts automatically optimize proxy nodes from subscription links for enhanced performance, security, and privacy. Rules are tailored for different proxy clients (e.g., Clash, Sing-box, Surge, Shadowrocket) and node configurations (e.g., relay, entrance).
