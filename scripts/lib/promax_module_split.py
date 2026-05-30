#!/usr/bin/env python3
"""Split mixed Surge modules into ad-only extracts for PROMAX."""

from __future__ import annotations

import os
import re
from typing import Dict, List, Optional, Tuple

from lib.promax_line_classifier import should_keep_promax_line

SECTION_HEADER_RE = re.compile(r"^\[([^\]]+)\]\s*$")


def split_module_sections(
    text: str, *, source_path: Optional[str] = None
) -> Tuple[Dict[str, List[str]], Dict[str, int]]:
    """
    Return (ad_sections, stats).
    stats keys: total_lines, kept_lines, skipped_lines
    """
    ad_sections: Dict[str, List[str]] = {}
    current_section: Optional[str] = None
    total = kept = skipped = 0

    for raw in text.splitlines():
        stripped = raw.strip()
        m = SECTION_HEADER_RE.match(stripped)
        if m and not stripped.startswith("#"):
            current_section = m.group(1)
            continue
        if not stripped or stripped.startswith("#"):
            continue
        if current_section is None:
            continue
        if current_section.lower() in ("general", "ponte"):
            skipped += 1
            total += 1
            continue
        total += 1
        if should_keep_promax_line(stripped, current_section, source_path=source_path):
            ad_sections.setdefault(current_section, []).append(stripped)
            kept += 1
        else:
            skipped += 1

    return ad_sections, {
        "total_lines": total,
        "kept_lines": kept,
        "skipped_lines": skipped,
    }


def format_split_module(
    source_path: str,
    ad_sections: Dict[str, List[str]],
    *,
    note: str,
) -> str:
    """Minimal sgmodule fragment for review under module/build/promax_splits/."""
    base = os.path.basename(source_path)
    lines = [
        f"#!name=PROMAX split · {base}",
        f"#!desc={note}",
        "#!author=ScriptHub-Automated",
        f"#!split-source={source_path}",
    ]
    order = (
        "Rule",
        "URL Rewrite",
        "Map Local",
        "Script",
        "Body Rewrite",
        "Header Rewrite",
        "MITM",
    )
    seen = set(ad_sections)
    for sec in order:
        if sec not in ad_sections:
            continue
        body = ad_sections[sec]
        if not body:
            continue
        lines.append("")
        lines.append(f"[{sec}]")
        lines.extend(body)
    for sec in sorted(seen):
        if sec in order:
            continue
        lines.append("")
        lines.append(f"[{sec}]")
        lines.extend(ad_sections[sec])
    lines.append("")
    return "\n".join(lines)
