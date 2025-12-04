# 广告拦截模块智能合并脚本使用说明

## 📋 功能概述

`merge_adblock_modules.sh` 是一个智能的广告拦截规则合并工具，可以：

1. **多源提取**：从 Surge 和小火箭（Shadowrocket）模块中提取广告拦截规则
2. **智能分类**：自动按策略分类（REJECT、REJECT-DROP、REJECT-NO-DROP）
3. **增量合并**：只添加新规则，自动去重，不删除现有规则
4. **多类型支持**：处理 Rule、URL Rewrite、Host、MITM 等多种规则类型
5. **合并到巨大规则文件**：自动合并 REJECT 规则到 `ruleset/Surge(Shadowkroket)/AdBlock_Merged.list`（235k+ 规则）
6. **自动同步**：合并完成后自动同步到小火箭模块

## 🚀 快速开始

### 基本使用

```bash
# 在项目根目录执行
bash scripts/sync/merge_adblock_modules.sh
```

### 执行流程

```
1. 创建临时目录
   ↓
2. 提取现有规则（从目标模块）
   ├─ Rule 规则（REJECT/REJECT-DROP/REJECT-NO-DROP）
   ├─ URL Rewrite 规则
   ├─ Host 规则
   └─ MITM hostname
   ↓
3. 扫描并合并模块
   ├─ Surge 模块目录
   └─ 小火箭模块目录（可选）
   ↓
4. 生成新模块文件
   ├─ 按策略分类
   ├─ 自动排序
   └─ 添加统计信息
   ↓
5. 合并到 AdBlock_Merged.list
   ├─ 提取现有规则（235k+）
   ├─ 添加新规则（去重）
   ├─ 更新统计信息
   └─ 自动备份原文件
   ↓
6. 同步到小火箭
   └─ 调用 sync_modules_to_shadowrocket.sh
   ↓
7. 清理临时文件
```

## 📊 输出示例

```
═══════════════════════════════════════════════════════════════
  广告拦截模块智能合并
═══════════════════════════════════════════════════════════════
目标模块: .../🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule

[INFO] 创建临时目录...

═══════════════════════════════════════════════════════════════
  提取现有规则
═══════════════════════════════════════════════════════════════
[INFO] 提取 Rule 规则...
[INFO] 提取 URL Rewrite 规则...
[INFO] 提取 Host 规则...
[INFO] 提取 MITM hostname...
[✓] 现有规则统计:
  - REJECT: 32
  - REJECT-DROP: 2
  - REJECT-NO-DROP: 0
  - URL Rewrite: 50
  - Host: 45

═══════════════════════════════════════════════════════════════
  扫描并合并模块
═══════════════════════════════════════════════════════════════
[INFO] 扫描 Surge 模块目录...
[INFO] 处理模块: xxx.sgmodule
[✓] 从 xxx.sgmodule 新增 15 条规则

═══════════════════════════════════════════════════════════════
  生成新模块文件
═══════════════════════════════════════════════════════════════
[INFO] 最终规则统计:
  - REJECT: 47
  - REJECT-DROP: 2
  - REJECT-NO-DROP: 0
  - URL Rewrite: 65
  - Host: 45
  - 总计: 49 条分流规则
[✓] 已备份原模块文件
[✓] 新模块文件已生成

═══════════════════════════════════════════════════════════════
  合并规则到 AdBlock_Merged.list
═══════════════════════════════════════════════════════════════
[✓] 已备份 AdBlock_Merged.list
[INFO] 提取现有规则...
[INFO] 现有规则: 235584 条
[INFO] 准备新规则...
[✓] 发现 32 条新规则
[INFO] 合并规则到 AdBlock_Merged.list...
[✓] 已合并到 AdBlock_Merged.list
[INFO] 总规则数: 235584 + 32 = 235616

═══════════════════════════════════════════════════════════════
  同步到小火箭
═══════════════════════════════════════════════════════════════
[INFO] 调用小火箭同步脚本...
[✓] 已同步到小火箭

═══════════════════════════════════════════════════════════════
  完成
═══════════════════════════════════════════════════════════════
[✓] 广告拦截模块合并完成！
```

## 🔧 配置说明

### 路径配置

脚本中的关键路径（可根据需要修改）：

```bash
# Surge 模块目录
SURGE_MODULE_DIR="$PROJECT_ROOT/module/surge(main)"

# 小火箭模块目录
SHADOWROCKET_MODULE_DIR="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules"

# 目标模块
TARGET_MODULE="$SURGE_MODULE_DIR/🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"
```

### 文件大小限制

```bash
# 跳过超过 100KB 的模块文件（防止处理过大文件导致卡顿）
if [[ $file_size -gt 100000 ]]; then
    log_warning "文件过大，跳过: $module_name"
    return
fi
```

## 📝 支持的规则类型

### 1. Rule 规则

支持的规则类型：
- `DOMAIN` - 域名规则
- `DOMAIN-SUFFIX` - 域名后缀规则
- `DOMAIN-KEYWORD` - 域名关键词规则
- `IP-CIDR` - IP 地址段规则
- `USER-AGENT` - User-Agent 规则
- `URL-REGEX` - URL 正则表达式规则

支持的策略：
- `REJECT` - 拒绝连接
- `REJECT-DROP` - 拒绝并丢弃（Surge 专用）
- `REJECT-NO-DROP` - 拒绝但不丢弃（Surge 专用）

### 2. URL Rewrite 规则

提取所有包含 `reject` 关键词的 URL 重写规则：

```
^https?://example.com/ad - reject
^https?://example.com/track - reject-dict
^https?://example.com/banner - reject-tinygif
```

### 3. Host 规则

提取所有指向 `0.0.0.0` 的 Host 规则（DNS 黑洞）：

```
ad.example.com = 0.0.0.0
tracker.example.com = 0.0.0.0
```

### 4. MITM Hostname

合并所有 MITM hostname，自动去重：

```
hostname = %APPEND% example.com, *.example.com, api.example.com
```

## ⚙️ 高级功能

### 1. 自动备份

每次运行时自动备份原模块文件：

```
🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule.backup.20251204_125401
```

### 2. 增量合并

- ✅ 只添加新规则，不删除现有规则
- ✅ 自动去重（完全相同的规则只保留一条）
- ✅ 保留规则顺序（按字母排序）

### 3. 智能跳过

自动跳过以下模块：
- 已同步的模块（以 `__` 开头）
- 特定名称的模块（Encrypted_DNS、URL_Rewrite 等）
- 过大的文件（> 100KB）
- 不包含广告拦截规则的模块

### 4. 错误处理

- 文件不存在：显示警告并跳过
- 文件过大：显示警告并跳过
- 解析失败：显示错误但继续处理其他文件

## 🔍 故障排查

### 问题1：脚本卡住不动

**原因**：某些模块文件可能存在兼容性问题

**解决方案**：
1. 检查是否有超大文件（> 100KB）
2. 查看日志中最后处理的模块名称
3. 将问题模块添加到黑名单

### 问题2：规则未被提取

**原因**：规则格式不符合标准

**解决方案**：
1. 检查规则是否在正确的 section 中（[Rule]、[URL Rewrite] 等）
2. 确认规则格式符合 Surge/Shadowrocket 标准
3. 查看脚本日志中的警告信息

### 问题3：MITM hostname 重复

**原因**：多个模块包含相同的 hostname

**解决方案**：
脚本会自动去重，无需手动处理

## 📚 相关脚本

- `sync_modules_to_shadowrocket.sh` - Surge 模块同步到小火箭
- `sync_to_shadowrocket.sh` - 旧版同步脚本（已废弃）

## 🎯 最佳实践

### 1. 定期运行

建议每周运行一次，保持规则最新：

```bash
# 添加到 crontab
0 0 * * 0 cd /path/to/script_hub && bash scripts/sync/merge_adblock_modules.sh
```

### 2. 手动审查

合并后建议手动审查新增的规则：

```bash
# 查看最新备份和当前文件的差异
diff "module/surge(main)/🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule.backup.最新时间戳" \
     "module/surge(main)/🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"
```

### 3. Git 提交

合并后及时提交到 Git：

```bash
git add module/surge(main)/*.sgmodule
git commit -m "feat: 合并广告拦截规则 - 新增 XX 条规则"
git push origin master
```

## 🚨 注意事项

1. **备份重要**：脚本会自动备份，但建议手动备份重要配置
2. **测试验证**：合并后在 Surge/Shadowrocket 中测试规则是否生效
3. **性能影响**：规则过多可能影响代理软件性能，建议定期清理无效规则
4. **兼容性**：某些规则可能只在特定软件中有效（如 REJECT-DROP 只在 Surge 中有效）

## 📖 参考资料

- [Surge 官方文档](https://manual.nssurge.com/)
- [Shadowrocket 使用指南](https://shadowrocket.org/)
- [广告拦截规则编写指南](https://github.com/blackmatrix7/ios_rule_script)

## 🤝 贡献

欢迎提交 Issue 和 Pull Request 来改进这个脚本！

---

**最后更新**: 2024-12-04  
**版本**: 1.0.0  
**作者**: nyamiiko
