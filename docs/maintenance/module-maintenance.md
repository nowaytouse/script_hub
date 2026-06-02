# Module Maintenance Guide

This document maps modules to their generation scripts for proper maintenance workflow.

## Important: DO NOT manually edit generated module files

Regenerate through maintenance scripts only.

## Maintenance Scripts and Module Mapping

### Head Expanse (Security / Protection)

| Module | Maintenance Script | Command |
|--------|--------------------|---------|
| Firewall Port Blocker | `scripts/firewall_sync.py` | `python3 scripts/firewall_sync.py --execute` |
| Universal Ad-Blocking Rules (PROMAX) | `scripts/adblock_manager.py` pipeline | `python3 scripts/main_update.py` |
| PROMAX Dependency Component | `scripts/adblock_manager.py` pipeline | `python3 scripts/main_update.py` |

### Amplify Nexus (Enhancement / Features)

| Module | Maintenance Script | Command |
|--------|--------------------|---------|
| YouTube增强合集 | `scripts/tasks/merge_youtube_bundle.py` | `python3 scripts/tasks/merge_youtube_bundle.py` |
| BiliBili增强合集 | `scripts/tasks/merge_bilibili_bundle.py` | `python3 scripts/tasks/merge_bilibili_bundle.py` |
| Apple服务增强合集 | `scripts/tasks/merge_apple_modules.py` | `python3 scripts/tasks/merge_apple_modules.py` |

## Fix Workflow (When a module is broken)

1. Identify the generator script (table above).
2. Fix script/source input, not generated module output.
3. Regenerate modules.
4. Run validators:
   - `python3 scripts/qa/validate_module_headers.py`
   - `python3 scripts/qa/validate_surge_rulesets.py`
5. Commit and push.

## Current Guardrails

- Firewall source rules require actions (`IN-PORT/DEST-PORT,...,<action>`).
- YouTube/BiliBili/Apple merge path uses script-level generation only.
- Module header structure is validated in both local pipeline and CI.

