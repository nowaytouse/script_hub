#!/usr/bin/env python3
"""Report PROMAX functional lines dropped by ingest mode + line classifier."""

from __future__ import annotations

import os
import sys

SCRIPT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPT_DIR)

from pipeline.adblock_manager import AdBlockManager, LOCAL_MODULES_DIR  # noqa: E402
from hub.promax_line_classifier import module_ingest_mode  # noqa: E402
from hub.promax_module_split import split_module_sections  # noqa: E402


def audit_path(path: str) -> None:
    text = open(path, encoding="utf-8").read()
    mode = module_ingest_mode(path, text)
    _, stats = split_module_sections(text, source_path=path)
    if stats["skipped_lines"] == 0 and mode == "full":
        return
    rel = os.path.relpath(path, ROOT)
    print(
        f"{rel}\n"
        f"  mode={mode}  kept={stats['kept_lines']}/{stats['total_lines']}  "
        f"dropped={stats['skipped_lines']}"
    )


def main() -> None:
    mgr = AdBlockManager()
    paths, _ = mgr.load_functional_source_paths()
    print("=== Functional sources with drops (split preview) ===")
    for path, _mode in paths:
        audit_path(path)
    if os.path.isdir(LOCAL_MODULES_DIR):
        print("\n=== LocalModules (all) ===")
        for name in sorted(os.listdir(LOCAL_MODULES_DIR)):
            if name.endswith(".sgmodule"):
                audit_path(os.path.join(LOCAL_MODULES_DIR, name))


if __name__ == "__main__":
    main()
