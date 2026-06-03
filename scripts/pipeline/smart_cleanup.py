#!/usr/bin/env python3
import os
import re
import sys
from typing import Dict, Iterable

from hub.common import get_project_root, write_file as _atomic_write, safe_remove

_ROOT = get_project_root()
RULESET_DIRS = [
    os.path.join(_ROOT, "rulesets/surge-shadowrocket"),
    os.path.join(_ROOT, "rulesets/AdBlock"),
]

# Specific rulesets should win over generic fallbacks. Direct stays above
# GlobalProxy, but below service-specific proxy rules, so accidental overlaps
# do not hijack routing before a more precise ruleset can match.
PRIORITY_ORDER = [
    "AdBlock_",
    "AdBlock.list",
    "reject-drop.list",
    "reject-no-drop.list",
    "HTTPDNS_Hijack.list",
    "AI.list",
    "AI_ip.list",
    "SocialMedia.list",
    "SocialMedia_ip.list",
    "NSFW.list",
    "NSFW_ip.list",
    "substore.list",
    "substore_ip.list",
    "AppleNews.list",
    "AppleNews_ip.list",
    "StreamUS.list",
    "StreamUS_ip.list",
    "StreamJP.list",
    "StreamJP_ip.list",
    "StreamKR.list",
    "StreamKR_ip.list",
    "StreamEU.list",
    "StreamEU_ip.list",
    "StreamHK.list",
    "StreamHK_ip.list",
    "StreamTW.list",
    "StreamTW_ip.list",
    "Spotify.list",
    "Spotify_ip.list",
    "Gaming.list",
    "Gaming_ip.list",
    "Google.list",
    "Google_ip.list",
    "Apple.list",
    "Apple_ip.list",
    "Microsoft.list",
    "Microsoft_ip.list",
    "GitHub.list",
    "GitHub_ip.list",
    "PayPal.list",
    "PayPal_ip.list",
    "Binance.list",
    "Binance_ip.list",
    "Cloudflare.list",
    "Cloudflare_ip.list",
    "Bilibili.list",
    "Bilibili_ip.list",
    "CDN.list",
    "CDN_ip.list",
    "Direct.list",
    "Direct_ip.list",
    "ChinaASN.list",
    "GlobalASN.list",
    "GlobalProxy.list",
    "GlobalProxy_ip.list",
]

MANDATORY_PROXY_KEYWORDS = [
    # Western services — never direct
    "google", "gstatic", "gmail", "ggpht", "youtube", "ytimg",
    "facebook", "fbcdn", "instagram", "twitter", "twimg", "t.co",
    "telegram", "netflix", "nflxvideo", "nflxext", "disney", "github",
    "akamai", "fastly", "cloudflare",
    # ByteDance INTERNATIONAL infrastructure (must not be Direct)
    "byteoversea", "tiktok", "tiktokv", "tiktokcdn", "tiktokeu",
    "tiktokw", "muscdn", "tiktokrow", "lark.com",
]

# Domains that are ad/trackers and should live ONLY in AdBlock_*,
# never in routing/diversion rulesets.
ADBLOCK_ONLY_PREFIXES = (
    "AdBlock_", "AdBlock.list",
)

# Keywords that identify adult/NSFW content — must NOT appear in SocialMedia.list
NSFW_CONTAMINATION_KEYWORDS = [
    "porn", "hentai", "nsfw", "xxx", "adult", "sex", "erotic",
    "nude", "naked", "fetish", "bdsm", "onlyfans",
    # Typosquatting / phishing variants that leak from upstream Global rules
    "facebookporn", "facebookofsex", "facboox", "faceboox", "faseboox", "feceboox",
]

# Keywords that should never appear in NSFW.list (belong in Social/Proxy)
# (currently not enforced at cleanup level but logged in audit)

TYPE_RANK = {
    "DOMAIN": 0,
    "DOMAIN-SUFFIX": 1,
    "DOMAIN-KEYWORD": 2,
    "DOMAIN-WILDCARD": 3,
    "DOMAIN-REGEX": 4,
    "IP-CIDR": 5,
    "IP-CIDR6": 5,
    "GEOIP": 6,
    "PROCESS-NAME": 7,
    "USER-AGENT": 8,
    "URL-REGEX": 9,
}

SEMANTIC_COVERAGE_RULESETS = {
    "AdBlock_",
    "AdBlock.list",
    "AI.list",
    "SocialMedia.list",
    "NSFW.list",
    "StreamUS.list",
    "StreamJP.list",
    "StreamKR.list",
    "StreamEU.list",
    "StreamHK.list",
    "StreamTW.list",
    "Spotify.list",
    "Gaming.list",
    "Google.list",
    "Apple.list",
    "AppleNews.list",
    "Microsoft.list",
    "GitHub.list",
    "PayPal.list",
    "Binance.list",
    "Cloudflare.list",
    "Bilibili.list",
}

SEMANTIC_TYPES = {"DOMAIN", "DOMAIN-SUFFIX"}


def is_valid_rule(line: str) -> bool:
    if line.startswith("RULE-SET"):
        return False
    if line.startswith("DOMAIN") and ",no-resolve" in line:
        return False
    return "," in line


def clean_rule(line: str) -> str:
    from hub.surge_compliance import strip_inline_comment

    line = strip_inline_comment(line.strip())
    if not line or line.startswith(("#", "//")):
        return ""
    if line.startswith("DOMAIN") and ",no-resolve" in line:
        line = line.replace(",no-resolve", "")
    return line


def extract_payload(rule: str) -> str:
    if "," not in rule:
        return ""
    return rule.split(",", 1)[1].split(",", 1)[0].strip().lower()


def extract_type(rule: str) -> str:
    return rule.split(",", 1)[0].strip().upper()


def prefer_rule(current: str, candidate: str) -> str:
    current_rank = TYPE_RANK.get(extract_type(current), 999)
    candidate_rank = TYPE_RANK.get(extract_type(candidate), 999)
    if candidate_rank < current_rank:
        return candidate
    if candidate_rank == current_rank and candidate < current:
        return candidate
    return current


def is_proxyish_domain(rule: str) -> bool:
    payload = extract_payload(rule)
    if not payload or extract_type(rule) not in {
        "DOMAIN", "DOMAIN-SUFFIX", "DOMAIN-KEYWORD", "DOMAIN-WILDCARD", "DOMAIN-REGEX"
    }:
        return False
    return any(keyword in payload for keyword in MANDATORY_PROXY_KEYWORDS)


def is_cn_tld(rule: str) -> bool:
    payload = extract_payload(rule)
    if not payload or extract_type(rule) not in {"DOMAIN", "DOMAIN-SUFFIX"}:
        return False
    return payload == "cn" or payload.endswith(".cn")


def iter_domain_parents(payload: str):
    parts = payload.split(".")
    if len(parts) < 2:
        yield payload
        return
    for index in range(len(parts) - 1):
        yield ".".join(parts[index:])


def rule_sort_key(rule: str):
    rule_type = extract_type(rule)
    payload = extract_payload(rule)
    if rule_type in SEMANTIC_TYPES:
        return (0, payload.count("."), len(payload), TYPE_RANK.get(rule_type, 999), payload, rule)
    return (1, TYPE_RANK.get(rule_type, 999), payload, rule)


def load_list(filepath: str) -> Dict[str, str]:
    rules: Dict[str, str] = {}
    if not os.path.exists(filepath):
        return rules
    with open(filepath, "r", encoding="utf-8", errors="ignore") as f:
        for raw_line in f:
            line = clean_rule(raw_line)
            if not line or not is_valid_rule(line):
                continue
            payload = extract_payload(line)
            if not payload:
                continue
            if payload in rules:
                rules[payload] = prefer_rule(rules[payload], line)
            else:
                rules[payload] = line
    return rules


def write_list(filepath: str, rules: Dict[str, str]):
    sorted_rules = sorted(rules.values())
    existing_header = []
    if os.path.exists(filepath):
        with open(filepath, "r", encoding="utf-8", errors="ignore") as f:
            for line in f:
                if line.startswith("#") or line.strip() == "":
                    existing_header.append(line)
                else:
                    break

    out_lines = []
    if existing_header:
        for line in existing_header:
            if re.match(r"^#\s*(Rules|Total|Total Rules):\s*", line):
                key = line.split(":", 1)[0]
                out_lines.append(f"{key}: {len(sorted_rules)}\n")
            else:
                out_lines.append(line)
        if existing_header and existing_header[-1].strip():
            out_lines.append("\n")
    else:
        filename = os.path.basename(filepath)
        out_lines.append(f"# Ruleset: {filename}\n")
        out_lines.append("# Cleaned by smart_cleanup.py\n\n")
    for rule in sorted_rules:
        out_lines.append(rule + "\n")

    _atomic_write(filepath, "".join(out_lines))


class RulesetCleanup:
    def __init__(self, ruleset_dirs: list = RULESET_DIRS):
        self.ruleset_dirs = ruleset_dirs
        self.file_content: Dict[str, Dict[str, str]] = {}
        self.file_paths: Dict[str, str] = {} # filename -> full path
        self.stats = {
            "files": 0,
            "within_file_dedup": 0,
            "moved_direct_to_proxy": 0,
            "moved_proxy_to_direct": 0,
            "nsfw_purged_from_social": 0,
            "cross_file_removed": 0,
            "semantic_cover_removed": 0,
            "direct_proxyish_remaining": 0,
            "globalproxy_cn_remaining": 0,
        }

    def load(self):
        self.adblock_payloads = set()
        for ruleset_dir in self.ruleset_dirs:
            if not os.path.exists(ruleset_dir): continue
            filenames = sorted(
                f for f in os.listdir(ruleset_dir)
                if f.endswith(".list")
            )
            for filename in filenames:
                filepath = os.path.join(ruleset_dir, filename)
                raw_unique = load_list(filepath)
                self.file_content[filename] = raw_unique
                self.file_paths[filename] = filepath
                # Collect all payloads that belong to dedicated AdBlock shards.
                if any(filename.startswith(pfx) for pfx in ADBLOCK_ONLY_PREFIXES):
                    self.adblock_payloads.update(raw_unique.keys())
        self.stats["files"] = len(self.file_content)

    def _upsert(self, rules: Dict[str, str], rule: str):
        payload = extract_payload(rule)
        if not payload:
            return
        if payload in rules:
            rules[payload] = prefer_rule(rules[payload], rule)
        else:
            rules[payload] = rule

    def rebalance_generic_rules(self):
        direct = self.file_content.setdefault("Direct.list", {})
        proxy = self.file_content.setdefault("GlobalProxy.list", {})

        for payload, rule in list(direct.items()):
            if is_proxyish_domain(rule):
                self._upsert(proxy, rule)
                del direct[payload]
                self.stats["moved_direct_to_proxy"] += 1

        for payload, rule in list(proxy.items()):
            if is_cn_tld(rule) and not is_proxyish_domain(rule):
                self._upsert(direct, rule)
                del proxy[payload]
                self.stats["moved_proxy_to_direct"] += 1

    def deduplicate_by_priority(self):
        all_lists = sorted(self.file_content)
        
        FALLBACK_RULESETS = {
            "Direct.list",
            "Direct_ip.list",
            "ChinaASN.list",
            "GlobalASN.list",
            "GlobalProxy.list",
            "GlobalProxy_ip.list",
        }
        
        # Separate specific rulesets and fallback rulesets
        fallback_files = [f for f in all_lists if f in FALLBACK_RULESETS]
        specific_files = [f for f in all_lists if f not in FALLBACK_RULESETS]
        
        # 1. Sort specific rulesets based on PRIORITY_ORDER (excluding fallback prefixes)
        expanded_specific = []
        seen_specific = set()
        for p in PRIORITY_ORDER:
            # Skip fallback prefixes during specific processing
            if any(fb.startswith(p) for fb in FALLBACK_RULESETS):
                continue
            for filename in specific_files:
                if filename.startswith(p) and filename not in seen_specific:
                    expanded_specific.append(filename)
                    seen_specific.add(filename)
        remaining_specific = [f for f in specific_files if f not in seen_specific]
        final_specific = expanded_specific + remaining_specific
        
        # 2. Sort fallback rulesets based on PRIORITY_ORDER
        expanded_fallback = []
        seen_fallback = set()
        for p in PRIORITY_ORDER:
            for filename in fallback_files:
                if filename.startswith(p) and filename not in seen_fallback:
                    expanded_fallback.append(filename)
                    seen_fallback.add(filename)
        remaining_fallback = [f for f in fallback_files if f not in seen_fallback]
        final_fallback = expanded_fallback + remaining_fallback
        
        # Combine: specific rulesets run first, generic fallbacks run last
        priority = final_specific + final_fallback
        
        # Determine semantic coverage for prefixes
        effective_semantic_coverage = set()
        for p in SEMANTIC_COVERAGE_RULESETS:
            for filename in all_lists:
                if filename.startswith(p):
                    effective_semantic_coverage.add(filename)

        seen_payloads = set()
        seen_covering_suffixes = set()

        for filename in priority:
            rules = self.file_content.get(filename)
            if rules is None:
                continue
            kept: Dict[str, str] = {}
            for payload, rule in sorted(rules.items(), key=lambda item: rule_sort_key(item[1])):
                # 0. Hard separation: any payload present in AdBlock shards
                #    must NOT live in routing rulesets.
                if not any(filename.startswith(pfx) for pfx in ADBLOCK_ONLY_PREFIXES):
                    if payload in getattr(self, "adblock_payloads", ()):
                        self.stats["semantic_cover_removed"] += 1
                        continue
                rule_type = extract_type(rule)
                if payload in seen_payloads:
                    self.stats["cross_file_removed"] += 1
                    continue
                if rule_type in SEMANTIC_TYPES:
                    if any(parent in seen_covering_suffixes for parent in iter_domain_parents(payload)):
                        self.stats["semantic_cover_removed"] += 1
                        continue
                seen_payloads.add(payload)
                kept[payload] = rule
                if filename in effective_semantic_coverage and rule_type == "DOMAIN-SUFFIX":
                    seen_covering_suffixes.add(payload)
            self.file_content[filename] = kept

    def purge_nsfw_from_social(self):
        """Remove adult/porn-keyword domains from SocialMedia.list (cross-contamination)."""
        social = self.file_content.get("SocialMedia.list", {})
        removed = 0
        for payload in list(social):
            if any(kw in payload for kw in NSFW_CONTAMINATION_KEYWORDS):
                del social[payload]
                removed += 1
        self.stats["nsfw_purged_from_social"] = removed

    def audit(self):
        direct = self.file_content.get("Direct.list", {})
        proxy = self.file_content.get("GlobalProxy.list", {})
        self.stats["direct_proxyish_remaining"] = sum(
            1 for rule in direct.values() if is_proxyish_domain(rule)
        )
        self.stats["globalproxy_cn_remaining"] = sum(
            1 for rule in proxy.values() if is_cn_tld(rule) and not is_proxyish_domain(rule)
        )

    def save(self):
        for filename, rules in self.file_content.items():
            filepath = self.file_paths.get(filename)
            if filepath:
                if len(rules) == 0 and filename != "AdBlock.list":
                    if os.path.exists(filepath):
                        if safe_remove(filepath):
                            print(f"✅ Deleted empty ruleset file: {filename}")
                        else:
                            print(f"❌ Failed to delete empty file: {filename}")
                else:
                    write_list(filepath, rules)

    def run(self) -> Dict[str, int]:
        self.load()
        self.rebalance_generic_rules()
        self.purge_nsfw_from_social()
        self.deduplicate_by_priority()
        self.audit()
        self.save()
        return self.stats


def format_stats(stats: Dict[str, int]) -> str:
    ordered_keys: Iterable[str] = (
        "files",
        "moved_direct_to_proxy",
        "moved_proxy_to_direct",
        "nsfw_purged_from_social",
        "cross_file_removed",
        "semantic_cover_removed",
        "direct_proxyish_remaining",
        "globalproxy_cn_remaining",
    )
    return ", ".join(f"{key}={stats.get(key, 0)}" for key in ordered_keys)


def run_cleanup(ruleset_dirs: list = RULESET_DIRS) -> Dict[str, int]:
    return RulesetCleanup(ruleset_dirs).run()


def main():
    print("Starting Smart Cleanup...")
    stats = run_cleanup()
    print(f"Smart Cleanup Complete: {format_stats(stats)}")


if __name__ == "__main__":
    main()
