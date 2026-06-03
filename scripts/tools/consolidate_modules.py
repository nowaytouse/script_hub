#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Module catalog pipeline (canonical entry):
  sanitize → scan → modules/helper/*.json → surge_module_helper.html

Invoked from pipeline/main_update.py after module conversion so the helper
web page stays in sync with one-click updates.
"""

import sys
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS_DIR))

from hub.module_catalog import (
    CATEGORIES,
    build_helper_html,
    normalize_categories_tree,
    sanitize_tree,
    scan_modules,
    write_modules_json,
    write_shadowrocket_modules_json,
)
from hub.paths import (
    MODULES_DATA_JSON,
    MODULES_HELPER_DIR,
    ROOT,
    SHADOWROCKET_MODULES_JSON,
    SURGE_MODULE_HELPER_HTML,
    SURGE_MODULE_HELPER_URL,
)

MODULE_DIR = Path(ROOT) / "modules" / "surge"
SR_MODULE_DIR = Path(ROOT) / "modules" / "shadowrocket"

REQUIRED_MODULE_STEMS = {
    "WeChat_Enhance",
    "📊 面板工具合集",
    "🧰 Script Hub 配套工具合集",
}


def _verify_html_coverage(modules: list) -> int:
    scanned_stems = {Path(m["path"]).stem for m in modules}
    missing = REQUIRED_MODULE_STEMS - scanned_stems
    if missing:
        print(f"  ⚠️  WARN: Required modules not found in scan: {sorted(missing)}")
        print("       → Check that .sgmodule file exists under modules/surge/<category>/")
    return len(missing)


def _check_sr_gaps(modules: list) -> int:
    gaps = []
    for m in modules:
        if m.get("merged_into"):
            continue
        sr_path = Path(ROOT) / m["path"].replace("surge", "shadowrocket", 1).replace(
            ".sgmodule", ".module"
        )
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
    print(f"   Helper URL: {SURGE_MODULE_HELPER_URL}")
    print("=" * 60)
    errors = 0
    Path(MODULES_HELPER_DIR).mkdir(parents=True, exist_ok=True)

    print("\n📂 Normalizing module categories (folder = source of truth)...")
    cat_surge = normalize_categories_tree(MODULE_DIR, Path(ROOT), "*.sgmodule")
    cat_sr = normalize_categories_tree(SR_MODULE_DIR, Path(ROOT), "*.module")
    print(f"   Fixed {cat_surge + cat_sr} file(s)")

    print("\n🧹 Sanitizing Surge modules...")
    surge_changed = sanitize_tree(MODULE_DIR, Path(ROOT), "*.sgmodule")

    print("\n🧹 Sanitizing Shadowrocket modules...")
    sr_changed = sanitize_tree(SR_MODULE_DIR, Path(ROOT), "*.module")

    modules = scan_modules(Path(ROOT), MODULE_DIR)
    total = len(modules)
    if total == 0:
        print("  ❌ FATAL: scan_modules returned 0 modules — aborting.")
        return 1

    print(f"\n🔍 Scanned {total} Surge modules ({surge_changed} files cleaned)")
    for cat, label in CATEGORIES.items():
        print(f"  - {label}: {sum(1 for m in modules if m['category'] == cat)}")

    print("\n🔒 Integrity checks...")
    errors += _verify_html_coverage(modules)
    sr_gaps = _check_sr_gaps(modules)
    if sr_gaps:
        print("  ℹ️  Run tools/convert_surge_to_shadowrocket.py to regenerate missing SR modules.")

    write_modules_json(modules, Path(MODULES_DATA_JSON))
    print(f"\n✅ Wrote {Path(MODULES_DATA_JSON).relative_to(ROOT)}")

    write_shadowrocket_modules_json(modules, Path(SHADOWROCKET_MODULES_JSON), Path(ROOT))
    print(f"✅ Wrote {Path(SHADOWROCKET_MODULES_JSON).relative_to(ROOT)}")

    build_helper_html(modules, Path(ROOT), Path(SURGE_MODULE_HELPER_HTML))
    print(f"✅ Wrote {Path(SURGE_MODULE_HELPER_HTML).relative_to(ROOT)}")
    print(f"✅ Shadowrocket sanitized: {sr_changed} files")
    print("=" * 60)

    if errors:
        print(f"⚠️  Pipeline completed with {errors} warning(s) — review output above.")
    return 0


if __name__ == "__main__":
    raise SystemExit(run())
