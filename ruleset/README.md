# Ruleset Collection - 规则集合集

Auto-generated and maintained proxy rulesets for Surge/Shadowrocket/Clash/Quantumult X.

## 📊 Ruleset Summary

| Ruleset | Rules | Policy Group | Purpose |
|---------|-------|--------------|---------|
| **AI.list** | 51 | 🤖AI平台🤖 | AI platforms (OpenAI, Claude, etc.) |
| **Telegram.list** | 63 | ☎️telegram✈️ | Telegram messaging |
| **TikTok.list** | 132 | 📱 TikTok 🧠 | TikTok/ByteDance |
| **Google.list** | 93 | ▶️ YouTube 🔴 | Google/YouTube services |
| **Apple.list** | 318 | 🍎 Apple 🍏 | Apple services |
| **Bilibili.list** | 65 | 📺 哔哩哔哩 📱 | Bilibili streaming |
| **Gaming.list** | 641 | 🎮 游戏平台 💻 | Steam/Sony/EA/Nintendo |
| **GlobalMedia.list** | 335 | 🌍 海外通用 🌍 | Netflix/Disney+/HBO/Emby |
| **GlobalProxy.list** | 7966 | 🌍 海外通用 🌍 | General overseas proxy |
| **Fediverse.list** | 60 | 🌍 海外通用 🌍 | Mastodon/Bluesky/Misskey |
| **Discord.list** | 55 | 🌍 海外通用 🌍 | Discord communication |
| **CDN.list** | 56 | 🌍 海外通用 🌍 | CDN services |
| **Microsoft.list** | 183 | 🗺️ 直连通用 🌏 | Microsoft/Bing |
| **ChinaDirect.list** | 1509 | 🗺️ 直连通用 🌏 | China direct domains |
| **ChinaIP.list** | 22636 | 🗺️ 直连通用 🌏 | China IP ranges |
| **LAN.list** | 190 | 🗺️ 直连通用 🌏 | Local network |
| **NSFW.list** | 738 | 🇯🇵 JP 🇯🇵 | Adult content |
| **StreamJP.list** | 82 | 🇯🇵 JP 🇯🇵 | Japan streaming |
| **StreamUS.list** | 84 | 🇺🇸 美国 🇺🇸 | US streaming |
| **StreamHK.list** | 70 | 🇭🇰 香港 🇭🇰 | Hong Kong streaming |
| **StreamTW.list** | 70 | 🇹🇼 台湾 🇹🇼 | Taiwan streaming |
| **StreamKR.list** | 35 | 🇰🇷 韩国 🇰🇷 | Korea streaming |
| **StreamEU.list** | 44 | 🇬🇧英国专线🧱 | Europe streaming |

## 🎯 Design Principles

### 1. Policy Group Based Merging
Rules are merged based on their **policy group** (routing strategy), not just by service type:
- Same policy group + Same purpose = Can merge
- Same policy group + Different purpose = Keep separate (e.g., Fediverse vs general overseas)

### 2. Purpose-Specific Separation
Some services are kept separate even with same policy group:
- **Fediverse.list** - Decentralized social networks (Mastodon, Bluesky, etc.)
- **Discord.list** - Voice/text communication platform
- **NSFW.list** - Adult content (may need different region routing)

### 3. Regional Streaming
Streaming services are separated by region for geo-restriction bypass:
- StreamJP, StreamUS, StreamHK, StreamTW, StreamKR, StreamEU

## 📁 File Structure

```
ruleset/
├── *.list              # Generated rulesets
├── *_sources.txt       # Source URLs for each ruleset
├── *_manual.txt        # Manual rules (auto-preserved)
├── ruleset_merger.sh   # Merger script
└── README.md           # This file
```

## 🔧 Usage

### Surge Configuration
```ini
[Rule]
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/AI.list,🤖AI平台🤖
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Telegram.list,☎️telegram✈️
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/GlobalMedia.list,🌍 海外通用 🌍
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/ChinaDirect.list,🗺️ 直连通用 🌏
```

### Manual Rules
Create `<name>_manual.txt` to add persistent rules:
```bash
# Example: Telegram_manual.txt
DOMAIN-SUFFIX,my-custom-telegram-bot.com
IP-CIDR,1.2.3.4/32,no-resolve
```

### Update Rulesets
```bash
# Update single ruleset
./ruleset_merger.sh -t base.list -l AI_sources.txt -o AI.list -n "AI" -g

# Setup daily auto-update
./ruleset_merger.sh --cron
```

## 📝 Changelog

### 2025-12-03
- Added GlobalMedia.list (Netflix/Disney+/HBO/Emby merged)
- Added GlobalProxy.list (GFW/Proxy lists merged)
- Added ChinaDirect.list (China domains merged)
- Added ChinaIP.list (China IP ranges merged)
- Added Fediverse.list (Mastodon/Bluesky/Misskey)
- Added Discord.list, CDN.list, LAN.list, Microsoft.list
- Improved policy-group based merging logic

## 📜 License

MIT License - Feel free to use and modify.
