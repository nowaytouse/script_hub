#!/usr/bin/env python3
"""Line-level ad vs enhancement classification for PROMAX merge."""

from __future__ import annotations

import re
from typing import Literal, Optional

LineVerdict = Literal["ad", "enhance", "neutral"]

# Script-path / rule-name signals
AD_SCRIPT_MARKERS = (
    "adblock",
    "/adblock/",
    ".adblock.",
    "anti-ad",
    "blockad",
    "remove_ads",
    "remove-ads",
    "去广告",
    "/ads/",
    "advertising",
    "reject",
    "spam",
    "tracking",
    "xwebads",
    "adultraplus",
    "allinone",
    "mock",
    "bilibili.helper",
    "helper.v2.js",
    "camoufox",  # generic block lists — rarely unlock
)

ENHANCE_SCRIPT_MARKERS = (
    "/enhanced/",
    ".enhanced.",
    "/enhance/",
    "biliuniverse/enhanced",
    "/global/",
    ".global.",
    "biliuniverse/global",
    "/redirect/",
    ".redirect.",
    "unlock",
    "解锁",
    "unblock",
    "vip",
    "premium",
    "crack",
    "破解",
    "camscanner.js",
    "external_links_unlock",
    "weixin_external",
    "1080p",
    "bilibili_json.js",  # deezertidal quality unlock
    "dualsubs",
    "iringo",
    "weatherkit",
    "zheye",
    "哲也",
    "translation",
    "翻译",
    "nsfw",
    "region",
    "intl",
    "bypass",
    "绕过",
    "pip",
    "画中画",
    "optimize",
    "功能增强",
)

ENHANCE_NAME_PREFIXES = (
    "bilibili.enhanced",
    "bilibili.global",
    "bilibili.redirect",
    "youtube.enhance",
    "wechat_enhance",
    "iringo.",
    "dualsubs",
)

AD_NAME_PREFIXES = (
    "bilibili.adblock",
    "adblock",
    "去广告",
    "xwebads",
    "anti-ad",
)

REJECT_POLICIES = frozenset(
    {
        "REJECT",
        "REJECT-DROP",
        "REJECT-NO-DROP",
        "REJECT-TINYGIF",
        "REJECT-IMG",
    }
)


def _line_lower(line: str) -> str:
    return line.strip().lower()


def _script_path(line: str) -> str:
    m = re.search(r"script-path\s*=\s*([^,\s]+)", line, re.I)
    return (m.group(1) if m else "").lower()


def _entry_name(line: str) -> str:
    if "=" not in line:
        return ""
    return line.split("=", 1)[0].strip().lower()


def classify_promax_line(line: str, section: Optional[str] = None) -> LineVerdict:
    """Classify one non-comment module line for PROMAX ingestion."""
    stripped = line.strip()
    if not stripped or stripped.startswith("#"):
        return "neutral"

    low = _line_lower(stripped)
    sec = (section or "").strip().lower()

    if sec == "mitm":
        if "hostname" not in low:
            return "neutral"
        if any(m in low for m in ("unlock", "解锁", "camscanner", "intsig.net")):
            return "enhance"
        return "neutral"

    if sec == "general":
        return "enhance"

    if sec == "header rewrite":
        return "enhance"

    name = _entry_name(stripped)
    spath = _script_path(stripped)

    if sec == "script" or "script-path=" in low:
        if any(m in name for m in AD_NAME_PREFIXES) or any(
            m in spath for m in AD_SCRIPT_MARKERS
        ):
            return "ad"
        if any(m in name for m in ENHANCE_NAME_PREFIXES) or any(
            m in spath for m in ENHANCE_SCRIPT_MARKERS
        ):
            return "enhance"
        if "helper" in name and "bili" in name:
            return "ad"
        return "enhance"

    if sec == "url rewrite" or (
        stripped.startswith("^") and (" reject" in low or " _ reject" in low)
    ):
        if " reject" in low or " _ reject" in low:
            return "ad"
        return "enhance"

    if sec in ("map local", "body rewrite", "maplocal"):
        if any(m in low for m in ("广告", "adcard", "splash", "deliver", "flash", "e-commerce")):
            return "ad"
        if 'data="{}"' in low or "data='{}'" in low:
            return "ad"
        if "del(.data.payment)" in low or "payment" in low and "del(" in low:
            return "ad"
        return "neutral"

    if sec == "rule" or stripped.split(",", 1)[0].upper() in (
        "DOMAIN",
        "DOMAIN-SUFFIX",
        "DOMAIN-KEYWORD",
        "DOMAIN-REGEX",
        "DOMAIN-WILDCARD",
        "URL-REGEX",
        "IP-CIDR",
        "IP-CIDR6",
        "USER-AGENT",
        "PROCESS-NAME",
        "DEST-PORT",
    ):
        parts = [p.strip() for p in stripped.split(",")]
        policy = parts[-1].upper() if parts else ""
        if policy in REJECT_POLICIES:
            if any(m in low for m in ("unlock", "解锁", "vip", "premium", "crack")):
                return "enhance"
            return "ad"
        # Bare DOMAIN / IP rules (no trailing policy) → merged as REJECT in AdBlock lists
        if parts and parts[0].upper() in (
            "DOMAIN",
            "DOMAIN-SUFFIX",
            "DOMAIN-KEYWORD",
            "DOMAIN-REGEX",
            "DOMAIN-WILDCARD",
            "URL-REGEX",
            "IP-CIDR",
            "IP-CIDR6",
            "USER-AGENT",
            "PROCESS-NAME",
            "DEST-PORT",
        ):
            if any(m in low for m in ("unlock", "解锁", "vip", "premium", "crack")):
                return "enhance"
            return "ad"
        return "enhance"

    if any(m in low for m in ENHANCE_SCRIPT_MARKERS):
        return "enhance"
    if any(m in low for m in AD_SCRIPT_MARKERS):
        return "ad"
    return "neutral"


def should_keep_promax_line(line: str, section: Optional[str] = None) -> bool:
    verdict = classify_promax_line(line, section)
    if verdict == "ad":
        return True
    if verdict == "enhance":
        return False
    sec = (section or "").strip().lower()
    if sec == "mitm" and "hostname" in line.lower():
        return True
    return False


def module_ingest_mode(path: str, text: str) -> str:
    """
    Return ingest mode for a module file:
      - skip: unlock-only or no ad content
      - full: pure ad module (all lines considered)
      - split: mixed ad + enhancement — line-level filter
    """
    import os

    name = os.path.basename(path).lower()
    if "解锁" in name or "unlock" in name:
        if not any(t in name for t in ("去广告", "adblock", "anti-ad")):
            return "skip"

    ad_lines = 0
    enhance_lines = 0
    current_section: Optional[str] = None

    for raw in text.splitlines():
        stripped = raw.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            current_section = stripped[1:-1]
            continue
        if not stripped or stripped.startswith("#"):
            continue
        v = classify_promax_line(stripped, current_section)
        if v == "ad":
            ad_lines += 1
        elif v == "enhance":
            enhance_lines += 1

    if ad_lines == 0:
        return "skip"
    if enhance_lines > 0:
        return "split"
    return "full"
