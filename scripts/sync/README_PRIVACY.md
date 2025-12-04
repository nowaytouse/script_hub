# 🔒 隐私保护说明

## 📁 目录结构

本项目的脚本分为两个版本：

### 1. 公开版本（Git仓库）
**位置**: `scripts/sync/*.sh`

这些是**模板版本**，包含占位符，需要用户自行配置：
- `YOUR_USERNAME` - 替换为你的用户名
- `YOUR_AUTHOR_NAME` - 替换为你的作者名
- `YOUR_REPO` - 替换为你的仓库名

### 2. 私密版本（本地）
**位置**: `conf隐私🔏/scripts🔏/*.sh`

这些是**实际使用的版本**，包含真实的：
- 用户名路径
- iCloud目录路径
- 个人信息

⚠️ **注意**: `conf隐私🔏/` 目录已在 `.gitignore` 中排除，不会上传到Git

---

## 🔧 使用方法

### 首次配置

1. **复制模板到私密目录**:
```bash
mkdir -p "conf隐私🔏/scripts🔏"
cp scripts/sync/*.sh "conf隐私🔏/scripts🔏/"
```

2. **编辑私密版本**，替换占位符为真实信息:
```bash
# 编辑 sync_modules_to_icloud.sh
nano "conf隐私🔏/scripts🔏/sync_modules_to_icloud.sh"

# 将 YOUR_USERNAME 替换为你的实际用户名
# 例如: /Users/YOUR_USERNAME/... → /Users/john/...
```

3. **使用私密版本**:
```bash
# 运行私密版本的脚本
"conf隐私🔏/scripts🔏/sync_modules_to_icloud.sh" --all
```

### 更新脚本

当公开版本更新时：

1. **拉取最新代码**:
```bash
git pull
```

2. **对比差异**:
```bash
diff scripts/sync/sync_modules_to_icloud.sh "conf隐私🔏/scripts🔏/sync_modules_to_icloud.sh"
```

3. **手动合并更新**（保留你的私密信息）

---

## 📋 需要配置的脚本

### sync_modules_to_icloud.sh
```bash
# 需要替换的占位符:
SURGE_ICLOUD_DIR="/Users/YOUR_USERNAME/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents"
SHADOWROCKET_ICLOUD_DIR="/Users/YOUR_USERNAME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules"
```

### sync_modules_to_shadowrocket.sh
```bash
# 需要替换的占位符:
SHADOWROCKET_MODULE_DIR="/Users/YOUR_USERNAME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules"
```

### merge_adblock_modules.sh
```bash
# 需要替换的占位符:
SHADOWROCKET_MODULE_DIR="/Users/YOUR_USERNAME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules"
#!author=YOUR_AUTHOR_NAME
#!homepage=https://github.com/YOUR_USERNAME/YOUR_REPO
```

---

## 🛡️ 安全建议

1. ✅ **永远不要**直接编辑 `scripts/sync/` 中的公开版本
2. ✅ **始终使用** `conf隐私🔏/scripts🔏/` 中的私密版本
3. ✅ **定期检查** `.gitignore` 确保私密目录被排除
4. ✅ **提交前检查** 确保没有泄露个人信息

### 检查命令
```bash
# 检查是否有敏感信息将被提交
git status
git diff

# 确认私密目录被忽略
git check-ignore "conf隐私🔏/scripts🔏/"
# 应该输出: conf隐私🔏/scripts🔏/
```

---

## 📝 占位符列表

| 占位符 | 说明 | 示例 |
|--------|------|------|
| `YOUR_USERNAME` | macOS用户名 | `john` |
| `YOUR_AUTHOR_NAME` | 作者名称 | `John Doe` |
| `YOUR_REPO` | GitHub仓库名 | `my-surge-config` |
| `YOUR_DDNS_DOMAIN` | DDNS域名 | `example.ddns.net` |

---

## 🔄 工作流程

```
┌─────────────────────────────────────────┐
│  Git仓库 (公开)                          │
│  scripts/sync/*.sh                      │
│  ├─ 包含占位符                           │
│  └─ 供他人参考使用                       │
└─────────────────────────────────────────┘
              │
              │ git pull (更新)
              ↓
┌─────────────────────────────────────────┐
│  本地私密目录 (不上传)                   │
│  conf隐私🔏/scripts🔏/*.sh              │
│  ├─ 包含真实信息                         │
│  ├─ 实际运行的脚本                       │
│  └─ 被 .gitignore 排除                  │
└─────────────────────────────────────────┘
              │
              │ 执行脚本
              ↓
┌─────────────────────────────────────────┐
│  iCloud 同步                             │
│  ├─ Surge iCloud                        │
│  └─ Shadowrocket iCloud                 │
└─────────────────────────────────────────┘
```

---

**最后更新**: 2025-12-04
