# 脚本中心 (Script Hub)

欢迎来到我的脚本中心！本仓库收集了各种旨在提高效率的实用工具脚本。

**核心设计原则**:
- **最完整的元数据保留**: 所有脚本在任何转换或处理过程中，都力求保留文件的内部元数据（EXIF, XMP）和系统元数据（时间戳）。
- **安全第一**: 破坏性操作（如删除或覆盖原始文件）必须仅通过明确的标志（例如 `--in-place` 或 `--delete-source`）启用。
- **强大的安全与响亮报错**: 脚本包含“危险目录”检查。如果尝试在受保护的系统目录上执行破坏性操作，脚本将大声报错并显示清晰的错误消息而中止。
- **批量处理能力**: 脚本被设计为能够高效处理指定目录中的文件。
- **验证后的安全删除**: 只有在确认转换/处理成功且元数据已正确传输后，才会删除或替换原始文件。

---

## 脚本列表

1.  [批量JPEG转JXL (jpeg_to_jxl.sh)](#批量jpeg转jxl-jpeg_to_jxlsh)
2.  [批量PNG转无损JXL (png_to_jxl.sh)](#批量png转无损jxl-png_to_jxlsh)
3.  [批量HEIC转无损PNG (heic_to_png.sh)](#批量heic转无损png-heic_to_pngsh)
4.  [动态图片转H.266/VVC视频 (imganim_to_vvc.sh)](#动态图片转h266vvc视频-imganim_to_vvcsh)
5.  [视频转高质量GIF (video_to_hq_gif.sh)](#视频转高质量gif-video_to_hq_gifsh)
6.  [批量合并XMP元数据 (merge_xmp.sh)](#批量合并xmp元数据-merge_xmpsh)
7.  [归档脚本 (archive_and_upload.sh)](#归档脚本-archive_and_uploadsh)
8.  [不兼容媒体转换器 (convert_incompatible_media.sh)](#不兼容媒体转换器-convert_incompatible_mediash)

---

### 批量JPEG转JXL (jpeg_to_jxl.sh)

#### 功能
批量将指定文件夹内的 JPEG 图片（`.jpg`、`.jpeg`）转换为高质量、高压缩率的 JXL 格式。

- **元数据**: 保留完整的系统文件时间戳。
- **原地转换**: 支持 `--in-place` 模式，成功转换后用 `.jxl` 文件替换原始图片。

#### 依赖
- **`jpeg-xl`**: 在 macOS 上通过 Homebrew 安装：`brew install jpeg-xl`

#### 使用方法
```bash
# 赋予执行权限
chmod +x jpeg_to_jxl.sh

# 标准模式（在旁边创建新的 .jxl 文件）
./jpeg_to_jxl.sh /path/to/images

# 原地转换模式
./jpeg_to_jxl.sh --in-place /path/to/images
```

---

### 批量PNG转无损JXL (png_to_jxl.sh)

#### 功能
批量将指定文件夹内的 PNG 图片（`.png`）转换为**数学无损**的 JXL 格式，实现极致的无损压缩。

- **元数据**: 保留完整的系统文件时间戳。
- **原地转换**: 支持 `--in-place` 模式，成功转换后用 `.jxl` 文件替换原始图片。

#### 依赖
- **`jpeg-xl`**: 在 macOS 上通过 Homebrew 安装：`brew install jpeg-xl`

#### 使用方法
```bash
# 赋予执行权限
chmod +x png_to_jxl.sh

# 标准模式（在旁边创建新的 .jxl 文件）
./png_to_jxl.sh /path/to/images

# 原地转换模式
./png_to_jxl.sh --in-place /path/to/images
```

---

### 批量HEIC转无损PNG (heic_to_png.sh)

#### 功能
批量将 Apple 设备常用的 HEIC/HEIF 图片（`.heic`、`.heif`）转换为兼容性更强的**无损 PNG** 格式。

- **元数据**: 使用 `exiftool` 确保内部元数据（EXIF、GPS、XMP 等）的最完整迁移，并保留系统文件时间戳。
- **原地转换**: 支持 `--in-place` 模式，成功转换后用 `.png` 文件替换原始图片。

#### 依赖
- **`libheif`**: 在 macOS 上通过 Homebrew 安装：`brew install libheif`
- **`exiftool`**: 在 macOS 上通过 Homebrew 安装：`brew install exiftool`

#### 使用方法
```bash
# 赋予执行权限
chmod +x heic_to_png.sh

# 标准模式（在旁边创建新的 .png 文件）
./heic_to_png.sh /path/to/images

# 原地转换模式
./heic_to_png.sh --in-place /path/to/images
```

---

### 动态图片转H.266/VVC视频 (imganim_to_vvc.sh)

#### 功能
通过智能识别文件类型（而不仅仅是扩展名），批量将目录下的动态图片（GIF、动态 WebP、APNG）转换为现代、高效的 H.266 (VVC) 视频格式（`.mp4`）。

- **元数据**: 努力保留内部元数据，并完整保留系统文件时间戳。
- **原地转换**: 支持 `--in-place` 模式，成功转换后用 `.mp4` 视频替换原始图片。

#### 依赖
- **`ffmpeg`**: 需要编译时支持 `libvvenc`。Homebrew 的 `ffmpeg` 版本可能默认不包含此支持，用户可能需要手动编译或使用其他来源。
- **`exiftool`**: 在 macOS 上通过 Homebrew 安装：`brew install exiftool`

#### 使用方法
```bash
# 赋予执行权限
chmod +x imganim_to_vvc.sh

# 标准模式（在旁边创建新的 .mp4 文件）
./imganim_to_vvc.sh /path/to/images

# 原地转换模式
./imganim_to_vvc.sh --in-place /path/to/images
```

---

### 视频转高质量GIF (video_to_hq_gif.sh)

#### 功能
批量将常见的视频文件（`.mp4`、`.mov` 等）转换为视觉效果惊艳的**高质量 GIF**。脚本采用 ffmpeg 的两阶段方法（视频分析 -> 优化调色板生成 -> 转换），并利用先进的抖动算法，以实现最佳效果和流畅的色彩过渡。

- **元数据**: 尝试迁移内部视频元数据，并完整保留系统文件时间戳。
- **源文件清理**: 支持 `--delete-source` 模式，成功生成 GIF 后删除原始视频文件。
- **自定义选项**: 允许设置自定义帧率和输出宽度。

#### 依赖
- **`ffmpeg`**: 在 macOS 上通过 Homebrew 安装：`brew install ffmpeg`

#### 使用方法
```bash
# 赋予执行权限
chmod +x video_to_hq_gif.sh

# 标准模式（保留原始视频）
./video_to_hq_gif.sh /path/to/videos

# 清理模式（成功转换后删除原始视频）
./video_to_hq_gif.sh --delete-source /path/to/videos

# 自定义帧率和宽度（例如，24 FPS，720px 宽度）
./video_to_hq_gif.sh -r 24 -s 720 /path/to/videos
```

---

### 批量合并XMP元数据 (merge_xmp.sh)

#### 功能
将专业照片/视频工作流中生成的 `.xmp` 侧车元数据文件，完全合并回其对应的主要媒体文件中。

- **安全措施**: `exiftool` 在修改原始文件之前，会自动创建备份（文件名后缀为 `_original`）。
- **源文件清理**: 支持 `--delete-xmp` 模式，成功合并元数据后删除 `.xmp` 文件。

#### 依赖
- **`ExifTool`**: 在 macOS 上通过 Homebrew 安装：`brew install exiftool`

#### 使用方法
```bash
# 赋予执行权限
chmod +x merge_xmp.sh

# 标准模式（保留 .xmp 文件）
./merge_xmp.sh /path/to/media

# 清理模式（成功合并后删除 .xmp 文件）
./merge_xmp.sh --delete-xmp /path/to/media
```

---

---

### 不兼容媒体转换器 (convert_incompatible_media.sh)

#### 功能
批量将不兼容的媒体格式转换为通用兼容格式，**完整保留所有元数据**：
- **HEIC/HEIF → PNG**：使用 macOS 原生 `sips` 或 `heif-convert` 进行无损转换
- **MP4 → 动画 WebP**：无损转换（保留原始帧率和分辨率，比GIF更流畅、文件更小）

#### 核心特性
- **原子操作**：临时文件 → 验证 → 替换（防止数据损坏）
- **完整元数据保留**：
  - 内部元数据（EXIF、XMP、IPTC、ICC Profile）
  - 系统元数据（创建时间、修改时间、访问时间）
- **自动备份**：转换前自动备份原始文件
- **多级验证**：文件存在性、大小、完整性检查
- **安全检查**：防止在受保护的系统目录上操作

#### 依赖
- **`libheif`**（可选）：`brew install libheif`
- **`exiftool`**：`brew install exiftool`
- **`ffmpeg`**：`brew install ffmpeg`

#### 使用方法
```bash
# 赋予执行权限
chmod +x convert_incompatible_media.sh

# 标准模式（转换并替换，自动备份）
./convert_incompatible_media.sh /path/to/media

# 预览模式（仅显示将要执行的操作）
./convert_incompatible_media.sh --dry-run /path/to/media

# 详细模式，指定备份目录
./convert_incompatible_media.sh --verbose --backup-dir /path/to/backup /path/to/media
```

**转换细节**：
- **HEIC/HEIF → PNG**：保留原始分辨率和所有元数据
- **MP4 → WebP**：无损转换，保留原始帧率（如30fps）、完整分辨率和完整帧数
- **无任何限制**：不缩放、不丢帧、不降低帧率
- **原地替换**：转换后自动删除原文件，备份保存在 `_backup_*` 目录

---

### Substore 脚本

一系列高级 JavaScript 规则文件，专为 [Sub-Store](https://github.com/sub-store-org/Sub-Store) 订阅管理工具设计。这些脚本自动优化来自订阅链接的代理节点，以增强性能、安全性和隐私。规则根据不同的代理客户端（例如 Clash、Sing-box、Surge、Shadowrocket）和节点配置（例如中继、入口）进行定制。

### 归档脚本 (archive_and_upload.sh)

#### 功能
自动将目录中的所有文件分割并归档为 `.zip` 压缩包（每块大约 500MB）。适合归档大型项目或数据集。

#### 使用方法
```bash
# 赋予执行权限
chmod +x archive_and_upload.sh

# 运行脚本
./archive_and_upload.sh ./source_folder
```

**注意**：压缩包将在当前目录创建，命名为 `archive_part_1.zip`、`archive_part_2.zip` 等。

---
