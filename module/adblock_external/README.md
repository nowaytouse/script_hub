# External AdBlock Modules (Cleaned Versions)

✅ **These are cleaned, optimized versions** - safe to use!

## 📋 What's Different?

These modules have been **automatically cleaned** by removing:
- ❌ Basic DOMAIN rules → Moved to AdBlock.list
- ❌ Basic IP-CIDR rules → Moved to AdBlock.list
- ❌ Duplicate rules → Deduplicated

**What remains:**
- ✅ URL-REGEX rules (cannot be in .list files)
- ✅ Script rules
- ✅ Complex logic rules (AND/OR combinations)
- ✅ Module-specific features

## 📦 Available Cleaned Modules

1. **可莉广告过滤器.beta.sgmodule** - 24 → 3 rules (URL-REGEX) ✅ Format verified
2. **广告平台拦截器.sgmodule** - 283 → 1 rule (URL-REGEX + [URL Rewrite] + [MITM]) ✅ Format verified
3. **blockHTTPDNS.module** - 169 → 45 rules (URL-REGEX + complex logic) ✅ Format verified

**Removed modules** (all rules extracted to AdBlock.list):
- ~~Adblock4limbo.sgmodule~~ - 0 unique rules
- ~~广告联盟.official.sgmodule~~ - 0 unique rules
- ~~AWAvenue-Ads-Rule-Surge-module.sgmodule~~ - 887 rules (all basic DOMAIN rules)

## ✅ What You Should Use Instead

Use the **merged AdBlock.list** in your Surge/Shadowrocket config:

```ini
[Rule]
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/AdBlock.list,REJECT
```

This single file contains **all rules** from:
- blackmatrix7 Advertising
- ACL4SSR BanAD
- SukkaW reject
- MetaCubeX category-ads-all
- **All 6 external modules in this directory**

Total: **236,830+ deduplicated rules**

## 📦 Downloaded Modules (For Reference)

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

The update process:
1. Downloads latest versions
2. Extracts rules
3. Merges into AdBlock.list
4. Deduplicates
5. Commits to Git

## 🔍 Sources

See `ruleset/Sources/Links/AdBlock_sources.txt` for the complete list of module URLs.

## 📝 Note

If you want to use the **original modules** directly (not recommended due to duplicates), use the original URLs:

```ini
[Module]
https://limbopro.com/Adblock4limbo.sgmodule
https://raw.githubusercontent.com/TG-Twilight/AWAvenue-Ads-Rule/main/Filters/AWAvenue-Ads-Rule-Surge-module.sgmodule
```

But remember: **This will cause duplicate rules** since these are already in AdBlock.list!

