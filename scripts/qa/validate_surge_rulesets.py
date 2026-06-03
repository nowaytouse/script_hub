#!/usr/bin/env python3
"""Validate Surge .list rulesets for known Surge-invalid patterns."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from hub.surge_compliance import validate_surge_ruleset_line  # noqa: E402

SURGE_DIR = ROOT / "rulesets" / "surge-shadowrocket"
ADBLOCK_DIR = ROOT / "rulesets" / "AdBlock"


def validate_directory(directory: Path, *, allow_domain_regex: bool) -> list[str]:
    all_issues: list[str] = []
    if not directory.is_dir():
        return [f"missing directory: {directory}"]
    for path in sorted(directory.glob("*.list")):
        if "skk_upstream" in path.parts:
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        for i, line in enumerate(text.splitlines(), 1):
            err = validate_surge_ruleset_line(line, allow_domain_regex=allow_domain_regex)
            if err:
                all_issues.append(f"{path.name}:{i}: {err}")
    return all_issues


def main() -> int:
    issues: list[str] = []
    if SURGE_DIR.is_dir():
        issues.extend(validate_directory(SURGE_DIR, allow_domain_regex=False))
    if ADBLOCK_DIR.is_dir():
        issues.extend(validate_directory(ADBLOCK_DIR, allow_domain_regex=True))
    if issues:
        print("Surge ruleset validation failed:", file=sys.stderr)
        for item in issues[:50]:
            print(f"  - {item}", file=sys.stderr)
        if len(issues) > 50:
            print(f"  ... and {len(issues) - 50} more", file=sys.stderr)
        return 1
    print("Surge ruleset validation OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
