#!/usr/bin/env python3
"""Guard against manual edits in generated trees.

Policy:
- Human commits cannot directly modify/add files under:
  - modules/
  - rulesets/
  - dns/
- Pure renames are allowed.
- If generated-tree content changes exist, the same commit must also modify
  at least one file under scripts/ (script-layer source of truth).
- Bot commits are exempt.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GENERATED_PREFIXES = ("modules/", "rulesets/", "dns/")
SCRIPT_PREFIX = "scripts/"
BOT_AUTHORS = {"github-actions[bot]"}


def sh(*args: str) -> str:
    p = subprocess.run(args, cwd=ROOT, capture_output=True, text=True, check=True)
    return p.stdout.strip()


def main() -> int:
    base = sh("git", "rev-parse", "HEAD~1")
    head = sh("git", "rev-parse", "HEAD")
    author = sh("git", "log", "-1", "--pretty=%an")

    if author in BOT_AUTHORS:
        print(f"✅ guard: bot commit ({author}) exempt.")
        return 0

    rows = sh("git", "diff", "--name-status", base, head).splitlines()
    if not rows:
        print("✅ guard: no changes.")
        return 0

    changed_scripts = False
    generated_content_changes: list[str] = []

    for row in rows:
        parts = row.split("\t")
        if not parts:
            continue
        status = parts[0]
        paths = parts[1:]
        for p in paths:
            if p.startswith(SCRIPT_PREFIX):
                changed_scripts = True

        # Rename/copy status uses old+new paths. Allow pure renames by policy.
        if status.startswith(("R", "C")):
            continue

        target = paths[-1] if paths else ""
        if target.startswith(GENERATED_PREFIXES):
            generated_content_changes.append(f"{status}\t{target}")

    if generated_content_changes and not changed_scripts:
        print("❌ guard: generated tree content changed without script-layer changes.")
        print("Please modify generation scripts under scripts/ and regenerate outputs.")
        for item in generated_content_changes:
            print(f"  - {item}")
        return 1

    print("✅ guard: generated-tree policy passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

