# 一键更新脚本使用指南

## 📋 功能概览

`full_update.sh` 是一个全面的自动化更新脚本，支持：

1. **Git 同步** - Pull 远程更新 & Push 本地更改
2. **核心更新** - Sing-box & Mihomo 最新版本
3. **规则同步** - MetaCubeX 规则库
4. **Sources 更新** - 规则源文件
5. **增量合并** - 智能去重合并
6. **广告拦截** - AdBlock 模块合并
7. **模块同步** - iCloud Surge/Shadowrocket
8. **配置同步** - Surge 配置文件 (注释关键词智能分类)
9. **SRS 生成** - Sing-box 二进制规则 (61个规则集 100%覆盖)
10. **Git 提交** - 自动提交并推送

## 📊 当前统计 (2025-12-07更新)

- **MetaCubeX规则**: 24个
- **Surge规则**: 61个
- **Sing-box SRS**: 61个 (100%覆盖率 ✅)
- **Sources文件**: 53个
- **Surge模块**: 6个
- **AdBlock规则**: 235,516条
- **Shadowrocket规则**: 71条

## 🚀 使用场景

### 场景1: 本地日常更新（推荐）

```bash
# 标准更新 (规则+SRS)
./merge_sync/full_update.sh

# 全面更新 (规则+核心+配置)
./merge_sync/full_update.sh --with-core

# 最全面更新 (Git+规则+核心+配置)
./merge_sync/full_update.sh --full --with-core
```

**包含步骤**:
- ✅ 核心更新 (Sing-box & Mihomo)
- ✅ 规则同步和合并
- ✅ Surge 配置同步 (吸取用户规则)
- ✅ 模块同步到 iCloud
- ✅ SRS 生成

### 场景2: CI/CD 自动化

```bash
# GitHub Actions / 定时任务
./merge_sync/full_update.sh --unattended
```

**自动跳过**:
- ❌ 核心更新 (CI 环境不需要)
- ❌ Surge 配置同步 (无 iCloud 访问)
- ❌ 模块同步 (无 iCloud 访问)

**自动启用**:
- ✅ Git Pull & Push
- ✅ 静默模式
- ✅ 自动确认

### 场景3: 快速更新

```bash
# 仅合并规则和生成 SRS
./merge_sync/full_update.sh --quick
```

**跳过**:
- ❌ Git 操作
- ❌ MetaCubeX 同步
- ❌ 模块同步

## 📖 参数说明

### 核心参数

| 参数 | 说明 | 默认 | 推荐场景 |
|------|------|------|----------|
| `--with-core` | 更新 Sing-box & Mihomo 核心 | 关闭 | 本地使用 |
| `--with-git` | 启用 Git pull/push | 关闭 | 手动同步 |
| `--full` | 完整模式 (含 Git) | - | 本地全面更新 |
| `--unattended` | 无人值守模式 (CI/CD) | - | GitHub Actions |

### 跳过选项

| 参数 | 说明 |
|------|------|
| `--skip-git` | 跳过 Git 操作 |
| `--skip-sync` | 跳过 MetaCubeX 同步 |
| `--skip-merge` | 跳过增量合并 |
| `--skip-adblock` | 跳过广告模块合并 |
| `--skip-module` | 跳过模块同步到 iCloud |
| `--skip-profile` | 跳过 Surge 配置同步 |
| `--skip-srs` | 跳过 SRS 生成 |

### 其他选项

| 参数 | 说明 |
|------|------|
| `--verbose` | 显示详细输出 |
| `--quiet` | 静默模式 (最少输出) |
| `-y, --yes` | 自动确认所有操作 |
| `--quick` | 快速模式 (跳过同步、模块和 Git) |

## 🔧 核心更新功能

### 独立使用

```bash
# 更新所有核心
./merge_sync/update_cores.sh

# 仅更新 Sing-box
./merge_sync/update_cores.sh --singbox-only

# 仅更新 Mihomo
./merge_sync/update_cores.sh --mihomo-only

# 仅检查版本
./merge_sync/update_cores.sh --check-only
```

### 功能特性

- ✅ 自动检测系统架构 (amd64/arm64/armv7)
- ✅ 自动检测操作系统 (macOS/Linux/Windows)
- ✅ 从 GitHub 下载最新版本
- ✅ 智能备份旧版本 (保留最近 3 个)
- ✅ 自动设置可执行权限
- ✅ 版本检查和对比

### 安装路径

- **Sing-box**: `/usr/local/bin/sing-box`
- **Mihomo**: `/usr/local/bin/mihomo`
- **备份**: `~/.local/share/proxy-cores/backup/`

## 📝 Surge 配置同步

### 注释关键词智能分类

在 Surge 配置文件的 `[Rule]` 区域添加规则，用注释指定分类：

```
[Rule]
DOMAIN-SUFFIX,adult-site.com,Proxy // NSFW
DOMAIN-SUFFIX,openai.com,Proxy // AI
DOMAIN-SUFFIX,steam.com,DIRECT // Gaming
# ============ 以上为新增 ============
...自动同步的规则...
```

### 支持的关键词

| 关键词 | 目标规则集 |
|--------|-----------|
| `NSFW`, `R18`, `Adult`, `成人` | NSFW.list |
| `AI`, `OpenAI`, `Claude`, `ChatGPT` | AI.list |
| `Gaming`, `游戏`, `Steam`, `Epic` | Gaming.list |
| `Netflix`, `奈飞` | Netflix.list |
| `Spotify`, `声田` | Spotify.list |
| `YouTube`, `油管` | YouTube.list |
| `Telegram`, `电报`, `TG` | Telegram.list |
| `Google`, `谷歌` | Google.list |
| `Direct`, `直连`, `国内` | Manual.list |
| `Proxy`, `代理`, `海外` | GlobalProxy.list |
| `AdBlock`, `广告`, `Block` | AdBlock.list |

## 🔄 完整工作流程

### 本地全面更新

```bash
./merge_sync/full_update.sh --full --with-core
```

执行步骤：
1. Git Pull (获取远程更新)
2. 更新 Sing-box & Mihomo 核心
3. 同步 MetaCubeX 规则
4. 更新 Sources 文件
5. 增量合并规则
6. 广告模块合并
7. 模块同步到 iCloud
8. Surge 配置同步 (吸取用户规则)
9. 生成 SRS 文件
10. Git Commit & Push

### CI/CD 自动化

```bash
./merge_sync/full_update.sh --unattended
```

执行步骤：
1. Git Pull
2. ~~核心更新~~ (跳过)
3. 同步 MetaCubeX 规则
4. 更新 Sources 文件
5. 增量合并规则
6. 广告模块合并
7. ~~模块同步~~ (跳过)
8. ~~配置同步~~ (跳过)
9. 生成 SRS 文件
10. Git Commit & Push

## ⚙️ 配置文件

### GitHub Actions

`.github/workflows/update_rulesets.yml`:

```yaml
- name: Execute Full Update Script (Unattended Mode)
  run: |
    chmod +x merge_sync/*.sh
    ./merge_sync/full_update.sh --unattended --verbose
```

### Cron 定时任务

```bash
# 每天凌晨 4 点更新
0 4 * * * cd /path/to/script_hub && ./merge_sync/full_update.sh --unattended
```

## 🐛 故障排查

### 核心更新失败

```bash
# 检查权限
sudo -v

# 手动更新
./merge_sync/update_cores.sh --verbose

# 检查版本
sing-box version
mihomo -v
```

### Git 冲突

```bash
# 查看状态
git status

# 手动解决冲突
git stash
git pull
git stash pop
```

### 规则合并失败

```bash
# 查看详细日志
./merge_sync/full_update.sh --verbose

# 单独运行合并
./merge_sync/incremental_merge_all.sh
```

## 📊 统计信息

更新完成后会显示：

- MetaCubeX 规则数量
- Surge 规则数量
- SingBox SRS 数量
- Sources 文件数量
- AdBlock 规则数量

## 🔗 相关文档

- [Sing-box 官方文档](https://sing-box.sagernet.org/)
- [Mihomo 官方文档](https://wiki.metacubex.one/)
- [Surge 官方文档](https://manual.nssurge.com/)


---

## 📦 Sing-box 规则集详细说明

### 完整规则集列表 (61个)

#### 🛡️ 广告拦截 (3个)
- `surge-adblock` - 基础广告拦截
- `surge-adblock-merged` - 合并广告拦截 (235,516条规则)
- `surge-blockhttpdns` - 阻止HTTP DNS劫持

#### 🤖 AI服务 (2个)
- `surge-ai` - AI服务 (OpenAI, Anthropic, Google Gemini等)
- `surge-aiprocess` - AI进程规则

#### 💬 社交媒体 (7个)
- `surge-telegram` - Telegram
- `surge-tiktok` - TikTok
- `surge-twitter` - Twitter/X
- `surge-instagram` - Instagram
- `surge-reddit` - Reddit
- `surge-discord` - Discord
- `surge-socialmedia` - 社交媒体通用

#### 🎬 流媒体 (11个)
- `surge-netflix` - Netflix
- `surge-disney` - Disney+
- `surge-youtube` - YouTube
- `surge-spotify` - Spotify
- `surge-globalmedia` - 全球流媒体
- `surge-bahamut` - 巴哈姆特动画疯
- `surge-streameu` - 欧洲流媒体
- `surge-streamhk` - 香港流媒体
- `surge-streamjp` - 日本流媒体
- `surge-streamkr` - 韩国流媒体
- `surge-streamtw` - 台湾流媒体
- `surge-streamus` - 美国流媒体

#### 🏢 科技公司 (7个)
- `surge-apple` - Apple服务
- `surge-applenews` - Apple News
- `surge-google` - Google服务
- `surge-googlecn` - Google中国
- `surge-microsoft` - Microsoft
- `surge-bing` - Bing搜索
- `surge-github` - GitHub

#### 🎮 游戏 (5个)
- `surge-gaming` - 游戏平台通用
- `surge-gamingprocess` - 游戏进程规则
- `surge-steam` - Steam
- `surge-epic` - Epic Games
- `surge-speedtest` - Speedtest测速

#### 💰 金融 (2个)
- `surge-paypal` - PayPal
- `surge-binance` - Binance币安

#### 🇨🇳 国内服务 (9个)
- `surge-bilibili` - 哔哩哔哩
- `surge-chinadirect` - 中国直连
- `surge-chinaip` - 中国IP段
- `surge-qq` - QQ
- `surge-wechat` - 微信
- `surge-tencent` - 腾讯服务
- `surge-xiaohongshu` - 小红书
- `surge-neteasemusic` - 网易云音乐
- `surge-tesla` - 特斯拉

#### 🌐 网络基础 (4个)
- `surge-lan` - 局域网
- `surge-cdn` - CDN服务
- `surge-firewallports` - 防火墙端口
- `surge-downloadprocess` - 下载进程

#### 🌍 全球代理 (2个)
- `surge-globalproxy` - 全球代理
- `surge-fediverse` - 联邦宇宙 (Mastodon, Pixelfed等)

#### ✋ 手动规则 (6个)
- `surge-manual` - 手动规则
- `surge-manual-global` - 手动全球规则
- `surge-manual-jp` - 手动日本规则
- `surge-manual-us` - 手动美国规则
- `surge-manual-west` - 手动西方规则
- `surge-directprocess` - 直连进程

#### 🔞 特殊分类 (2个)
- `surge-nsfw` - NSFW内容
- `surge-substore` - SubStore订阅管理

### 规则集配置

所有规则集使用以下配置：

```json
{
  "tag": "surge-xxx",
  "type": "remote",
  "format": "binary",
  "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/XXX_Singbox.srs",
  "download_detour": "direct-select",
  "update_interval": "24h"
}
```

### 更新规则集

规则集通过 `sync_all_configs.sh` 脚本自动同步：

```bash
# 单独运行配置同步
./merge_sync/sync_all_configs.sh

# 或通过完整更新脚本
./merge_sync/full_update.sh
```

### 验证规则集

```bash
# 检查Sing-box配置中的规则集数量
python3 -c "import json; data=json.load(open('substore/Singbox_substore_1.13.0+.json')); print(f'规则集数量: {len(data[\"route\"][\"rule_set\"])}')"

# 列出所有规则集标签
python3 -c "import json; data=json.load(open('substore/Singbox_substore_1.13.0+.json')); tags=[rs['tag'] for rs in data['route']['rule_set']]; print('\n'.join(sorted(tags)))"

# 检查SRS文件覆盖率
ls ruleset/SingBox/*.srs | wc -l
```

### 规则集优先级

规则集在 `route.rules` 中按以下优先级匹配：

1. **广告拦截** - 最高优先级，阻止广告和追踪
2. **手动规则** - 用户自定义规则
3. **特定服务** - AI、社交媒体、流媒体等
4. **地区规则** - 国内/国外分流
5. **默认规则** - 兜底规则

### 注意事项

- 所有规则集每24小时自动更新
- 使用SRS二进制格式，加载速度快
- 规则集通过GitHub raw链接下载
- 建议定期运行 `full_update.sh` 保持规则最新
