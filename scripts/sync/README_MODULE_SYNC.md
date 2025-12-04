# 模块同步脚本使用说明

## 📋 功能概述

`sync_modules_to_icloud.sh` 脚本用于将 Surge 模块同步到 iCloud，支持：

1. ✅ 同步到 Surge iCloud 目录
2. ✅ 同步到 Shadowrocket iCloud 目录（自动转换兼容格式）
3. ✅ 自动排除敏感信息文件
4. ✅ 支持全部同步或选择性同步
5. ✅ 清理旧的同步文件

## 🚀 快速开始

### 同步所有模块

```bash
bash scripts/sync/sync_modules_to_icloud.sh
```

或

```bash
bash scripts/sync/sync_modules_to_icloud.sh --all
```

### 同步指定模块

```bash
bash scripts/sync/sync_modules_to_icloud.sh "URL Rewrite Module 🔄🌐.sgmodule"
```

### 列出所有可同步的模块

```bash
bash scripts/sync/sync_modules_to_icloud.sh --list
```

### 清理旧的同步文件

```bash
bash scripts/sync/sync_modules_to_icloud.sh --clean
```

## 📂 目录结构

```
源目录:
  /Users/nyamiiko/Library/Mobile Documents/com~apple~CloudDocs/Application/script_hub/module/surge(main)

目标目录:
  Surge iCloud:
    /Users/nyamiiko/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents
  
  Shadowrocket iCloud:
    /Users/nyamiiko/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules
```

## 🔒 敏感信息排除

脚本会自动跳过包含以下关键词的文件：

- `敏感`
- `私密`
- `private`
- `secret`
- `password`
- `token`
- `api-key`
- `YOUR_`

**示例**：
- ✅ `URL Rewrite Module 🔄🌐.sgmodule` - 会同步
- ❌ `敏感profile 排除上传git` - 会跳过
- ❌ `private_config.sgmodule` - 会跳过

## 🔄 Shadowrocket 兼容性转换

脚本会自动转换以下 Surge 特有参数为 Shadowrocket 兼容格式：

| Surge 参数 | Shadowrocket 转换 |
|-----------|------------------|
| `extended-matching` | 移除 |
| `pre-matching` | 移除 |
| `update-interval=86400` | 移除 |
| `REJECT-DROP` | 转换为 `REJECT` |
| `REJECT-NO-DROP` | 转换为 `REJECT` |
| `hostname = %APPEND%` | 转换为 `hostname =` |

**转换示例**：

**Surge 原始**：
```
RULE-SET,https://example.com/rules.list,REJECT-DROP,extended-matching,pre-matching,"update-interval=86400"
```

**Shadowrocket 转换后**：
```
RULE-SET,https://example.com/rules.list,REJECT
```

## 📝 文件命名规则

### Surge iCloud

文件名保持不变：
```
URL Rewrite Module 🔄🌐.sgmodule
```

### Shadowrocket iCloud

文件名添加 `__` 前缀（标识为同步文件）：
```
__URL Rewrite Module 🔄🌐.sgmodule
```

## 🎯 使用场景

### 场景 1: 日常开发后同步

```bash
# 1. 修改 Surge 模块
vim "module/surge(main)/URL Rewrite Module 🔄🌐.sgmodule"

# 2. 同步到 iCloud
bash scripts/sync/sync_modules_to_icloud.sh

# 3. 在设备上刷新模块列表
```

### 场景 2: 只同步特定模块

```bash
# 只同步 URL Rewrite 模块
bash scripts/sync/sync_modules_to_icloud.sh "URL Rewrite Module 🔄🌐.sgmodule"
```

### 场景 3: 清理旧文件后重新同步

```bash
# 1. 清理旧文件
bash scripts/sync/sync_modules_to_icloud.sh --clean

# 2. 重新同步所有模块
bash scripts/sync/sync_modules_to_icloud.sh --all
```

### 场景 4: 检查哪些模块会被同步

```bash
# 列出所有模块及其状态
bash scripts/sync/sync_modules_to_icloud.sh --list
```

## 📊 输出示例

### 成功同步

```
═══════════════════════════════════════════════════════════════
  模块同步脚本
═══════════════════════════════════════════════════════════════

═══════════════════════════════════════════════════════════════
  检查目录
═══════════════════════════════════════════════════════════════
[✓] 源目录: /Users/nyamiiko/Library/Mobile Documents/com~apple~CloudDocs/Application/script_hub/module/surge(main)
[✓] Surge iCloud: /Users/nyamiiko/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents
[✓] Shadowrocket iCloud: /Users/nyamiiko/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules

═══════════════════════════════════════════════════════════════
  同步所有模块
═══════════════════════════════════════════════════════════════
[INFO] 处理: URL Rewrite Module 🔄🌐.sgmodule
[✓] Surge: URL Rewrite Module 🔄🌐.sgmodule
[✓] Shadowrocket: __URL Rewrite Module 🔄🌐.sgmodule

[INFO] 处理: Encrypted DNS Module 🔒🛡️DNS.sgmodule
[✓] Surge: Encrypted DNS Module 🔒🛡️DNS.sgmodule
[✓] Shadowrocket: __Encrypted DNS Module 🔒🛡️DNS.sgmodule

[⚠] 跳过敏感文件: 敏感profile 排除上传git

═══════════════════════════════════════════════════════════════
  同步统计
═══════════════════════════════════════════════════════════════
Surge: 5 个模块
Shadowrocket: 5 个模块
跳过: 1 个敏感文件

═══════════════════════════════════════════════════════════════
  完成
═══════════════════════════════════════════════════════════════
[✓] 模块同步完成！

下一步：
1. 打开 Surge 或 Shadowrocket 应用
2. 刷新模块列表
3. 启用需要的模块
```

### 列出模块

```
═══════════════════════════════════════════════════════════════
  可同步的模块列表
═══════════════════════════════════════════════════════════════
[可同步] URL Rewrite Module 🔄🌐.sgmodule
[可同步] Encrypted DNS Module 🔒🛡️DNS.sgmodule
[可同步] 🚀💪General Enhanced⬆️⬆️ plus.sgmodule
[可同步] 🔥 Firewall Port Blocker 🛡️🚫.sgmodule
[可同步] 🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule
[敏感] 敏感profile 排除上传git

可同步: 5 个模块
敏感文件: 1 个（将被跳过）
```

## ⚠️ 注意事项

### 1. iCloud 同步延迟

- iCloud 同步可能需要几秒到几分钟
- 建议在设备上手动刷新模块列表

### 2. 文件冲突

- 如果在设备上修改了同步的模块，可能会产生冲突
- 建议始终在源目录修改，然后重新同步

### 3. Shadowrocket 前缀

- Shadowrocket 中的 `__` 前缀表示这是同步文件
- 不要在 Shadowrocket 中直接修改这些文件
- 如需修改，请在源目录修改后重新同步

### 4. 敏感信息

- 确保敏感信息文件名包含排除关键词
- 脚本只检查文件名，不检查文件内容
- 建议在文件名中明确标注敏感性质

## 🔧 故障排查

### 问题 1: 目录不存在

**错误**：
```
[✗] 源目录不存在: /path/to/source
```

**解决**：
- 检查路径是否正确
- 确保 iCloud 已登录并同步

### 问题 2: 权限问题

**错误**：
```
Permission denied
```

**解决**：
```bash
chmod +x scripts/sync/sync_modules_to_icloud.sh
```

### 问题 3: 模块未出现在设备上

**解决**：
1. 等待 iCloud 同步完成（可能需要几分钟）
2. 在设备上手动刷新模块列表
3. 检查 iCloud 存储空间是否充足
4. 重启 Surge/Shadowrocket 应用

### 问题 4: Shadowrocket 模块不兼容

**解决**：
- 检查转换后的文件内容
- 确认 Shadowrocket 版本支持该功能
- 查看 Shadowrocket 日志获取详细错误信息

## 📚 相关脚本

- `sync_to_shadowrocket.sh` - 旧版 Shadowrocket 同步脚本（已废弃）
- `sync_modules_to_icloud.sh` - 新版统一同步脚本（推荐）
- `merge_adblock_modules.sh` - 广告拦截模块合并脚本
- `sync_all_rulesets.sh` - 规则集同步脚本

## 🎯 最佳实践

1. **定期同步**：每次修改模块后立即同步
2. **版本控制**：源目录的模块文件应提交到 Git
3. **敏感信息**：始终使用明确的文件名标识敏感文件
4. **测试验证**：同步后在设备上验证模块是否正常工作
5. **备份重要**：定期备份源目录的模块文件

## 📝 更新日志

### v1.0.0 (2024-12-04)

- ✅ 初始版本
- ✅ 支持 Surge iCloud 同步
- ✅ 支持 Shadowrocket iCloud 同步
- ✅ 自动转换兼容格式
- ✅ 敏感信息排除
- ✅ 支持选择性同步

---

**脚本位置**: `scripts/sync/sync_modules_to_icloud.sh`  
**文档更新**: 2024-12-04
