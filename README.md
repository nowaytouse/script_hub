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
- **WebP Compression**: Binary search for optimal quality (15-20MB target)
- **GIF Compression**: Frame-preserving compression with quality control
- **Date Analyzer**: Deep EXIF/XMP date extraction

**Key Principles:**
- ✅ Complete metadata preservation (EXIF, XMP, ICC, timestamps)
- ✅ 100% FPS and frame count preservation
- ✅ Whitelist-only processing for safety
- ✅ Parallel processing optimized

### 🔄 Merge & Sync (`ruleset/merge_sync/`)
Core automation tools for proxy rule management:

- **Rule Ingestion**: `ingest_from_surge.sh` - Auto-import new rules from Surge profiles, classify them, and backup safely.
- **Rule Merger**: `merge_all_rulesets.sh` - Aggregates rules from 3rd-party sources and local `sources/` into unified lists.
- **AdBlock Merger**: `merge_adblock_modules.sh` - Intelligent merger for AdBlock modules with Surge/Singbox/Clash support.
- **Sync Pipeline**: `sync_all_rulesets.sh` - End-to-end automation: Ingest -> Merge -> Convert -> Git Push.

### 🌐 Network Scripts (`scripts/network/`)
Configuration management:
- **Config Manager**: Auto-update proxy configurations
- **SingBox Converter**: Batch convert Surge lists to Sing-box binary format

### 📋 Rulesets (`ruleset/`)
- **Sources (`ruleset/Sources/`)**:
  - `conf/`: Ingested rules (Auto-generated)
  - `custom/`: Manual rules (User-defined)
- **Generated**:
  - `Surge(Shadowkroket)/`: Final merged lists for Surge/Shadowrocket
  - `SingBox/`: Binary rulesets (`.srs`) for Sing-box

---

## Quick Start

```bash
# Clone repository
git clone https://github.com/YOUR_USERNAME/script_hub.git
cd script_hub

# Make scripts executable
chmod +x scripts/media/*.sh ruleset/merge_sync/*.sh

# Example: Ingest new rules from Surge profile (Dry Run)
./ruleset/merge_sync/ingest_from_surge.sh

# Example: Full Sync (Ingest -> Merge -> Git Push)
./ruleset/merge_sync/sync_all_rulesets.sh
```

### Automation (Unattended)
Scripts support `--no-backup` flag and detect `CI=true` environment to skip local backups during automated runs.
A GitHub Action workflow is included for daily updates.

---

## Dependencies

### Media Scripts
```bash
brew install jpeg-xl libheif exiftool ffmpeg webp
```

### Network Scripts
- **Rust Toolchain** (for some compiled tools)
- **Sing-box** (for rule conversion)

---

## Recent Updates

### 2025-12-06: Infrastructure Overhaul
- **New Structure**: Centralized sync tools in `ruleset/merge_sync/`.
- **Git Automation**: Full GitHub Actions workflow for daily unattended updates.
- **Smart Ingestion**: Improved logic to classify rules from Surge profiles into dedicated source files.
- **Privacy First**: Strict exclusion of sensitive data (`隐私🔏`).

### 2025-12-04: WebP FPS Preservation
- Fixed ffmpeg 25fps limitation using `img2webp` for precise frame timing.

---

## License
MIT License.

---

# 中文说明

一个实用脚本集合，用于媒体转换、网络配置管理和代理规则优化。

## 功能特性

### 🎬 媒体脚本 (`scripts/media/`)
批量媒体转换工具，支持**完整元数据保留**和**健康检查验证**：
- **JPEG/PNG → JXL**: 高效无损/有损压缩
- **HEIC → PNG**: 苹果格式转换
- **MP4 → WebP**: **真实帧率保留**，完美复刻原视频流畅度
- **Video → GIF**: 高质量调色板优化

### 🔄 合并与同步 (`ruleset/merge_sync/`)
代理规则管理的核心自动化工具：
- **规则吸纳 (`ingest`)**: 从 Surge 配置文件智能提取新规则，分类并归档。
- **规则合并 (`merge`)**: 聚合第三方源和本地 `sources/` 规则，生成去重后的统一列表。
- **广告拦截合并**: 智能合并 Surge/Clash/Singbox 格式的去广告模块。
- **全流程同步**: `sync_all_rulesets.sh` 实现 "吸纳 -> 合并 -> 转换 -> Git推送" 一键死人值守。

### 📋 规则集 (`ruleset/`)
- **源文件 (`ruleset/Sources/`)**:
  - `conf/`: 自动吸纳的规则文件
  - `custom/`: 用户手动维护的规则文件
- **生成产物**:
  - `Surge(Shadowkroket)/`: 适用于 Surge 和 Shadowrocket 的最终规则
  - `SingBox/`: 适用于 Sing-box 的二进制规则 (`.srs`)

## 快速开始

```bash
# 赋予执行权限
chmod +x scripts/media/*.sh ruleset/merge_sync/*.sh

# 示例：从 Surge 配置提取新规则 (试运行)
./ruleset/merge_sync/ingest_from_surge.sh

# 示例：执行全量同步 (合并+转换+推送)
./ruleset/merge_sync/sync_all_rulesets.sh
```

### 无人值守自动化
脚本支持 `--no-backup` 参数，并能自动检测 `CI=true` 环境以跳过本地备份步骤，适合 Cron 或 GitHub Actions 每日自动运行。

## 最近更新

### 2025-12-06: 架构重构
- **目录调整**: 同步工具集中至 `ruleset/merge_sync/`。
- **自动化**: 集成 GitHub Actions 实现每日自动更新。
- **隐私保护**: 严格排除敏感目录 (`隐私🔏`)。
- **智能分类**: Ingest 脚本现在能将规则分类到 `conf/` 下的独立文件中。

### 2025-12-04: WebP 帧率修复
- 使用 `img2webp` 彻底解决了 ffmpeg 导致 WebP 帧率被锁定在 25fps 的问题。
