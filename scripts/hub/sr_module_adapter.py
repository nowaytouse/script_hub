#!/usr/bin/env python3
"""Shadowrocket adaptations for Surge modules (script params, MITM, bundle-specific rules)."""

from __future__ import annotations

import re

# Surge-only script parameters that break or OOM on Shadowrocket
_SR_STRIP_SCRIPT_PARAMS = (
    (re.compile(r',?\s*ability=(?:"[^"]*"|[^\s,]+)', re.I), ""),
    (re.compile(r",?\s*engine=auto\b", re.I), ""),
    (re.compile(r',?\s*engine=\{\{\{engine\}\}\}', re.I), ""),
)

_SUB_STORE_LINE = re.compile(r"sub\.store|sub-store-org", re.I)
_SCRIPT_HUB_LINE = re.compile(r"script\.hub|Script Hub", re.I)

DEVTOOLS_STEM = "🧰 Script Hub 配套工具合集"
SUB_STORE_NOABILITY_URL = (
    "https://raw.githubusercontent.com/sub-store-org/Sub-Store/master/config/Surge-Noability.sgmodule"
)


def adapt_script_line_for_sr(line: str, *, module_stem: str = "") -> str:
    """Strip Surge-only script fields; apply bundle-specific SR rules."""
    stripped = line.strip()
    if not stripped or stripped.startswith("#"):
        return line

    out = line
    for pattern, repl in _SR_STRIP_SCRIPT_PARAMS:
        out = pattern.sub(repl, out)

    if _SUB_STORE_LINE.search(out):
        out = _adapt_substore_script_line(out, module_stem=module_stem)

    if _SCRIPT_HUB_LINE.search(out):
        out = re.sub(r"\s+,", ",", out)
        out = re.sub(r",\s+,", ",", out)

    return out


def _adapt_substore_script_line(line: str, *, module_stem: str = "") -> str:
    """Sub-Store on SR: follow Surge-Noability (no ability/engine; fixed timeout)."""
    out = line
    out = re.sub(r',?\s*max-size=(?:"[^"]*"|[^\s,]+)', "", out, flags=re.I)
    out = re.sub(r"timeout=\{\{\{timeout\}\}\}", "timeout=120", out)
    out = re.sub(r',?\s*argument="cors=\{\{\{cors\}\}\}"', "", out)

    if re.search(r"\{\{\{produce\}\}\}=type=cron", out, re.I):
        out = re.sub(
            r"^\{\{\{produce\}\}\}",
            "Sub-Store Produce",
            out.strip(),
            flags=re.I,
        )

    if re.search(r"\{\{\{sync\}\}\}=type=cron", out, re.I):
        out = re.sub(
            r"^\{\{\{sync\}\}\}",
            "Sub-Store Sync",
            out.strip(),
            flags=re.I,
        )

    return out


def adapt_mitm_line_for_sr(line: str) -> str:
    """Convert Surge MITM hostname line for Shadowrocket.
    Shadowrocket supports %APPEND% — preserve it so multiple modules
    can coexist without overwriting each other's MITM hostnames.
    """
    stripped = line.strip()
    if not stripped.lower().startswith("hostname"):
        return line
    part = re.sub(r"^hostname\s*=\s*", "", stripped, flags=re.I)
    # Detect whether original had %APPEND% or %INSERT%
    append_match = re.match(r"^(%APPEND%|%INSERT%)\s*", part, flags=re.I)
    prefix = "%APPEND% " if append_match else ""
    part = re.sub(r"^(%APPEND%|%INSERT%)\s*", "", part, flags=re.I)
    hosts = [h.strip() for h in part.split(",") if h.strip() and h.strip() not in {"%INSERT%", "%APPEND%"}]
    if not hosts:
        return line
    return f"hostname = {prefix}{', '.join(hosts)}"


def adapt_section_lines_for_sr(
    section: str,
    lines: list[str],
    *,
    module_stem: str = "",
) -> list[str]:
    adapted: list[str] = []
    for line in lines:
        if section == "Script":
            adapted.append(adapt_script_line_for_sr(line, module_stem=module_stem))
        elif section == "MITM":
            adapted.append(adapt_mitm_line_for_sr(line))
        else:
            adapted.append(line)
    return adapted


def module_stem_from_meta(meta: dict[str, str], fallback: str = "") -> str:
    return meta.get("name", fallback).strip()
