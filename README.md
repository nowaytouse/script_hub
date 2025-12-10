# Script Hub 🛠️

A comprehensive toolkit for **proxy rule management**, **Surge/Shadowrocket module collections**, and **high-quality media format conversion** with complete metadata preservation.

[中文说明](#中文说明)

---

## ⏰ Auto Update Schedule

| Time (UTC) | Time (Beijing) | Description |
|------------|----------------|-------------|
| 20:00 | 04:00 (Next Day) | Early morning update |
| 04:00 | 12:00 | Noon update |

Rules and modules are automatically updated **twice daily** via GitHub Actions.

---

## 📦 Module Collections (Recommended)

We provide **3 mega-collections** that merge multiple modules into single files for easier management:

| Collection | Modules | Description |
|------------|---------|-------------|
| 🚀 **功能增强大合集** | 23 | BiliBili/iRingo/YouTube/TikTok/DNS/BoxJs etc. |
| 🛡️ **广告拦截大合集** | 11 | AWAvenue/毒奶/可莉/Sukka/Ad Platform Blocker etc. |
| 🎯 **App去广告大合集** | 32 | WeChat/Weibo/Taobao/JD/Zhihu/Xiaohongshu etc. |

### Quick Import URLs

**Surge:**
```
# 功能增强大合集 (Feature Enhancement)
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/amplify_nexus/%F0%9F%9A%80%20%E5%8A%9F%E8%83%BD%E5%A2%9E%E5%BC%BA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# 广告拦截大合集 (Ad Blocking)
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%9B%A1%EF%B8%8F%20%E5%B9%BF%E5%91%8A%E6%8B%A6%E6%88%AA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# App去广告大合集 (App Ad Removal)
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%8E%AF%20App%E5%8E%BB%E5%B9%BF%E5%91%8A%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule
```

**Shadowrocket:** Replace `surge%28main%29` with `shadowrocket` in URLs.

### 🌐 Module Helper Website

Visit our interactive module helper: [surge_module_helper.html](module/surge_module_helper.html)

- 📦 Filter by "推荐合集" to see merged collections
- 🚀 One-click copy URL for Surge/Shadowrocket
- 📊 Track your installation progress
- �  Search and filter modules

---

## 🎬 Media Conversion Tools

### Modern Format Boost (`tools/modern_format_boost/`)

High-performance Rust tools for batch media format upgrade with **complete metadata preservation**:

| Tool | Input | Output | Encoder | Use Case |
|------|-------|--------|---------|----------|
| **imgquality** | Images/GIF | JXL / AV1 MP4 | SVT-AV1 | Best compression ratio |
| **imgquality-hevc** | Images/GIF | JXL / HEVC MP4 | x265 | Apple device compatibility |
| **vidquality** | Videos | AV1 MP4 | SVT-AV1 | Best compression ratio |
| **vidquality-hevc** | Videos | HEVC MP4 | x265 | Apple device compatibility |

**Key Features:**
- 🎯 **Smart Quality Matching**: `--match-quality` auto-calculates optimal CRF based on input quality
- 📊 **Complete Metadata**: EXIF/IPTC/XMP/ICC Profile/timestamps/xattr preserved via ExifTool
- 🔄 **Smart Rollback**: Auto-reverts if output is larger than input
- 📈 **Progress Visualization**: Real-time progress bar with ETA
- 🛡️ **Safety Checks**: Dangerous directory detection, verified safe deletes

**Quick Start:**
```bash
cd tools/modern_format_boost
cargo build --release

# Convert images to JXL, animations to HEVC MP4
./target/release/imgquality-hevc /path/to/images --match-quality --delete-original

# Convert videos to HEVC
./target/release/vidquality-hevc /path/to/videos --match-quality --delete-original
```

### Shell Scripts (`media/`)

Legacy bash scripts for media conversion:
- **JPEG/PNG → JXL**: Lossless transcoding with metadata preservation
- **HEIC → PNG**: Apple format conversion
- **Animated Images → AV1/AVIF/VVC**: Modern codec conversion
- **Video → High-Quality GIF**: Two-pass palette optimization

---

## 📋 Rulesets

Multi-platform proxy rule support:

| Platform | Path | Format |
|----------|------|--------|
| **Surge/Shadowrocket** | `ruleset/Surge(Shadowkroket)/` | `.list` text |
| **Sing-box** | `ruleset/SingBox/` | `.srs` binary |
| **MetaCubeX** | `ruleset/MetaCubeX/` | `.yaml` |

---

## 🔄 Automation (`ruleset/merge_sync/`)

Core automation tools for proxy rule management:

- **Rule Merger**: Aggregates rules from 3rd-party sources
- **AdBlock Merger**: Intelligent merger for AdBlock modules with deduplication
- **Module Merger**: Combines multiple modules into mega-collections
- **Surge → Shadowrocket**: Auto-converts modules for Shadowrocket compatibility
- **Sing-box Converter**: Converts text rules to binary `.srs` format

---

## 🚀 Quick Start

```bash
# Clone repository
git clone https://github.com/nowaytouse/script_hub.git
cd script_hub

# Build Rust tools
cd tools/modern_format_boost && cargo build --release && cd ../..

# Run full update (merge rules + modules + git push)
chmod +x ruleset/merge_sync/*.sh
./ruleset/merge_sync/full_update.sh
```

---

## 📦 Dependencies

```bash
# macOS
brew install jpeg-xl ffmpeg exiftool webp libheif sing-box

# Verify
cjxl --version && ffmpeg -version && exiftool -ver
```

---

## 📁 Project Structure

```
script_hub/
├── media/                    # Shell scripts for media conversion
├── module/                   # Surge/Shadowrocket modules
│   ├── surge(main)/          # Surge modules
│   ├── shadowrocket/         # Shadowrocket modules
│   └── surge_module_helper.html
├── ruleset/                  # Proxy rulesets
│   ├── Surge(Shadowkroket)/  # Surge/Shadowrocket rules
│   ├── SingBox/              # Sing-box binary rules
│   ├── MetaCubeX/            # MetaCubeX rules
│   └── merge_sync/           # Automation scripts
├── substore/                 # Sub-Store scripts
└── tools/
    ├── modern_format_boost/  # Rust media conversion tools
    └── static2jxl/           # C tool for static image → JXL
```

---

## License

MIT License

---

# 中文说明

一个综合工具集，用于**代理规则管理**、**Surge/Shadowrocket 模块合集**和**高质量媒体格式转换**（完整元数据保留）。

---

## ⏰ 自动更新时间

| UTC时间 | 北京时间 | 说明 |
|---------|----------|------|
| 20:00 | 04:00 (次日) | 凌晨更新 |
| 04:00 | 12:00 | 中午更新 |

规则和模块通过 GitHub Actions **每日自动更新两次**。

---

## 📦 推荐合集

我们提供 **3个大合集**，将多个模块合并为单个文件，方便管理：

| 合集名称 | 模块数 | 包含内容 |
|----------|--------|----------|
| 🚀 **功能增强大合集** | 23 | BiliBili增强/iRingo/YouTube/TikTok/DNS/BoxJs等 |
| 🛡️ **广告拦截大合集** | 11 | AWAvenue/毒奶/可莉/Sukka/广告平台拦截器等 |
| 🎯 **App去广告大合集** | 32 | 微信/微博/淘宝/京东/知乎/小红书等App专项去广告 |

### 快速导入链接

**Surge 用户:**
```
# 功能增强大合集
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/amplify_nexus/%F0%9F%9A%80%20%E5%8A%9F%E8%83%BD%E5%A2%9E%E5%BC%BA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# 广告拦截大合集
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%9B%A1%EF%B8%8F%20%E5%B9%BF%E5%91%8A%E6%8B%A6%E6%88%AA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# App去广告大合集
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%8E%AF%20App%E5%8E%BB%E5%B9%BF%E5%91%8A%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule
```

**Shadowrocket 用户:** 将链接中的 `surge%28main%29` 替换为 `shadowrocket` 即可。

### 🌐 模块导入助手

访问交互式模块助手网页: [surge_module_helper.html](module/surge_module_helper.html)

- 📦 点击"推荐合集"筛选查看合集模块
- 🚀 一键复制 Surge/Shadowrocket 导入链接
- 📊 追踪安装进度
- 🔍 搜索和筛选模块

---

## 🎬 媒体转换工具

### Modern Format Boost (`tools/modern_format_boost/`)

高性能 Rust 工具，用于批量媒体格式升级，**完整保留元数据**：

| 工具 | 输入 | 输出 | 编码器 | 适用场景 |
|------|------|------|--------|----------|
| **imgquality** | 图像/动图 | JXL / AV1 MP4 | SVT-AV1 | 最佳压缩率 |
| **imgquality-hevc** | 图像/动图 | JXL / HEVC MP4 | x265 | Apple设备兼容 |
| **vidquality** | 视频 | AV1 MP4 | SVT-AV1 | 最佳压缩率 |
| **vidquality-hevc** | 视频 | HEVC MP4 | x265 | Apple设备兼容 |

**核心特性:**
- 🎯 **智能质量匹配**: `--match-quality` 根据输入质量自动计算最佳 CRF
- 📊 **完整元数据保留**: EXIF/IPTC/XMP/ICC Profile/时间戳/xattr 通过 ExifTool 保留
- 🔄 **智能回退**: 输出大于输入时自动回退
- 📈 **进度可视化**: 实时进度条和 ETA 估算
- 🛡️ **安全检查**: 危险目录检测、验证后删除

**快速开始:**
```bash
cd tools/modern_format_boost
cargo build --release

# 图像转 JXL，动图转 HEVC MP4
./target/release/imgquality-hevc /path/to/images --match-quality --delete-original

# 视频转 HEVC
./target/release/vidquality-hevc /path/to/videos --match-quality --delete-original
```

### Shell 脚本 (`media/`)

传统 bash 脚本用于媒体转换：
- **JPEG/PNG → JXL**: 无损转码，保留元数据
- **HEIC → PNG**: Apple 格式转换
- **动图 → AV1/AVIF/VVC**: 现代编码格式转换
- **视频 → 高质量 GIF**: 双通道调色板优化

---

## 📋 规则集

多平台代理规则支持：

| 平台 | 路径 | 格式 |
|------|------|------|
| **Surge/Shadowrocket** | `ruleset/Surge(Shadowkroket)/` | `.list` 文本 |
| **Sing-box** | `ruleset/SingBox/` | `.srs` 二进制 |
| **MetaCubeX** | `ruleset/MetaCubeX/` | `.yaml` |

---

## 🔄 自动化工具 (`ruleset/merge_sync/`)

代理规则管理的核心自动化工具：

- **规则合并**: 聚合第三方源规则
- **广告拦截合并**: 智能合并去广告模块，自动去重
- **模块合并**: 将多个模块合并为大合集
- **Surge → Shadowrocket**: 自动转换模块以兼容 Shadowrocket
- **Sing-box 转换**: 将文本规则转换为二进制 `.srs` 格式

---

## 🚀 快速开始

```bash
# 克隆仓库
git clone https://github.com/nowaytouse/script_hub.git
cd script_hub

# 编译 Rust 工具
cd tools/modern_format_boost && cargo build --release && cd ../..

# 执行全量更新 (合并规则+模块+Git推送)
chmod +x ruleset/merge_sync/*.sh
./ruleset/merge_sync/full_update.sh
```

---

## 📦 依赖安装

```bash
# macOS
brew install jpeg-xl ffmpeg exiftool webp libheif sing-box

# 验证安装
cjxl --version && ffmpeg -version && exiftool -ver
```

---

## 📁 项目结构

```
script_hub/
├── media/                    # 媒体转换 Shell 脚本
├── module/                   # Surge/Shadowrocket 模块
│   ├── surge(main)/          # Surge 模块
│   ├── shadowrocket/         # Shadowrocket 模块
│   └── surge_module_helper.html
├── ruleset/                  # 代理规则集
│   ├── Surge(Shadowkroket)/  # Surge/Shadowrocket 规则
│   ├── SingBox/              # Sing-box 二进制规则
│   ├── MetaCubeX/            # MetaCubeX 规则
│   └── merge_sync/           # 自动化脚本
├── substore/                 # Sub-Store 脚本
└── tools/
    ├── modern_format_boost/  # Rust 媒体转换工具
    └── static2jxl/           # C 工具：静态图像 → JXL
```

---

## 许可证

MIT License
