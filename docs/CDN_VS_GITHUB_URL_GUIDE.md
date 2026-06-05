# CDN vs GitHub URL 使用指南

## 概述

Script Hub 现在为所有模块和规则集提供**两种 URL 版本**：

1. **CDN URL** (jsDelivr) - 推荐日常使用
2. **GitHub Raw URL** - 用于立即访问最新更新

---

## 两种 URL 的区别

### 📦 CDN URL (jsDelivr)

```
https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/...
```

**优点 ✅**
- 🚀 **全球加速**：通过 jsDelivr 全球 CDN 节点分发
- ⚡ **极速访问**：缓存响应，加载速度快
- 🌍 **稳定可靠**：多节点容错，高可用性
- 🇨🇳 **国内友好**：在中国大陆访问速度优秀

**缺点 ⚠️**
- ⏰ **缓存延迟**：更新后需要 **12-24 小时** CDN 缓存才会刷新
- 🔄 **非即时**：推送新版本后，CDN 上的文件不会立即更新

**适用场景 📌**
- 日常使用（默认推荐）
- 对更新时效性要求不高
- 需要最佳加载速度
- 网络环境不稳定的用户

---

### 🐙 GitHub Raw URL

```
https://raw.githubusercontent.com/nowaytouse/script_hub/master/...
```

**优点 ✅**
- ⚡ **即时更新**：推送到 GitHub 后立即生效
- 🎯 **最新版本**：永远获取最新的文件内容
- 🔒 **无缓存**：直接从 GitHub 读取，无中间层

**缺点 ⚠️**
- 🐌 **速度较慢**：无 CDN 加速，直连 GitHub 服务器
- 🌐 **地域差异**：中国大陆访问可能较慢或不稳定
- 📉 **可靠性**：依赖 GitHub 单点服务

**适用场景 📌**
- 需要立即获取最新更新
- CDN 缓存未刷新时的临时方案
- 测试新功能或修复
- 对速度要求不高，但需要最新版本

---

## 使用方法

### 1️⃣ 模块安装 (Surge / Shadowrocket)

#### 方法 A: 通过 Helper HTML 复制链接 (推荐)

访问 Helper 页面：
```
https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/helper/surge_module_helper.html
```

在页面右上角的 URL 源切换按钮中选择：
- **CDN** (默认) - 使用 jsDelivr CDN 链接
- **GitHub** - 使用 GitHub Raw 链接

点击"复制链接"后，URL 会根据当前选择的源自动切换。

#### 方法 B: 手动拼接 URL

**CDN 版本 (推荐)**
```
https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/surge/amplify_nexus/📺 BiliBili增强合集.sgmodule
```

**GitHub 版本 (立即生效)**
```
https://raw.githubusercontent.com/nowaytouse/script_hub/master/modules/surge/amplify_nexus/📺 BiliBili增强合集.sgmodule
```

**注意**：特殊字符（空格、表情符号、括号等）在 URL 中需要编码：
- 空格 → `%20`
- 📺 → `%F0%9F%93%BA`
- ( → `%28`
- ) → `%29`

Helper HTML 会自动处理 URL 编码。

---

### 2️⃣ AdBlock 规则集

#### 在 catalog.json 中查看两种 URL

```bash
# 查看 AdBlock catalog
curl https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/catalog.json
```

每个规则集 shard 都包含两个字段：

```json
{
  "file": "rulesets/AdBlock/AdBlock_Advertising_03.list",
  "category": "Advertising",
  "rule_count": 76178,
  "cdn_url": "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_Advertising_03.list",
  "github_url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Advertising_03.list"
}
```

#### 在 Surge 配置中使用

**使用 CDN 版本 (推荐)**
```ini
[Rule]
RULE-SET,https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_Advertising_01.list,REJECT
RULE-SET,https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_Advertising_02.list,REJECT
RULE-SET,https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_Advertising_03.list,REJECT
```

**使用 GitHub 版本 (立即生效)**
```ini
[Rule]
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Advertising_01.list,REJECT
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Advertising_02.list,REJECT
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Advertising_03.list,REJECT
```

---

### 3️⃣ PROMAX 模块

#### Surge 版本

**CDN (推荐)**
```
https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/surge/head_expanse/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.sgmodule
```

**GitHub (立即生效)**
```
https://raw.githubusercontent.com/nowaytouse/script_hub/master/modules/surge/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule
```

#### Shadowrocket 版本

**CDN (推荐)**
```
https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/shadowrocket/head_expanse/%F0%9F%9A%AB%20Universal%20Ad-Blocking%20Rules%20Dependency%20Component%20PROMAX%20%28Kali-style%29.module
```

**GitHub (立即生效)**
```
https://raw.githubusercontent.com/nowaytouse/script_hub/master/modules/shadowrocket/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module
```

---

## 推荐策略

### 🎯 日常使用流程

1. **默认使用 CDN URL**
   - 安装模块或规则集时，优先使用 CDN 链接
   - 享受全球加速和稳定性

2. **更新发布后的等待期**
   - GitHub 推送新版本后，等待 **12-24 小时** CDN 缓存刷新
   - 可通过 commit 时间戳判断更新是否应该生效

3. **需要立即使用最新版本时**
   - 切换到 GitHub Raw URL
   - 测试新功能或验证修复
   - CDN 缓存刷新后再切回 CDN URL

### ⚡ 应急使用场景

**场景 1: Telegram 被屏蔽修复**
```
问题：刚推送了 IP-CIDR 白名单修复 (commit c170e72f)
需求：立即测试 Telegram 是否恢复

解决方案：
1. 临时使用 GitHub Raw URL 的 AdBlock 规则集
2. 验证 Telegram IP (149.154.175.59 等) 不再被屏蔽
3. 24小时后切回 CDN URL
```

**场景 2: 紧急安全更新**
```
问题：发现严重广告拦截规则错误
需求：立即部署修复版本

解决方案：
1. 运行 main_update.py 生成新规则
2. 推送到 GitHub
3. 使用 GitHub Raw URL 立即获取修复
4. 运行 purge_cache.py 加速 CDN 刷新
```

**场景 3: 功能测试**
```
问题：开发新模块需要测试
需求：快速迭代，立即看到效果

解决方案：
1. 开发时使用 GitHub Raw URL
2. 每次推送后立即生效
3. 测试通过后再使用 CDN URL 正式发布
```

---

## CDN 缓存刷新

### 自动刷新

CDN 缓存会在 **12-24 小时**后自动过期并刷新。

### 手动刷新 (加速)

运行缓存清除脚本：

```bash
python3 scripts/pipeline/purge_cache.py
```

**注意**：手动刷新也需要一定时间传播到全球节点，通常 **10-30 分钟**。

### 验证 CDN 缓存状态

使用 commit hash URL (特定版本)：

```
# 使用 commit hash 而不是 @master
https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@c170e72f/rulesets/AdBlock/AdBlock_Advertising_03.list
```

这会绕过 `@master` 分支缓存，直接访问特定 commit 的文件。

---

## URL 转换对照表

| CDN Pattern | GitHub Raw Pattern |
|------------|-------------------|
| `https://cdn.jsdelivr.net/gh/{owner}/{repo}@{branch}/` | `https://raw.githubusercontent.com/{owner}/{repo}/{branch}/` |
| `@master` | `/master/` |
| `@main` | `/main/` |
| `@{commit_hash}` | `/{commit_hash}/` |

**示例转换**：

```javascript
// CDN → GitHub Raw 转换函数
function convertUrl(url) {
  return url.replace(
    /https:\/\/cdn\.jsdelivr\.net\/gh\/([^\/]+)\/([^\/]+)@([^\/]+)\//,
    'https://raw.githubusercontent.com/$1/$2/$3/'
  );
}

// GitHub Raw → CDN 转换函数
function convertUrlToCDN(url) {
  return url.replace(
    /https:\/\/raw\.githubusercontent\.com\/([^\/]+)\/([^\/]+)\/([^\/]+)\//,
    'https://cdn.jsdelivr.net/gh/$1/$2@$3/'
  );
}
```

---

## 常见问题 FAQ

### Q1: 为什么 CDN 版本不是最新的？

**A**: jsDelivr 使用 12-24 小时缓存来提高全球访问速度。这是 CDN 的正常行为。如需立即使用最新版本，请切换到 GitHub Raw URL。

### Q2: GitHub Raw URL 在中国大陆能访问吗？

**A**: 可以访问，但速度可能较慢或不稳定。如果遇到访问问题，建议：
1. 使用代理工具
2. 等待 CDN 缓存刷新后使用 CDN URL
3. 使用 commit hash URL 绕过 @master 缓存

### Q3: Helper HTML 切换 URL 后，已复制的链接会变吗？

**A**: 不会。已复制的链接保持不变。切换 URL 源后，需要重新点击"复制链接"才会使用新的 URL 类型。

### Q4: 两种 URL 的文件内容一样吗？

**A**: 在 CDN 缓存刷新后，内容完全一致。在刷新前，GitHub Raw URL 始终是最新的，CDN URL 可能是旧版本（最多延迟 24 小时）。

### Q5: 如何判断应该使用哪种 URL？

**决策流程**：
```
需要最新版本？
├─ 是 → 使用 GitHub Raw URL
└─ 否 → 需要最快速度？
    ├─ 是 → 使用 CDN URL
    └─ 否 → 网络环境稳定？
        ├─ 是 → CDN URL (推荐)
        └─ 否 → 根据实际测试选择
```

### Q6: 可以混用两种 URL 吗？

**A**: 可以！在配置中混用没有问题。例如：
```ini
# 重要的规则用 GitHub Raw (确保最新)
RULE-SET,https://raw.githubusercontent.com/.../AdBlock_Security_01.list,REJECT

# 一般规则用 CDN (速度优先)
RULE-SET,https://cdn.jsdelivr.net/gh/.../AdBlock_Advertising_01.list,REJECT
```

---

## 技术实现

### catalog.json 结构

```json
{
  "shards": [
    {
      "file": "rulesets/AdBlock/AdBlock_Advertising_03.list",
      "category": "Advertising",
      "rule_count": 76178,
      "cdn_url": "https://cdn.jsdelivr.net/gh/...",
      "github_url": "https://raw.githubusercontent.com/..."
    }
  ],
  "modules": {
    "surge_promax": {
      "path": "modules/surge/head_expanse/...",
      "install_url": "https://cdn.jsdelivr.net/gh/...",
      "github_install_url": "https://raw.githubusercontent.com/..."
    }
  }
}
```

### Helper HTML JavaScript

```javascript
let urlSource = 'cdn'; // 'cdn' or 'github'

function switchUrlSource(source) {
  urlSource = source;
  // 更新 UI
  document.querySelectorAll('.url-source-btn').forEach(b => 
    b.classList.toggle('active', b.dataset.source === source)
  );
  // 清除已复制状态
  copiedSet.clear();
  // 重新渲染
  render();
}

function convertUrl(url) {
  if (urlSource === 'github') {
    return url.replace(
      /https:\/\/cdn\.jsdelivr\.net\/gh\/([^\/]+)\/([^\/]+)@([^\/]+)\//,
      'https://raw.githubusercontent.com/$1/$2/$3/'
    );
  }
  return url; // 默认返回 CDN URL
}
```

---

## 总结

| 特性 | CDN URL | GitHub Raw URL |
|------|---------|---------------|
| **速度** | ⭐⭐⭐⭐⭐ 极快 | ⭐⭐⭐ 较慢 |
| **稳定性** | ⭐⭐⭐⭐⭐ 高 | ⭐⭐⭐⭐ 中 |
| **实时性** | ⭐⭐ 延迟12-24h | ⭐⭐⭐⭐⭐ 即时 |
| **国内访问** | ⭐⭐⭐⭐⭐ 优秀 | ⭐⭐⭐ 一般 |
| **推荐场景** | 日常使用 | 紧急更新、测试 |

**推荐默认策略**：CDN URL（速度和稳定性优先）  
**应急备用策略**：GitHub Raw URL（立即获取最新更新）

---

**文档更新**: 2026-06-06  
**相关 Commits**: 
- c170e72f: IP-CIDR 白名单修复
- 3fa8133b: 双 URL 支持实现
