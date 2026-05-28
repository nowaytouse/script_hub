#!/usr/bin/env python3
"""Refresh 📺 YouTube增强合集 from Maasea upstream, stripping ad-blocking sections."""

import os
import sys
import re

SCRIPTS_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPTS_DIR)

from lib.common import get_project_root, Logger, safe_download
from lib.module_sanitizer import parse_module, format_header, format_module

ROOT = get_project_root()
OUTPUT = os.path.join(
    ROOT,
    "module/surge(main)/amplify_nexus/📺 YouTube增强合集.sgmodule",
)


def main() -> None:
    Logger.section("YouTube upstream bundle merge (Stripped of ADBlock)")
    url = "https://raw.githubusercontent.com/Maasea/sgmodule/master/YouTube.Enhance.sgmodule"
    Logger.info(f"Downloading {url}...")
    text = safe_download(url)
    if not text:
        Logger.error("Failed to download YouTube.Enhance.sgmodule")
        sys.exit(1)

    meta, sections = parse_module(text)

    # Strip Rule and Map Local sections, keep only Script and others (if any)
    cleaned_sections = []
    mitm_hosts = set()
    for name, lines in sections:
        if name in ("Rule", "Map Local"):
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
                        if h_clean and h_clean != "*.googlevideo.com":
                            mitm_hosts.add(h_clean)

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
        "desc": "合并 YouTube 增强 (Maasea 上游)\n🎬 Enhance: 画中画/后台播放/字幕翻译",
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
        "# Upstream module processed by scripts/maintenance/merge_youtube_bundle.py",
        f"# - Source: {url}",
        "# - Stripped: [Rule], [Map Local] and *.googlevideo.com from MITM"
    ]

    header_lines = format_header(HEADER, extra_lines=extra)
    os.makedirs(os.path.dirname(OUTPUT) or ".", exist_ok=True)
    with open(OUTPUT, "w", encoding="utf-8") as f:
        f.write(format_module(header_lines, final_sections, dedupe=True))

    Logger.success(f"Wrote stripped bundle: {OUTPUT}")


if __name__ == "__main__":
    main()
