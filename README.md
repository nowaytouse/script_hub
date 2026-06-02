# Script Hub

## 原则

1. **PROMAX** 只做域名级拦截（RULE-SET 分片）；不合并各增强模块的 `#!arguments`。
2. **功能模块** 各自独立安装，保留 Surge/小火箭模块配置界面。
3. **去重**：规则在 `smart_cleanup` + AdBlock 层；模块段在 `module_sanitizer`。
4. **勿重复安装** `modules/local_sources` 及 `modules_data.json` 中标注 `merged_into` 的旧模块。

