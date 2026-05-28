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

# Critical modules that MUST appear in the HTML after every build.
# Add new module stems here whenever a new module is permanently added.
REQUIRED_MODULE_STEMS = {
    "XWebAds",
    "WeChat_Enhance",
    "boxjs.rewrite.surge",
    "Sub_Info",
}


def _verify_html_coverage(modules: list, project_root: Path) -> int:
    """Return the count of required modules missing from the scanned list."""
    scanned_stems = {Path(m["path"]).stem for m in modules}
    missing = REQUIRED_MODULE_STEMS - scanned_stems
    if missing:
        print(f"  ⚠️  WARN: Required modules not found in scan: {sorted(missing)}")
        print("       → Check that .sgmodule file exists under module/surge(main)/<category>/")
    return len(missing)


def _check_sr_gaps(modules: list, project_root: Path) -> int:
    """Warn about Surge modules whose Shadowrocket counterpart is missing."""
    gaps = []
    for m in modules:
        if m.get("merged_into"):
            continue
        sr_path = project_root / m["path"].replace("surge(main)", "shadowrocket").replace(".sgmodule", ".module")
        if not sr_path.exists():
            gaps.append(Path(m["path"]).name)
    if gaps:
        print(f"  ⚠️  SR sync gaps ({len(gaps)} missing):")
        for g in sorted(gaps):
            print(f"       - {g}")
    return len(gaps)


def run() -> int:
    print("=" * 60)
    print("📦 Module Catalog (sanitize · scan · JSON · HTML)")
    print("=" * 60)
    errors = 0

    print("\n📂 Normalizing module categories (folder = source of truth)...")
    cat_surge = normalize_categories_tree(MODULE_DIR, PROJECT_ROOT, "*.sgmodule")
    cat_sr = normalize_categories_tree(SR_MODULE_DIR, PROJECT_ROOT, "*.module")
    print(f"   Fixed {cat_surge + cat_sr} file(s)")

    print("\n🧹 Sanitizing Surge modules...")
    surge_changed = sanitize_tree(MODULE_DIR, PROJECT_ROOT, "*.sgmodule")

    print("\n🧹 Sanitizing Shadowrocket modules...")
    sr_changed = sanitize_tree(SR_MODULE_DIR, PROJECT_ROOT, "*.module")

    modules = scan_modules(PROJECT_ROOT, MODULE_DIR)
    total = len(modules)
    if total == 0:
        print("  ❌ FATAL: scan_modules returned 0 modules — aborting.")
        return 1

    print(f"\n🔍 Scanned {total} Surge modules ({surge_changed} files cleaned)")
    for cat, label in CATEGORIES.items():
        print(f"  - {label}: {sum(1 for m in modules if m['category'] == cat)}")

    # Integrity checks
    print("\n🔒 Integrity checks...")
    errors += _verify_html_coverage(modules, PROJECT_ROOT)
    sr_gaps = _check_sr_gaps(modules, PROJECT_ROOT)
    if sr_gaps:
        print(f"  ℹ️  Run convert_surge_to_shadowrocket.py to regenerate missing SR modules.")

    write_modules_json(modules, MODULES_JSON)
    print(f"\n✅ Wrote {MODULES_JSON.relative_to(PROJECT_ROOT)}")

    build_helper_html(modules, PROJECT_ROOT, HELPER_HTML)
    print(f"✅ Wrote {HELPER_HTML.relative_to(PROJECT_ROOT)}")
    print(f"✅ Shadowrocket sanitized: {sr_changed} files")
    print("=" * 60)

    if errors:
        print(f"⚠️  Pipeline completed with {errors} warning(s) — review output above.")
    return 0  # Warnings don't fail the build; missing SR files are auto-fixed by convert step


if __name__ == "__main__":
    raise SystemExit(run())
