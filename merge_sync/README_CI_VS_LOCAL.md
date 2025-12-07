# 🔄 CI/CD vs 本地执行对比

## 📋 概述

`full_update.sh` 脚本支持两种运行模式：
1. **本地手动执行** - 完整功能，包括核心更新、配置同步
2. **CI/CD 自动执行** - 仅规则更新，跳过本地专用功能

## 🎯 功能对比表

| 功能 | 本地执行 | CI/CD 执行 | 说明 |
|------|---------|-----------|------|
| **规则集更新** | ✅ | ✅ | 从上游源下载和合并规则 |
| **智能去重** | ✅ | ✅ | 广告 > 细分 > 兜底优先级 |
| **空规则集清理** | ✅ | ✅ | 自动删除空规则集 |
| **端口规则同步** | ✅ | ✅ | 同步到防火墙模块 |
| **广告模块合并** | ✅ | ✅ | 合并广告拦截模块 |
| **SRS 文件生成** | ✅ | ✅ | Sing-box 规则集编译 |
| **Git 操作** | ✅ | ✅ | Pull/Push 到 GitHub |
| **备份** | ✅ | ❌ | CI 环境自动跳过 |
| **核心更新** | ✅ (可选) | ❌ | Sing-box/Mihomo 核心下载 |
| **Singbox 配置下载** | ✅ (可选) | ❌ | 从 Substore 下载配置 |
| **iCloud 模块同步** | ✅ (可选) | ❌ | 同步到 Surge iCloud |
| **Surge 配置同步** | ✅ (可选) | ❌ | 同步到 Surge 配置文件 |

## 🚀 使用方法

### 本地手动执行

#### 1. 标准更新（推荐）
```bash
cd merge_sync
./full_update.sh
```
**执行内容**:
- ✅ 规则集更新和合并
- ✅ 智能去重和清理
- ✅ 端口规则同步
- ✅ SRS 文件生成
- ❌ 不执行 Git 操作
- ❌ 不更新核心
- ❌ 不同步配置

#### 2. 完整更新（含 Git）
```bash
./full_update.sh --full
```
**执行内容**:
- ✅ 所有标准更新功能
- ✅ Git Pull/Push
- ❌ 不更新核心
- ❌ 不同步配置

#### 3. 本地全面更新（含核心和配置）
```bash
./full_update.sh --with-core
```
**执行内容**:
- ✅ 所有标准更新功能
- ✅ 更新 Sing-box 和 Mihomo 核心
- ✅ 下载 Singbox 配置
- ✅ 同步模块到 iCloud
- ✅ 同步 Surge 配置
- ❌ 不执行 Git 操作

#### 4. 最全面更新（所有功能）
```bash
./full_update.sh --full --with-core
```
**执行内容**:
- ✅ 所有功能全部启用
- ✅ Git 操作
- ✅ 核心更新
- ✅ 配置同步

#### 5. 快速更新（仅合并和 SRS）
```bash
./full_update.sh --quick
```
**执行内容**:
- ✅ 规则集合并
- ✅ SRS 文件生成
- ❌ 跳过 MetaCubeX 同步
- ❌ 跳过模块同步
- ❌ 跳过 Git 操作

### CI/CD 自动执行

#### GitHub Actions Workflow
```yaml
- name: Execute Full Update Script (Unattended Mode)
  run: |
    ./merge_sync/full_update.sh --unattended --verbose
  env:
    CI: true
```

**执行内容**:
- ✅ 规则集更新和合并
- ✅ 智能去重和清理
- ✅ 端口规则同步到防火墙模块
- ✅ 广告模块合并
- ✅ SRS 文件生成
- ✅ Git Pull/Push
- ❌ 跳过备份（`CI=true`）
- ❌ 跳过核心更新（`--unattended`）
- ❌ 跳过 Singbox 配置下载（`--unattended`）
- ❌ 跳过 iCloud 同步（`--unattended`）
- ❌ 跳过 Surge 配置同步（`--unattended`）

## 🔧 参数说明

### 模式参数

| 参数 | 说明 | 适用场景 |
|------|------|---------|
| `--full` | 启用 Git 操作 | 需要提交到 GitHub |
| `--with-core` | 更新核心和配置 | 本地全面更新 |
| `--unattended` | 无人值守模式 | CI/CD 自动化 |
| `--ci` | CI 模式（同 `--unattended`） | GitHub Actions |
| `--cron` | 定时任务模式（同 `--unattended`） | Cron Job |
| `--quick` | 快速模式 | 仅需要合并和 SRS |

### 跳过参数

| 参数 | 说明 |
|------|------|
| `--skip-git` | 跳过 Git 操作 |
| `--skip-sync` | 跳过 MetaCubeX 同步 |
| `--skip-merge` | 跳过规则集合并 |
| `--skip-adblock` | 跳过广告模块合并 |
| `--skip-module` | 跳过 iCloud 模块同步 |
| `--skip-profile` | 跳过 Surge 配置同步 |
| `--skip-srs` | 跳过 SRS 文件生成 |

### 其他参数

| 参数 | 说明 |
|------|------|
| `--verbose` | 显示详细输出 |
| `--quiet` | 静默模式（最少输出） |
| `-y, --yes` | 自动确认所有操作 |
| `-h, --help` | 显示帮助 |

## 🔍 环境变量

### CI 环境变量

**作用**: 自动跳过所有备份操作

**设置方法**:
```bash
export CI=true
./full_update.sh
```

**影响的脚本**:
- `ingest_from_surge.sh` - 跳过 Surge 配置备份
- `sync_ports_to_firewall_module.sh` - 跳过防火墙模块备份
- 其他支持 `--no-backup` 的脚本

### GITHUB_TOKEN

**作用**: GitHub API 认证（用于 Git 操作）

**设置方法**:
```yaml
env:
  GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

## 📊 执行流程对比

### 本地执行流程（--full --with-core）

```
1. Git Pull (获取远程更新)
2. 更新 Sing-box 和 Mihomo 核心
3. 同步 MetaCubeX 规则
4. 更新 Sources 文件
5. 增量合并规则
6. 空规则集检查 + 智能去重
   ├─ 检查空规则集
   ├─ 智能去重（广告 > 细分 > 兜底）
   ├─ 更新规则集 Header
   ├─ 清理空规则集
   └─ 🔥 同步端口规则到防火墙模块
7. 广告模块合并
8. 同步模块到 iCloud
9. 同步 Surge 配置
10. 生成 SRS 文件
11. Git Push (提交更新)
```

### CI/CD 执行流程（--unattended）

```
1. Git Pull (获取远程更新)
2. ❌ 跳过核心更新
3. 同步 MetaCubeX 规则
4. 更新 Sources 文件
5. 增量合并规则
6. 空规则集检查 + 智能去重
   ├─ 检查空规则集
   ├─ 智能去重（广告 > 细分 > 兜底）
   ├─ 更新规则集 Header
   ├─ 清理空规则集
   └─ 🔥 同步端口规则到防火墙模块（无备份）
7. 广告模块合并
8. ❌ 跳过 iCloud 同步
9. ❌ 跳过 Surge 配置同步
10. 生成 SRS 文件
11. Git Push (提交更新)
```

## 🚨 注意事项

### 1. CI/CD 环境限制

**不可用的功能**:
- ❌ iCloud 同步（需要 macOS 环境）
- ❌ Surge 配置同步（需要本地 Surge 配置文件）
- ❌ 核心更新（不需要在 CI 环境中更新）

**原因**:
- GitHub Actions 运行在 Ubuntu 容器中
- 无法访问 iCloud Drive
- 无法访问本地 Surge 配置文件

### 2. 备份策略

**本地执行**:
- ✅ 默认创建备份
- ✅ 保留最近 3 个备份
- ✅ 可以使用 `--no-backup` 跳过

**CI/CD 执行**:
- ❌ 自动跳过备份（`CI=true`）
- ✅ Git 历史作为备份
- ✅ 可以通过 Git 回滚

### 3. Git 操作

**本地执行**:
- ❌ 默认不执行 Git 操作
- ✅ 使用 `--full` 启用
- ✅ 适合本地测试

**CI/CD 执行**:
- ✅ 自动执行 Git 操作
- ✅ 自动提交和推送
- ✅ 适合自动化更新

### 4. 核心更新

**本地执行**:
- ✅ 使用 `--with-core` 启用
- ✅ 下载最新 Sing-box 和 Mihomo
- ✅ 下载 Singbox 配置

**CI/CD 执行**:
- ❌ 始终跳过
- ❌ 不需要在 CI 环境中更新核心
- ✅ 本地手动更新即可

## 📚 相关文档

- [一键更新脚本说明](README_UPDATE.md) - full_update.sh 详细文档
- [防火墙规则同步](README_FIREWALL_SYNC.md) - 端口规则同步说明
- [GitHub Actions Workflow](.github/workflows/update_rulesets.yml) - CI/CD 配置

## 🎉 总结

### 本地执行特点
- ✅ 完整功能
- ✅ 支持核心更新
- ✅ 支持配置同步
- ✅ 创建备份
- ✅ 灵活控制

### CI/CD 执行特点
- ✅ 自动化
- ✅ 定时运行
- ✅ 无需人工干预
- ❌ 功能受限（仅规则更新）
- ❌ 不创建备份（依赖 Git）

**推荐使用**:
- 日常更新: `./full_update.sh`
- 提交 GitHub: `./full_update.sh --full`
- 全面更新: `./full_update.sh --full --with-core`
- 自动化: GitHub Actions（自动运行）

---

**最后更新**: 2024-12-07
**维护者**: nyamiiko
