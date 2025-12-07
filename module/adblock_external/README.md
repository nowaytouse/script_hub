# External AdBlock Modules

This directory contains AdBlock modules downloaded from external sources.

## 📦 Modules

These modules are automatically downloaded by `merge_sync/download_adblock_modules.sh`:

1. **Adblock4limbo.sgmodule** - limbopro's comprehensive ad blocking
2. **广告联盟.official.sgmodule** - QingRex ad network blocking
3. **广告平台拦截器.sgmodule** - QingRex ad platform blocker
4. **可莉广告过滤器.beta.sgmodule** - QingRex Klee ad filter (Beta)
5. **blockHTTPDNS.module** - fmz200 HTTP DNS blocking
6. **AWAvenue-Ads-Rule-Surge-module.sgmodule** - TG-Twilight comprehensive rules

## 🔄 Auto-Update

These modules are automatically updated when you run:

```bash
bash merge_sync/full_update.sh
```

Or manually:

```bash
bash merge_sync/download_adblock_modules.sh
```

## 🔗 Usage

### Option 1: Use GitHub URLs (Recommended)

Reference modules directly from GitHub in your Surge/Shadowrocket config:

```ini
[Module]
# Adblock4limbo
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/adblock_external/Adblock4limbo.sgmodule

# AWAvenue Ads Rule
https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/adblock_external/AWAvenue-Ads-Rule-Surge-module.sgmodule
```

### Option 2: Use Original URLs

You can also reference the original URLs directly:

```ini
[Module]
https://limbopro.com/Adblock4limbo.sgmodule
https://raw.githubusercontent.com/TG-Twilight/AWAvenue-Ads-Rule/main/Filters/AWAvenue-Ads-Rule-Surge-module.sgmodule
```

## 📝 Note

- These modules are **extracted for rules only** in the main AdBlock.list
- You don't need to manually install them
- Rules are automatically merged into `ruleset/Surge(Shadowkroket)/AdBlock.list`

## 🔍 Sources

See `ruleset/Sources/Links/AdBlock_sources.txt` for the complete list of module URLs.

