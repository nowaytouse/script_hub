#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Module catalog pipeline (canonical entry):
  sanitize → scan → modules_data.json → surge_module_helper.html

Replaces: build_module_helper.py, generate_helper_v2.py, update_helper_web.py
"""

import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

from lib.module_catalog import (
    CATEGORIES,
    build_helper_html,
    normalize_categories_tree,
    sanitize_tree,
    scan_modules,
    write_modules_json,
)

PROJECT_ROOT = SCRIPT_DIR.parent
MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)"
SR_MODULE_DIR = PROJECT_ROOT / "module" / "shadowrocket"
MODULES_JSON = PROJECT_ROOT / "module" / "modules_data.json"
HELPER_HTML = PROJECT_ROOT / "module" / "surge_module_helper.html"


def run() -> int:
    print("=" * 60)
    print("📦 Module Catalog (sanitize · scan · JSON · HTML)")
    print("=" * 60)

    print("\n📂 Normalizing module categories (folder = source of truth)...")
    cat_surge = normalize_categories_tree(MODULE_DIR, PROJECT_ROOT, "*.sgmodule")
    cat_sr = normalize_categories_tree(SR_MODULE_DIR, PROJECT_ROOT, "*.module")
    print(f"   Fixed {cat_surge + cat_sr} file(s)")

    print("\n🧹 Sanitizing Surge modules...")
    surge_changed = sanitize_tree(MODULE_DIR, PROJECT_ROOT, "*.sgmodule")

    print("\n🧹 Sanitizing Shadowrocket modules...")
    sr_changed = sanitize_tree(SR_MODULE_DIR, PROJECT_ROOT, "*.module")

    modules = scan_modules(PROJECT_ROOT, MODULE_DIR)
    print(f"\n🔍 Scanned {len(modules)} Surge modules ({surge_changed} files cleaned)")
    for cat, label in CATEGORIES.items():
        print(f"  - {label}: {sum(1 for m in modules if m['category'] == cat)}")

    write_modules_json(modules, MODULES_JSON)
    print(f"✅ Wrote {MODULES_JSON.relative_to(PROJECT_ROOT)}")

    build_helper_html(modules, PROJECT_ROOT, HELPER_HTML)
    print(f"✅ Wrote {HELPER_HTML.relative_to(PROJECT_ROOT)}")
    print(f"✅ Shadowrocket sanitized: {sr_changed} files")
    print("=" * 60)
    return 0


if __name__ == "__main__":
    raise SystemExit(run())
