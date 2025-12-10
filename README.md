# Script Hub

Proxy rule management, Surge/Shadowrocket modules, and high-quality media conversion tools.

[中文](#中文说明)

---

## Auto Update

Rules and modules update **twice daily** via GitHub Actions (04:00 & 12:00 Beijing Time).

---

## Module Collections

| Collection | Modules | Content |
|------------|---------|---------|
| **功能增强大合集** | 23 | BiliBili/iRingo/YouTube/TikTok/DNS/BoxJs |
| **广告拦截大合集** | 11 | AWAvenue/毒奶/可莉/Sukka |
| **App去广告大合集** | 32 | WeChat/Weibo/Taobao/JD/Zhihu |

### Import URLs (Surge)

```
# 功能增强
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/amplify_nexus/%F0%9F%9A%80%20%E5%8A%9F%E8%83%BD%E5%A2%9E%E5%BC%BA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# 广告拦截
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%9B%A1%EF%B8%8F%20%E5%B9%BF%E5%91%8A%E6%8B%A6%E6%88%AA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# App去广告
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%8E%AF%20App%E5%8E%BB%E5%B9%BF%E5%91%8A%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule
```

**Shadowrocket:** Replace `surge%28main%29` with `shadowrocket`.

**Module Helper:** [surge_module_helper.html](module/surge_module_helper.html)

---

## Media Tools

### Modern Format Boost (Rust)

High-quality batch conversion with complete metadata preservation.

| Tool | Output | Encoder | Use Case |
|------|--------|---------|----------|
| `imgquality-hevc` | JXL / HEVC MP4 | x265 | Apple compatible |
| `vidquality-hevc` | HEVC MP4 | x265 | Apple compatible |
| `imgquality` | JXL / AV1 MP4 | SVT-AV1 | Best compression |
| `vidquality` | AV1 MP4 | SVT-AV1 | Best compression |

```bash
cd tools/modern_format_boost && cargo build --release

# Convert images (JPEG→JXL, GIF→HEVC MP4)
./target/release/imgquality-hevc auto /path --match-quality --delete-original

# Convert videos (H.264→HEVC)
./target/release/vidquality-hevc auto /path --match-quality --delete-original
```

**Features:** Smart quality matching, metadata preservation (EXIF/XMP/ICC/timestamps), smart rollback, progress bar.

### static2jxl (C)

Fast batch JPEG/PNG → JXL conversion.

```bash
cd tools/static2jxl && make
./static2jxl --in-place /path/to/images
```

---

## Rulesets

| Platform | Path | Format |
|----------|------|--------|
| Surge/Shadowrocket | `ruleset/Surge(Shadowkroket)/` | `.list` |
| Sing-box | `ruleset/SingBox/` | `.srs` |
| MetaCubeX | `ruleset/MetaCubeX/` | `.yaml` |

---

## Quick Start

```bash
git clone https://github.com/nowaytouse/script_hub.git
cd script_hub

# Build Rust tools
cd tools/modern_format_boost && cargo build --release

# Run full update
chmod +x ruleset/merge_sync/*.sh
./ruleset/merge_sync/full_update.sh
```

**Dependencies:** `brew install jpeg-xl ffmpeg exiftool sing-box`

---

# 中文说明

代理规则管理、Surge/Shadowrocket 模块合集、高质量媒体转换工具。

---

## 自动更新

规则和模块通过 GitHub Actions **每日两次**自动更新（北京时间 04:00 和 12:00）。

---

## 推荐合集

| 合集 | 模块数 | 内容 |
|------|--------|------|
| **功能增强大合集** | 23 | BiliBili/iRingo/YouTube/TikTok/DNS/BoxJs |
| **广告拦截大合集** | 11 | AWAvenue/毒奶/可莉/Sukka |
| **App去广告大合集** | 32 | 微信/微博/淘宝/京东/知乎 |

### 导入链接 (Surge)

```
# 功能增强
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/amplify_nexus/%F0%9F%9A%80%20%E5%8A%9F%E8%83%BD%E5%A2%9E%E5%BC%BA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# 广告拦截
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%9B%A1%EF%B8%8F%20%E5%B9%BF%E5%91%8A%E6%8B%A6%E6%88%AA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# App去广告
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%8E%AF%20App%E5%8E%BB%E5%B9%BF%E5%91%8A%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule
```

**Shadowrocket:** 将 `surge%28main%29` 替换为 `shadowrocket`。

**模块助手:** [surge_module_helper.html](module/surge_module_helper.html)

---

## 媒体工具

### Modern Format Boost (Rust)

高质量批量转换，完整元数据保留。

| 工具 | 输出 | 编码器 | 适用 |
|------|------|--------|------|
| `imgquality-hevc` | JXL / HEVC MP4 | x265 | Apple 兼容 |
| `vidquality-hevc` | HEVC MP4 | x265 | Apple 兼容 |
| `imgquality` | JXL / AV1 MP4 | SVT-AV1 | 最佳压缩 |
| `vidquality` | AV1 MP4 | SVT-AV1 | 最佳压缩 |

```bash
cd tools/modern_format_boost && cargo build --release

# 图像转换 (JPEG→JXL, GIF→HEVC MP4)
./target/release/imgquality-hevc auto /path --match-quality --delete-original

# 视频转换 (H.264→HEVC)
./target/release/vidquality-hevc auto /path --match-quality --delete-original
```

**特性:** 智能质量匹配、元数据保留 (EXIF/XMP/ICC/时间戳)、智能回退、进度条。

### static2jxl (C)

快速批量 JPEG/PNG → JXL 转换。

```bash
cd tools/static2jxl && make
./static2jxl --in-place /path/to/images
```

---

## 规则集

| 平台 | 路径 | 格式 |
|------|------|------|
| Surge/Shadowrocket | `ruleset/Surge(Shadowkroket)/` | `.list` |
| Sing-box | `ruleset/SingBox/` | `.srs` |
| MetaCubeX | `ruleset/MetaCubeX/` | `.yaml` |

---

## 快速开始

```bash
git clone https://github.com/nowaytouse/script_hub.git
cd script_hub

# 编译 Rust 工具
cd tools/modern_format_boost && cargo build --release

# 执行全量更新
chmod +x ruleset/merge_sync/*.sh
./ruleset/merge_sync/full_update.sh
```

**依赖:** `brew install jpeg-xl ffmpeg exiftool sing-box`

---

MIT License
