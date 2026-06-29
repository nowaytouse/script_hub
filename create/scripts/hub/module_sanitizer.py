#!/usr/bin/env python3
"""
Normalize Surge / Shadowrocket module files: dedupe sections, preserve #!arguments,
standard section order, and readable grouping comments.
"""

from __future__ import annotations

import re
from typing import Dict, List, Optional, Set, Tuple

SECTION_ORDER = [
    "General",
    "Host",
    "Rule",
    "URL Rewrite",
    "Map Local",
    "Script",
    "Body Rewrite",
    "Header Rewrite",
    "MITM",
]

HEADER_KEYS_ORDER = [
    "name",
    "desc",
    "author",
    "icon",
    "category",
    "tag",
    "date",
    "arguments",
    "arguments-desc",
    "system-proxy",
    "ability",
    "update-interval",
]

META_LINE_RE = re.compile(r"^#!\s*([A-Za-z0-9_-]+)\s*[=:]\s*(.*)$", re.IGNORECASE)
SCRIPT_NAME_RE = re.compile(r"\bname\s*=\s*([^,\s]+)", re.IGNORECASE)
SCRIPT_LABEL_RE = re.compile(r"^(.+?)\s*=\s*type=", re.IGNORECASE)
SCRIPT_PATH_RE = re.compile(r"\bscript-path\s*=\s*([^,\s]+)", re.IGNORECASE)
SCRIPT_PATTERN_RE = re.compile(r"\bpattern\s*=\s*([^,]+)", re.IGNORECASE)


def parse_module(text: str) -> Tuple[Dict[str, str], List[Tuple[str, List[str]]]]:
    """Return (header_meta, sections) where header_meta keys are lowercased."""
    header_meta: Dict[str, str] = {}
    sections: List[Tuple[str, List[str]]] = []
    current_name: Optional[str] = None
    current_lines: List[str] = []
    in_body = False

    for raw in text.splitlines():
        stripped = raw.strip()
        if stripped.startswith("[") and stripped.endswith("]") and not stripped.startswith("#"):
            if current_name is not None:
                sections.append((current_name, current_lines))
            current_name = stripped[1:-1]
            current_lines = []
            in_body = True
            continue

        if not in_body:
            match = META_LINE_RE.match(stripped)
            if match:
                header_meta[match.group(1).lower()] = match.group(2).strip()
            elif stripped.startswith("#!") or not stripped:
                pass
            continue

        if current_name is not None:
            current_lines.append(raw.rstrip())

    if current_name is not None:
        sections.append((current_name, current_lines))
    return header_meta, sections


def _dedupe_key(section: str, line: str) -> str:
    stripped = line.strip()
    if not stripped or stripped.startswith("#"):
        return ""
    lowered = stripped.lower()

    if section == "Script":
        label = SCRIPT_LABEL_RE.match(stripped)
        if label:
            return f"script:label:{label.group(1).strip()}"
        name = SCRIPT_NAME_RE.search(stripped)
        if name:
            return f"script:name:{name.group(1)}"
        path = SCRIPT_PATH_RE.search(stripped)
        pattern = SCRIPT_PATTERN_RE.search(stripped)
        if path and pattern:
            return f"script:{path.group(1)}:{pattern.group(1).strip()}"
        if path:
            return f"script:{path.group(1)}"
        return f"script:{stripped}"

    if section == "URL Rewrite":
        return f"rewrite:{stripped.split(',')[0] if ',' in stripped else stripped}"

    if section == "Map Local":
        return f"maplocal:{stripped}"

    if section == "Body Rewrite":
        return f"body:{stripped}"

    if section == "Header Rewrite":
        return f"header:{stripped}"

    if section == "Rule":
        return f"rule:{stripped}"

    if section == "MITM":
        hosts = re.sub(r"^hostname\s*=\s*(%APPEND%\s*)?", "", stripped, flags=re.I)
        return f"mitm:{hosts.strip()}"
    return f"{section}:{stripped}"

def dedupe_section_lines(section: str, lines: List[str]) -> List[str]:
    seen: Set[str] = set()
    result: List[str] = []
    for line in lines:
        stripped = line.strip()
        if not stripped:
            if result and result[-1] != "":
                result.append("")
            continue
        if stripped.startswith("#"):
            result.append(stripped)
            continue
        key = _dedupe_key(section, line)
        if not key or key in seen:
            continue
        seen.add(key)
        result.append(stripped)

    # Beautify alignment for sections with key=value formats
    if section in ("Script", "Host", "MITM", "General"):
        parsed = []
        max_len = 0
        for r in result:
            if r.startswith("#"):
                parsed.append((r, None, None))
                continue
            parts = r.split("=", 1)
            if len(parts) == 2:
                left = parts[0].strip()
                right = parts[1].strip()
                # To prevent absurd padding from malformed lines, cap max_len at 60
                if len(left) < 60:
                    max_len = max(max_len, len(left))
                parsed.append((r, left, right))
            else:
                parsed.append((r, None, None))
        
        aligned_result = []
        for r, left, right in parsed:
            if left is not None and right is not None and len(left) <= 60:
                aligned_result.append(f"{left:<{max_len}} = {right}")
            else:
                aligned_result.append(r)
        return aligned_result

    return result


def format_header(meta: Dict[str, str], extra_lines: Optional[List[str]] = None) -> List[str]:
    lines: List[str] = []
    used = set()
    for key in HEADER_KEYS_ORDER:
        if key in meta:
            lines.append(f"#!{key}={meta[key]}")
            used.add(key)
    for key, value in sorted(meta.items()):
        if key not in used:
            lines.append(f"#!{key}={value}")
    if extra_lines:
        lines.extend(extra_lines)
    return lines


def format_module(
    header_lines: List[str],
    sections: List[Tuple[str, List[str]]],
    *,
    dedupe: bool = True,
    section_comments: Optional[Dict[str, str]] = None,
) -> str:
    """Build module text with standard section order."""
    section_map = {name: lines for name, lines in sections}
    out: List[str] = list(header_lines)
    while out and not out[-1].strip():
        out.pop()
    if out:
        out.append("")

    for section_name in SECTION_ORDER:
        lines = section_map.get(section_name)
        if not lines:
            continue
        if dedupe:
            lines = dedupe_section_lines(section_name, lines)
        if not lines:
            continue
        if section_comments and section_name in section_comments:
            out.append(section_comments[section_name])
        out.append(f"[{section_name}]")
        out.extend(lines)
        out.append("")

    return "\n".join(out).rstrip() + "\n"


def sanitize_file_content(text: str, *, dedupe: bool = True) -> str:
    meta, sections = parse_module(text)
    header = format_header(meta) if meta else []
    if not header:
        # Legacy: keep non-meta preamble lines before first section
        header = []
        for raw in text.splitlines():
            if raw.strip().startswith("["):
                break
            if raw.strip().startswith("#!") or not raw.strip():
                header.append(raw.rstrip())
    return format_module(header, sections, dedupe=dedupe)


def merge_mitm_hosts(sections: List[Tuple[str, List[str]]]) -> List[Tuple[str, List[str]]]:
    hosts: Set[str] = set()
    other: List[Tuple[str, List[str]]] = []
    skip_tokens = {"%INSERT%", "%APPEND%"}
    for name, lines in sections:
        if name != "MITM":
            other.append((name, lines))
            continue
        for line in lines:
            stripped = line.strip()
            if stripped.lower().startswith("hostname"):
                part = re.sub(r"^hostname\s*=\s*", "", stripped, flags=re.I)
                part = re.sub(r"^(%APPEND%|%INSERT%)\s*", "", part, flags=re.I)
                for token in part.split(","):
                    token = token.strip()
                    if token and token not in skip_tokens:
                        hosts.update({token})
    if hosts:
        inclusions = sorted(h for h in hosts if not h.startswith("-"))
        exclusions = sorted(h for h in hosts if h.startswith("-"))
        merged = ", ".join(inclusions + exclusions)
        other.append(("MITM", [f"hostname = %APPEND% {merged}"]))
    return other
