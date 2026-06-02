# Script Hub

Surge / Shadowrocket 规则集与模块仓库：广告拦截分片、功能增强模块、Sing-box SRS，由 Python 流水线自动维护。

## 快速开始

| 目标 | 操作 |
|------|------|
| 规则集自动更新（推荐） | Actions `Update Rulesets`：跑完 **自动 push**（push 前冷却 ≥3 分钟，见 [scripts/README.md](scripts/README.md)） |
| 本地紧急全量并推送 | `python3 scripts/main_update.py --execute` |
| 仅刷新模块目录与导入页 | `python3 scripts/consolidate_modules.py` |
| 安装广告拦截 | 只装 **PROMAX** → 链接见 [`rulesets/AdBlock/catalog.json`](rulesets/AdBlock/catalog.json) |
| 安装 B站/YouTube 等增强 | 打开 [`modules/surge_module_helper.html`](modules/surge_module_helper.html) 复制对应模块链接 |

## 仓库结构

```
script_hub/
├── .github/workflows/     # CI：定时 ruleset 更新
├── scripts/               # 维护脚本（见 scripts/README.md）
├── rulesets/               # Surge / AdBlock 分片 / SingBox SRS
├── modules/                # Surge(.sgmodule) 与 Shadowrocket(.module)
├── docs/                  # 调用链文档
└── Smart-Config-Kit/      # 多客户端策略参考（子项目）
```

## 文档

- [scripts/README.md](scripts/README.md) — 脚本入口与目录说明
- [docs/AdBlock_callchain.md](docs/AdBlock_callchain.md) — 广告规则流水线
- [docs/Modules_callchain.md](docs/Modules_callchain.md) — PROMAX vs 功能模块
- [rulesets/AdBlock/README.md](rulesets/AdBlock/README.md) — 广告分片索引

## 原则

1. **PROMAX** 只做域名级拦截（RULE-SET 分片）；不合并各增强模块的 `#!arguments`。
2. **功能模块** 各自独立安装，保留 Surge/小火箭模块配置界面。
3. **去重**：规则在 `smart_cleanup` + AdBlock 层；模块段在 `module_sanitizer`。
4. **勿重复安装** `modules/local_sources` 及 `modules_data.json` 中标注 `merged_into` 的旧模块。

## 其他

- MosDNS 配置见 [dns/README.md](dns/README.md)
