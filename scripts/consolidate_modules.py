#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Surge module consolidation: metadata JSON, duplicate detection, and sanitization.

- Preserves #!arguments / #!arguments-desc per module (never merge across files)
- Dedupes [Script], [URL Rewrite], etc. within each file
- Flags modules whose rules overlap PROMAX (install PROMAX only for domain blocking)
"""

import json
import os
import re
import sys
import urllib.parse
from datetime import datetime
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
sys.path.insert(0, str(SCRIPT_DIR))

from lib.module_sanitizer import parse_module, sanitize_file_content

PROJECT_ROOT = SCRIPT_DIR.parent
MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)"
SR_MODULE_DIR = PROJECT_ROOT / "module" / "shadowrocket"
OUTPUT_DIR = PROJECT_ROOT / "module"
PROMAX_NAME = "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule"

CATEGORIES = {
    "amplify_nexus": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "head_expanse": "『 🔝 Head Expanse › 首端扩域 』",
    "narrow_pierce": "『 🎯 Narrow Pierce › 窄域穿刺 』",
}

# Standalone modules superseded by a merged bundle (do not install both)
MERGED_ALIASES = {
    "BiliBili.Enhanced.sgmodule": "📺 BiliBili增强合集",
    "BiliBili.Global.sgmodule": "📺 BiliBili增强合集",
    "BiliBili.Redirect.sgmodule": "📺 BiliBili增强合集",
    "YouTube.Enhance.sgmodule": "📺 YouTube增强合集",
    "iRingo.Maps.sgmodule": "🍎 Apple服务增强合集",
    "iRingo.WeatherKit.sgmodule": "🍎 Apple服务增强合集",
}


def scan_modules():
    deduped_modules = {}
    for cat_key in CATEGORIES:
        cat_path = MODULE_DIR / cat_key
        if not cat_path.exists():
            continue
        for module_file in cat_path.glob("*.sgmodule"):
            info = {
                "id": module_file.stem,
                "filename": module_file.name,
                "category": cat_key,
                "path": str(module_file.relative_to(PROJECT_ROOT)),
                "has_arguments": False,
                "merged_into": MERGED_ALIASES.get(module_file.name),
                "install_url": (
                    f"https://raw.githubusercontent.com/nowaytouse/script_hub/master/"
                    f"{urllib.parse.quote(str(module_file.relative_to(PROJECT_ROOT)))}"
                ),
            }
            try:
                content = module_file.read_text(encoding="utf-8")
                meta, _ = parse_module(content)
                info["name"] = meta.get("name", module_file.stem)
                info["desc"] = meta.get("desc", "")
                info["tags"] = [t.strip() for t in meta.get("tag", "").split(",") if t.strip()]
                info["has_arguments"] = "arguments" in meta
                if info["merged_into"]:
                    info["essential"] = False
                    info["note"] = f"已合并进「{info['merged_into']}」，请勿重复安装"
            except Exception as exc:
                print(f"  ❌ Error parsing {module_file.name}: {exc}")
                continue

            dedupe_key = (cat_key, urllib.parse.unquote(module_file.name))
            existing = deduped_modules.get(dedupe_key)
            prefer = existing is None or ("%" in module_file.name and "%" not in existing["filename"])
            if prefer:
                deduped_modules[dedupe_key] = info

    return sorted(
        deduped_modules.values(),
        key=lambda item: (item["category"], item["filename"].lower()),
    )


def sanitize_tree(base_dir: Path, pattern: str) -> int:
    changed = 0
    for path in sorted(base_dir.rglob(pattern)):
        if path.name == PROMAX_NAME:
            continue
        original = path.read_text(encoding="utf-8")
        cleaned = sanitize_file_content(original, dedupe=True)
        if cleaned != original:
            path.write_text(cleaned, encoding="utf-8")
            changed += 1
            print(f"  🧹 Sanitized: {path.relative_to(PROJECT_ROOT)}")
    return changed


def generate_json_data(modules):
    output_file = OUTPUT_DIR / "modules_data.json"
    data = {
        "generated": datetime.now().isoformat(),
        "total": len(modules),
        "policy": {
            "adblock": "仅安装 PROMAX + catalog 分片；勿重复安装 local_sources 源模块",
            "features": "功能模块各自保留 #!arguments；已标注 merged_into 的勿重复安装",
        },
        "modules": modules,
    }
    output_file.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"✅ Metadata saved to {output_file.name}")


if __name__ == "__main__":
    print("=" * 60)
    print("📦 Surge / Shadowrocket Module Consolidation")
    print("=" * 60)

    print("\n🧹 Sanitizing Surge modules...")
    surge_changed = sanitize_tree(MODULE_DIR, "*.sgmodule")

    print("\n🧹 Sanitizing Shadowrocket modules...")
    sr_changed = 0
    if SR_MODULE_DIR.exists():
        sr_changed = sanitize_tree(SR_MODULE_DIR, "*.module")

    modules = scan_modules()
    print(f"\n🔍 Scanned {len(modules)} Surge modules ({surge_changed} sanitized)")
    for cat, label in CATEGORIES.items():
        count = sum(1 for m in modules if m["category"] == cat)
        print(f"  - {label}: {count}")

    if modules:
        generate_json_data(modules)

    print(f"✅ Shadowrocket sanitized: {sr_changed} files")
    print("=" * 60)
    print("✅ Consolidation Complete!")
