# Script Hub 🛠️

A collection of utility scripts for media conversion, network configuration management, proxy rule optimization, and **Surge/Shadowrocket module management**.

[中文说明](#中文说明)

---

## ⏰ Auto Update Schedule

| Time (UTC) | Time (Beijing) | Description |
|------------|----------------|-------------|
| 20:00 | 04:00 (次日) | 凌晨更新 |
| 04:00 | 12:00 | 中午更新 |

Rules and modules are automatically updated **twice daily** via GitHub Actions.

---

## 📦 Module Collections (推荐合集)

We provide **3 mega-collections** that merge multiple modules into single files for easier management:

| Collection | Modules Merged | Description |
|------------|----------------|-------------|
| 🚀 **功能增强大合集** | 23 | BiliBili/iRingo/YouTube/TikTok/DNS/BoxJs etc. |
| 🛡️ **广告拦截大合集** | 11 | AWAvenue/毒奶/可莉/Sukka/广告平台拦截器 etc. |
| 🎯 **App去广告大合集** | 32 | 微信/微博/淘宝/京东/知乎/小红书 etc. |

### Quick Import URLs

**Surge:**
```
# 功能增强大合集
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/amplify_nexus/%F0%9F%9A%80%20%E5%8A%9F%E8%83%BD%E5%A2%9E%E5%BC%BA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# 广告拦截大合集
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%9B%A1%EF%B8%8F%20%E5%B9%BF%E5%91%8A%E6%8B%A6%E6%88%AA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# App去广告大合集
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%8E%AF%20App%E5%8E%BB%E5%B9%BF%E5%91%8A%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule
```

**Shadowrocket:** Same URLs but replace `surge%28main%29` with `shadowrocket`.

### 🌐 Module Helper Website

Visit our interactive module helper: [surge_module_helper.html](module/surge_module_helper.html)

Features:
- 📦 Filter by "推荐合集" to see merged collections
- 🚀 One-click copy URL for Surge/Shadowrocket
- 📊 Track your installation progress
- 🔍 Search and filter modules

---

## Features

### 🎬 Media Scripts (`media/`)
Batch media conversion tools with **complete metadata preservation**:

- **JPEG/PNG → JXL**: High-compression with full metadata preservation
- **HEIC/HEIF → PNG**: Apple format to universal PNG
- **MP4 → WebP**: True FPS preservation using `img2webp`
- **Animated Images → AV1/AVIF**: Modern codec conversion
- **Video → High-Quality GIF**: Two-pass palette optimization

### 🔄 Merge & Sync (`ruleset/merge_sync/`)
Core automation tools for proxy rule management:

- **Rule Merger**: Aggregates rules from 3rd-party sources
- **AdBlock Merger**: Intelligent merger for AdBlock modules
- **Module Merger**: Combines multiple modules into mega-collections
- **Sync Pipeline**: End-to-end automation with Git push

### 📋 Rulesets (`ruleset/`)
- **Surge/Shadowrocket**: `ruleset/Surge(Shadowkroket)/`
- **Sing-box**: `ruleset/SingBox/` (binary `.srs` format)
- **MetaCubeX**: `ruleset/MetaCubeX/`

---

## Quick Start

```bash
# Clone repository
git clone https://github.com/nowaytouse/script_hub.git
cd script_hub

# Make scripts executable
chmod +x ruleset/merge_sync/*.sh

# Full update (merge rules + modules + git push)
./ruleset/merge_sync/full_update.sh
```

---

## Dependencies

```bash
# macOS
brew install jpeg-xl libheif exiftool ffmpeg webp

# For Sing-box rule conversion
brew install sing-box
```

---

## Recent Updates

### 2025-12-10: Module Mega-Collections
- **NEW**: 🚀 功能增强大合集 (23 modules merged)
- **NEW**: 🛡️ 广告拦截大合集 (11 modules merged)
- **NEW**: 🎯 App去广告大合集 (32 modules merged)
- **NEW**: Twice-daily auto updates (04:00 & 12:00 Beijing Time)
- **NEW**: Module helper website with collection filters

### 2025-12-06: Infrastructure Overhaul
- Centralized sync tools in `ruleset/merge_sync/`
- Full GitHub Actions workflow for daily updates
- Smart rule classification from Surge profiles

---

## License
MIT License.

---

# 中文说明

一个实用脚本集合，用于媒体转换、网络配置管理、代理规则优化和 **Surge/Shadowrocket 模块管理**。

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

| 合集名称 | 合并模块数 | 包含内容 |
|----------|-----------|----------|
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

访问我们的交互式模块助手网页: [surge_module_helper.html](module/surge_module_helper.html)

功能特点:
- 📦 点击"推荐合集"筛选查看合集模块
- 🚀 一键复制 Surge/Shadowrocket 导入链接
- 📊 追踪你的安装进度
- 🔍 搜索和筛选模块

---

## 功能特性

### 🎬 媒体脚本 (`media/`)
批量媒体转换工具，支持**完整元数据保留**：
- **JPEG/PNG → JXL**: 高效压缩，保留所有元数据
- **HEIC → PNG**: 苹果格式转换
- **MP4 → WebP**: 真实帧率保留
- **动图 → AV1/AVIF**: 现代编码格式转换

### 🔄 合并与同步 (`ruleset/merge_sync/`)
代理规则管理的核心自动化工具：
- **规则合并**: 聚合第三方源规则
- **广告拦截合并**: 智能合并去广告模块
- **模块合并**: 将多个模块合并为大合集
- **全流程同步**: 一键完成合并+转换+Git推送

### 📋 规则集 (`ruleset/`)
- **Surge/Shadowrocket**: `ruleset/Surge(Shadowkroket)/`
- **Sing-box**: `ruleset/SingBox/` (二进制 `.srs` 格式)
- **MetaCubeX**: `ruleset/MetaCubeX/`

---

## 快速开始

```bash
# 克隆仓库
git clone https://github.com/nowaytouse/script_hub.git
cd script_hub

# 赋予执行权限
chmod +x ruleset/merge_sync/*.sh

# 执行全量更新 (合并规则+模块+Git推送)
./ruleset/merge_sync/full_update.sh
```

---

## 依赖安装

```bash
# macOS
brew install jpeg-xl libheif exiftool ffmpeg webp

# Sing-box 规则转换
brew install sing-box
```

---

## 最近更新

### 2025-12-10: 模块大合集
- **新增**: 🚀 功能增强大合集 (合并23个模块)
- **新增**: 🛡️ 广告拦截大合集 (合并11个模块)
- **新增**: 🎯 App去广告大合集 (合并32个模块)
- **新增**: 每日两次自动更新 (北京时间04:00和12:00)
- **新增**: 模块助手网页支持合集筛选

### 2025-12-06: 架构重构
- 同步工具集中至 `ruleset/merge_sync/`
- 集成 GitHub Actions 实现每日自动更新
- 智能规则分类

---

## 许可证
MIT License.
