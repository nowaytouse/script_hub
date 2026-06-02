# 📦 Module Maintenance Guide

This document maps modules to their generation scripts for proper maintenance workflow.

## ⚠️ Important: DO NOT manually edit module files!
All module files should be regenerated through their respective maintenance scripts.

## Maintenance Scripts & Module Mapping

### Head Expanse (首端扩域) - Security & Protection

| Module | Maintenance Script | Command |
|--------|-------------------|---------|
| 🔥 Firewall Port Blocker | `scripts/firewall_sync.py` | `python3 scripts/firewall_sync.py --execute` |
| 📱 Universal Ad-Blocking Rules (PROMAX) | N/A - Manual management | - |
| 🚫 PROMAX Dependency Component | N/A - Manual management | - |

### Amplify Nexus (增幅枢纽) - Enhancement & Features

| Module | Maintenance Script | Command |
|--------|-------------------|---------|
| 📺 YouTube增强合集 | `scripts/maintenance/merge_youtube_bundle.py` | `python3 scripts/maintenance/merge_youtube_bundle.py` |
| 📺 BiliBili增强合集 | `scripts/maintenance/merge_bilibili_bundle.py` | `python3 scripts/maintenance/merge_bilibili_bundle.py` |
| 🍎 Apple服务增强合集 | `scripts/maintenance/merge_apple_modules.py` | `python3 scripts/maintenance/merge_apple_modules.py` |
| Reddit 增强合集 | N/A - Static | - |
| WeChat_Enhance | N/A - Static | - |
| Script Hub 重写 & 规则集转换 | N/A - Static | - |
| Other modules | N/A - Static | - |

## Workflow: How to Fix a Broken Module

### Problem: Module content is empty or incomplete

**✗ WRONG**: Manually edit the `.sgmodule` file

**✓ CORRECT**: 
1. Identify which script generates this module (see table above)
2. Run: `python3 scripts/maintenance/merge_{module}.py`
3. Verify the module was regenerated correctly
4. Commit and push: `git add . && git commit -m "Regenerate {module}" && git push`

### Problem: Module has configuration errors

**Root causes**:
- Port rule missing action: `DEST-PORT,8076` ❌ should be `DEST-PORT,8076,REJECT-DROP` ✅
- Invalid section format
- Missing MITM hostname

**Solution**: Fix the upstream script, not the module:
1. Locate the source data (e.g., `ruleset/Sources/conf/SurgeConf_DirectPorts.list`)
2. Fix the source data
3. Re-run the maintenance script
4. Verify output
5. Commit changes

## Validation Rules

### Firewall Module (`firewall_sync.py`)
- All port rules MUST have an action (3+ comma-separated values)
- Valid formats: `DEST-PORT,<port>,<action>`, `IN-PORT,<host>:<port>,<action>`
- Script now validates this before writing

### YouTube/BiliBili Modules
- Modules must contain at least 100 bytes
- Script verifies content size before writing
- Checks for required sections (Script, MITM, etc.)

## Quick Reference

```bash
# Refresh firewall rules
python3 scripts/firewall_sync.py --execute

# Refresh YouTube bundle
python3 scripts/maintenance/merge_youtube_bundle.py

# Refresh BiliBili bundle
python3 scripts/maintenance/merge_bilibili_bundle.py

# View changes
git diff

# Commit and push
git add .
git commit -m "Update modules via maintenance scripts"
git push origin master
```

---
Last updated: 2026-06-02
