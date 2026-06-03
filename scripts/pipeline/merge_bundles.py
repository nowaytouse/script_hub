#!/usr/bin/env python3
"""Upstream Surge module bundle merges (BiliBili, YouTube, Weibo, Apple, DNS)."""

from __future__ import annotations

import os
import re
import sys
import urllib.request
from datetime import datetime
from typing import Dict, List, Optional, Sequence, Tuple

from hub.common import Logger, get_project_root, read_file, safe_download, write_file
from hub.merge_upstream import merge_upstream_modules
from hub.module_sanitizer import (
    format_header,
    format_module,
    merge_mitm_hosts,
    parse_module,
)

ROOT = get_project_root()

# --- BiliBili ---

BILIBILI_OUTPUT = os.path.join(
    ROOT, "modules/surge/amplify_nexus/📺 BiliBili增强合集.sgmodule"
)
BILIBILI_SOURCES = [
    ("Enhanced", "https://github.com/BiliUniverse/Enhanced/releases/latest/download/BiliBili.Enhanced.sgmodule"),
    ("Global", "https://github.com/BiliUniverse/Global/releases/latest/download/BiliBili.Global.sgmodule"),
    ("Redirect", "https://github.com/BiliUniverse/Redirect/releases/latest/download/BiliBili.Redirect.sgmodule"),
    ("Helper", "https://raw.githubusercontent.com/Maasea/sgmodules/master/Bilibili.Helper.sgmodule"),
]
BILIBILI_HEADER = {
    "name": "📺 BiliBili增强合集",
    "desc": "合并 BiliUniverse + Maasea 上游（Enhanced/Global/Redirect/Helper）",
    "author": "BiliUniverse, Maasea",
    "icon": "https://www.bilibili.com/favicon.ico",
    "category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "tag": "BiliBili, 增强",
}

# --- Apple ---

APPLE_OUTPUT = os.path.join(
    ROOT, "modules/surge/amplify_nexus/🍎 Apple服务增强合集.sgmodule"
)
APPLE_SOURCES = [
    ("iRingo.Maps", "https://github.com/NSRingo/GeoServices/releases/latest/download/iRingo.Maps.sgmodule"),
    ("iRingo.WeatherKit", "https://github.com/NSRingo/WeatherKit/releases/latest/download/iRingo.WeatherKit.sgmodule"),
]
APPLE_HEADER = {
    "name": "🍎 Apple服务增强合集",
    "desc": "整合 iRingo 系列模块\\n包含: Maps(地图增强) + WeatherKit(天气增强)\\n解锁Apple服务的国际版功能",
    "author": "VirgilClyne[https://github.com/VirgilClyne]",
    "homepage": "https://NSRingo.github.io",
    "icon": "https://developer.apple.com/assets/elements/icons/sf-symbols/sf-symbols-128x128.png",
    "category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
}

# --- Weibo ---

WEIBO_LOCAL = os.path.join(
    ROOT, "rulesets/Sources/LocalModules/🐦 微博去广告合集.sgmodule"
)
WEIBO_SOURCES = [
    ("Main", "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/weibo.module"),
    ("Intl", "https://raw.githubusercontent.com/iab0x00/ProxyRules/main/Rewrite/WeiboIntl.sgmodule"),
]
WEIBO_HEADER = {
    "name": "🐦 微博去广告",
    "desc": (
        "微博+国际版去广告 · PROMAX 构建源（域名规则+脚本已并入 PROMAX，勿单独安装）"
        "\\n\\n上游: fmz200/wool_scripts, iab0x00"
    ),
    "author": "fmz200, iab0x00, ScriptHub",
    "tag": "去广告, 微博, PROMAX-build",
}
WEIBO_SECTION_ORDER = (
    "Rule",
    "URL Rewrite",
    "Body Rewrite",
    "Map Local",
    "Script",
    "MITM",
)

# --- YouTube ---

YOUTUBE_OUTPUT = os.path.join(
    ROOT, "modules/surge/amplify_nexus/📺 YouTube增强合集.sgmodule"
)
YOUTUBE_ADBLOCK_OUTPUT = os.path.join(
    ROOT, "modules/source/local_sources/YouTube.ADBlock.sgmodule"
)
YOUTUBE_UPSTREAM_URL = "https://raw.githubusercontent.com/Maasea/sgmodules/master/YouTube.Enhance.sgmodule"

# --- DNS (GetSomeFries rule injection) ---

DNS_OUTPUT = os.path.join(
    ROOT, "modules/surge/amplify_nexus/🌐 DNS & Host Enhanced.sgmodule"
)
DNS_HTTPDNS_URL = "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodules/HTTPDNS.Block.sgmodule"
DNS_ASN_URL = "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodules/ASN.China.sgmodule"


def merge_bilibili() -> None:
    Logger.section("BiliBili upstream bundle merge")
    merge_upstream_modules(BILIBILI_SOURCES, BILIBILI_OUTPUT, header_meta=BILIBILI_HEADER)


def merge_apple() -> None:
    Logger.section("Apple services upstream bundle merge")
    merge_upstream_modules(APPLE_SOURCES, APPLE_OUTPUT, header_meta=APPLE_HEADER)


def _load_preserved_rule_sections(path: str) -> dict[str, list[str]]:
    if not os.path.isfile(path):
        return {}
    _, sections = parse_module("".join(read_file(path)))
    return {name: list(lines) for name, lines in sections if name == "Rule" and lines}


def _rewrite_weibo_with_preserved(path: str, preserved: dict[str, list[str]]) -> None:
    if not preserved:
        return
    meta, sections = parse_module("".join(read_file(path)))
    merged: dict[str, list[str]] = {name: list(lines) for name, lines in sections}
    for name, lines in preserved.items():
        merged[name] = lines
    ordered = [(name, merged[name]) for name in WEIBO_SECTION_ORDER if name in merged and merged[name]]
    for name in sorted(merged):
        if name not in WEIBO_SECTION_ORDER and merged[name]:
            ordered.append((name, merged[name]))
    ordered = merge_mitm_hosts(ordered)
    header_lines = format_header(
        meta,
        extra_lines=["# PROMAX build source — install head_expanse/PROMAX only"],
    )
    write_file(path, format_module(header_lines, ordered, dedupe=True))


def merge_weibo() -> None:
    Logger.section("Weibo → LocalModules (PROMAX build source)")
    preserved = _load_preserved_rule_sections(WEIBO_LOCAL)
    merge_upstream_modules(
        WEIBO_SOURCES,
        WEIBO_LOCAL,
        header_meta=WEIBO_HEADER,
        provenance_comment="# Merged for rulesets/Sources/LocalModules → PROMAX ingest",
    )
    _rewrite_weibo_with_preserved(WEIBO_LOCAL, preserved)
    Logger.info(f"Build source: {os.path.relpath(WEIBO_LOCAL, ROOT)}")


def merge_youtube() -> None:
    Logger.section("YouTube upstream bundle merge (Stripped of ADBlock)")
    Logger.info(f"Downloading {YOUTUBE_UPSTREAM_URL}...")
    text = safe_download(YOUTUBE_UPSTREAM_URL)
    if not text:
        Logger.error("Failed to download YouTube.Enhance.sgmodule")
        sys.exit(1)

    meta, sections = parse_module(text)
    cleaned_sections = []
    mitm_hosts: set[str] = set()
    adblock_rules: list[str] = []
    adblock_map_locals: list[str] = []
    adblock_mitm_hosts: set[str] = set()

    for name, lines in sections:
        if name == "Rule":
            adblock_rules.extend(lines)
            Logger.info(f"  - Stripping section: {name}")
            continue
        if name == "Map Local":
            adblock_map_locals.extend(lines)
            Logger.info(f"  - Stripping section: {name}")
            continue

        cleaned_sections.append((name, lines))
        if name == "MITM":
            for line in lines:
                if line.strip().startswith("hostname"):
                    hosts = re.sub(r"^hostname\s*=\s*(%APPEND%\s*)?", "", line.strip())
                    for h in hosts.split(","):
                        h_clean = h.strip()
                        if not h_clean:
                            continue
                        if h_clean == "*.googlevideo.com":
                            adblock_mitm_hosts.add(h_clean)
                        else:
                            mitm_hosts.add(h_clean)
                            if "youtubei" in h_clean:
                                adblock_mitm_hosts.add(h_clean)

    final_sections: list[tuple[str, list[str]]] = []
    for name, lines in cleaned_sections:
        if name == "MITM":
            if mitm_hosts:
                final_sections.append(
                    ("MITM", [f"hostname = %APPEND% {', '.join(sorted(mitm_hosts))}"])
                )
        else:
            final_sections.append((name, lines))

    header = {
        "name": "📺 YouTube增强合集",
        "desc": "合并 YouTube 增强 (Maasea 上游) | Enhance: 画中画/后台播放/字幕翻译",
        "author": "Maasea",
        "icon": "https://raw.githubusercontent.com/Koolson/Qure/master/IconSet/Color/YouTube.png",
        "category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
        "tag": "YouTube, 增强",
    }
    if "arguments" in meta:
        header["arguments"] = meta["arguments"]
    if "arguments-desc" in meta:
        header["arguments-desc"] = meta["arguments-desc"]

    extra = [
        "# Upstream module processed by scripts/pipeline/merge_bundles.py",
        f"# - Source: {YOUTUBE_UPSTREAM_URL}",
        "# - Stripped: [Rule], [Map Local] and *.googlevideo.com from MITM",
    ]
    header_lines = format_header(header, extra_lines=extra)
    os.makedirs(os.path.dirname(YOUTUBE_OUTPUT) or ".", exist_ok=True)
    with open(YOUTUBE_OUTPUT, "w", encoding="utf-8") as f:
        content = format_module(header_lines, final_sections, dedupe=True)
        if not content or len(content.strip()) < 100:
            Logger.error(f"Generated content too small or empty. Aborting write to {YOUTUBE_OUTPUT}")
            sys.exit(1)
        f.write(content)
    Logger.success(f"Wrote stripped bundle: {YOUTUBE_OUTPUT}")

    adblock_header = [
        "#!name=YouTube 去广告",
        "#!desc=YouTube 广告过滤 (自动从 Maasea 上游提取并合并进入 PROMAX)",
        "#!category=🪐 local_sources",
        "",
    ]
    adblock_sections_lines: list[str] = []
    if adblock_rules:
        adblock_sections_lines.append("[Rule]")
        adblock_sections_lines.extend(adblock_rules)
        adblock_sections_lines.append("")
    if adblock_map_locals:
        adblock_sections_lines.append("[Map Local]")
        adblock_sections_lines.extend(adblock_map_locals)
        adblock_sections_lines.append("")
    if adblock_mitm_hosts:
        adblock_sections_lines.append("[MITM]")
        adblock_sections_lines.append(
            f"hostname = %APPEND% {', '.join(sorted(adblock_mitm_hosts))}, "
            "-github.com, -api.github.com, -*.githubusercontent.com"
        )
        adblock_sections_lines.append("")

    adblock_content = "\n".join(adblock_header) + "\n".join(adblock_sections_lines)
    os.makedirs(os.path.dirname(YOUTUBE_ADBLOCK_OUTPUT) or ".", exist_ok=True)
    with open(YOUTUBE_ADBLOCK_OUTPUT, "w", encoding="utf-8") as f:
        f.write(adblock_content.strip() + "\n")
    Logger.success(f"Wrote extracted adblock module: {YOUTUBE_ADBLOCK_OUTPUT}")


def _dns_download(url: str, label: str) -> Optional[str]:
    try:
        with urllib.request.urlopen(url, timeout=30) as resp:
            return resp.read().decode("utf-8")
    except Exception as e:
        Logger.error(f"{label} download failed: {e}")
        return None


def _dns_extract_rules(content: str) -> str:
    match = re.search(r"\[Rule\](.*?)(?=\n\[|$)", content, re.DOTALL)
    if not match:
        return ""
    lines = []
    for line in match.group(1).strip().split("\n"):
        line = line.strip()
        if line and (
            line.startswith("#")
            or line.startswith("DOMAIN")
            or line.startswith("IP-")
            or line.startswith("RULE-SET")
        ):
            lines.append(line)
    return "\n".join(lines)


def merge_dns() -> None:
    Logger.section("DNS module merge (GetSomeFries rules)")
    httpdns = _dns_download(DNS_HTTPDNS_URL, "HTTPDNS.Block")
    asn = _dns_download(DNS_ASN_URL, "ASN.China")
    if not httpdns or not asn:
        raise RuntimeError("DNS upstream download failed")

    if not os.path.exists(DNS_OUTPUT):
        raise FileNotFoundError(f"Local DNS module not found: {DNS_OUTPUT}")

    with open(DNS_OUTPUT, "r", encoding="utf-8") as f:
        local = f.read()

    httpdns_rules = _dns_extract_rules(httpdns)
    asn_rules = _dns_extract_rules(asn)
    local = re.sub(r"\n\[Rule\].*?(?=\n\[MITM\])", "", local, flags=re.DOTALL)

    date_str = datetime.now().strftime("%Y.%m.%d")
    rule_section = f"""
[Rule]
# FROM: GetSomeFries HTTPDNS.Block (Block HTTPDNS hijacking)
# AUTO-MERGED: {date_str}
{httpdns_rules}

# FROM: GetSomeFries ASN.China (China ASN Direct)
# AUTO-MERGED: {date_str}
{asn_rules}
"""
    if "[MITM]" in local:
        local = local.replace("[MITM]", rule_section + "\n[MITM]")
    else:
        local += rule_section

    local = re.sub(r"^#!version=.*$", f"#!version={date_str}", local, flags=re.MULTILINE)
    with open(DNS_OUTPUT, "w", encoding="utf-8") as f:
        f.write(local)

    httpdns_count = len([l for l in httpdns_rules.split("\n") if l.startswith(("DOMAIN", "IP-"))])
    asn_count = len([l for l in asn_rules.split("\n") if l.startswith("IP-ASN")])
    Logger.success(
        f"DNS merge: HTTPDNS={httpdns_count} rules, ASN={asn_count} rules (DoH preserved)"
    )


def run_all() -> None:
    merge_bilibili()
    merge_youtube()
    merge_weibo()
    merge_apple()
    merge_dns()


if __name__ == "__main__":
    run_all()
