# Script Hub

## 原则

1. **PROMAX** 只做域名级拦截（RULE-SET 分片）；不合并各增强模块的 `#!arguments`。
2. **功能模块** 各自独立安装，保留 Surge/小火箭模块配置界面。
3. **去重**：规则在 `smart_cleanup` + AdBlock 层；模块段在 `module_sanitizer`。
4. **勿重复安装** `modules/source/local` 及 `modules/helper/modules_data.json` 中标注 `merged_into` 的旧模块。

## 目录命名

| 路径 | 含义 |
|------|------|
| `rulesets/list/` | Surge / Shadowrocket 编译后的 `.list` 规则集 |
| `modules/source/local/` | PROMAX 构建用的本地 sgmodule 源（上游同步） |
| `modules/helper/surge_module_helper.html` | 模块安装助手网页（`main_update` / `consolidate_modules` 自动再生） |

模块助手页（推送后可用）：
https://raw.githubusercontent.com/nowaytouse/script_hub/master/modules/helper/surge_module_helper.html

Maasea 上游仓库已更名为 [Maasea/sgmodule](https://github.com/Maasea/sgmodule)（旧 `sgmodules` 路径 404）。

## 脚本目录

| 路径 | 用途 |
|------|------|
| `scripts/hub/` | 共享库（合规、模块清洗、合并工具） |
| `scripts/pipeline/` | 主更新流水线（`main_update.py`） |
| `scripts/qa/` | 回归测试与生成树守卫 |
| `scripts/tools/` | 独立维护工具（模块目录、SR 转换等） |

