#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Surge → Shadowrocket module sync
Surge is the primary source; Shadowrocket modules are the generated target.
"""

import os
import re
import json
import shutil
from pathlib import Path
from datetime import datetime

SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
SURGE_MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)"
SR_MODULE_DIR = PROJECT_ROOT / "module" / "shadowrocket"

GITHUB_RAW_BASE_SR = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/shadowrocket"

# ── Surge-only [General] keys to remove entirely ──────────────────────────────
SURGE_ONLY_KEYS = {
    "use-local-host-item-for-proxy",
    "encrypted-dns-follow-outbound-mode",
    "encrypted-dns-skip-cert-verification",
    "force-http-engine-hosts",
    "always-raw-tcp-hosts",
    "always-raw-tcp-keywords",
    "tun-included-routes",
}

# ── Surge [General] key → SR [General] key ────────────────────────────────────
GENERAL_KEY_MAP = {
    "encrypted-dns-server": "doh-server",
    "tun-excluded-routes":  "bypass-tun",
}

# ── SR [General] defaults injected after [General] header ─────────────────────
SR_GENERAL_DEFAULTS = """\
bypass-system = true
ipv6 = true
prefer-ipv6 = true
hijack-dns = *:53
dns-direct-fallback-proxy = false"""

# ── Rule value replacements ────────────────────────────────────────────────────
RULE_REPLACEMENTS = {
    "REJECT-DROP":    "REJECT",
    "REJECT-TINYGIF": "REJECT",
    "REJECT-NO-DROP": "REJECT",
}

# ── Rewrite modifier removals ──────────────────────────────────────────────────
REWRITE_MODIFIER_RE = re.compile(r',\s*(extended-matching|pre-matching)\b|'
                                  r'\b(extended-matching|pre-matching)\s*,?')

# ── update-interval in script lines ───────────────────────────────────────────
UPDATE_INTERVAL_RE = re.compile(r',"update-interval=\d+"')


def _convert_header_line(line: str) -> str:
    """Convert Surge #!key=value header to Rocket # key: value"""
    m = re.match(r'^#!\s*(\S+?)\s*=\s*(.*)$', line)
    if m:
        key, val = m.group(1).strip(), m.group(2)
        if key == "desc":
            # inject [🚀SR] marker if not already present
            if "[🚀SR]" not in val:
                val = f"[🚀SR] {val}"
        return f"# {key}: {val}"
    return line


def _is_surge_only_general_line(line: str) -> bool:
    """Return True if this [General] line uses a Surge-only key."""
    m = re.match(r'^\s*([a-zA-Z0-9_-]+)\s*=', line)
    if m and m.group(1) in SURGE_ONLY_KEYS:
        return True
    return False


def _translate_general_line(line: str) -> str:
    """Rename Surge [General] keys to their SR equivalents."""
    m = re.match(r'^(\s*)([a-zA-Z0-9_-]+)(\s*=.*)$', line)
    if m:
        indent, key, rest = m.group(1), m.group(2), m.group(3)
        if key in GENERAL_KEY_MAP:
            return f"{indent}{GENERAL_KEY_MAP[key]}{rest}"
    return line


def convert_content(content: str) -> str:
    lines = content.split('\n')
    out = []
    section = None          # current INI section
    general_defaults_added = False

    for line in lines:
        stripped = line.strip()

        # ── Section header ─────────────────────────────────────────────────────
        if stripped.startswith('[') and stripped.endswith(']'):
            section = stripped[1:-1]
            out.append(line)
            if section == "General" and not general_defaults_added:
                out.append(SR_GENERAL_DEFAULTS)
                general_defaults_added = True
            continue

        # ── Module header lines (#!key=value) ─────────────────────────────────
        if re.match(r'^#!', line):
            # Remove Surge-only header directives
            if re.match(r'^#!\s*(update-interval|ability)\s*=', line, re.IGNORECASE):
                continue
            out.append(_convert_header_line(line))
            continue

        # ── [General] section processing ──────────────────────────────────────
        if section == "General" and not stripped.startswith('#') and stripped:
            if _is_surge_only_general_line(line):
                # Keep as comment so user can see what was removed
                out.append(f"# [SR不支持,已移除] {line.lstrip()}")
                continue
            line = _translate_general_line(line)

        # ── Strip %INSERT% / %APPEND% prefixes ────────────────────────────────
        line = re.sub(r'%(?:INSERT|APPEND)%\s*', '', line)

        # ── Rule value replacements ────────────────────────────────────────────
        for old, new in RULE_REPLACEMENTS.items():
            line = line.replace(old, new)

        # ── Rewrite modifier cleanup ───────────────────────────────────────────
        line = REWRITE_MODIFIER_RE.sub('', line)

        # ── update-interval in script entries ─────────────────────────────────
        line = UPDATE_INTERVAL_RE.sub('', line)

        # ── Clean up stray commas ──────────────────────────────────────────────
        line = re.sub(r',\s*,', ',', line)
        line = re.sub(r',\s*$', '', line)

        out.append(line)

    return '\n'.join(out)


def _sr_filename(surge_path: Path) -> str:
    """Return the Shadowrocket output filename (.module extension)."""
    stem = surge_path.stem
    return stem + ".module"


def process_all_modules():
    if SR_MODULE_DIR.exists():
        shutil.rmtree(SR_MODULE_DIR)
    SR_MODULE_DIR.mkdir(parents=True)

    categories = ["amplify_nexus", "head_expanse", "narrow_pierce"]
    stats = {"total": 0, "converted": 0, "failed": 0}
    log = []

    for cat in categories:
        (SR_MODULE_DIR / cat).mkdir(exist_ok=True)
        cat_path = SURGE_MODULE_DIR / cat
        if not cat_path.exists():
            continue

        for module_file in sorted(cat_path.glob("*.sgmodule")):
            stats["total"] += 1
            try:
                content = module_file.read_text(encoding="utf-8")
                converted = convert_content(content)
                out_name = _sr_filename(module_file)
                out_path = SR_MODULE_DIR / cat / out_name
                out_path.write_text(converted, encoding="utf-8")
                stats["converted"] += 1
                log.append({"src": str(module_file.name), "dst": out_name, "status": "ok"})
            except Exception as e:
                stats["failed"] += 1
                log.append({"src": str(module_file.name), "status": "error", "error": str(e)})

    # Write conversion log
    log_path = PROJECT_ROOT / "module" / "shadowrocket_conversion_log.json"
    log_path.write_text(
        json.dumps({"generated": datetime.now().isoformat(), "stats": stats, "log": log},
                   ensure_ascii=False, indent=2),
        encoding="utf-8"
    )
    return stats


if __name__ == "__main__":
    stats = process_all_modules()
    print(f"Done: {stats['converted']}/{stats['total']} converted, {stats['failed']} failed.")
