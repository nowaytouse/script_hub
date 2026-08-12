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
| `modules/helper/surge_module_helper.html` | 模块安装助手网页（由 Rust 更新流程维护） |

模块助手页（推送后可用）：
https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/helper/surge_module_helper.html

Maasea 上游仓库已更名为 [Maasea/sgmodule](https://github.com/Maasea/sgmodule)（旧 `sgmodules` 路径 404）。

## 更新入口

| 路径 | 用途 |
|------|------|
| `create/processor/src/bin/update.rs` | 唯一更新入口（Rust 网络抓取、本地合并、校验、产物生成） |
| `create/processor/src/bin/guard.rs` | 生成树守卫（Rust） |
| `scripts/hub/` | 离线维护工具（不参与自动更新） |
| `scripts/tools/` | 独立维护工具（不参与自动更新） |

Go/Python 主更新入口已移除；GitHub Actions 与部署脚本统一调用 Rust 更新器。
