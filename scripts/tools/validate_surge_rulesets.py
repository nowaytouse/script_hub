#!/usr/bin/env python3
"""Validate Surge .list rulesets for known Surge-invalid patterns."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SURGE_DIR = ROOT / "ruleset" / "Surge(Shadowkroket)"

INVALID_DOMAIN_REGEX = frozenset({"", "$", ",", "-", ".", "2", "6", "]", "["})
INVALID_URL_REGEX = frozenset({
    "", "https:", "http:", "https", "http",
    "^https?:", "^https?://", "^https?:\\/\\/",
})


def _check_line(path: Path, lineno: int, line: str) -> list[str]:
    issues: list[str] = []
    s = line.strip()
    if not s or s.startswith("#"):
        return issues

    if s.startswith("DOMAIN-REGEX,"):
        payload = s.split(",", 1)[1].strip().strip('"')
        if payload in INVALID_DOMAIN_REGEX or len(payload) < 2:
            issues.append(f"{path.name}:{lineno}: invalid DOMAIN-REGEX payload {payload!r}")
        elif " " in payload and not s.startswith('DOMAIN-REGEX,"'):
            issues.append(f"{path.name}:{lineno}: DOMAIN-REGEX contains spaces")

    if s.startswith("URL-REGEX,"):
        payload = s.split(",", 1)[1].strip().strip('"')
        low = payload.lower()
        if low in INVALID_URL_REGEX or len(payload) < 3:
            issues.append(f"{path.name}:{lineno}: truncated/invalid URL-REGEX {payload!r}")
        else:
            try:
                re.compile(payload)
            except re.error as exc:
                issues.append(f"{path.name}:{lineno}: URL-REGEX compile error: {exc}")

    return issues


def validate_directory(directory: Path) -> list[str]:
    all_issues: list[str] = []
    if not directory.is_dir():
        return [f"missing directory: {directory}"]
    for path in sorted(directory.glob("*.list")):
        if "skk_upstream" in path.parts:
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        for i, line in enumerate(text.splitlines(), 1):
            all_issues.extend(_check_line(path, i, line))
    return all_issues


def main() -> int:
    targets = [SURGE_DIR, ROOT / "ruleset" / "AdBlock"]
    issues: list[str] = []
    for target in targets:
        if target.is_dir():
            issues.extend(validate_directory(target))
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
