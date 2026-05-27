# scripts/archive — 历史脚本（已废弃）

此目录中的 **shell 脚本** 已由 `scripts/main_update.py` 及对应 Python 模块取代。

| 旧脚本 | 现用替代 |
|--------|----------|
| `full_update.sh` / `incremental_merge_all.sh` | `main_update.py` |
| `merge_adblock_*.sh` | `adblock_manager.py` |
| `sync_metacubex_rules.sh` | `upstream_sync.py` + `srs_generator.py` |
| `download_modules.sh` | `upstream_sync.py` |
| `merge_*_modules.sh` (bilibili/weibo/youtube) | `maintenance/merge_*.py` 或 amplify 合集模块 |

**请勿在 CI 或文档中引用本目录脚本。**

一次性工具：`deprecated_update_json_metadata.py`（硬编码路径，仅作历史参考）。
