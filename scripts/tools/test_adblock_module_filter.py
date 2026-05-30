#!/usr/bin/env python3
"""Regression: PROMAX line-split vs full vs skip."""
import os
import sys

SCRIPT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPT_DIR)

from adblock_manager import (  # noqa: E402
    AMPLIFY_NEXUS_DIR,
    AdBlockManager,
    LOCAL_MODULES_DIR,
)
from lib.promax_line_classifier import classify_promax_line, module_ingest_mode
from lib.promax_module_split import split_module_sections


def test_filename_gate() -> None:
    mgr = AdBlockManager()
    cases = [
        (os.path.join(LOCAL_MODULES_DIR, "知乎去广告.sgmodule"), "split"),
        (os.path.join(LOCAL_MODULES_DIR, "扫描全能王解锁.sgmodule"), "skip"),
        (os.path.join(AMPLIFY_NEXUS_DIR, "BiliBili.Enhanced.sgmodule"), "skip"),
        (os.path.join(AMPLIFY_NEXUS_DIR, "扫描全能王解锁.sgmodule"), "skip"),
        (os.path.join(AMPLIFY_NEXUS_DIR, "WeChat_Enhance.sgmodule"), "skip"),
    ]
    for path, expected in cases:
        if not os.path.isfile(path):
            continue
        text = open(path, encoding="utf-8").read()
        assert module_ingest_mode(path, text) == expected, path


def test_bilibili_bundle_split_keeps_ad_only() -> None:
    path = os.path.join(
        AMPLIFY_NEXUS_DIR, "📺 BiliBili增强合集.sgmodule"
    )
    if not os.path.isfile(path):
        return
    text = open(path, encoding="utf-8").read()
    ad_sections, stats = split_module_sections(text)
    assert stats["kept_lines"] > 0
    assert stats["kept_lines"] < stats["total_lines"]
    scripts = ad_sections.get("Script", [])
    assert any("ADBlock" in s or "helper" in s.lower() for s in scripts)
    assert not any("BiliBili.Enhanced" in s for s in scripts)
    assert not any("BiliBili.Global" in s for s in scripts)
    rules = ad_sections.get("Rule", [])
    assert any("REJECT" in r for r in rules)


def test_classifier_adblock_script() -> None:
    line = (
        "📺 BiliBili.ADBlock.feed.index.request.json = type=http-request,"
        "script-path=https://github.com/BiliUniverse/ADBlock/releases/download/v0.6.24/request.bundle.js"
    )
    assert classify_promax_line(line, "Script") == "ad"
    enh = (
        "📺 BiliBili.Enhanced.x.resource.show.tab.v2 = type=http-response,"
        "script-path=https://github.com/BiliUniverse/Enhanced/releases/download/v0.5.13/response.bundle.js"
    )
    assert classify_promax_line(enh, "Script") == "enhance"


def test_functional_resolve_includes_split_bundle() -> None:
    mgr = AdBlockManager()
    paths, _ = mgr.load_functional_source_paths()
    by_path = {p: m for p, m in paths}
    bundle = os.path.join(AMPLIFY_NEXUS_DIR, "📺 BiliBili增强合集.sgmodule")
    if os.path.isfile(bundle):
        assert by_path.get(os.path.normpath(bundle)) == "split"
    assert not any("BiliBili.Enhanced.sgmodule" in p for p in by_path)
    assert not any("WeChat_Enhance" in p for p in by_path)


if __name__ == "__main__":
    test_classifier_adblock_script()
    test_filename_gate()
    test_bilibili_bundle_split_keeps_ad_only()
    test_functional_resolve_includes_split_bundle()
    print("test_adblock_module_filter: OK")
