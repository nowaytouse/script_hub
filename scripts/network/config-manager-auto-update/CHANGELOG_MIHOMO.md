# Mihomo 更新功能 - 变更日志

## 版本 2.1.0 (2024-12-02)

### 🎉 新增功能

#### Mihomo 核心自动更新支持

完整实现了 Mihomo (原 Clash.Meta) 核心的自动更新功能，与 sing-box 核心更新并行工作。

### ✅ 已修复的问题

根据用户反馈，修复了以下关键问题：

1. **二进制文件名处理**
   - ❌ 旧问题：更新时使用错误的文件名
   - ✅ 新实现：正确识别 `mihomo` 或 `mihomo.exe` (Windows)
   - 实现位置：`src/updater.rs` - `MihomoUpdater::extract_gz()`

2. **旧版本替换**
   - ❌ 旧问题：更新后原文件和新文件同时存在
   - ✅ 新实现：自动备份为 `.bak` 后替换
   - 实现位置：`src/updater.rs` - `MihomoUpdater::install_file()`

3. **可执行权限**
   - ❌ 旧问题：更新后文件无法执行
   - ✅ 新实现：自动设置 `0o755` 权限 (Unix/Linux/macOS)
   - 实现位置：`src/updater.rs` - 两处权限设置

4. **多路径同时更新**
   - ❌ 旧问题：只能更新单个位置
   - ✅ 新实现：支持同时更新多个安装路径
   - 实现位置：`src/types.rs` - `install_paths: Vec<PathBuf>`

### 📝 技术实现细节

#### 1. 数据结构 (`src/types.rs`)

```rust
/// Mihomo 核心更新配置
pub struct MihomoCoreUpdate {
    pub enabled: bool,
    pub check_prerelease: bool,
    pub install_paths: Vec<PathBuf>,  // 支持多路径
}
```

#### 2. 更新器实现 (`src/updater.rs`)

```rust
pub struct MihomoUpdater;

impl MihomoUpdater {
    // 获取最新版本（支持稳定版和预发布版）
    async fn get_latest_version(&self, check_prerelease: bool) -> Result<String>
    
    // 构建下载 URL
    fn get_download_url(&self, version: &str) -> String
    
    // 下载并解压 .gz 文件
    pub async fn download_and_extract(&self, check_prerelease: bool) -> Result<PathBuf>
    
    // 解压 .gz（单文件压缩）
    async fn extract_gz(&self, data: &[u8], dest_dir: &Path) -> Result<PathBuf>
    
    // 安装文件（带备份）
    pub async fn install_file(&self, source: &Path, dest: &Path) -> Result<()>
    
    // 运行更新（所有路径）
    pub async fn run_all(&self, config: &MihomoCoreUpdate) -> Result<()>
}
```

#### 3. CLI 集成 (`src/cli.rs`)

```rust
async fn run_tasks(config: &Config) -> Result<()> {
    // 更新订阅
    sub_manager.run_all(config).await?;
    
    // 更新 sing-box
    core_updater.run_all(config).await?;
    
    // 更新 mihomo（如果配置）
    if let Some(mihomo_config) = &config.mihomo_core_update {
        mihomo_updater.run_all(mihomo_config).await?;
    }
    
    Ok(())
}
```

#### 4. 交互式界面 (`src/interactive.rs`)

新增菜单选项：
- `[5] 更新 Mihomo 核心` - 单独更新 mihomo
- `[6] 执行所有更新` - 包含 mihomo 更新
- `配置设置` → `[3] 配置 Mihomo 核心更新` - 配置管理

### 🔧 配置示例

#### 完整配置 (`config.json`)

```json
{
  "subscriptions": [...],
  "update_interval_hours": 24,
  "singbox_core_update": {
    "enabled": true,
    "check_prerelease": false,
    "install_path": "/Users/nyamiiko/Library/Application Support/v2rayN/bin/sing-box"
  },
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

### 📊 更新流程

```
1. 检查最新版本
   ├─ 稳定版：GitHub redirect (无 API 限制)
   └─ 预发布版：GitHub API

2. 下载二进制文件
   ├─ 平台识别：darwin/linux/windows
   ├─ 架构识别：amd64/arm64/386
   └─ URL: mihomo-{os}-{arch}-{version}.gz

3. 解压缩
   ├─ 解压 .gz 文件（单文件压缩）
   ├─ 提取到临时目录
   └─ 设置可执行权限 (0o755)

4. 安装到所有路径
   ├─ 备份现有文件 (.bak)
   ├─ 复制新文件
   └─ 设置可执行权限

5. 清理临时文件
```

### 🎯 使用方法

#### 命令行模式

```bash
# 一次性更新所有（包括 mihomo）
./singbox-manager --once

# 定时自动更新
./singbox-manager
```

#### 交互式模式

```bash
./singbox-manager --interactive

# 在菜单中选择：
# [5] 更新 Mihomo 核心
# [6] 执行所有更新
# [7] 配置设置 → [3] 配置 Mihomo 核心更新
```

### 📈 测试结果

```bash
✅ 编译成功：零警告
✅ 配置解析：正确识别 mihomo_core_update
✅ 版本检测：成功获取最新版本
✅ 下载解压：正确处理 .gz 格式
✅ 多路径安装：同时更新两个位置
✅ 权限设置：自动设置 0o755
✅ 备份机制：自动创建 .bak 文件
```

### 🔍 与 Sing-box 的区别

| 特性 | Sing-box | Mihomo |
|------|----------|--------|
| 压缩格式 | tar.gz | gz |
| 文件结构 | 目录/sing-box | mihomo (单文件) |
| 安装路径 | 单路径 | 多路径 |
| 备份机制 | 无 | 自动 .bak |
| GitHub 仓库 | SagerNet/sing-box | MetaCubeX/mihomo |

### 📚 相关文档

- `MIHOMO_UPDATE_GUIDE.md` - 完整使用指南
- `README.md` - 项目主文档
- `README_CN.md` - 中文文档

### 🐛 已知问题

无已知问题。

### 🚀 未来计划

- [ ] 支持手动指定版本
- [ ] 版本回滚功能
- [ ] 更新前自动停止服务
- [ ] 更新后自动重启服务
- [ ] 版本变更通知

### 👥 贡献者

- 实现：根据用户反馈完整实现
- 测试：macOS arm64 平台验证通过

### 📄 许可证

MIT License

---

**版本**: 2.1.0  
**发布日期**: 2024-12-02  
**平台**: macOS / Linux / Windows  
**语言**: Rust  
