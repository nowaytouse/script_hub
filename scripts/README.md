# Script Hub — 脚本目录

## 唯一入口（日常 / CI）

```bash
# 全量更新：规则集 + 广告分片 + 模块转换 + SRS + 可选 git push
python3 scripts/main_update.py --execute

# CI / 无人值守（等同 --execute）
python3 scripts/main_update.py --unattended

# 本地可选：更新 sing-box / mihomo 二进制后再跑流水线
python3 scripts/main_update.py --with-core --execute
```

GitHub Actions 调用链见 [`.github/workflows/update_rulesets.yml`](../.github/workflows/update_rulesets.yml)（内部执行 `main_update.py --execute`）。

## 目录结构

| 路径 | 职责 |
|------|------|
| `main_update.py` | **主编排器**（上游同步 → 规则合并 → AdBlock → 模块 → SRS → 汇总 → Git） |
| `adblock_manager.py` | 广告规则分片 + PROMAX 模块 |
| `ruleset_manager.py` | 业务 ruleset 增量合并、策略头、弃用清理 |
| `smart_cleanup.py` | 跨 ruleset 去重 |
| `upstream_sync.py` | skk / MetaCubeX / nexus 同步 |
| `convert_surge_to_shadowrocket.py` | Surge → 小火箭（保留 `#!arguments` 与 AdBlock RULE-SET） |
| `consolidate_modules.py` | 模块清洗 + `module/modules_data.json` + `surge_module_helper.html` |
| `lib/module_sanitizer.py` | 模块段内去重、标准段顺序 |
| `lib/module_catalog.py` | 模块扫描 / JSON / HTML |
| `lib/merge_upstream_bundle.py` | 上游多模块合并（Bili/YouTube/Weibo 维护脚本共用） |
| `lib/pipeline_report.py` | 流水线结束统计与 SRS 覆盖率检查 |
| `lib/common.py` | 日志、路径、安全下载 |
| `tools/update_cores.sh` | 本地 sing-box / mihomo 更新（`--with-core`） |
| `maintenance/` | 按需手动维护（合集刷新、iCloud 导入等） |

## 文档

- [docs/AdBlock_callchain.md](../docs/AdBlock_callchain.md) — 广告规则流水线
- [docs/Modules_callchain.md](../docs/Modules_callchain.md) — 功能模块与 PROMAX 分工
- [ruleset/AdBlock/README.md](../ruleset/AdBlock/README.md) — 分片索引

## 已合并 / 废弃（仍保留薄包装，会提示改用 consolidate）

- `build_module_helper.py`
- `generate_helper_v2.py`
- `update_helper_web.py`

原 `scripts/archive/` 下 shell 流水线已并入 Python；历史脚本已删除。

## 按需维护（不纳入 main_update）

- `maintenance/merge_bilibili_bundle.py` — 刷新 BiliBili 增强合集
- `maintenance/merge_youtube_bundle.py` — 刷新 YouTube 增强合集
- `maintenance/merge_weibo_bundle.py` — 刷新微博去广告合集
- `maintenance/merge_apple_modules.py` — Apple 服务增强合集
- `maintenance/merge_dns_modules.py` — DNS 模块合并
- `maintenance/import_from_icloud_sr.py` — 从本机 iCloud 小火箭目录导入

执行后请运行：

```bash
python3 scripts/consolidate_modules.py
python3 scripts/convert_surge_to_shadowrocket.py
```
