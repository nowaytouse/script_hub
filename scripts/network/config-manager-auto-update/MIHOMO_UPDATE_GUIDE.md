# Mihomo 核心更新功能指南

## 概述

config-manager 现已支持 **Mihomo (原 Clash.Meta)** 核心的自动更新功能，与 sing-box 核心更新并行工作。

## 主要特性

### ✅ 已修复的问题

1. **正确的二进制文件名处理**
   - 自动识别 `mihomo` 或 `mihomo.exe` (Windows)
   - 不再使用错误的文件名

2. **完整的旧版本替换**
   - 自动备份现有文件为 `.bak`
   - 安全替换旧版本

3. **自动权限设置**
   - Unix/Linux/macOS: 自动设置 `0o755` 可执行权限
   - Windows: 保持系统默认权限

4. **多路径同时更新**
   - 支持同时更新多个安装位置
   - 例如：v2rayN 目录 + `/usr/local/bin/mihomo`

## 配置说明

### 配置文件格式 (config.json)

```json
{
  "subscriptions": [...],
  "update_interval_hours": 24,
  "singbox_core_update": {...},
  "mihomo_core_update": {
    "enabled": true,
    "check_prerelease": false,
    "install_paths": [
      "/Users/nyamiiko/Library/Application Support/v2rayN/bin/mihomo",
      "/usr/local/bin/mihomo"
    ]
  }
}
```

### 配置项说明

- **enabled**: 是否启用 mihomo 自动更新
- **check_prerelease**: 是否检查预发布版本（alpha/beta/rc）
- **install_paths**: 安装路径列表（支持多个路径）

### 默认路径

- **macOS/Linux**: `/usr/local/bin/mihomo`
- **Windows**: `C:\Program Files\mihomo\mihomo.exe`

## 使用方法

### 1. 命令行模式

#### 一次性更新所有内容
```bash
./singbox-manager --once
```

#### 定时自动更新
```bash
./singbox-manager
```

### 2. 交互式模式

```bash
./singbox-manager --interactive
```

在交互式菜单中：
- 选择 `[5] 更新 Mihomo 核心` - 单独更新 mihomo
- 选择 `[6] 执行所有更新` - 更新订阅 + sing-box + mihomo
- 选择 `[7] 配置设置` → `[3] 配置 Mihomo 核心更新` - 配置 mihomo 更新

### 3. 配置 Mihomo 更新

在交互式模式中：

1. 进入 `配置设置` → `配置 Mihomo 核心更新`
2. 选择是否启用自动更新
3. 选择是否检查预发布版本
4. 管理安装路径：
   - 添加路径：输入完整路径
   - 删除路径：从列表中选择
   - 完成配置：保存设置

## 更新流程

### Mihomo 更新步骤

1. **检查最新版本**
   - 稳定版：通过 GitHub redirect 获取
   - 预发布版：通过 GitHub API 获取

2. **下载二进制文件**
   - 自动识别平台：darwin/linux/windows
   - 自动识别架构：amd64/arm64/386
   - 下载格式：`mihomo-{os}-{arch}-{version}.gz`

3. **解压缩**
   - 解压 `.gz` 文件（单文件压缩）
   - 提取到临时目录

4. **安装到所有配置路径**
   - 备份现有文件（如果存在）
   - 复制新文件到目标位置
   - 设置可执行权限（Unix/Linux/macOS）

5. **清理临时文件**
   - 删除临时目录

## 示例输出

```
🔄 Updating mihomo core (direct download)...
✅ Found latest version: v1.18.10
📥 Downloading mihomo from: https://github.com/MetaCubeX/mihomo/releases/download/v1.18.10/mihomo-darwin-arm64-v1.18.10.gz
📦 Extracting to: /var/folders/.../mihomo-xxxxx
✅ Extracted mihomo to /var/folders/.../mihomo-xxxxx/mihomo

📍 Installing to: /Users/nyamiiko/Library/Application Support/v2rayN/bin/mihomo
💾 Backing up existing file to /Users/nyamiiko/Library/Application Support/v2rayN/bin/mihomo.bak
📦 Installing /var/folders/.../mihomo to /Users/nyamiiko/Library/Application Support/v2rayN/bin/mihomo
✅ Installation successful! mihomo updated to /Users/nyamiiko/Library/Application Support/v2rayN/bin/mihomo

📍 Installing to: /usr/local/bin/mihomo
💾 Backing up existing file to /usr/local/bin/mihomo.bak
📦 Installing /var/folders/.../mihomo to /usr/local/bin/mihomo
✅ Installation successful! mihomo updated to /usr/local/bin/mihomo

✅ mihomo core update complete for all paths
```

## 故障排除

### 权限问题

如果更新 `/usr/local/bin/mihomo` 时遇到权限错误：

```bash
# 方法 1: 使用 sudo 运行
sudo ./singbox-manager --once

# 方法 2: 修改目录权限
sudo chown -R $USER /usr/local/bin
```

### 下载失败

如果下载失败，检查：
1. 网络连接
2. GitHub 访问是否正常
3. 代理设置（如需要）

### 版本检测失败

如果无法检测最新版本：
1. 检查 GitHub API 访问
2. 尝试禁用 `check_prerelease`
3. 手动指定版本（未来功能）

## 技术细节

### 下载源

- **GitHub Release**: https://github.com/MetaCubeX/mihomo/releases
- **文件格式**: `.gz` (单文件压缩，不是 tar.gz)
- **命名规则**: `mihomo-{os}-{arch}-{version}.gz`

### 平台支持

| 平台 | OS 标识 | 架构支持 |
|------|---------|----------|
| macOS | darwin | amd64, arm64 |
| Linux | linux | amd64, arm64, 386 |
| Windows | windows | amd64, arm64, 386 |

### 与 sing-box 的区别

| 特性 | sing-box | mihomo |
|------|----------|--------|
| 压缩格式 | tar.gz | gz |
| 文件结构 | 目录/sing-box | mihomo (单文件) |
| 多路径支持 | 单路径 | 多路径 |
| 备份机制 | 无 | 自动备份 .bak |

## 安全建议

1. **定期备份配置**
   - 工具会自动备份二进制文件
   - 建议手动备份配置文件

2. **测试更新**
   - 首次使用建议在测试环境验证
   - 检查更新后的版本号

3. **权限管理**
   - 避免不必要的 sudo 权限
   - 使用用户目录而非系统目录

## 未来计划

- [ ] 支持手动指定版本
- [ ] 版本回滚功能
- [ ] 更新前自动停止服务
- [ ] 更新后自动重启服务
- [ ] 版本变更通知

## 相关链接

- [Mihomo GitHub](https://github.com/MetaCubeX/mihomo)
- [Mihomo 文档](https://wiki.metacubex.one/)
- [config-manager 项目](https://github.com/your-repo/config-manager)
