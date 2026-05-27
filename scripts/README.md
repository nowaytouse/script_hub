# Script Hub — 脚本目录

## 唯一入口（日常 / CI）

```bash
# 全量更新：规则集 + 广告分片 + 模块转换 + SRS + 可选 git push
python3 scripts/main_update.py --execute

# 仅模块：清洗、去重、生成 modules_data.json 与导入助手 HTML
python3 scripts/consolidate_modules.py
```

GitHub Actions 调用链见 [`.github/workflows/update_rulesets.yml`](../.github/workflows/update_rulesets.yml)（内部执行 `main_update.py --execute`）。

## 目录结构

| 路径 | 职责 |
|------|------|
| `main_update.py` | **主编排器**（上游同步 → 规则合并 → AdBlock → 模块 → SRS → Git） |
| `adblock_manager.py` | 广告规则分片 + PROMAX 模块 |
| `ruleset_manager.py` | 业务 ruleset 增量合并 |
| `smart_cleanup.py` | 跨 ruleset 去重 |
| `upstream_sync.py` | skk / MetaCubeX / nexus 同步 |
| `convert_surge_to_shadowrocket.py` | Surge → 小火箭（保留 `#!arguments` 与 AdBlock RULE-SET） |
| `consolidate_modules.py` | 模块清洗 + `module/modules_data.json` + `surge_module_helper.html` |
| `lib/module_sanitizer.py` | 模块段内去重、标准段顺序 |
| `lib/module_catalog.py` | 模块扫描 / JSON / HTML（供 consolidate 调用） |
| `lib/common.py` | 日志、路径、读写工具 |
| `maintenance/` | 按需手动维护（iCloud 导入、MITM 清理等） |
| `archive/` | 已废弃的 shell / 一次性脚本（勿在 CI 使用） |

## 文档

- [docs/AdBlock_callchain.md](../docs/AdBlock_callchain.md) — 广告规则流水线
- [docs/Modules_callchain.md](../docs/Modules_callchain.md) — 功能模块与 PROMAX 分工
- [ruleset/AdBlock/README.md](../ruleset/AdBlock/README.md) — 分片索引

## 已合并 / 废弃（仍保留薄包装，会提示改用 consolidate）

- `build_module_helper.py`
- `generate_helper_v2.py`
- `update_helper_web.py`

## 按需维护（不纳入 main_update）

- `maintenance/merge_apple_modules.py` — Apple 合集
- `maintenance/merge_dns_modules.py` — DNS 模块合并
- `maintenance/import_from_icloud_sr.py` — 从本机 iCloud 小火箭目录导入（需改路径）
