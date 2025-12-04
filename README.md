# Script Hub 🛠️

A collection of utility scripts for media conversion, network configuration management, and proxy rule optimization.

[中文说明](#中文说明)

---

## Features

### 🎬 Media Scripts (`scripts/media/`)
Batch media conversion tools with **complete metadata preservation** and **health validation**:

- **JPEG → JXL**: High-compression conversion with full metadata preservation
- **PNG → JXL**: Mathematically lossless compression
- **HEIC/HEIF → PNG**: Apple format to universal PNG
- **MP4 → WebP**: **True FPS preservation** using `img2webp` (fixes ffmpeg's 25fps limitation)
- **Animated Images → H.266/VVC**: Modern video codec conversion
- **Video → High-Quality GIF**: Two-pass palette optimization

**Key Principles:**
- ✅ Complete metadata preservation (EXIF, XMP, ICC Profile, timestamps)
- ✅ 100% FPS and frame count preservation for animations
- ✅ Health check validation before deleting originals
- ✅ Whitelist-only processing for safety

### 🌐 Network Scripts (`scripts/network/`)
Configuration management and auto-update tools for proxy applications:

- **Config Manager**: Auto-update proxy configurations
- **Rule Sync**: Synchronize rulesets across platforms

### 📦 Substore Scripts (`substore/`)
Advanced JavaScript rules for [Sub-Store](https://github.com/sub-store-org/Sub-Store):

- Node filtering and optimization
- Region-based routing rules
- Multi-client support (Clash, Sing-box, Surge, Shadowrocket)

### 📋 Rulesets (`ruleset/`)
Proxy rulesets for multiple platforms:

- Surge / Shadowrocket
- Sing-Box
- MetaCubeX (Clash Meta)

---

## Quick Start

```bash
# Clone repository
git clone https://github.com/YOUR_USERNAME/script_hub.git
cd script_hub

# Make scripts executable
chmod +x scripts/media/*.sh

# Example: Convert incompatible media
./scripts/media/convert_incompatible_media.sh /path/to/media --keep-only-incompatible
```

## Dependencies

### Media Scripts
```bash
# macOS (Homebrew)
brew install jpeg-xl libheif exiftool ffmpeg webp
```

### Required Tools
| Tool | Purpose | Install |
|------|---------|---------|
| `cjxl` | JPEG XL encoding | `brew install jpeg-xl` |
| `heif-convert` | HEIC/HEIF decoding | `brew install libheif` |
| `exiftool` | Metadata handling | `brew install exiftool` |
| `ffmpeg/ffprobe` | Video processing | `brew install ffmpeg` |
| `img2webp` | WebP animation (FPS-accurate) | `brew install webp` |

---

## Recent Updates

### 2024-12-04: WebP FPS Preservation Fix
- **Problem**: ffmpeg's libwebp encoder hardcodes 25fps limit
- **Solution**: Rewrote MP4→WebP conversion using `img2webp` for exact frame timing
- **Result**: 30fps videos now correctly convert to 30fps WebP (33ms/frame)

---

## License

MIT License - See individual script headers for details.

---

# 中文说明

一个实用脚本集合，用于媒体转换、网络配置管理和代理规则优化。

## 功能特性

### 🎬 媒体脚本 (`scripts/media/`)
批量媒体转换工具，支持**完整元数据保留**和**健康检查验证**：

- **JPEG → JXL**: 高压缩率转换，完整保留元数据
- **PNG → JXL**: 数学无损压缩
- **HEIC/HEIF → PNG**: Apple格式转通用PNG
- **MP4 → WebP**: **真实FPS保留**，使用`img2webp`（修复ffmpeg的25fps限制）
- **动图 → H.266/VVC**: 现代视频编码转换
- **视频 → 高质量GIF**: 双通道调色板优化

**核心原则：**
- ✅ 完整元数据保留（EXIF、XMP、ICC配置文件、时间戳）
- ✅ 动画100%帧率和帧数保留
- ✅ 删除原文件前进行健康检查验证
- ✅ 仅处理白名单格式，确保安全

### 🌐 网络脚本 (`scripts/network/`)
代理应用的配置管理和自动更新工具：

- **配置管理器**: 自动更新代理配置
- **规则同步**: 跨平台同步规则集

### 📦 Substore脚本 (`substore/`)
[Sub-Store](https://github.com/sub-store-org/Sub-Store)的高级JavaScript规则：

- 节点过滤和优化
- 基于地区的路由规则
- 多客户端支持（Clash、Sing-box、Surge、Shadowrocket）

### 📋 规则集 (`ruleset/`)
多平台代理规则集：

- Surge / Shadowrocket
- Sing-Box
- MetaCubeX (Clash Meta)

---

## 快速开始

```bash
# 克隆仓库
git clone https://github.com/YOUR_USERNAME/script_hub.git
cd script_hub

# 添加执行权限
chmod +x scripts/media/*.sh

# 示例：转换不兼容媒体
./scripts/media/convert_incompatible_media.sh /path/to/media --keep-only-incompatible
```

## 依赖安装

```bash
# macOS (Homebrew)
brew install jpeg-xl libheif exiftool ffmpeg webp
```

---

## 最近更新

### 2024-12-04: WebP帧率保留修复
- **问题**: ffmpeg的libwebp编码器硬编码25fps限制
- **解决方案**: 使用`img2webp`重写MP4→WebP转换，实现精确帧时序
- **结果**: 30fps视频现在正确转换为30fps WebP（33ms/帧）

---

## 许可证

MIT许可证 - 详见各脚本文件头部说明。
