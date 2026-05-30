#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Surge external RULE-SET compliance — single source of truth.

Lessons from recent incidents (git history May 2026):
- 053a145d: wrongly stripped PROCESS-NAME / URL-REGEX from .list files
- 427a9228: URL-REGEX truncated when https:// treated as // comment
- b87b2e0f: MetaCubeX DOMAIN-REGEX,$ char-split junk from deco lists
- 0af5f592: quoting DOMAIN-REGEX does NOT fix Surge — type is unsupported
- 0f524ff5: Netflix DOMAIN-REGEX must become DOMAIN-KEYWORD for Surge .list
- d804f1d0: IP-CIDR,...,no-resolve was dropped by naive comma split
"""

from __future__ import annotations

import re
from typing import FrozenSet, Optional, Tuple

# https://manual.nssurge.com/rule/ruleset.html — external .list RULE-SET types
SURGE_RULESET_ALLOWED: FrozenSet[str] = frozenset({
    "DOMAIN", "DOMAIN-SUFFIX", "DOMAIN-KEYWORD", "DOMAIN-SET",
    "IP-CIDR", "IP-CIDR6", "IP-ASN", "GEOIP",
    "USER-AGENT", "URL-REGEX", "PROCESS-NAME",
    "DEST-PORT", "SRC-PORT", "IN-PORT",
})

# Clash-only in external Surge .list (always Invalid line)
SURGE_RULESET_FORBIDDEN: FrozenSet[str] = frozenset({"DOMAIN-REGEX", "DOMAIN-WILDCARD"})

PAYLOAD_SAFE_RULE_TYPES: FrozenSet[str] = frozenset({
    "DOMAIN-REGEX", "URL-REGEX", "USER-AGENT", "PROCESS-NAME",
})

INVALID_DOMAIN_REGEX_VALUES: FrozenSet[str] = frozenset({
    "", "$", ",", "-", ".", "2", "6", "]", "[",
})

INVALID_URL_REGEX_VALUES: FrozenSet[str] = frozenset({
    "", "https:", "http:", "https", "http",
    "^https?:", "^https?://", r"^https?:\/\/",
})

STRIP_POLICY_TOKENS: FrozenSet[str] = frozenset({
    "REJECT", "REJECT-DROP", "REJECT-NO-DROP", "DIRECT", "PROXY",
    "REJECT-TINYGIF", "REJECT-IMG",
    "EXTENDED-MATCHING", "PRE-MATCHING", "NO-RESOLVE", "FORCE-CELLULAR",
    "{{{PROXY}}}",
})


def rule_head(line: str) -> str:
    if "," not in line:
        return line.strip().upper()
    return line.split(",", 1)[0].strip().upper()


def strip_inline_comment(line: str) -> str:
    """Strip # and // comments without breaking https:// inside URL-REGEX."""
    line = line.strip()
    if not line:
        return ""
    if line.startswith("//"):
        return ""
    head = rule_head(line)
    if head in PAYLOAD_SAFE_RULE_TYPES:
        return line
    if "//" in line:
        line = line.split("//", 1)[0].strip()
    if "#" in line:
        line = line.split("#", 1)[0].strip()
    return line


def split_rule(line: str) -> Optional[Tuple[str, str]]:
    """Return (TYPE, payload) after comment strip; None if not a rule line."""
    line = strip_inline_comment(line)
    if not line or "," not in line:
        return None
    rule_type, payload = line.split(",", 1)
    rule_type = rule_type.strip().upper()
    payload = payload.strip()
    if rule_type.startswith("IP-"):
        payload = payload.split(",", 1)[0].strip()
    if payload.startswith('"') and payload.endswith('"'):
        payload = payload[1:-1]
    return rule_type, payload


def strip_trailing_policy(rule: str) -> str:
    """Remove trailing policy/options; preserve commas inside regex payloads."""
    rule = rule.strip()
    if "," not in rule:
        return rule
    head = rule_head(rule)
    if head in PAYLOAD_SAFE_RULE_TYPES:
        rule_type, payload = rule.split(",", 1)
        payload = payload.strip()
        while "," in payload:
            tail = payload.rsplit(",", 1)[-1].strip().upper()
            if tail in STRIP_POLICY_TOKENS or tail.startswith("UPDATE-INTERVAL="):
                payload = payload.rsplit(",", 1)[0].strip()
            else:
                break
        return f"{rule_type.strip()},{payload}"
    parts = [p.strip() for p in rule.split(",")]
    while len(parts) > 2 and (
        parts[-1].upper() in STRIP_POLICY_TOKENS
        or parts[-1].upper().startswith("UPDATE-INTERVAL=")
    ):
        parts = parts[:-1]
    return ",".join(parts)


def is_invalid_domain_regex_payload(payload: str) -> bool:
    return len(payload.strip()) < 2 or payload in INVALID_DOMAIN_REGEX_VALUES


def is_invalid_url_regex_payload(payload: str) -> bool:
    val = payload.strip()
    if len(val) < 3:
        return True
    return val.lower() in INVALID_URL_REGEX_VALUES


def convert_domain_regex_for_surge(rule: str) -> str:
    """Convert Clash DOMAIN-REGEX → Surge-safe DOMAIN-KEYWORD; '' if unmappable."""
    if not rule.upper().startswith("DOMAIN-REGEX,"):
        return ""
    payload = rule.split(",", 1)[1].strip().strip('"')
    if is_invalid_domain_regex_payload(payload):
        return ""

    literal = payload.replace(r"\.", ".")

    m = re.match(r"^\(\^\|\.\)(.+?)-\.\+\.", literal)
    if m:
        return f"DOMAIN-KEYWORD,{m.group(1)}"

    m = re.match(r"^(?:\(\^\|\.\)|\^)([a-zA-Z0-9][-a-zA-Z0-9.]*)$", literal)
    if m:
        return f"DOMAIN-KEYWORD,{m.group(1)}"

    m = re.match(r"^\.\+(.+)$", literal)
    if m and len(m.group(1)) >= 3:
        return f"DOMAIN-KEYWORD,{m.group(1)}"

    return ""


def format_url_regex_for_surge(payload: str) -> str:
    """Quote URL-REGEX when payload contains commas (Surge .list safety)."""
    if "," in payload or " " in payload:
        return f'URL-REGEX,"{payload}"'
    return f"URL-REGEX,{payload}"


def validate_surge_ruleset_line(line: str, *, allow_domain_regex: bool = False) -> Optional[str]:
    """Return error message if line is invalid for Surge external .list; else None."""
    s = line.strip()
    if not s or s.startswith("#"):
        return None

    parsed = split_rule(s)
    if not parsed:
        return "missing rule type or payload"

    rule_type, payload = parsed

    if rule_type in SURGE_RULESET_FORBIDDEN and not allow_domain_regex:
        return f"{rule_type} is not supported in Surge external RULE-SET"

    if rule_type not in SURGE_RULESET_ALLOWED and rule_type not in SURGE_RULESET_FORBIDDEN:
        return f"unknown rule type {rule_type}"

    if rule_type == "URL-REGEX":
        raw = s.split(",", 1)[1].strip()
        quoted = raw.startswith('"') and raw.endswith('"')
        body = raw[1:-1] if quoted else raw
        if "," in body and not quoted:
            return "URL-REGEX payload with comma must be double-quoted"
        if is_invalid_url_regex_payload(body):
            return f"truncated/invalid URL-REGEX {body!r}"
        try:
            re.compile(body)
        except re.error as exc:
            return f"URL-REGEX compile error: {exc}"

    if rule_type == "DOMAIN-REGEX" and allow_domain_regex:
        if is_invalid_domain_regex_payload(payload):
            return f"invalid DOMAIN-REGEX payload {payload!r}"

    return None
