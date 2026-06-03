#!/usr/bin/env python3
"""Upstream Surge module bundle merges (BiliBili, YouTube, Weibo, Apple)."""

from __future__ import annotations

import os
import re
import sys
from typing import Callable, List, Optional

from hub.common import Logger, get_project_root, read_file, safe_download, write_file
from hub.merge_upstream import merge_upstream_modules
from hub.module_sanitizer import (
    format_header,
    format_module,
    merge_mitm_hosts,
    parse_module,
)
from hub.paths import BILIBILI_HELPER_URL, LOCAL_DIR, YOUTUBE_ENHANCE_URL

ROOT = get_project_root()

BILIBILI_OUTPUT = os.path.join(
    ROOT, "modules/surge/amplify_nexus/📺 BiliBili增强合集.sgmodule"
)
BILIBILI_SOURCES = [
    ("Enhanced", "https://github.com/BiliUniverse/Enhanced/releases/latest/download/BiliBili.Enhanced.sgmodule"),
    ("Global", "https://github.com/BiliUniverse/Global/releases/latest/download/BiliBili.Global.sgmodule"),
    ("Redirect", "https://github.com/BiliUniverse/Redirect/releases/latest/download/BiliBili.Redirect.sgmodule"),
    ("Helper", BILIBILI_HELPER_URL),
    ("Bili1080P", "https://yfamilys.com/module/bili.module"),
]
BILIBILI_HEADER = {
    "name": "📺 BiliBili增强合集",
    "desc": "合并 BiliUniverse + Maasea + Bili1080P（Enhanced/Global/Redirect/Helper）",
    "author": "BiliUniverse, Maasea",
    "icon": "https://www.bilibili.com/favicon.ico",
    "category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "tag": "BiliBili, 增强",
}

APPLE_OUTPUT = os.path.join(
    ROOT, "modules/surge/amplify_nexus/🍎 Apple服务增强合集.sgmodule"
)
APPLE_SOURCES = [
    ("iRingo.Maps", "https://github.com/NSRingo/GeoServices/releases/latest/download/iRingo.Maps.sgmodule"),
    ("iRingo.WeatherKit", "https://github.com/NSRingo/WeatherKit/releases/latest/download/iRingo.WeatherKit.sgmodule"),
    ("iRingo.News", "https://github.com/NSRingo/News/releases/latest/download/iRingo.News.sgmodule"),
    ("iRingo.TV", "https://github.com/NSRingo/TV/releases/latest/download/iRingo.TV.sgmodule"),
    ("DualSubs.Universal", "https://github.com/DualSubs/Universal/releases/latest/download/DualSubs.Universal.sgmodule"),
]
APPLE_HEADER = {
    "name": "🍎 Apple服务增强合集",
    "desc": (
        "整合 Apple 生态增强：Maps · WeatherKit · News · TV · DualSubs 双语字幕"
        "\\nTV 需开启 SSL Pinning 绕过以配合 DualSubs；News 可自定义地区"
    ),
    "author": "VirgilClyne[https://github.com/VirgilClyne]",
    "homepage": "https://NSRingo.github.io",
    "icon": "https://developer.apple.com/assets/elements/icons/sf-symbols/sf-symbols-128x128.png",
    "category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
}

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

YOUTUBE_OUTPUT = os.path.join(
    ROOT, "modules/surge/amplify_nexus/📺 YouTube增强合集.sgmodule"
)
YOUTUBE_ADBLOCK_OUTPUT = os.path.join(LOCAL_DIR, "YouTube.ADBlock.sgmodule")
YOUTUBE_LOCAL_FALLBACKS = (
    os.path.join(ROOT, "modules/surge/amplify_nexus/YouTube.Enhance.sgmodule"),
    YOUTUBE_OUTPUT,
)

UTILITIES_OUTPUT = os.path.join(
    ROOT, "modules/surge/amplify_nexus/📊 面板工具合集.sgmodule"
)
UTILITIES_SOURCES = [
    ("Timecard", "https://raw.githubusercontent.com/Rabbit-Spec/Surge/Master/Module/Panel/Timecard/Moore/Timecard.sgmodule"),
    ("net-lsp-x", "https://raw.githubusercontent.com/xream/scripts/main/surge/module/network-info/net-lsp-x.sgmodule"),
    ("Sub_Info", "https://raw.githubusercontent.com/Coldvvater/Mononoke/refs/heads/master/Surge/Module/Tool/Sub_Info.sgmodule"),
]
UTILITIES_HEADER = {
    "name": "📊 面板工具合集",
    "desc": "节假日信息 · 网络信息 𝕏 · 机场订阅流量/到期面板",
    "author": "Rabbit-Spec, xream, Coldvvater",
    "icon": "https://raw.githubusercontent.com/Koolson/Qure/master/IconSet/Color/Tool.png",
    "category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "tag": "面板, 工具",
}


def merge_bilibili() -> None:
    Logger.section("BiliBili upstream bundle merge")
    merge_upstream_modules(
        BILIBILI_SOURCES,
        BILIBILI_OUTPUT,
        header_meta=BILIBILI_HEADER,
        optional_labels=("Helper",),
    )


def merge_apple() -> None:
    Logger.section("Apple services upstream bundle merge")
    merge_upstream_modules(APPLE_SOURCES, APPLE_OUTPUT, header_meta=APPLE_HEADER)


def merge_utilities() -> None:
    Logger.section("Panel utilities upstream bundle merge")
    nexus = os.path.join(ROOT, "modules/surge/amplify_nexus")
    merge_upstream_modules(
        UTILITIES_SOURCES,
        UTILITIES_OUTPUT,
        header_meta=UTILITIES_HEADER,
        local_fallbacks={
            "Timecard": os.path.join(nexus, "Timecard.sgmodule"),
            "net-lsp-x": os.path.join(nexus, "net-lsp-x.sgmodule"),
            "Sub_Info": os.path.join(nexus, "Sub_Info.sgmodule"),
        },
    )


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


def _fetch_youtube_upstream() -> Optional[str]:
    text = safe_download(YOUTUBE_ENHANCE_URL, retries=2, timeout=60)
    if text and len(text.strip()) > 100:
        Logger.info(f"Using upstream: {YOUTUBE_ENHANCE_URL}")
        return text
    Logger.warn(f"YouTube upstream unavailable: {YOUTUBE_ENHANCE_URL}")
    for path in YOUTUBE_LOCAL_FALLBACKS:
        if os.path.isfile(path):
            Logger.warn(f"Using local fallback: {os.path.relpath(path, ROOT)}")
            return "".join(read_file(path))
    return None


def merge_youtube() -> None:
    Logger.section("YouTube upstream bundle merge (Stripped of ADBlock)")
    text = _fetch_youtube_upstream()
    if not text:
        raise RuntimeError(
            "YouTube.Enhance unavailable (Maasea upstream 404 and no local fallback). "
            "Run upstream_sync.sync_nexus() first or retry later."
        )

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
        "# - Stripped: [Rule], [Map Local] and *.googlevideo.com from MITM",
    ]
    header_lines = format_header(header, extra_lines=extra)
    os.makedirs(os.path.dirname(YOUTUBE_OUTPUT) or ".", exist_ok=True)
    with open(YOUTUBE_OUTPUT, "w", encoding="utf-8") as f:
        content = format_module(header_lines, final_sections, dedupe=True)
        if not content or len(content.strip()) < 100:
            raise RuntimeError(f"Generated YouTube bundle too small: {YOUTUBE_OUTPUT}")
        f.write(content)
    Logger.success(f"Wrote stripped bundle: {YOUTUBE_OUTPUT}")

    adblock_header = [
        "#!name=YouTube 去广告",
        "#!desc=YouTube 广告过滤 (自动从 Maasea 上游提取并合并进入 PROMAX)",
        "#!category=🪐 local",
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

    if adblock_sections_lines:
        adblock_content = "\n".join(adblock_header) + "\n".join(adblock_sections_lines)
        os.makedirs(os.path.dirname(YOUTUBE_ADBLOCK_OUTPUT) or ".", exist_ok=True)
        with open(YOUTUBE_ADBLOCK_OUTPUT, "w", encoding="utf-8") as f:
            f.write(adblock_content.strip() + "\n")
        Logger.success(f"Wrote extracted adblock module: {YOUTUBE_ADBLOCK_OUTPUT}")
    else:
        Logger.warn("No YouTube adblock sections extracted; kept existing local module if any.")


def run_all(*, strict: bool = False) -> List[str]:
    """Run all bundle merges. Returns labels that failed (empty if all OK)."""
    steps: list[tuple[str, Callable[[], None]]] = [
        ("bilibili", merge_bilibili),
        ("youtube", merge_youtube),
        ("weibo", merge_weibo),
        ("apple", merge_apple),
        ("utilities", merge_utilities),
    ]
    failures: List[str] = []
    for label, fn in steps:
        try:
            fn()
        except Exception as exc:
            Logger.error(f"Bundle merge '{label}' failed: {exc}")
            failures.append(label)
            if strict:
                raise
    if failures:
        Logger.warn(f"Bundle merges incomplete: {', '.join(failures)}")
    else:
        Logger.success("All upstream bundle merges completed.")
    return failures


if __name__ == "__main__":
    strict = "--strict" in sys.argv
    failed = run_all(strict=strict)
    sys.exit(1 if failed else 0)
