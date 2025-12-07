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
9. **SRS 生成** - Sing-box 二进制规则
10. **Git 提交** - 自动提交并推送

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
