# Merge Sync 脚本目录

## 目录结构

### 核心脚本 (自动更新流程)
- `full_update.sh` - 🚀 **主入口脚本**，一键更新所有内容
- `incremental_merge_all.sh` - 增量合并规则集
- `batch_convert_to_singbox.sh` - 转换 SRS 文件
- `ruleset_cleaner.sh` - 🔥 **合并版清理脚本** (空规则集 + 混入域名)

### 下载脚本
- `download_modules.sh` - 下载模块
- `download_adblock_modules.sh` - 下载广告拦截模块
- `sync_metacubex_rules.sh` - 同步 MetaCubeX 规则

### 合并脚本
- `merge_adblock_core_modules.sh` - 合并核心广告拦截模块
- `merge_adblock_modules.sh` - 合并广告拦截模块
- `consolidate_rulesets.py` - 合并相关规则集

### 修复脚本
- `fix_ruleset_policies.py` - 修复规则集策略
- `fix_broken_modules.py` - 修复损坏模块
- `smart_cleanup.py` - 智能去重

### 同步脚本
- `sync_organized_modules.sh` - 同步组织模块
- `sync_ports_to_firewall_module.sh` - 同步端口规则

### 更新脚本
- `update_cores.sh` - 更新内核
- `update_ruleset_headers.sh` - 更新规则集头部
- `update_sources_metacubex.sh` - 更新 MetaCubeX 源

### 归档目录
- `archive/` - 归档的脚本
  - `verification/` - 验证类脚本 (8个)
  - `legacy/` - 旧版脚本 (5个)
  - `tools/` - 工具类脚本 (6个)
- `backup/` - 备份的脚本
- `config-manager-auto-update/` - 配置管理器

## 使用方法

### 一键更新
```bash
./full_update.sh --auto --parallel
```

### 手动清理
```bash
./ruleset_cleaner.sh
```

### 验证配置
```bash
./archive/verification/validate_singbox_config.sh
```

## 脚本统计

| 类型 | 数量 | 说明 |
|------|------|------|
| 活跃脚本 | 31 | 在 merge_sync 根目录 |
| 归档脚本 | 19 | 在 archive/ 目录 |
| **总计** | **50** | **已整理完成** |

## 优化成果

- ✅ 合并 3 个清理脚本为 1 个
- ✅ 归档 19 个未使用脚本
- ✅ 简化 full_update.sh 流程
- ✅ 提高维护性和可读性