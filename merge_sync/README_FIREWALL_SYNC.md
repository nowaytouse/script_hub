# 🔥 防火墙端口规则双向同步系统

## 📋 概述

本系统实现了防火墙端口规则的**双向智能同步**：
1. **Surge → 规则集**：自动排除端口规则，防止错误合并
2. **规则集 → 防火墙模块**：自动同步新的端口规则到防火墙模块

## 🎯 核心功能

### 1. 自动排除机制（防止污染）

**脚本**: `ingest_from_surge.sh`

**功能**:
- 识别所有端口规则（IN-PORT/DEST-PORT/SRC-PORT）
- 自动跳过这些规则，不合并到内容过滤规则集
- 显示警告信息，告知用户哪些规则被跳过

**示例输出**:
```
---------- PREVIEW: RULES TO INGEST ----------
RULE                                                         | POLICY               | TARGET LIST
----------------------------------------------------------------------------------------------------------------
DEST-PORT,445,REJECT-DROP // SMB                            | REJECT-DROP          | ⚠️ SKIPPED (Firewall)
DOMAIN-SUFFIX,google.com,Proxy                              | Proxy                | SurgeConf_GlobalProxy.list
----------------------------------------------------------------------------------------------------------------
⚠️  Skipped 1 firewall port rules (should stay in module)
```

### 2. 智能同步机制（自动更新）

**脚本**: `sync_ports_to_firewall_module.sh`

**功能**:
- 从 `SurgeConf_DirectPorts.list` 读取端口规则
- 自动去重（跳过已存在的规则）
- 添加到防火墙模块的 SECTION 8
- 自动转换策略为 `REJECT-DROP`（防火墙应该阻止）

**示例输出**:
```
╔══════════════════════════════════════════════════════════════╗
║                    Sync Complete                             ║
╠══════════════════════════════════════════════════════════════╣
║  New Rules Added:     2                                  ║
║  Duplicates Skipped:  0                                  ║
╚══════════════════════════════════════════════════════════════╝
```

## 🔄 完整工作流程

```
┌─────────────────────────────────────────────────────────────┐
│ 1. 用户在 Surge 配置中添加规则                               │
│    - DEST-PORT,8076,DIRECT                                  │
│    - DOMAIN-SUFFIX,example.com,Proxy                        │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│ 2. 运行 ingest_from_surge.sh --execute                      │
│    ├─ DEST-PORT,8076 → ⚠️ 跳过（保存到 DirectPorts.list）  │
│    └─ DOMAIN-SUFFIX,example.com → ✅ 分类到 GlobalProxy    │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│ 3. 运行 full_update.sh                                      │
│    ├─ 合并所有规则集                                         │
│    ├─ 智能去重（广告 > 细分 > 兜底）                        │
│    └─ 🔥 sync_ports_to_firewall_module.sh                  │
│        ├─ 读取 DirectPorts.list                            │
│        ├─ 检查重复（IN-PORT,8076 已存在？）                 │
│        └─ 添加新规则到防火墙模块 SECTION 8                  │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│ 4. 防火墙模块自动更新                                        │
│    module/surge(main)/🔥 Firewall Port Blocker.sgmodule    │
│    [SECTION 8: Auto-synced from Surge Config]              │
│    DEST-PORT,8076,REJECT-DROP                               │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│ 5. 用户重新加载 Surge → 规则生效                            │
└─────────────────────────────────────────────────────────────┘
```

## 📝 使用方法

### 方法1: 一键更新（推荐）

```bash
cd merge_sync
./full_update.sh
```

**自动执行**:
- ✅ 规则集合并
- ✅ 智能去重
- ✅ 端口规则同步到防火墙模块
- ✅ 生成 Sing-box SRS 文件
- ✅ 同步到三个代理软件配置

### 方法2: 单独同步端口规则

```bash
cd merge_sync

# 预览模式（不执行）
./sync_ports_to_firewall_module.sh

# 执行同步
./sync_ports_to_firewall_module.sh --execute

# 执行同步（不备份）
./sync_ports_to_firewall_module.sh --execute --no-backup
```

### 方法3: 手动吸纳规则

```bash
cd merge_sync

# 预览模式
./ingest_from_surge.sh

# 执行吸纳
./ingest_from_surge.sh --execute
```

## 🔍 验证和测试

### 测试1: 验证端口规则排除

```bash
cd merge_sync
./test_firewall_exclusion.sh
```

**预期输出**:
```
✅ SKIPPED (Correct): DEST-PORT,445,REJECT-DROP // SMB
✅ SKIPPED (Correct): IN-PORT,8080,DIRECT // Local proxy
✅ PROCESSED (Correct): DOMAIN-SUFFIX,google.com,Proxy
```

### 测试2: 验证同步功能

```bash
# 1. 添加测试规则到 DirectPorts.list
echo "DEST-PORT,9999" >> ruleset/Sources/conf/SurgeConf_DirectPorts.list

# 2. 运行同步
./sync_ports_to_firewall_module.sh --execute

# 3. 检查防火墙模块
tail -20 "module/surge(main)/🔥 Firewall Port Blocker 🛡️🚫.sgmodule"
```

**预期结果**: 应该看到 `DEST-PORT,9999,REJECT-DROP` 在 SECTION 8

### 测试3: 验证去重功能

```bash
# 再次运行同步（应该跳过已存在的规则）
./sync_ports_to_firewall_module.sh --execute --no-backup
```

**预期输出**:
```
[INFO]   Skipped (exists): DEST-PORT,9999
[OK] All port rules already exist in module. No changes needed.
```

## 📊 文件结构

```
merge_sync/
├── ingest_from_surge.sh                    # 规则吸纳（自动排除端口规则）
├── sync_ports_to_firewall_module.sh        # 端口规则同步到防火墙模块
├── test_firewall_exclusion.sh              # 测试脚本
├── full_update.sh                          # 一键更新（集成所有功能）
├── FIREWALL_RULES_POLICY.md                # 防火墙规则管理策略
└── README_FIREWALL_SYNC.md                 # 本文档

ruleset/Sources/conf/
└── SurgeConf_DirectPorts.list              # 端口规则临时存储

module/surge(main)/
└── 🔥 Firewall Port Blocker 🛡️🚫.sgmodule  # 防火墙模块（最终目标）
```

## ⚙️ 配置说明

### 端口规则格式

**输入格式**（SurgeConf_DirectPorts.list）:
```
IN-PORT,8076
DEST-PORT,8076
SRC-PORT,53
```

**输出格式**（防火墙模块）:
```
IN-PORT,8076,REJECT-DROP
DEST-PORT,8076,REJECT-DROP
SRC-PORT,53,REJECT-DROP
```

**策略转换**:
- 所有端口规则自动转换为 `REJECT-DROP` 策略
- 原因：防火墙规则应该**阻止**危险端口，而不是直连

### 去重逻辑

脚本会检查防火墙模块中是否已存在相同的端口规则：
- 匹配条件：`规则类型` + `端口号`
- 示例：`DEST-PORT,445` 已存在 → 跳过

## 🚨 注意事项

### 1. 策略自动转换

⚠️ **重要**: 所有同步到防火墙模块的端口规则都会被转换为 `REJECT-DROP` 策略

**原因**:
- 防火墙模块的目的是**阻止**危险端口
- 如果需要允许某个端口，应该在主配置文件中添加规则，而不是在防火墙模块

### 2. 手动规则优先

如果你在防火墙模块中手动添加了规则，自动同步**不会**覆盖它们：
- 自动同步只添加到 SECTION 8
- 手动规则通常在 SECTION 1-7
- 两者不会冲突

### 3. 重新加载 Surge

⚠️ **必须重新加载 Surge 才能使防火墙规则生效**

方法：
- Surge Mac: `Surge → Reload Profile`
- Surge iOS: 下拉刷新配置

### 4. 备份机制

- 默认情况下，脚本会自动备份防火墙模块
- 备份位置：`merge_sync/backup/`
- 保留最近 3 个备份
- CI/CD 环境自动跳过备份（使用 `--no-backup`）

## 🔧 故障排查

### 问题1: 端口规则被合并到 AdBlock.list

**原因**: `ingest_from_surge.sh` 版本过旧

**解决**:
```bash
# 检查脚本版本
grep "CRITICAL: Exclude Firewall Port Rules" merge_sync/ingest_from_surge.sh

# 如果没有找到，说明需要更新脚本
```

### 问题2: 同步后防火墙模块没有变化

**可能原因**:
1. 规则已存在（去重跳过）
2. `SurgeConf_DirectPorts.list` 为空
3. 权限问题

**检查**:
```bash
# 1. 检查源文件
cat ruleset/Sources/conf/SurgeConf_DirectPorts.list

# 2. 运行详细模式
./sync_ports_to_firewall_module.sh --execute --verbose

# 3. 检查权限
ls -la "module/surge(main)/🔥 Firewall Port Blocker 🛡️🚫.sgmodule"
```

### 问题3: 规则重复添加

**原因**: 去重逻辑失效

**检查**:
```bash
# 查看防火墙模块中的重复规则
grep -E "^(IN-PORT|DEST-PORT|SRC-PORT)" "module/surge(main)/🔥 Firewall Port Blocker 🛡️🚫.sgmodule" | sort | uniq -d
```

**修复**:
```bash
# 手动删除 SECTION 8，重新同步
# 编辑防火墙模块，删除 SECTION 8 的所有内容
# 然后重新运行同步
./sync_ports_to_firewall_module.sh --execute
```

## 📚 相关文档

- [防火墙规则管理策略](FIREWALL_RULES_POLICY.md) - 详细的策略说明
- [一键更新脚本说明](README_UPDATE.md) - full_update.sh 使用指南
- [规则吸纳脚本说明](README_MERGE_RULES.md) - ingest_from_surge.sh 使用指南

## 🎉 总结

通过这套双向同步系统：

✅ **防止污染**: 端口规则不会被错误合并到内容过滤规则集
✅ **自动更新**: 新的端口规则自动同步到防火墙模块
✅ **智能去重**: 避免重复规则
✅ **完全自动化**: 集成到一键更新脚本
✅ **安全可靠**: 自动备份，可回滚

---

**最后更新**: 2024-12-07
**维护者**: nyamiiko
