#!/usr/bin/env python3
"""Refresh 📺 YouTube增强合集 from Maasea upstream."""

import os
import sys

SCRIPTS_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPTS_DIR)

from lib.common import get_project_root, Logger
from lib.merge_upstream_bundle import merge_upstream_modules

ROOT = get_project_root()
OUTPUT = os.path.join(
    ROOT,
    "module/surge(main)/amplify_nexus/📺 YouTube增强合集.sgmodule",
)

SOURCES = [
    ("Enhance", "https://raw.githubusercontent.com/Maasea/sgmodule/master/YouTube.Enhance.sgmodule"),
    ("ADBlock", "https://raw.githubusercontent.com/Maasea/sgmodule/master/YouTube.ADBlock.sgmodule"),
]

HEADER = {
    "name": "📺 YouTube增强合集",
    "desc": "合并 YouTube 增强 + 去广告 (Maasea 上游)\\n🎬 Enhance: 画中画/后台播放/字幕翻译\\n🛡️ ADBlock: 去广告",
    "author": "Maasea",
    "icon": "https://raw.githubusercontent.com/Koolson/Qure/master/IconSet/Color/YouTube.png",
    "category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "tag": "YouTube, 增强, 去广告",
}


def main() -> None:
    Logger.section("YouTube upstream bundle merge")
    merge_upstream_modules(SOURCES, OUTPUT, header_meta=HEADER)
    Logger.info("Next: python3 scripts/consolidate_modules.py && python3 scripts/convert_surge_to_shadowrocket.py")


if __name__ == "__main__":
    main()
