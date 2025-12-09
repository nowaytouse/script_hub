# Module Organization

This directory contains Surge and Shadowrocket modules organized by their functional groups.

[中文说明](#中文说明)

---

## ⏰ Auto Update Schedule

| UTC Time | Beijing Time | Description |
|----------|--------------|-------------|
| 20:00 | 04:00 (next day) | Early morning update |
| 04:00 | 12:00 | Noon update |

Modules are automatically updated **twice daily** via GitHub Actions.

**✅ Collections auto-update**: All 3 mega-collections are automatically regenerated from upstream modules.

---

## 📦 Recommended Collections (推荐合集)

We provide **3 mega-collections** that merge multiple modules into single files:

| Collection | Modules | Content |
|------------|---------|---------|
| 🚀 **功能增强大合集** | 23 | BiliBili/iRingo/YouTube/TikTok/DNS/BoxJs |
| 🛡️ **广告拦截大合集** | 11 | AWAvenue/毒奶/可莉/Sukka/广告平台拦截器 |
| 🎯 **App去广告大合集** | 32 | WeChat/Weibo/Taobao/JD/Zhihu/RedNote |

### Quick Import (Surge)

```
# 功能增强大合集 (Feature Enhancement)
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/amplify_nexus/%F0%9F%9A%80%20%E5%8A%9F%E8%83%BD%E5%A2%9E%E5%BC%BA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# 广告拦截大合集 (Ad Blocking Platform)
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%9B%A1%EF%B8%8F%20%E5%B9%BF%E5%91%8A%E6%8B%A6%E6%88%AA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# App去广告大合集 (App-specific Ad Blocking)
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%8E%AF%20App%E5%8E%BB%E5%B9%BF%E5%91%8A%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule
```

**Shadowrocket:** Replace `surge%28main%29` with `shadowrocket` in URLs.

### 🌐 Module Helper Website

Visit: [surge_module_helper.html](surge_module_helper.html)

- 📦 Click "推荐合集" to filter collections
- 🚀 One-click copy URL
- 📊 Track installation progress

---

## Directory Structure

```
module/
├── surge(main)/           # Surge modules
│   ├── amplify_nexus/     # 『 🛠️ Amplify Nexus › 增幅枢纽 』
│   │   └── 🚀 功能增强大合集.sgmodule  ⭐ RECOMMENDED
│   ├── head_expanse/      # 『 🔝 Head Expanse › 首端扩域 』
│   │   ├── 🛡️ 广告拦截大合集.sgmodule  ⭐ RECOMMENDED
│   │   └── 🎯 App去广告大合集.sgmodule  ⭐ RECOMMENDED
│   └── narrow_pierce/     # 『 🎯 Narrow Pierce › 窄域穿刺 』
│
└── shadowrocket/          # Shadowrocket modules (auto-synced)
    ├── amplify_nexus/
    ├── head_expanse/
    └── narrow_pierce/
```

## Module Groups

### 🛠️ Amplify Nexus › 增幅枢纽
**Purpose**: Enhancement features and core functionality

Includes: BiliBili Enhanced, iRingo suite, YouTube Enhance, TikTok Unlock, DNS management, BoxJS, Sub-Store, etc.

### 🔝 Head Expanse › 首端扩域
**Purpose**: Ad blocking platforms and comprehensive filtering

Includes: AWAvenue, Adblock4limbo, 可莉广告过滤器, Sukka modules, Script Hub, etc.

### 🎯 Narrow Pierce › 窄域穿刺
**Purpose**: App-specific ad blocking

Includes: WeChat, Weibo, Taobao, JD, Zhihu, RedNote, BiliBili, YouTube, Spotify, etc.

---

## Management Scripts

```bash
# Merge all modules into collections
python3 ruleset/merge_sync/merge_amplify_nexus_modules.py
python3 ruleset/merge_sync/merge_head_expanse_modules.py
python3 ruleset/merge_sync/merge_narrow_pierce_modules.py

# Update website data
python3 ruleset/merge_sync/consolidate_modules.py
```

---

# 中文说明

本目录包含按功能分组的 Surge 和 Shadowrocket 模块。

---

## ⏰ 自动更新时间

| UTC时间 | 北京时间 | 说明 |
|---------|----------|------|
| 20:00 | 04:00 (次日) | 凌晨更新 |
| 04:00 | 12:00 | 中午更新 |

模块通过 GitHub Actions **每日自动更新两次**。

**✅ 合集自动更新**: 所有3个大合集会在每次更新时自动从上游模块重新生成。

---

## 📦 推荐合集

我们提供 **3个大合集**，将多个模块合并为单个文件：

| 合集名称 | 模块数 | 包含内容 |
|----------|--------|----------|
| 🚀 **功能增强大合集** | 23 | BiliBili/iRingo/YouTube/TikTok/DNS/BoxJs |
| 🛡️ **广告拦截大合集** | 11 | AWAvenue/毒奶/可莉/Sukka/广告平台拦截器 |
| 🎯 **App去广告大合集** | 32 | 微信/微博/淘宝/京东/知乎/小红书 |

### 快速导入 (Surge)

```
# 功能增强大合集
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/amplify_nexus/%F0%9F%9A%80%20%E5%8A%9F%E8%83%BD%E5%A2%9E%E5%BC%BA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# 广告拦截大合集
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%9B%A1%EF%B8%8F%20%E5%B9%BF%E5%91%8A%E6%8B%A6%E6%88%AA%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule

# App去广告大合集
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29/head_expanse/%F0%9F%8E%AF%20App%E5%8E%BB%E5%B9%BF%E5%91%8A%E5%A4%A7%E5%90%88%E9%9B%86.sgmodule
```

**Shadowrocket 用户:** 将链接中的 `surge%28main%29` 替换为 `shadowrocket`。

### 🌐 模块导入助手

访问: [surge_module_helper.html](surge_module_helper.html)

- 📦 点击"推荐合集"筛选合集模块
- 🚀 一键复制导入链接
- 📊 追踪安装进度

---

## 目录结构

```
module/
├── surge(main)/           # Surge 模块
│   ├── amplify_nexus/     # 『 🛠️ 增幅枢纽 』
│   │   └── 🚀 功能增强大合集.sgmodule  ⭐ 推荐
│   ├── head_expanse/      # 『 🔝 首端扩域 』
│   │   ├── 🛡️ 广告拦截大合集.sgmodule  ⭐ 推荐
│   │   └── 🎯 App去广告大合集.sgmodule  ⭐ 推荐
│   └── narrow_pierce/     # 『 🎯 窄域穿刺 』
│
└── shadowrocket/          # Shadowrocket 模块 (自动同步)
```

## 模块分组

### 🛠️ 增幅枢纽 (Amplify Nexus)
**用途**: 功能增强和核心功能模块

包含: BiliBili增强、iRingo套件、YouTube增强、TikTok解锁、DNS管理、BoxJS、Sub-Store等

### 🔝 首端扩域 (Head Expanse)
**用途**: 广告拦截平台和综合过滤

包含: AWAvenue、毒奶、可莉广告过滤器、Sukka模块、Script Hub等

### 🎯 窄域穿刺 (Narrow Pierce)
**用途**: App专项去广告

包含: 微信、微博、淘宝、京东、知乎、小红书、BiliBili、YouTube、Spotify等

---

## 管理脚本

```bash
# 合并所有模块为大合集
python3 ruleset/merge_sync/merge_amplify_nexus_modules.py
python3 ruleset/merge_sync/merge_head_expanse_modules.py
python3 ruleset/merge_sync/merge_narrow_pierce_modules.py

# 更新网页数据
python3 ruleset/merge_sync/consolidate_modules.py
```

---

## 更新历史

- **2025-12-10**: 新增3个大合集，每日两次自动更新
- **2024-12-08**: 按分组整理所有模块到子目录
