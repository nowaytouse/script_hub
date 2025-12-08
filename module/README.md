# Module Organization

This directory contains Surge and Shadowrocket modules organized by their functional groups.

## Directory Structure

```
module/
├── surge(main)/           # Surge modules (main)
│   ├── amplify_nexus/     # 『 🛠️ Amplify Nexus › 增幅枢纽 』 (21 modules)
│   ├── head_expanse/      # 『 🔝 Head Expanse › 首端扩域 』 (13 modules)
│   ├── narrow_pierce/     # 『 🎯 Narrow Pierce › 窄域穿刺 』 (26 modules)
│   └── [6 ungrouped modules in root]
│
└── shadowrocket/          # Shadowrocket modules (synced from surge)
    ├── amplify_nexus/     # Same structure as surge(main)
    ├── head_expanse/
    ├── narrow_pierce/
    └── [6 ungrouped modules in root]
```

## Module Groups

### 🛠️ Amplify Nexus › 增幅枢纽 (21 modules)
**Purpose**: Enhancement features and core functionality modules

Includes:
- BiliBili Enhanced/Global/Redirect
- DNS management
- DualSubs Universal
- iRingo suite (Location, Maps, News, TV, WeatherKit)
- Sub-Store
- TikTok Unlock
- YouTube Enhance
- BoxJS
- Network info panels
- And more...

### 🔝 Head Expanse › 首端扩域 (13 modules)
**Purpose**: Ad blocking platforms and comprehensive ad filtering

Includes:
- Adblock4limbo
- AWAvenue Ads Rule
- AllInOne Mock
- Sukka's ad blocking modules
- Script Hub
- 可莉广告过滤器
- 广告联盟/广告平台拦截器
- 小程序和应用懒人去广告合集
- 新手友好の去广告集合
- And more...

### 🎯 Narrow Pierce › 窄域穿刺 (26 modules)
**Purpose**: App-specific ad blocking modules

Includes:
- 12306去广告
- BiliBili ADBlock/Helper
- IT之家去广告
- Jump去广告
- QQ音乐去广告
- Reddit去广告
- Spotify去广告
- WeChat Enhance
- Weibo去广告
- YouTube去广告
- 京东去广告
- 哔哩哔哩漫画去广告
- 夸克去广告
- 小宇宙去广告
- 小红书去广告
- 拼多多去广告
- 淘宝去广告
- 滴滴出行去广告
- 百度网盘去广告
- 知乎去广告
- 菜鸟去广告
- 闲鱼去广告
- 阿里云盘去广告
- 高德地图去广告
- And more...

## Ungrouped Modules (6 modules)

These modules don't have a `#!group=` metadata tag and remain in the root directory:

1. `Encrypted DNS Module 🔒🛡️DNS.sgmodule`
2. `URL Rewrite Module 🔄🌐.sgmodule`
3. `__Extracted_YouTube_remove_ads.sgmodule`
4. `🔥 Firewall Port Blocker 🛡️🚫.sgmodule`
5. `🚀💪General Enhanced⬆️⬆️ plus.sgmodule`
6. `🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule`

## Management Scripts

### Download Modules
```bash
./merge_sync/download_modules.sh --url "URL" --group "GROUP_NAME"
```

Downloads modules from URLs and assigns them to a specific group.

### Organize Modules
```bash
./merge_sync/organize_modules_by_group.sh
```

Organizes modules into subfolders based on their `#!group=` metadata.

### Sync to Shadowrocket
```bash
./merge_sync/sync_organized_modules.sh
```

Syncs the organized module structure from surge(main) to shadowrocket, preserving the subdirectory organization.

## Module Metadata

Each module should include a `#!group=` metadata tag to specify its group:

```
#!name=Module Name
#!group=『 🛠️ Amplify Nexus › 增幅枢纽 』
#!desc=Module description
```

Valid group names:
- `『 🛠️ Amplify Nexus › 增幅枢纽 』`
- `『 🔝 Head Expanse › 首端扩域 』`
- `『 🎯 Narrow Pierce › 窄域穿刺 』`

## Notes

- **Shadowrocket modules**: The `#!group=` metadata is commented out in Shadowrocket modules as it's not officially supported, but kept for organizational reference.
- **iCloud module**: The `iCloud_Private_Relay_Gateway.sgmodule` was previously ignored by `.gitignore` due to the `*private*` pattern. It has been force-added to git using `git add -f`.
- **Total modules**: 66 modules (60 organized + 6 ungrouped)

## History

- **2024-12-08**: Organized all modules by group into subfolders
- **2024-12-08**: Created organization and sync scripts
- **2024-12-08**: Force-added iCloud_Private_Relay_Gateway.sgmodule to git
