# Script Hub — 脚本目录

## 自动维护（推荐）

**master 上的规则集由 GitHub Actions 自动更新**，无需本地定时跑 `--execute`：

| 触发 | 频率 | 工作流 |
|------|------|--------|
| `schedule` | 每天 3 次（北京时间约 12:17 / 20:17 / 04:17，错开整点减轻 Actions 排队） | [update_rulesets.yml](../.github/workflows/update_rulesets.yml) |
| `workflow_dispatch` | 手动 | 同上 |

跑完后会 **自动 push 到 master**（raw/CDN 才能用到最新规则）；提交者为 `github-actions[bot]`，消息 `chore(ruleset): automated update …`。

Push 前默认 **等待 3 分钟** 再 `pull --rebase` 后 push，避免与其它 workflow 抢仓库；仅 **bot 的 chore(ruleset) push** 不会再次触发本工作流。需要更长间隔可在 workflow 里把 `PUSH_COOLDOWN_SECONDS` 改为 `1800`（30 分钟）。

本地日常不必 `--execute` push；调试可只跑 `main_update.py` 不 push。

## 本地入口（调试 / 紧急）

```bash
# 全量跑流水线但不 push
python3 scripts/main_update.py

# 本地确认后再 push（非日常路径）
python3 scripts/main_update.py --execute

# 与 CI 相同：跑完并 push（紧急时用）
python3 scripts/main_update.py --unattended

# 可选：先更新本机 sing-box / mihomo
python3 scripts/main_update.py --with-core --execute
```

sing-box CI 兜底版本见 [`.github/ci/sing-box-linux-amd64.version`](../.github/ci/sing-box-linux-amd64.version)。

## 目录结构

| 路径 | 职责 |
|------|------|
| `main_update.py` | **主编排器**（上游同步 → 规则合并 → AdBlock → 模块 → SRS → 汇总 → Git） |
| `adblock_manager.py` | 广告规则分片 + PROMAX 模块 |
| `ruleset_manager.py` | 业务 ruleset 增量合并、策略头、弃用清理 |
| `smart_cleanup.py` | 跨 ruleset 去重 |
| `upstream_sync.py` | skk / MetaCubeX / nexus 同步 |
| `merge_smart_config_kit.py` | Smart-Config-Kit 补充规则合并（从本地 vendor） |
| `firewall_sync.py` | 防火墙端口模块同步 |
| `convert_surge_to_shadowrocket.py` | Surge → 小火箭（保留 `#!arguments` 与 AdBlock RULE-SET） |
| `consolidate_modules.py` | 模块清洗 + 分组校正 + `modules_data.json` + `surge_module_helper.html` |
| `srs_generator.py` | 编译 Sing-box `.srs` 规则集 |
| `generate_surge_host_dns.py` | 将 DNS_mapping ruleset 展开为 Surge `[Host]` 条目 |
| `core/common.py` | 日志、路径、安全下载 |
| `core/surge_compliance.py` | **Surge RULE-SET 合规单源**（URL-REGEX 截断、DOMAIN-REGEX 禁入、IP no-resolve） |
| `core/rule_processor.py` | 规则标准化（依赖 `surge_compliance`） |
| `core/module_sanitizer.py` | 模块段内去重、标准段顺序 |
| `core/module_catalog.py` | 模块扫描 / 分组校正 / JSON / HTML |
| `core/merge_upstream_bundle.py` | 上游多模块合并（维护脚本共用） |
| `core/pipeline_report.py` | 流水线结束统计与 SRS 覆盖率检查 |
| `qa/update_cores.sh` | 本地 sing-box / mihomo 更新（`--with-core`） |
| `tasks/` | 按需手动维护（合集刷新、iCloud 导入等） |

## 文档

- [docs/AdBlock_callchain.md](../docs/AdBlock_callchain.md) — 广告规则流水线
- [docs/Modules_callchain.md](../docs/Modules_callchain.md) — 功能模块与 PROMAX 分工
- [docs/maintenance/module-maintenance.md](../docs/maintenance/module-maintenance.md) — 模块维护映射与修复流程
- [docs/standards/module-header-spec.md](../docs/standards/module-header-spec.md) — 模块头规范（iOS 清空问题防线）
- [rulesets/AdBlock/README.md](../rulesets/AdBlock/README.md) — 分片索引

## 合规门禁（Surge Invalid line 回归）

```bash
# 单元回归（053a145d / 427a9228 / 0f524ff5 等事故）
python3 scripts/qa/test_surge_compliance.py

# 扫描已生成的 .list 文件
python3 scripts/qa/validate_surge_rulesets.py

# 扫描所有模块头结构（防止 iOS 模块字段清空）
python3 scripts/qa/validate_module_headers.py
```

`main_update.py` 启动时会跑前者；合并后会跑后者。CI 在 `update_rulesets.yml` 中同样执行。

## 按需维护（不纳入 main_update）

```bash
# 刷新功能模块合集（执行后运行 consolidate + convert）
python3 scripts/tasks/merge_bilibili_bundle.py
python3 scripts/tasks/merge_youtube_bundle.py
python3 scripts/tasks/merge_weibo_bundle.py
python3 scripts/tasks/merge_apple_modules.py

# DNS Host 段重新生成
python3 scripts/generate_surge_host_dns.py --write

# iCloud 小火箭导入（需先改脚本内 SR_DIR 路径）
python3 scripts/tasks/import_from_icloud_sr.py
```
