# External AdBlock Modules (Cleaned Versions)

✅ **These are cleaned, optimized versions** - safe to use without duplicates!

## 📋 What's Different?

These modules have been **automatically cleaned** by removing:
- ❌ **ALL rules** → Extracted to AdBlock.list (including DOMAIN, IP-CIDR, URL-REGEX, PROCESS-NAME, AND/OR logic)
- ❌ Duplicate rules → Deduplicated in AdBlock.list

**What remains:**
- ✅ [URL Rewrite] sections (cannot be in .list files)
- ✅ [MITM] sections
- ✅ [Script] sections
- ✅ Other module-specific features

## 📦 Available Cleaned Modules

1. **可莉广告过滤器.beta.sgmodule** - Extracted 24 rules → Kept [URL Rewrite] + [MITM]
2. **广告平台拦截器.sgmodule** - Extracted 287 rules → Kept [URL Rewrite] + [MITM]
3. **blockHTTPDNS.module** - Extracted 170 rules → Kept [URL Rewrite]

**Removed modules** (only had rules, no other features):
- ~~Adblock4limbo.sgmodule~~ - 0 rules extracted
- ~~广告联盟.official.sgmodule~~ - 0 rules extracted
- ~~AWAvenue-Ads-Rule-Surge-module.sgmodule~~ - 887 rules extracted (all moved to AdBlock.list)

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

Total: **236,835+ deduplicated rules** (including all URL-REGEX, DOMAIN, IP-CIDR, PROCESS-NAME, AND/OR rules)

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

