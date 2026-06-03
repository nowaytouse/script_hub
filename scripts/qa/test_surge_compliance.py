#!/usr/bin/env python3
"""Regression tests for Surge RULE-SET compliance (incidents from May 2026)."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from hub.rule_processor import RuleProcessor  # noqa: E402
from hub.surge_compliance import (  # noqa: E402
    convert_domain_regex_for_surge,
    strip_inline_comment,
    validate_surge_ruleset_line,
)
from pipeline.ruleset_manager import RulesetManager  # noqa: E402
from pipeline.adblock_manager import AdBlockManager  # noqa: E402


def assert_eq(label: str, got, want) -> None:
    if got != want:
        raise AssertionError(f"{label}: got {got!r}, want {want!r}")


def assert_none(label: str, got) -> None:
    if got is not None:
        raise AssertionError(f"{label}: expected no error, got {got!r}")


def assert_err(label: str, got, substr: str) -> None:
    if got is None or substr not in got:
        raise AssertionError(f"{label}: expected error containing {substr!r}, got {got!r}")


def test_url_regex_not_truncated() -> None:
    raw = r"URL-REGEX,https://www\.google\.com/.*continue=https://gemini\.google\.com.+"
    assert_eq("strip comment", strip_inline_comment(raw), raw)
    p = RuleProcessor()
    out = p.normalize_rule(raw)
    assert out is not None, "URL-REGEX must not be dropped"
    payload = out.split(",", 1)[1]
    assert "gemini" in payload, f"truncated payload: {payload!r}"
    assert payload != "https:", f"truncated to scheme only: {payload!r}"


def test_invalid_url_regex_rejected() -> None:
    p = RuleProcessor()
    assert p.normalize_rule("URL-REGEX,https:") is None
    assert p.normalize_rule("URL-REGEX,^https?://") is None


def test_process_name_preserved() -> None:
    p = RuleProcessor()
    assert_eq("PROCESS-NAME", p.normalize_rule("PROCESS-NAME,Music"), "PROCESS-NAME,Music")
    assert_eq("USER-AGENT", p.normalize_rule("USER-AGENT,*Music?"), "USER-AGENT,*Music?")


def test_ip_cidr_no_resolve() -> None:
    p = RuleProcessor()
    assert_eq(
        "IP-CIDR no-resolve",
        p.normalize_rule("IP-CIDR,23.41.4.0/22,no-resolve"),
        "IP-CIDR,23.41.4.0/22",
    )


def test_domain_regex_junk_dropped() -> None:
    p = RuleProcessor()
    assert p.normalize_rule("DOMAIN-REGEX,$") is None
    assert p.normalize_rule("DOMAIN-REGEX,c") is None


def test_netflix_domain_regex_converted() -> None:
    p = RuleProcessor()
    raw = r"DOMAIN-REGEX,(^|\.)apiproxy-device-prod-nlb-.+\.amazonaws\.com$"
    normalized = p.normalize_rule(raw)
    assert normalized is not None
    converted = RulesetManager._surge_convert_domain_regex(normalized)
    assert_eq("netflix convert", converted, "DOMAIN-KEYWORD,apiproxy-device-prod-nlb")


def test_surge_list_forbids_domain_regex() -> None:
    line = r'DOMAIN-REGEX,"(^|\.)foo$"'
    assert_err("forbidden", validate_surge_ruleset_line(line), "not supported")


def test_adblock_skips_script_only_module() -> None:
  m = AdBlockManager()
  text = (
      "#!name=Test\n"
      "[URL Rewrite]\n"
      "^https://example.com/ad _ reject\n"
      "[Script]\n"
      "x=type=http-response,pattern=^https://api.example.com,script-path=https://example.com/a.js\n"
  )
  m.extract_from_text(text, rules_only=True)
  total = sum(len(bucket.get("Other", set())) for bucket in m.rules.values())
  assert_eq("script-only module rules", total, 0)


def test_adblock_rule_section_only() -> None:
  m = AdBlockManager()
  text = (
      "[Rule]\n"
      "DOMAIN,ad.example.com\n"
      "x=type=http-response,pattern=^https://api.example.com,script-path=https://example.com/a.js\n"
  )
  m.extract_from_text(text, rules_only=True)
  other = m.rules.get("REJECT", {}).get("Other", set())
  assert_eq("domain kept", "DOMAIN,ad.example.com" in other, True)
  assert_eq("script line dropped", any("script-path" in r for r in other), False)


def test_surge_list_allows_keyword() -> None:
    assert_none("keyword ok", validate_surge_ruleset_line("DOMAIN-KEYWORD,apiproxy-device-prod-nlb"))


def main() -> int:
    tests = [
        test_url_regex_not_truncated,
        test_invalid_url_regex_rejected,
        test_process_name_preserved,
        test_ip_cidr_no_resolve,
        test_domain_regex_junk_dropped,
        test_netflix_domain_regex_converted,
        test_surge_list_forbids_domain_regex,
        test_surge_list_allows_keyword,
        test_adblock_skips_script_only_module,
        test_adblock_rule_section_only,
    ]
    failed = 0
    for fn in tests:
        try:
            fn()
            print(f"  OK  {fn.__name__}")
        except Exception as exc:
            failed += 1
            print(f"  FAIL {fn.__name__}: {exc}", file=sys.stderr)
    if failed:
        print(f"\n{failed}/{len(tests)} failed", file=sys.stderr)
        return 1
    print(f"\nAll {len(tests)} compliance tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
