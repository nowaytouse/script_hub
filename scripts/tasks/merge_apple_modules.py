#!/usr/bin/env python3
"""Refresh 🍎 Apple服务增强合集 from iRingo upstream (Maps + WeatherKit)."""

import os
import sys

SCRIPTS_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPTS_DIR)

from core.common import get_project_root, Logger
from core.merge_upstream_bundle import merge_upstream_modules

ROOT = get_project_root()
OUTPUT = os.path.join(
    ROOT,
    "module/surge(main)/amplify_nexus/🍎 Apple服务增强合集.sgmodule",
)

SOURCES = [
    ("iRingo.Maps", "https://github.com/NSRingo/GeoServices/releases/latest/download/iRingo.Maps.sgmodule"),
    ("iRingo.WeatherKit", "https://github.com/NSRingo/WeatherKit/releases/latest/download/iRingo.WeatherKit.sgmodule"),
]

HEADER = {
    "name": "🍎 Apple服务增强合集",
    "desc": "整合 iRingo 系列模块\\n包含: Maps(地图增强) + WeatherKit(天气增强)\\n解锁Apple服务的国际版功能",
    "author": "VirgilClyne[https://github.com/VirgilClyne]",
    "homepage": "https://NSRingo.github.io",
    "icon": "https://developer.apple.com/assets/elements/icons/sf-symbols/sf-symbols-128x128.png",
    "category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
}


def main() -> None:
    Logger.section("Apple services upstream bundle merge")
    merge_upstream_modules(SOURCES, OUTPUT, header_meta=HEADER)
    Logger.info("Next: python3 scripts/consolidate_modules.py && python3 scripts/convert_surge_to_shadowrocket.py")


if __name__ == "__main__":
    main()
