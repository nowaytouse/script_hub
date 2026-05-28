#!/usr/bin/env python3
"""Refresh 📺 BiliBili增强合集 from BiliUniverse + Maasea upstream."""

import os
import sys

SCRIPTS_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPTS_DIR)

from lib.common import get_project_root, Logger
from lib.merge_upstream_bundle import merge_upstream_modules

ROOT = get_project_root()
OUTPUT = os.path.join(
    ROOT,
    "module/surge(main)/amplify_nexus/📺 BiliBili增强合集.sgmodule",
)

SOURCES = [
    ("Enhanced", "https://github.com/BiliUniverse/Enhanced/releases/latest/download/BiliBili.Enhanced.sgmodule"),
    ("Global", "https://github.com/BiliUniverse/Global/releases/latest/download/BiliBili.Global.sgmodule"),
    ("Redirect", "https://github.com/BiliUniverse/Redirect/releases/latest/download/BiliBili.Redirect.sgmodule"),
    ("Helper", "https://raw.githubusercontent.com/Maasea/sgmodule/master/Bilibili.Helper.sgmodule"),
]

HEADER = {
    "name": "📺 BiliBili增强合集",
    "desc": "合并 BiliUniverse + Maasea 上游（Enhanced/Global/Redirect/Helper）",
    "author": "BiliUniverse, Maasea",
    "icon": "https://www.bilibili.com/favicon.ico",
    "category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "tag": "BiliBili, 增强",
}


def main() -> None:
    Logger.section("BiliBili upstream bundle merge")
    merge_upstream_modules(SOURCES, OUTPUT, header_meta=HEADER)
    Logger.info("Next: python3 scripts/consolidate_modules.py && python3 scripts/convert_surge_to_shadowrocket.py")


if __name__ == "__main__":
    main()
