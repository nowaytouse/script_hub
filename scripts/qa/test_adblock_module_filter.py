#!/usr/bin/env python3
"""Regression: PROMAX line-split vs full vs skip."""
import os
import sys

SCRIPT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, SCRIPT_DIR)

from pipeline.adblock_manager import (  # noqa: E402
    AMPLIFY_NEXUS_DIR,
    AdBlockManager,
    LOCAL_MODULES_DIR,
)
from hub.promax_line_classifier import (
    classify_promax_line,
    module_ingest_mode,
    should_keep_promax_line,
)
from hub.promax_module_split import split_module_sections


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
    ad_sections, stats = split_module_sections(text, source_path=path)
    assert stats["kept_lines"] > 0
    assert stats["kept_lines"] < stats["total_lines"]
    scripts = ad_sections.get("Script", [])
    assert any("ADBlock" in s or "helper" in s.lower() for s in scripts)
    assert not any("BiliBili.Enhanced" in s for s in scripts)
    assert not any("BiliBili.Global" in s for s in scripts)
    rules = ad_sections.get("Rule", [])
    assert any("REJECT" in r for r in rules)


def test_pdd_module_full_ingest_keeps_all_body_rewrite() -> None:
    path = os.path.join(LOCAL_MODULES_DIR, "拼多多去广告.sgmodule")
    if not os.path.isfile(path):
        return
    text = open(path, encoding="utf-8").read()
    assert module_ingest_mode(path, text) == "full"
    jq_count = text.count("http-response-jq")
    _, stats = split_module_sections(text, source_path=path)
    assert stats["skipped_lines"] == 0
    ad_sections, _ = split_module_sections(text, source_path=path)
    assert len(ad_sections.get("Body Rewrite", [])) == jq_count


def test_weibo_ad_scripts_kept_in_split() -> None:
    path = os.path.join(LOCAL_MODULES_DIR, "🐦 微博去广告合集.sgmodule")
    if not os.path.isfile(path):
        return
    line = (
        "微博热搜页面广告 = type=http-response, pattern=^https?:\\/\\/m?api\\.weibo\\.c(n|om)\\/2\\/"
        "(page|flowpage)\\?, script-path=https://raw.githubusercontent.com/fmz200/wool_scripts/main/"
        "Scripts/weibo/weibo_ads.js, requires-body=true"
    )
    assert classify_promax_line(line, "Script", source_path=path) == "ad"


def test_zhihu_legacy_body_rewrite_kept() -> None:
    line = (
        r'http-response ^https:\/\/api\.zhihu\.com\/search\/recommend_query\/v2\? '
        r'"recommend_queries":\{.+\} "recommend_queries":{}'
    )
    assert classify_promax_line(line, "Body Rewrite") == "ad"


def test_rednote_scripts_full_module() -> None:
    path = os.path.join(LOCAL_MODULES_DIR, "RedNote.sgmodule")
    if not os.path.isfile(path):
        return
    text = open(path, encoding="utf-8").read()
    assert module_ingest_mode(path, text) == "full"
    _, stats = split_module_sections(text, source_path=path)
    assert stats["skipped_lines"] == 0


def test_unlock_weibo_script_still_dropped() -> None:
    path = os.path.join(LOCAL_MODULES_DIR, "🐦 微博去广告合集.sgmodule")
    line = (
        "解锁微博会员APP图标 = type=http-response, pattern=^https?:\\/\\/new\\.vip\\.weibo\\.c(n|om)\\/"
        "aj\\/appicon\\/list, script-path=https://example/unlock.js"
    )
    assert not should_keep_promax_line(line, "Script", source_path=path)


def test_pdd_bottom_tabs_body_rewrite_kept() -> None:
    line = (
        "http-response-jq ^https:\\/\\/api\\.pinduoduo\\.com\\/api\\/alexa\\/homepage\\/hub\\? "
        "'.result.bottom_tabs? |= map(select(.link | IN(\"index.html\", \"chat_list.html\", \"personal.html\")))'"
    )
    assert classify_promax_line(line, "Body Rewrite") == "ad"


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


def test_localmodules_only_functional_scan() -> None:
    mgr = AdBlockManager()
    paths, _ = mgr.load_functional_source_paths()
    joined = "\n".join(p for p, _m in paths)
    assert "narrow_pierce" not in joined
    assert "LocalModules" in joined or any("LocalModules" in p for p, _ in paths)


def test_amplify_nexus_not_auto_scanned() -> None:
    mgr = AdBlockManager()
    paths, _ = mgr.load_functional_source_paths()
    joined = "\n".join(p for p, _m in paths)
    assert "iRingo" not in joined
    assert "YouTube.Enhance" not in joined
    assert "Apple服务增强" not in joined


def test_sukka_adblock_allowlisted_full() -> None:
    from pipeline.adblock_manager import LOCAL_SOURCES_DIR

    path = os.path.join(
        LOCAL_SOURCES_DIR, "[Sukka] Enhance Better ADBlock for Surge.sgmodule"
    )
    if not os.path.isfile(path):
        return
    mgr = AdBlockManager()
    assert mgr.resolve_module_ingest_mode(path) == "full"


if __name__ == "__main__":
    test_pdd_module_full_ingest_keeps_all_body_rewrite()
    test_weibo_ad_scripts_kept_in_split()
    test_zhihu_legacy_body_rewrite_kept()
    test_rednote_scripts_full_module()
    test_unlock_weibo_script_still_dropped()
    test_pdd_bottom_tabs_body_rewrite_kept()
    test_classifier_adblock_script()
    test_filename_gate()
    test_bilibili_bundle_split_keeps_ad_only()
    test_functional_resolve_includes_split_bundle()
    test_localmodules_only_functional_scan()
    test_amplify_nexus_not_auto_scanned()
    test_sukka_adblock_allowlisted_full()
    print("test_adblock_module_filter: OK")
