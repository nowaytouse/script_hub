#!/usr/bin/env python3
"""Refresh 微博去广告 into LocalModules (PROMAX build source; not a separate install target)."""

import os
import sys

SCRIPTS_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPTS_DIR)

from core.common import get_project_root, Logger, read_file, write_file
from core.merge_upstream_bundle import merge_upstream_modules
from core.module_sanitizer import format_header, format_module, merge_mitm_hosts, parse_module

ROOT = get_project_root()
LOCAL_WEIBO = os.path.join(
    ROOT,
    "rulesets/Sources/LocalModules/🐦 微博去广告合集.sgmodule",
)

SOURCES = [
    ("Main", "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/weibo.module"),
    ("Intl", "https://raw.githubusercontent.com/iab0x00/ProxyRules/main/Rewrite/WeiboIntl.sgmodule"),
]

HEADER = {
    "name": "🐦 微博去广告",
    "desc": (
        "微博+国际版去广告 · PROMAX 构建源（域名规则+脚本已并入 PROMAX，勿单独安装）"
        "\\n\\n上游: fmz200/wool_scripts, iab0x00"
    ),
    "author": "fmz200, iab0x00, ScriptHub",
    "tag": "去广告, 微博, PROMAX-build",
}

SECTION_ORDER = (
    "Rule",
    "URL Rewrite",
    "Body Rewrite",
    "Map Local",
    "Script",
    "MITM",
)


def _load_preserved_sections(path: str) -> dict[str, list[str]]:
    if not os.path.isfile(path):
        return {}
    _, sections = parse_module("".join(read_file(path)))
    return {name: list(lines) for name, lines in sections if name == "Rule" and lines}


def _rewrite_with_preserved(path: str, preserved: dict[str, list[str]]) -> None:
    if not preserved:
        return
    meta, sections = parse_module("".join(read_file(path)))
    merged: dict[str, list[str]] = {name: list(lines) for name, lines in sections}
    for name, lines in preserved.items():
        merged[name] = lines
    ordered = [(name, merged[name]) for name in SECTION_ORDER if name in merged and merged[name]]
    for name in sorted(merged):
        if name not in SECTION_ORDER and merged[name]:
            ordered.append((name, merged[name]))
    ordered = merge_mitm_hosts(ordered)
    header_lines = format_header(
        meta,
        extra_lines=["# PROMAX build source — install head_expanse/PROMAX only"],
    )
    write_file(path, format_module(header_lines, ordered, dedupe=True))


def main() -> None:
    Logger.section("Weibo → LocalModules (PROMAX build source)")
    preserved = _load_preserved_sections(LOCAL_WEIBO)
    merge_upstream_modules(
        SOURCES,
        LOCAL_WEIBO,
        header_meta=HEADER,
        provenance_comment="# Merged for rulesets/Sources/LocalModules → PROMAX ingest",
    )
    _rewrite_with_preserved(LOCAL_WEIBO, preserved)
    Logger.info(f"Build source: {os.path.relpath(LOCAL_WEIBO, ROOT)}")


if __name__ == "__main__":
    main()
