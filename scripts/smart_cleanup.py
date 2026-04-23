#!/usr/bin/env python3
import os
from typing import Dict, Iterable

RULESET_DIR = os.path.join(os.path.dirname(__file__), "../ruleset/Surge(Shadowkroket)")

# Specific rulesets should win over generic fallbacks. Direct stays above
# GlobalProxy, but below service-specific proxy rules, so accidental overlaps
# do not hijack routing before a more precise ruleset can match.
PRIORITY_ORDER = [
    "AdBlock.list",
    "reject-drop.list",
    "reject-no-drop.list",
    "HTTPDNS_Hijack.list",
    "AI.list",
    "SocialMedia.list",
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
    "CDN.list",
    "Cloudflare.list",
    "Bilibili.list",
    "Direct.list",
    "GlobalProxy.list",
]

MANDATORY_PROXY_KEYWORDS = [
    "google", "gstatic", "gmail", "ggpht", "youtube", "ytimg",
    "facebook", "fbcdn", "instagram", "twitter", "twimg", "t.co",
    "telegram", "netflix", "nflxvideo", "nflxext", "disney", "github",
    "akamai", "fastly", "cloudflare",
]

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


def is_valid_rule(line: str) -> bool:
    if line.startswith("RULE-SET"):
        return False
    if line.startswith("DOMAIN") and ",no-resolve" in line:
        return False
    return "," in line


def clean_rule(line: str) -> str:
    line = line.strip()
    if not line or line.startswith(("#", "//")):
        return ""
    if "#" in line:
        line = line.split("#", 1)[0].strip()
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

    with open(filepath, "w", encoding="utf-8") as f:
        if existing_header:
            for line in existing_header:
                f.write(line)
            if existing_header[-1].strip():
                f.write("\n")
        else:
            filename = os.path.basename(filepath)
            f.write(f"# Ruleset: {filename}\n")
            f.write("# Cleaned by smart_cleanup.py\n\n")
        for rule in sorted_rules:
            f.write(rule + "\n")


class RulesetCleanup:
    def __init__(self, ruleset_dir: str = RULESET_DIR):
        self.ruleset_dir = ruleset_dir
        self.file_content: Dict[str, Dict[str, str]] = {}
        self.stats = {
            "files": 0,
            "within_file_dedup": 0,
            "moved_direct_to_proxy": 0,
            "moved_proxy_to_direct": 0,
            "cross_file_removed": 0,
            "direct_proxyish_remaining": 0,
            "globalproxy_cn_remaining": 0,
        }

    def load(self):
        filenames = sorted(
            f for f in os.listdir(self.ruleset_dir)
            if f.endswith(".list")
        )
        self.stats["files"] = len(filenames)
        for filename in filenames:
            filepath = os.path.join(self.ruleset_dir, filename)
            raw_unique = load_list(filepath)
            self.file_content[filename] = raw_unique

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
        priority = PRIORITY_ORDER + [name for name in all_lists if name not in PRIORITY_ORDER]
        seen_payloads = set()

        for filename in priority:
            rules = self.file_content.get(filename)
            if rules is None:
                continue
            kept: Dict[str, str] = {}
            for payload, rule in sorted(rules.items()):
                if payload in seen_payloads:
                    self.stats["cross_file_removed"] += 1
                    continue
                seen_payloads.add(payload)
                kept[payload] = rule
            self.file_content[filename] = kept

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
            filepath = os.path.join(self.ruleset_dir, filename)
            write_list(filepath, rules)

    def run(self) -> Dict[str, int]:
        self.load()
        self.rebalance_generic_rules()
        self.deduplicate_by_priority()
        self.audit()
        self.save()
        return self.stats


def format_stats(stats: Dict[str, int]) -> str:
    ordered_keys: Iterable[str] = (
        "files",
        "moved_direct_to_proxy",
        "moved_proxy_to_direct",
        "cross_file_removed",
        "direct_proxyish_remaining",
        "globalproxy_cn_remaining",
    )
    return ", ".join(f"{key}={stats[key]}" for key in ordered_keys)


def run_cleanup(ruleset_dir: str = RULESET_DIR) -> Dict[str, int]:
    return RulesetCleanup(ruleset_dir).run()


def main():
    print("Starting Smart Cleanup...")
    stats = run_cleanup()
    print(f"Smart Cleanup Complete: {format_stats(stats)}")


if __name__ == "__main__":
    main()
