#!/usr/bin/env python3
"""Refresh 📺 YouTube增强合集 from Maasea upstream, stripping ad-blocking sections."""

import os
import sys
import re

SCRIPTS_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPTS_DIR)

from ..core.common import get_project_root, Logger, safe_download
from ..core.module_sanitizer import parse_module, format_header, format_module

ROOT = get_project_root()
OUTPUT = os.path.join(
    ROOT,
    "modules/surge/amplify_nexus/📺 YouTube增强合集.sgmodule",
)


def main() -> None:
    Logger.section("YouTube upstream bundle merge (Stripped of ADBlock)")
    url = "https://raw.githubusercontent.com/Maasea/sgmodules/master/YouTube.Enhance.sgmodule"
    Logger.info(f"Downloading {url}...")
    text = safe_download(url)
    if not text:
        Logger.error("Failed to download YouTube.Enhance.sgmodule")
        sys.exit(1)

    meta, sections = parse_module(text)

    # 1. Strip sections for the clean/stripped YouTube.Enhance bundle
    # and extract adblocking sections for YouTube.ADBlock.sgmodule
    cleaned_sections = []
    mitm_hosts = set()
    adblock_rules = []
    adblock_map_locals = []
    adblock_mitm_hosts = set()

    for name, lines in sections:
        if name == "Rule":
            adblock_rules.extend(lines)
            Logger.info(f"  - Stripping section: {name}")
            continue
        elif name == "Map Local":
            adblock_map_locals.extend(lines)
            Logger.info(f"  - Stripping section: {name}")
            continue

        cleaned_sections.append((name, lines))
        if name == "MITM":
            for line in lines:
                if line.strip().startswith("hostname"):
                    # Extract hosts
                    hosts = re.sub(r"^hostname\s*=\s*(%APPEND%\s*)?", "", line.strip())
                    for h in hosts.split(","):
                        h_clean = h.strip()
                        if h_clean:
                            if h_clean == "*.googlevideo.com":
                                adblock_mitm_hosts.add(h_clean)
                            else:
                                mitm_hosts.add(h_clean)
                                if "youtubei" in h_clean:
                                    adblock_mitm_hosts.add(h_clean)

    # Rebuild MITM hostname line if we have it
    final_sections = []
    for name, lines in cleaned_sections:
        if name == "MITM":
            if mitm_hosts:
                # Keep only active hosts (exclude *.googlevideo.com which was for Map Local)
                final_sections.append(("MITM", [f"hostname = %APPEND% {', '.join(sorted(mitm_hosts))}"]))
        else:
            final_sections.append((name, lines))

    HEADER = {
        "name": "📺 YouTube增强合集",
        # Keep desc as single line to avoid breaking module metadata parsing on clients.
        "desc": "合并 YouTube 增强 (Maasea 上游) | Enhance: 画中画/后台播放/字幕翻译",
        "author": "Maasea",
        "icon": "https://raw.githubusercontent.com/Koolson/Qure/master/IconSet/Color/YouTube.png",
        "category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
        "tag": "YouTube, 增强",
    }

    # Extract arguments and arguments-desc from metadata
    if "arguments" in meta:
        HEADER["arguments"] = meta["arguments"]
    if "arguments-desc" in meta:
        HEADER["arguments-desc"] = meta["arguments-desc"]

    extra = [
        "# Upstream module processed by scripts/tasks/merge_youtube_bundle.py",
        f"# - Source: {url}",
        "# - Stripped: [Rule], [Map Local] and *.googlevideo.com from MITM"
    ]

    header_lines = format_header(HEADER, extra_lines=extra)
    os.makedirs(os.path.dirname(OUTPUT) or ".", exist_ok=True)
    with open(OUTPUT, "w", encoding="utf-8") as f:
        content = format_module(header_lines, final_sections, dedupe=True)
        if not content or len(content.strip()) < 100:
            Logger.error(f"Generated content too small or empty. Aborting write to {OUTPUT}")
            Logger.error(f"Content length: {len(content) if content else 0}")
            sys.exit(1)
        f.write(content)

    Logger.success(f"Wrote stripped bundle: {OUTPUT}")

    # 2. Rebuild and write YouTube.ADBlock.sgmodule
    ADBLOCK_OUTPUT = os.path.join(
        ROOT,
        "modules/source/local_sources/YouTube.ADBlock.sgmodule",
    )

    adblock_header = [
        "#!name=YouTube 去广告",
        "#!desc=YouTube 广告过滤 (自动从 Maasea 上游提取并合并进入 PROMAX)",
        "#!category=🪐 local_sources",
        ""
    ]

    adblock_sections_lines = []
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
        adblock_sections_lines.append(f"hostname = %APPEND% {', '.join(sorted(adblock_mitm_hosts))}, -github.com, -api.github.com, -*.githubusercontent.com")
        adblock_sections_lines.append("")

    adblock_content = "\n".join(adblock_header) + "\n".join(adblock_sections_lines)
    os.makedirs(os.path.dirname(ADBLOCK_OUTPUT) or ".", exist_ok=True)
    with open(ADBLOCK_OUTPUT, "w", encoding="utf-8") as f:
        f.write(adblock_content.strip() + "\n")

    Logger.success(f"Wrote extracted adblock module: {ADBLOCK_OUTPUT}")


if __name__ == "__main__":
    main()
