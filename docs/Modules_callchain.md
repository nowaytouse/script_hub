# 功能模块调用链与去重策略

## 设计原则

| 层级 | 职责 | 用户安装 |
|------|------|----------|
| **PROMAX** | 域名级广告拦截（RULE-SET 分片 + 少量复杂 Rule + local_sources 脚本） | 1 个 head_expanse 模块 |
| **amplify_nexus** | 带 `#!arguments` 的功能增强（B站/YouTube/iRingo/Sub-Store…） | 按需各装各的，**保留独立配置** |
| **narrow_pierce** | App 专项去广告脚本 | 按需 |
| **local_sources** | PROMAX 的**源文件**，勿直接安装 | 否 |

**禁止**：把多个功能模块的 `#!arguments` 合并进 PROMAX；禁止把 Nexus 里带配置的增强模块 Script 段并入 PROMAX。

## 流水线

```
main_update.py
  ├─ AdBlockManager.merge()
  │    ├─ 远程/清单源 → 仅提取 Rule → ruleset/AdBlock/*.list
  │    ├─ local_sources → Rule + Script/Rewrite（去重后写入 PROMAX）
  │    └─ amplify_nexus 广告类 → 仅 Rule 进分片（不并 Script）
  ├─ convert_surge_to_shadowrocket.py
  │    ├─ 保留 #!arguments / #!arguments-desc
  │    ├─ AdBlock 分片保留 RULE-SET（不内联百万行）
  │    └─ module_sanitizer 段内去重
  └─ consolidate_modules.py
       ├─ 全库 .sgmodule / .module 格式化 + 段内去重
       ├─ modules_data.json（含 merged_into / install_url）
       └─ 跳过 PROMAX 本体（由 AdBlockManager 生成）
```

## 去重范围

- **规则**：`AdBlockManager` 跨分类 dedup + `smart_cleanup` 跨 ruleset
- **模块段**：`lib/module_sanitizer.py` 按 Script 名 / URL Rewrite 模式 / MITM 主机名
- **安装面**：`modules_data.json` 标注已合并进合集的旧模块（如 BiliBili.* → 📺 BiliBili增强合集）

## 相关文档

- 广告规则分片：[`ruleset/AdBlock/README.md`](../ruleset/AdBlock/README.md)
- 广告流水线：[`AdBlock_callchain.md`](AdBlock_callchain.md)
