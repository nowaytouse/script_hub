#!/usr/bin/env python3
"""Refresh 🐦 微博去广告合集 from wool_scripts + iab0x00 upstream."""

import os
import sys

SCRIPTS_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPTS_DIR)

from lib.common import get_project_root, Logger
from lib.merge_upstream_bundle import merge_upstream_modules

ROOT = get_project_root()
OUTPUT = os.path.join(
    ROOT,
    "module/surge(main)/narrow_pierce/🐦 微博去广告合集.sgmodule",
)

SOURCES = [
    ("Main", "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/weibo.module"),
    ("Intl", "https://raw.githubusercontent.com/iab0x00/Surge/main/Module/WeiboIntl.sgmodule"),
]

HEADER = {
    "name": "🐦 微博去广告合集",
    "desc": "合并 fmz200 微博净化 + iab0x00 国际版去广告",
    "author": "fmz200, iab0x00",
    "category": "『 🎯 Narrow Pierce › 窄域穿刺 』",
    "tag": "Weibo, 去广告",
}


def main() -> None:
    Logger.section("Weibo upstream bundle merge")
    merge_upstream_modules(SOURCES, OUTPUT, header_meta=HEADER)
    Logger.info("Next: python3 scripts/consolidate_modules.py && python3 scripts/convert_surge_to_shadowrocket.py")


if __name__ == "__main__":
    main()
