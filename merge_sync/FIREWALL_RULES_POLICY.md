# 🔥 Firewall Rules Management Policy

## 核心原则

**防火墙端口规则必须保持在模块中，不得合并到主规则集。**

## 为什么？

### 1. 功能性质不同
- **防火墙规则**: 系统级安全策略，阻止危险端口
- **内容过滤规则**: 应用层过滤，阻止特定域名/IP

### 2. 策略类型不同
- **防火墙规则**: 使用 `REJECT-DROP` 策略（静默丢弃）
- **广告拦截规则**: 使用 `REJECT` 策略（返回错误）

### 3. 管理方式不同
- **防火墙规则**: 用户可能需要临时禁用整个模块
- **内容过滤规则**: 通常保持启用状态

### 4. 风险级别不同
- **防火墙规则**: 错误配置可能导致网络异常
- **内容过滤规则**: 错误配置最多导致某些网站无法访问

## 技术实现

### 规则类型识别

防火墙规则的特征：
```
DEST-PORT,445,REJECT-DROP    // 目标端口
IN-PORT,8080,DIRECT          // 入站端口
SRC-PORT,53,DIRECT           // 源端口
```

### 双向同步机制

#### 1. 自动排除机制（Surge → 规则集）

`ingest_from_surge.sh` 脚本会自动识别并跳过这些规则：

```bash
# 🔥 CRITICAL: Exclude Firewall Port Rules
if [[ "$rule" =~ ^(IN-PORT|DEST-PORT|SRC-PORT) ]]; then
    echo "SKIP_FIREWALL_RULE"
    return
fi
```

**效果**：防止端口规则被错误合并到内容过滤规则集（如 AdBlock.list）

#### 2. 智能同步机制（规则集 → 防火墙模块）

`sync_ports_to_firewall_module.sh` 脚本会自动同步端口规则到防火墙模块：

```bash
# 从 SurgeConf_DirectPorts.list 提取端口规则
# 自动去重（跳过已存在的规则）
# 添加到防火墙模块的 SECTION 8
```

**效果**：从分流规则配置、新增渠道获取的端口规则会自动更新到防火墙模块

**集成到一键更新**：
```bash
./full_update.sh  # 自动执行端口规则同步
```

### 完整处理流程

```
┌─────────────────────────────────────────────────────────────┐
│ 用户在 Surge 配置中添加规则                                  │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│ 运行 ingest_from_surge.sh                                   │
│   ├─ 端口规则 → ⚠️ 跳过（保存到 SurgeConf_DirectPorts.list）│
│   └─ 其他规则 → ✅ 分类到对应规则集                         │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│ 运行 full_update.sh（一键更新）                             │
│   ├─ 合并规则集                                             │
│   ├─ 智能去重                                               │
│   └─ 🔥 sync_ports_to_firewall_module.sh                   │
│       ├─ 读取 SurgeConf_DirectPorts.list                   │
│       ├─ 自动去重（跳过已存在规则）                         │
│       └─ 添加到防火墙模块 SECTION 8                         │
└─────────────────┬───────────────────────────────────────────┘
                  ↓
┌─────────────────────────────────────────────────────────────┐
│ 防火墙模块更新完成                                           │
│ 用户重新加载 Surge 生效                                      │
└─────────────────────────────────────────────────────────────┘
```

## 正确的防火墙规则管理

### ✅ 正确做法

1. **保持在模块中**
   - 文件位置: `module/surge(main)/🔥 Firewall Port Blocker 🛡️🚫.sgmodule`
   - 用户可以通过 Surge 界面启用/禁用整个模块

2. **独立维护**
   - 防火墙规则应该单独更新和测试
   - 不要与其他规则混合

3. **明确标注**
   - 每个规则都应该有注释说明用途
   - 例如: `DEST-PORT,445,REJECT-DROP // SMB (WannaCry vector)`

### ❌ 错误做法

1. **合并到 AdBlock.list**
   - 防火墙规则不是广告拦截
   - 会导致用户无法单独控制

2. **合并到 Manual.list**
   - 防火墙规则不是手动直连规则
   - 策略类型不匹配

3. **分散到多个文件**
   - 难以维护和审计
   - 可能导致重复或冲突

## 测试验证

运行测试脚本验证排除机制：

```bash
cd merge_sync
./test_firewall_exclusion.sh
```

预期输出：
```
✅ SKIPPED (Correct): DEST-PORT,445,REJECT-DROP // SMB
✅ SKIPPED (Correct): DEST-PORT,3389,REJECT-DROP // RDP
✅ SKIPPED (Correct): IN-PORT,8080,DIRECT // Local proxy
✅ SKIPPED (Correct): SRC-PORT,53,DIRECT // DNS
✅ PROCESSED (Correct): DOMAIN-SUFFIX,google.com,Proxy // Normal rule
✅ PROCESSED (Correct): IP-CIDR,192.168.0.0/16,DIRECT // Normal rule
```

## 常见问题

### Q: 如果我需要添加新的防火墙规则怎么办？

A: 直接编辑模块文件：
```bash
module/surge(main)/🔥 Firewall Port Blocker 🛡️🚫.sgmodule
```

### Q: 如果我不小心在 Surge 配置中添加了端口规则？

A: 不用担心，`ingest_from_surge.sh` 会自动跳过这些规则并显示警告。

### Q: 为什么不能把防火墙规则放到 AdBlock.list？

A: 因为：
1. 功能性质完全不同（端口阻止 vs 域名阻止）
2. 用户可能需要临时禁用防火墙但保留广告拦截
3. 防火墙规则错误配置风险更高

### Q: 如果我想在其他代理软件中使用防火墙规则？

A: 防火墙规则是 Surge 特有的模块功能，其他软件可能需要：
- Shadowrocket: 手动添加到配置文件的 [Rule] 部分
- Sing-box: 需要转换为 JSON 格式的 route rules
- Clash: 需要转换为 YAML 格式的 rules

## 相关文件

- 防火墙模块: `module/surge(main)/🔥 Firewall Port Blocker 🛡️🚫.sgmodule`
- 吸纳脚本: `merge_sync/ingest_from_surge.sh`
- 测试脚本: `merge_sync/test_firewall_exclusion.sh`
- 本文档: `merge_sync/FIREWALL_RULES_POLICY.md`

---

**最后更新**: 2024-12-07
**维护者**: nyamiiko
