#!/usr/bin/env python3
"""Upstream Surge module bundle merges (BiliBili, YouTube, Weibo, Apple)."""

from __future__ import annotations

import os
import re
import sys
from typing import Callable, List, Optional

from hub.common import Logger, get_project_root, read_file, safe_download, safe_download_binary, safe_remove, write_file
from hub.merge_upstream import merge_upstream_modules
from hub.module_sanitizer import (
    SECTION_ORDER,
    format_header,
    format_module,
    merge_mitm_hosts,
    parse_module,
)
from hub.paths import (
    AMPLIFY_NEXUS_DIR,
    BILIBILI_HELPER_URL,
    LOCAL_DIR,
    SCRIPTS_DIR,
    SCRIPT_RAW_PREFIX,
    YOUTUBE_ENHANCE_URL,
)

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
    ("net-lsp-x", "https://raw.githubusercontent.com/xream/scripts/main/surge/modules/network-info/net-lsp-x.sgmodule"),
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

DEVTOOLS_OUTPUT = os.path.join(
    ROOT, "modules/surge/amplify_nexus/🧰 Script Hub 配套工具合集.sgmodule"
)
SCRIPT_HUB_MODULE_URL = (
    "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/modules/script-hub.surge.sgmodule"
)
SCRIPT_HUB_LOCAL = os.path.join(LOCAL_DIR, "script_hub.surge.sgmodule")

DEVTOOLS_SOURCES = [
    (
        "BoxJs",
        "https://raw.githubusercontent.com/chavyleung/scripts/master/box/rewrite/boxjs.rewrite.surge.sgmodule",
    ),
    (
        "Sub-Store",
        "https://raw.githubusercontent.com/sub-store-org/Sub-Store/master/config/Surge-Beta.sgmodule",
    ),
    ("ScriptHub", SCRIPT_HUB_MODULE_URL),
]

SUB_STORE_SCRIPT_SOURCES = {
    "github_com_sub-store-org_sub-store-1.min.js": (
        "https://github.com/sub-store-org/Sub-Store/releases/latest/download/sub-store-1.min.js"
    ),
    "github_com_sub-store-org_sub-store-0.min.js": (
        "https://github.com/sub-store-org/Sub-Store/releases/latest/download/sub-store-0.min.js"
    ),
    "github_com_sub-store-org_cron-sync-artifacts.min.js": (
        "https://github.com/sub-store-org/Sub-Store/releases/latest/download/cron-sync-artifacts.min.js"
    ),
}

SUB_STORE_SCRIPT_PATH_MAP = {
    "sub-store-1.min.js": "github_com_sub-store-org_sub-store-1.min.js",
    "sub-store-0.min.js": "github_com_sub-store-org_sub-store-0.min.js",
    "cron-sync-artifacts.min.js": "github_com_sub-store-org_cron-sync-artifacts.min.js",
}

SCRIPT_HUB_SCRIPT_SOURCES = {
    "raw_githubusercontent_com_f59ef7_script-hub.js": (
        "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/script-hub.js"
    ),
    "raw_githubusercontent_com_4fd0f5_Rewrite-Parser.js": (
        "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/Rewrite-Parser.js"
    ),
    "raw_githubusercontent_com_0aa101_rule-parser.js": (
        "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/rule-parser.js"
    ),
    "raw_githubusercontent_com_95a370_script-converter.js": (
        "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/script-converter.js"
    ),
}

MERGED_STANDALONE_SURGE = (
    "boxjs.rewrite.surge.sgmodule",
    "Surge-Beta.sgmodule",
    "Script Hub 重写 & 规则集转换.sgmodule",
    "BiliBili.Enhanced.sgmodule",
    "BiliBili.Global.sgmodule",
    "BiliBili.Redirect.sgmodule",
    "bili.sgmodule",
    "YouTube.Enhance.sgmodule",
    "iRingo.Maps.sgmodule",
    "iRingo.WeatherKit.sgmodule",
    "iRingo.News.sgmodule",
    "iRingo.TV.sgmodule",
    "DualSubs.Universal.sgmodule",
    "Timecard.sgmodule",
    "net-lsp-x.sgmodule",
    "Sub_Info.sgmodule",
)
DEVTOOLS_HEADER = {
    "name": "🧰 Script Hub 配套工具合集",
    "desc": (
        "Script Hub 转换 (script.hub) · BoxJs · Sub-Store(β)"
        "\\nScript Hub / Sub-Store 脚本已 vendored 至 modules/source/scripts；BoxJs 仍走 chavyleung 上游"
    ),
    "author": "@小白脸 @xream @keywos @ckyb, ChavyLeung, sub-store-org",
    "icon": "https://raw.githubusercontent.com/Koolson/Qure/master/IconSet/Color/Scriptable.png",
    "category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "tag": "ScriptHub, BoxJs, Sub-Store",
    "homepage": "https://script.hub",
}

SCRIPT_HUB_VENDORED_SCRIPTS = (
    "raw_githubusercontent_com_f59ef7_script-hub.js",
    "raw_githubusercontent_com_4fd0f5_Rewrite-Parser.js",
    "raw_githubusercontent_com_0aa101_rule-parser.js",
    "raw_githubusercontent_com_95a370_script-converter.js",
)


SCRIPT_HUB_SCRIPT_PATH_MAP = {
    "script-hub.js": "raw_githubusercontent_com_f59ef7_script-hub.js",
    "Rewrite-Parser.js": "raw_githubusercontent_com_4fd0f5_Rewrite-Parser.js",
    "rule-parser.js": "raw_githubusercontent_com_0aa101_rule-parser.js",
    "script-converter.js": "raw_githubusercontent_com_95a370_script-converter.js",
}


def _pin_sub_store_script_paths(module_path: str) -> None:
    """Pin Sub-Store min.js URLs to this repo's vendored copies."""
    if not os.path.isfile(module_path):
        return
    text = "".join(read_file(module_path))
    changed = 0
    for upstream_suffix, local_name in SUB_STORE_SCRIPT_PATH_MAP.items():
        local = os.path.join(SCRIPTS_DIR, local_name)
        if not os.path.isfile(local):
            continue
        canonical = SCRIPT_RAW_PREFIX + local_name
        new_text, n = re.subn(
            rf"script-path=https?://[^\s,]*{re.escape(upstream_suffix)}",
            f"script-path={canonical}",
            text,
        )
        if n:
            text = new_text
            changed += n
    if changed:
        write_file(module_path, text)
        Logger.info(
            f"  Pinned {changed} Sub-Store script-path URL(s) in {os.path.basename(module_path)}"
        )


def _sync_sub_store_scripts() -> None:
    """Refresh Sub-Store backend scripts from sub-store-org releases."""
    os.makedirs(SCRIPTS_DIR, exist_ok=True)
    updated = 0
    for local_name, url in SUB_STORE_SCRIPT_SOURCES.items():
        content = safe_download_binary(url, retries=2, timeout=90)
        if not content:
            Logger.warn(f"Sub-Store script sync skipped: {local_name}")
            continue
        target = os.path.join(SCRIPTS_DIR, local_name)
        write_file(target, content.decode("utf-8", errors="replace"))
        updated += 1
    if updated:
        Logger.success(
            f"Synced {updated}/{len(SUB_STORE_SCRIPT_SOURCES)} Sub-Store scripts to modules/source/scripts/"
        )


def _pin_script_hub_script_paths(module_path: str) -> None:
    """Rewrite Script Hub script-path URLs to this repo's vendored copies."""
    if not os.path.isfile(module_path):
        return
    text = "".join(read_file(module_path))
    changed = 0
    for upstream_suffix, local_name in SCRIPT_HUB_SCRIPT_PATH_MAP.items():
        local = os.path.join(SCRIPTS_DIR, local_name)
        if not os.path.isfile(local):
            continue
        canonical = SCRIPT_RAW_PREFIX + local_name
        new_text, n = re.subn(
            rf"script-path=https?://[^\s,]*{re.escape(upstream_suffix)}",
            f"script-path={canonical}",
            text,
        )
        if n:
            text = new_text
            changed += n
        new_text, n = re.subn(
            rf"script-path=https?://[^\s,]*{re.escape(local_name)}",
            f"script-path={canonical}",
            text,
        )
        if n:
            text = new_text
            changed += n
    if changed:
        write_file(module_path, text)
        Logger.info(f"  Pinned {changed} Script Hub script-path URL(s) in {os.path.basename(module_path)}")


def _sync_script_hub_scripts() -> None:
    """Refresh Script Hub converter scripts from Script-Hub-Org upstream."""
    os.makedirs(SCRIPTS_DIR, exist_ok=True)
    updated = 0
    for local_name, url in SCRIPT_HUB_SCRIPT_SOURCES.items():
        content = safe_download_binary(url, retries=2, timeout=60)
        if not content:
            Logger.warn(f"Script Hub script sync skipped: {local_name}")
            continue
        target = os.path.join(SCRIPTS_DIR, local_name)
        write_file(target, content.decode("utf-8", errors="replace"))
        updated += 1
    if updated:
        Logger.success(f"Synced {updated}/{len(SCRIPT_HUB_SCRIPT_SOURCES)} Script Hub scripts from upstream")


def _cleanup_merged_standalone_modules() -> None:
    """Remove standalone copies superseded by bundle modules in amplify_nexus."""
    sr_dir = os.path.join(ROOT, "modules/shadowrocket/amplify_nexus")
    removed = 0
    for name in MERGED_STANDALONE_SURGE:
        surge_path = os.path.join(AMPLIFY_NEXUS_DIR, name)
        if safe_remove(surge_path):
            removed += 1
        sr_name = name.replace(".sgmodule", ".module")
        sr_path = os.path.join(sr_dir, sr_name)
        if safe_remove(sr_path):
            removed += 1
    if removed:
        Logger.info(f"Removed {removed} merged standalone module file(s)")


def _pin_script_hub_bundle_scripts(output_path: str) -> None:
    _pin_script_hub_script_paths(output_path)


def merge_bilibili() -> None:
    Logger.section("BiliBili upstream bundle merge")
    merge_upstream_modules(
        BILIBILI_SOURCES,
        BILIBILI_OUTPUT,
        header_meta=BILIBILI_HEADER,
        optional_labels=("Helper",),
        content_replacements=[
            ('Proxies.HKG:"🇭🇰香港"', 'Proxies.HKG:"📺 哔哩哔哩 📱"'),
            ('Proxies.MAC:"🇲🇴澳门"', 'Proxies.MAC:"📺 哔哩哔哩 📱"'),
            ('Proxies.TWN:"🇹🇼台湾"', 'Proxies.TWN:"📺 哔哩哔哩 📱"'),
            ('Bottom:"home,dynamic,ogv,会员购Bottom,我的Bottom"', 'Bottom:"home,dynamic,ogv,我的Bottom"'),
            ('Home.Tab:"直播tab,推荐tab,hottopic,bangumi,anime,film,koreavtw"', 'Home.Tab:"直播tab,推荐tab,bangumi,anime"'),
            ('Host.OverseaVideo:"upos-sz-mirrorali.bilivideo.com"', 'Host.OverseaVideo:"upos-sz-mirrorakam.akamaized.net"'),
            ('Host.BStar:"upos-sz-mirrorali.bilivideo.com"', 'Host.BStar:"upos-sz-mirrorakam.akamaized.net"'),
        ]
    )


def merge_apple() -> None:
    Logger.section("Apple services upstream bundle merge")
    merge_upstream_modules(
        APPLE_SOURCES, 
        APPLE_OUTPUT, 
        header_meta=APPLE_HEADER,
        content_replacements=[
            ('Proxy:🇺🇸美国', 'Proxy:"🍎 Apple 🍏"'),
            (',🇺🇸美国', ',{{{Proxy}}}'), # In case there are hardcoded ones
            ('AirQuality.Calculate.Algorithm:"EU_EAQI"', 'AirQuality.Calculate.Algorithm:"US_AQI"'),
        ]
    )


def merge_utilities() -> None:
    Logger.section("Panel utilities upstream bundle merge")
    merge_upstream_modules(
        UTILITIES_SOURCES,
        UTILITIES_OUTPUT,
        header_meta=UTILITIES_HEADER,
        local_fallbacks={
            "Timecard": os.path.join(AMPLIFY_NEXUS_DIR, "Timecard.sgmodule"),
            "net-lsp-x": os.path.join(AMPLIFY_NEXUS_DIR, "net-lsp-x.sgmodule"),
            "Sub_Info": os.path.join(AMPLIFY_NEXUS_DIR, "Sub_Info.sgmodule"),
        },
    )


def _finalize_devtools_bundle(output_path: str) -> None:
    """Ensure merged devtools MITM is valid and Sub-Store scripts are complete."""
    devtools_mitm_exclusions = (
        "-github.com",
        "-api.github.com",
        "-*.githubusercontent.com",
    )
    text = "".join(read_file(output_path))
    meta, sections = parse_module(text)
    merged: dict[str, list[str]] = {name: list(lines) for name, lines in sections}

    script_lines = merged.get("Script", [])
    script_labels = {
        m.group(1).strip()
        for line in script_lines
        if (m := re.match(r"^(.+?)\s*=\s*type=", line.strip(), re.I))
    }
    required_sub_store = {"Sub-Store Core", "Sub-Store Simple", "{{{sync}}}", "{{{produce}}}"}
    missing = required_sub_store - script_labels
    if missing:
        Logger.warn(f"Sub-Store scripts incomplete in bundle (missing: {', '.join(sorted(missing))})")

    hosts: set[str] = set()
    for line in merged.get("MITM", []):
        if not line.strip().lower().startswith("hostname"):
            continue
        part = re.sub(r"^hostname\s*=\s*", "", line.strip(), flags=re.I)
        part = re.sub(r"^(%APPEND%|%INSERT%)\s*", "", part, flags=re.I)
        hosts.update(token.strip() for token in part.split(",") if token.strip() and token.strip() not in {"%INSERT%", "%APPEND%"})
    hosts.update(devtools_mitm_exclusions)
    inclusions = sorted(h for h in hosts if not h.startswith("-"))
    exclusions = sorted(h for h in hosts if h.startswith("-"))
    merged["MITM"] = [f"hostname = %APPEND% {', '.join(inclusions + exclusions)}"]

    section_list = [(name, merged[name]) for name in SECTION_ORDER if name in merged and merged[name]]
    for name in sorted(merged):
        if name not in SECTION_ORDER and merged[name]:
            section_list.append((name, merged[name]))
    header_lines = format_header(meta)
    write_file(output_path, format_module(header_lines, section_list, dedupe=False))


def merge_devtools() -> None:
    Logger.section("Script Hub devtools upstream bundle merge")
    _sync_sub_store_scripts()
    _sync_script_hub_scripts()
    if os.path.isfile(SCRIPT_HUB_LOCAL):
        _pin_script_hub_script_paths(SCRIPT_HUB_LOCAL)
    merge_upstream_modules(
        DEVTOOLS_SOURCES,
        DEVTOOLS_OUTPUT,
        header_meta=DEVTOOLS_HEADER,
        local_fallbacks={
            "BoxJs": os.path.join(AMPLIFY_NEXUS_DIR, "boxjs.rewrite.surge.sgmodule"),
            "Sub-Store": os.path.join(AMPLIFY_NEXUS_DIR, "Surge-Beta.sgmodule"),
            "ScriptHub": SCRIPT_HUB_LOCAL,
        },
        content_replacements=[
            ('cors:"https://sub-store.vercel.app"', 'cors:"https://sub.store"'),
        ],
    )
    _pin_sub_store_script_paths(DEVTOOLS_OUTPUT)
    _pin_script_hub_bundle_scripts(DEVTOOLS_OUTPUT)
    _finalize_devtools_bundle(DEVTOOLS_OUTPUT)


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
        args = meta["arguments"]
        args = args.replace('字幕翻译语言:off', '字幕翻译语言:"zh-Hans"')
        args = args.replace('歌词翻译语言:off', '歌词翻译语言:"zh-Hans"')
        args = args.replace('屏蔽Shorts按钮:false', '屏蔽Shorts按钮:true')
        header["arguments"] = args
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
        ("devtools", merge_devtools),
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
    _cleanup_merged_standalone_modules()
    if failures:
        Logger.warn(f"Bundle merges incomplete: {', '.join(failures)}")
    else:
        Logger.success("All upstream bundle merges completed.")
    return failures


if __name__ == "__main__":
    strict = "--strict" in sys.argv
    failed = run_all(strict=strict)
    sys.exit(1 if failed else 0)
