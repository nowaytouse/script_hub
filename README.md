# 脚本中心 (Script Hub)

欢迎来到我的脚本中心！这里收集了一些为了提高效率而编写的实用工具脚本。

**核心设计原则**:
- **元数据保留**: 所有脚本都将尽力保留文件的内部元数据（EXIF, XMP）和系统元数据（时间戳）。
- **安全第一**: 具有破坏性操作（如删除或替换原文件）的功能都必须通过明确的标志（如 `--in-place` 或 `--delete-source`）来启用。

---

## 脚本列表

1.  [批量JPEG转JXL (jpeg_to_jxl.sh)](#批量jpeg转jxl-jpeg_to_jxlsh)
2.  [批量PNG转无损JXL (png_to_jxl.sh)](#批量png转无损jxl-png_to_jxlsh)
3.  [批量HEIC转无损PNG (heic_to_png.sh)](#批量heic转无损png-heic_to_pngsh)
4.  [视频转高质量GIF (video_to_gif.sh)](#视频转高质量gif-video_to_gifsh)
5.  [批量合并XMP元数据 (merge_xmp.sh)](#批量合并xmp元数据-merge_xmpsh)
6.  [分包压缩并上传到GitHub (archive_and_upload.sh)](#分包压缩并上传到github-archive_and_uploadsh)

---

### 批量JPEG转JXL (jpeg_to_jxl.sh)

#### 功能
将指定文件夹内的 JPEG 图片 (`.jpg`, `.jpeg`) 批量转换为高质量、高压缩率的 JXL 格式。

- **元数据**: 完整保留系统文件时间戳。
- **原地转换**: 支持 `--in-place` 模式，成功后用 `.jxl` 文件替换原始图片。

#### 依赖
- **`jpeg-xl`**: `brew install jpeg-xl`

#### 使用方法
```bash
# 赋予执行权限
chmod +x jpeg_to_jxl.sh

# 标准模式 (在旁边创建 .jxl 文件)
./jpeg_to_jxl.sh /path/to/images

# 原地转换模式
./jpeg_to_jxl.sh --in-place /path/to/images
```

---

### 批量PNG转无损JXL (png_to_jxl.sh)

#### 功能
将指定文件夹内的 PNG 图片 (`.png`) 批量转换为 **数学无损** 的 JXL 格式，实现极致的无损压缩。

- **元数据**: 完整保留系统文件时间戳。
- **原地转换**: 支持 `--in-place` 模式，成功后用 `.jxl` 文件替换原始图片。

#### 依赖
- **`jpeg-xl`**: `brew install jpeg-xl`

#### 使用方法
```bash
# 赋予执行权限
chmod +x png_to_jxl.sh

# 标准模式 (在旁边创建 .jxl 文件)
./png_to_jxl.sh /path/to/images

# 原地转换模式
./png_to_jxl.sh --in-place /path/to/images
```

---

### 批量HEIC转无损PNG (heic_to_png.sh)

#### 功能
将苹果设备常用的 HEIC/HEIF 格式图片 (`.heic`, `.heif`) 批量转换为兼容性更强的 **无损PNG** 格式。

- **元数据**: 使用 `exiftool` 确保内部元数据（EXIF, GPS, XMP等）被最完整地迁移，并保留系统文件时间戳。
- **原地转换**: 支持 `--in-place` 模式，成功后用 `.png` 文件替换原始图片。

#### 依赖
- **`libheif`**: `brew install libheif`
- **`exiftool`**: `brew install exiftool`

#### 使用方法
```bash
# 赋予执行权限
chmod +x heic_to_png.sh

# 标准模式 (在旁边创建 .png 文件)
./heic_to_png.sh /path/to/images

# 原地转换模式
./heic_to_png.sh --in-place /path/to/images
```

---

### 视频转高质量GIF (video_to_gif.sh)

#### 功能
将常见的视频文件（`.mp4`, `.mov` 等）转换为色彩鲜艳、动态流畅的高质量 GIF。脚本采用两步法（分析视频 -> 生成最优调色板 -> 转换）以达到最佳效果。

- **元数据**: 尝试迁移视频的内部元数据，并完整保留系统文件时间戳。
- **源文件清理**: 支持 `--delete-source` 模式，在成功生成 GIF 后删除原始视频。

#### 依赖
- **`ffmpeg`**: `brew install ffmpeg`

#### 使用方法
```bash
# 赋予执行权限
chmod +x video_to_gif.sh

# 标准模式 (保留原始视频)
./video_to_gif.sh /path/to/videos

# 清理模式 (删除原始视频)
./video_to_gif.sh --delete-source /path/to/videos
```

---

### 批量合并XMP元数据 (merge_xmp.sh)

#### 功能
将在专业的图片和视频工作流中产生的 `.xmp` "边车"元数据文件，完整地合并回其对应的主媒体文件中。

- **安全措施**: `exiftool` 会自动创建原始文件的备份（文件名以 `_original` 结尾）。
- **源文件清理**: 支持 `--delete-xmp` 模式，在成功合并后删除 `.xmp` 文件。

#### 依赖
- **`ExifTool`**: `brew install exiftool`

#### 使用方法
```bash
# 赋予执行权限
chmod +x merge_xmp.sh

# 标准模式 (保留 .xmp 文件)
./merge_xmp.sh /path/to/media

# 清理模式 (删除 .xmp 文件)
./merge_xmp.sh --delete-xmp /path/to/media
```

---

### 分包压缩并上传到GitHub (archive_and_upload.sh)

#### 功能
将一个文件夹内的所有文件，按大约 500MB 的大小为单位，自动分割打包成 `.tar.gz` 压缩文件，并上传到指定 GitHub 仓库的 Releases 中。适合归档不便直接存入 Git 的大型项目或数据。

#### 依赖
- **`gh` (GitHub CLI)**: `brew install gh`

#### 使用方法
```bash
# 登录 GitHub
gh auth login

# 赋予执行权限
chmod +x archive_and_upload.sh

# 运行脚本
./archive_and_upload.sh ./source_folder your_github_username/repo_name
```