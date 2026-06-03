#!/usr/bin/env python3
"""Fix legacy script-path URLs and hotlink scripts that 404 or block bots."""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS_DIR))

from hub.common import Logger, get_project_root, write_file
from hub.paths import (
    LEGACY_SCRIPT_RAW_PREFIX,
    SCRIPT_RAW_PREFIX,
    SCRIPTS_DIR as SCRIPTS_DIR_STR,
)

ROOT = Path(get_project_root())
SCRIPTS_PATH = Path(SCRIPTS_DIR_STR)

# kelee.one blocks headless fetch; vendored copies live under modules/source/scripts/
KELEE_BASENAME_TO_LOCAL: dict[str, str] = {}
for path in SCRIPTS_PATH.glob("kelee_one_*"):
    # kelee_one_<hash>_<original_filename>.js
    parts = path.name.split("_", 3)
    if len(parts) >= 4:
        basename = parts[3]
        KELEE_BASENAME_TO_LOCAL[basename] = path.name

MODULE_GLOBS = (
    ROOT / "modules/surge",
    ROOT / "modules/shadowrocket",
    ROOT / "rulesets/Sources/vendor",
    ROOT / "rulesets/Sources/LocalModules",
    ROOT / "modules/source/local",
)


def repair_text(text: str) -> tuple[str, int]:
    changes = 0
    if LEGACY_SCRIPT_RAW_PREFIX in text:
        n = text.count(LEGACY_SCRIPT_RAW_PREFIX)
        text = text.replace(LEGACY_SCRIPT_RAW_PREFIX, SCRIPT_RAW_PREFIX)
        changes += n

    for basename, local_name in KELEE_BASENAME_TO_LOCAL.items():
        # .../AppName/basename
        needle = f"https://kelee.one/Resource/JavaScript/"
        for m in re.finditer(re.escape(needle) + r"[^/]+/" + re.escape(basename), text):
            old = m.group(0)
            new = SCRIPT_RAW_PREFIX + local_name
            if old in text:
                text = text.replace(old, new)
                changes += 1
    return text, changes


def repair_file(path: Path) -> int:
    raw = path.read_text(encoding="utf-8")
    new, n = repair_text(raw)
    if n:
        write_file(str(path), new)
        Logger.info(f"  {path.relative_to(ROOT)} ({n} replacements)")
    return n


def main() -> None:
    Logger.section("Repair script-path URLs")
    total = 0
    files = 0
    for base in MODULE_GLOBS:
        if not base.is_dir():
            continue
        for ext in ("*.sgmodule", "*.module"):
            for path in base.rglob(ext):
                n = repair_file(path)
                if n:
                    files += 1
                    total += n
    Logger.success(f"Updated {files} file(s), {total} replacement(s)")


if __name__ == "__main__":
    main()
