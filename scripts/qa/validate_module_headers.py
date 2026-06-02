#!/usr/bin/env python3
"""Validate Surge/Shadowrocket module header integrity.

Guards against iOS client "empty module fields" regressions caused by malformed
metadata blocks (for example, stray non-comment lines before the first section).
"""

from __future__ import annotations

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]
MODULE_ROOTS = [
    ROOT / "module" / "surge(main)",
    ROOT / "module" / "shadowrocket",
]

META_RE = re.compile(r"^#!\s*([A-Za-z0-9_-]+)\s*=\s*.*$")
RECOMMENDED_KEYS = ("name", "desc", "author", "category")
MAX_META_LINE_LENGTH = 4096


def validate_module(path: Path) -> tuple[list[str], list[str]]:
    issues: list[str] = []
    warnings: list[str] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except Exception as exc:  # pragma: no cover - best effort diagnostics
        return [f"{path}: failed to read file: {exc}"], []

    if not lines:
        return [f"{path}: empty module file"], []

    in_header = True
    seen_section = False
    seen_keys: set[str] = set()

    for lineno, raw in enumerate(lines, 1):
        stripped = raw.lstrip("\ufeff").strip()

        if in_header and stripped.startswith("[") and stripped.endswith("]"):
            in_header = False
            seen_section = True
            continue

        if in_header:
            if not stripped:
                continue
            if stripped.startswith("#!"):
                if len(stripped) > MAX_META_LINE_LENGTH:
                    issues.append(f"{path}:{lineno}: metadata line too long (> {MAX_META_LINE_LENGTH})")
                m = META_RE.match(stripped)
                if not m:
                    issues.append(f"{path}:{lineno}: malformed metadata line")
                    continue
                seen_keys.add(m.group(1).lower())
                continue
            if stripped.startswith("#"):
                continue
            issues.append(
                f"{path}:{lineno}: illegal header content before first section: {stripped[:80]}"
            )
            continue

    if not seen_section:
        issues.append(f"{path}: missing any [Section] block")

    missing = [k for k in RECOMMENDED_KEYS if k not in seen_keys]
    if missing:
        warnings.append(f"{path}: missing recommended metadata keys: {', '.join(missing)}")

    return issues, warnings


def main() -> int:
    all_issues: list[str] = []
    all_warnings: list[str] = []
    for base in MODULE_ROOTS:
        if not base.exists():
            all_issues.append(f"{base}: directory not found")
            continue
        pattern = "*.sgmodule" if "surge(main)" in str(base) else "*.module"
        for fp in sorted(base.rglob(pattern)):
            issues, warnings = validate_module(fp)
            all_issues.extend(issues)
            all_warnings.extend(warnings)

    if all_issues:
        print("❌ Module header validation failed:")
        for issue in all_issues:
            print(f"  - {issue}")
        return 1

    if all_warnings:
        print("⚠️ Module header validation warnings:")
        for warning in all_warnings:
            print(f"  - {warning}")

    print("✅ Module header validation passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

