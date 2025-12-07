# Singbox 规则集映射文档

## 概述

本文档说明如何将Singbox配置中引用的外部规则集映射到项目本地已有的规则集。

## 映射原则

1. **优先使用本地规则集** - 项目已经维护了完整的规则集，无需依赖外部源
2. **避免重复** - 不创建功能重复的规则集
3. **语义对应** - 将外部规则集映射到功能相同的本地规则集

## 规则集映射表

### 1. 广告拦截类

| 外部标签 | 本地规则集 | 说明 |
|---------|-----------|------|
| `adblock-merged` | `AdBlock` | 广告拦截已合并 |
| `geosite-advertising` | `AdBlock` | geosite广告规则 |
| `geosite-hijacking` | `AdBlock` | 劫持检测规则 |

### 2. 中国直连类

| 外部标签 | 本地规则集 | 说明 |
|---------|-----------|------|
| `cnsite` | `ChinaDirect` | 中国站点已合并 |
| `geosite-cn` | `ChinaDirect` | geosite中国站点 |
| `cnip` | `ChinaIP` | 中国IP段（已单独定义） |

### 3. 全球代理类

| 外部标签 | 本地规则集 | 说明 |
|---------|-----------|------|
| `gfw` | `GlobalProxy` | GFW列表已合并 |
| `geosite-geolocation-!cn` | `GlobalProxy` | 非中国站点 |

### 4. 游戏类

| 外部标签 | 本地规则集 | 说明 |
|---------|-----------|------|
| `cngames` | `Gaming` | 中国游戏已合并 |

### 5. AI服务类

| 外部标签 | 本地规则集 | 说明 |
|---------|-----------|------|
| `geosite-openai` | `AI` | OpenAI服务 |
| `geosite-anthropic` | `AI` | Anthropic服务 |
| `geosite-google-gemini` | `AI` | Google Gemini |

### 6. 流媒体类

| 外部标签 | 本地规则集 | 说明 |
|---------|-----------|------|
| `geosite-disney` | `Disney` | Disney服务 |

### 7. 网络基础类

| 外部标签 | 本地规则集 | 说明 |
|---------|-----------|------|
| `geosite-private` | `LAN` | 私有网络 |
| `sukka-cdn` | `CDN` | CDN服务 |
| `sukka-speedtest` | `Speedtest` | 测速服务 |

### 8. Apple类

| 外部标签 | 本地规则集 | 说明 |
|---------|-----------|------|
| `sukka-apple-cdn` | `Apple` | Apple CDN |

### 9. GeoIP类（临时映射）

| 外部标签 | 本地规则集 | 说明 |
|---------|-----------|------|
| `geoip-jp` | `ChinaIP` | 日本IP（暂用ChinaIP） |
| `geoip-us` | `ChinaIP` | 美国IP（暂用ChinaIP） |
| `geoip-kr` | `ChinaIP` | 韩国IP（暂用ChinaIP） |

**注意**: GeoIP规则集目前暂时映射到ChinaIP，实际使用时可能需要创建专门的国家IP规则集。

### 10. Manual规则集

| 外部标签 | 本地规则集 | 说明 |
|---------|-----------|------|
| `surge-manual_global` | `Manual_Global` | 全球手动规则 |
| `surge-manual_jp` | `Manual_JP` | 日本手动规则 |
| `surge-manual_us` | `Manual_US` | 美国手动规则 |
| `surge-manual_west` | `Manual_West` | 西方手动规则 |

## 本地规则集列表

项目维护的完整规则集（62个）：

```
AdBlock, AI, AIProcess, Apple, AppleNews, Bahamut, Bilibili, Binance, 
Bing, BlockHttpDNS, CDN, ChinaDirect, ChinaIP, DirectProcess, Discord, 
Disney, DownloadProcess, Epic, Fediverse, FirewallPorts, Gaming, 
GamingProcess, GitHub, GlobalMedia, GlobalProxy, Google, GoogleCN, 
Instagram, LAN, Manual, Manual_Global, Manual_JP, Manual_US, Manual_West, 
Microsoft, NSFW, NetEaseMusic, Netflix, PayPal, QQ, Reddit, SocialMedia, 
Speedtest, Spotify, Steam, StreamEU, StreamHK, StreamJP, StreamKR, 
StreamTW, StreamUS, Telegram, Tencent, Tesla, TikTok, Twitter, WeChat, 
XiaoHongShu, YouTube, substore
```

## 规则集URL格式

所有本地规则集使用统一的URL格式：

```
https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/{RulesetName}_Singbox.srs
```

例如：
- AdBlock: `https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/AdBlock_Singbox.srs`
- ChinaDirect: `https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/ChinaDirect_Singbox.srs`

## 使用方法

### 自动添加映射

运行脚本自动添加所有缺失规则集的映射：

```bash
./merge_sync/add_missing_rulesets.sh
```

### 手动添加单个映射

在Singbox配置的`route.rule_set`数组中添加：

```json
{
  "tag": "外部标签名",
  "type": "remote",
  "format": "binary",
  "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/本地规则集名_Singbox.srs",
  "download_detour": "direct-select",
  "update_interval": "24h"
}
```

## 验证映射

运行验证脚本检查所有规则集是否正确配置：

```bash
./merge_sync/validate_singbox_config.sh
```

## 注意事项

1. **不要重复创建规则集** - 如果本地已有功能相同的规则集，直接映射即可
2. **保持同步** - 本地规则集通过`full_update.sh`定期更新
3. **GeoIP限制** - 目前GeoIP规则集映射到ChinaIP，如需精确的国家IP规则，需要单独添加
4. **测试验证** - 添加映射后务必测试Singbox是否能正常启动

## 相关文档

- `SINGBOX_CNIP_FIX.md` - cnip规则集修复文档
- `TASK_11_SUMMARY.md` - 任务11总结
- `validate_singbox_config.sh` - 配置验证脚本
- `add_missing_rulesets.sh` - 自动添加规则集脚本

## 更新日期

2025-12-07
