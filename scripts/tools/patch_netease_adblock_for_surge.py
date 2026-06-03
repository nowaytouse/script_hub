#!/usr/bin/env python3
"""Vendor ddgksf2013 netease.adblock.js with Surge JSC-safe wrapper."""

from __future__ import annotations

import os
import sys

SCRIPTS_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPTS_DIR)

from hub.common import Logger, get_project_root, safe_download, write_file
from hub.paths import SCRIPTS_DIR, SCRIPT_RAW_PREFIX

ROOT = get_project_root()
GIST_URL = (
    "https://gist.githubusercontent.com/ddgksf2013/"
    "4f53b7c6083678df25fecc8ff68b52c4/raw/netease.adblock.js"
)
OUT_PATH = os.path.join(
    SCRIPTS_DIR,
    "gist_githubusercontent_com_4f53b7_netease.adblock.js",
)
ADULT_FILES = (
    os.path.join(ROOT, "modules/source/local/adultraplus.sgmodule"),
    os.path.join(ROOT, "rulesets/Sources/vendor/adultraplus.sgmodule"),
)
LOCAL_SCRIPT_URL = SCRIPT_RAW_PREFIX + "gist_githubusercontent_com_4f53b7_netease.adblock.js"


def wrap_for_surge(raw: str) -> str:
    lines = raw.splitlines()
    header = lines[0] if lines else "const version = 'unknown';"
    body = "\n".join(lines[2:]) if len(lines) > 2 else raw
    return (
        f"{header}\n"
        "// Patched for Surge JSC (module.exports UMD + obfuscator) — ScriptHub\n"
        "(() => {\n"
        "  var module = { exports: {} };\n"
        "  var exports = module.exports;\n"
        "  var require = function() { return {}; };\n"
        f"{body}\n"
        "})();\n"
    )


def patch_adultraplus_modules() -> int:
    n = 0
    for path in ADULT_FILES:
        if not os.path.isfile(path):
            continue
        text = open(path, encoding="utf-8").read()
        if GIST_URL not in text:
            continue
        write_file(path, text.replace(GIST_URL, LOCAL_SCRIPT_URL))
        n += 1
        Logger.info(f"  Updated script-path: {os.path.relpath(path, ROOT)}")
    return n


def main() -> None:
    Logger.section("Patch netease.adblock.js for Surge")
    raw = safe_download(GIST_URL, retries=2, timeout=60)
    if not raw or len(raw) < 1000:
        raise SystemExit(f"Failed to download upstream: {GIST_URL}")
    write_file(OUT_PATH, wrap_for_surge(raw))
    Logger.success(f"Wrote {os.path.relpath(OUT_PATH, ROOT)}")
    patched = patch_adultraplus_modules()
    Logger.info(f"Patched {patched} adultraplus module file(s)")
    Logger.info("Next: python3 scripts/adblock_manager.py --execute")


if __name__ == "__main__":
    main()
