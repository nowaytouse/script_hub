#!/usr/bin/env python3
import os
import hashlib
import zlib
import gzip
import concurrent.futures
from typing import List, Set, Dict, Optional
from hub.common import (
    Logger, get_project_root, read_file, write_file,
    safe_download, safe_download_binary, _has_dangerous_chars,
    safe_remove
)
from hub.rule_processor import RuleProcessor
from hub.surge_compliance import convert_domain_regex_for_surge

ROOT = get_project_root()
SURGE_DIR = os.path.join(ROOT, "rulesets/RULE-SET")
SOURCES_DIR = os.path.join(ROOT, "rulesets/Sources/Links")
METACUBEX_DIR = os.path.join(ROOT, "rulesets/Sources/MetaCubeX")
CACHE_FILE = os.path.join(ROOT, ".cache/merge_hashes.txt")
POLICY_MAP_FILE = os.path.join(ROOT, "rulesets/Sources/ruleset_policy_map.txt")

# --- CONFIGURATION ---

PROTECTED_RULESETS = []

SKIP_CONFLICT_CHECK = [
    "SocialMedia", "GlobalProxy", "GlobalMedia", "SYSTEM", "Direct", "Spotify", 
    "TikTok", "Telegram", "Twitter", "Twitch", "Netflix", "Facebook", "Instagram", 
    "Reddit", "StreamUS", "StreamJP", "StreamKR", "StreamEU", "StreamHK", "StreamTW"
]

CONFLICT_DOMAINS = [
    "x.com", "twitter.com", "facebook.com", "instagram.com", "reddit.com",
    "discord.com", "discordapp.com", "discordapp.net",
    "media.discordapp.net", "cdn.discordapp.com",
    "netflix.com", "hbomax.com", "hbo.com", "youtube.com", "youtu.be",
    "twitch.tv", "spotify.com", "itch.io", "steampowered.com", "epicgames.com",
    "images.pexels.com", "imgur.com", "happymag.tv", "wortfm.org"
]

DEPRECATED_RULESETS = ["SYSTEM", "BlockHttpDNS", "FirewallPorts", "YouTube", "GoogleCN", "Steam", "Epic", "GamingProcess", "QQ", "WeChat", "DownloadProcess", "GlobalMedia", "XiaoHongShu", "NetEaseMusic", "Tencent", "AIProcess", "LAN", "Manual", "Manual_JP", "Manual_US", "Manual_West", "Manual_Global", "Telegram", "TikTok", "Twitter", "Instagram", "Reddit", "Discord", "Fediverse", "Bing", "Tesla", "ChinaDirect", "DirectProcess", "DownloadDirect"]

# Surge external .list must never contain DOMAIN-REGEX (Clash-only); see lib/surge_compliance.py

# --- HARDENING: PROTECTIVE FILTERS ---
# These keywords are NEVER allowed in the [Direct] ruleset.
# Even if an upstream source mistakenly lists them as direct, we strip them.
MANDATORY_PROXY_KEYWORDS = [
    # Western services (never direct regardless of source)
    "google", "gstatic", "gmail", "ggpht", "youtube", "ytimg",
    "facebook", "fbcdn", "instagram", "twitter", "twimg", "t.co",
    "telegram", "netflix", "nflxvideo", "nflxext", "disney", "github",
    "akamai", "fastly", "cloudflare", "browserleaks",
    # ByteDance INTERNATIONAL/OVERSEAS infrastructure (not Mainland China)
    # byteoversea.com = ByteDance global CDN (non-CN), must not be Direct
    "byteoversea",
    # TikTok (international brand, distinct from Douyin which is CN-only)
    "tiktok", "tiktokv", "tiktokcdn", "tiktokeu", "tiktokw", "muscdn",
    # Lark international (lark.com = global, larkoffice for global users via VPN)
    "lark.com",
    # ByteOversea CDN row (tiktokrow = TikTok international row CDN)
    "tiktokrow",
]

RULESETS = [
    "AI", "Gaming", "GlobalProxy", "Microsoft", "NSFW",
    "SocialMedia",
    "Netflix", "Disney", "Spotify", "Bahamut", "AppleNews",
    "Google", "Apple", "GitHub", "PayPal", "Binance",
    "Direct", "Bilibili",
    "CDN",
    "StreamJP", "StreamUS", "StreamKR", "StreamHK", "StreamTW", "StreamEU"
]

SPECIAL_MANAGED_RULESETS = {"AdBlock", "substore"}

# --- RULESET MANAGER CLASS ---

class RulesetManager:
    def __init__(self, force: bool = False):
        self.force = force
        self.hashes = self._load_hashes()
        self.policy_map = self._load_policy_map()
        self.stats = {"merged": 0, "skipped": 0, "deleted": 0}
        self._processor = RuleProcessor()

    @staticmethod
    def _find_case_insensitive_file(directory: str, filename: str) -> Optional[str]:
        exact = os.path.join(directory, filename)
        if os.path.exists(exact):
            return exact
        filename_lower = filename.lower()
        try:
            for entry in os.listdir(directory):
                if entry.lower() == filename_lower:
                    return os.path.join(directory, entry)
        except FileNotFoundError:
            return None
        return None

    def _download(self, url: str) -> str:
        """Download remote content with hardened protections (via lib/common)."""
        is_lsr = url.lower().endswith('.lsr')
        if is_lsr:
            raw = safe_download_binary(url, retries=2)
            if raw is None:
                return ""
            return self._decode_lsr_bytes(raw)
        else:
            content = safe_download(url, retries=2)
            return content if content else ""

    @staticmethod
    def _try_decompress(data: bytes) -> Optional[bytes]:
        """Try gzip, zlib, and raw deflate decompression."""
        try:
            return gzip.decompress(data)
        except Exception:
            pass
        try:
            return zlib.decompress(data)
        except Exception:
            pass
        for offset in range(1, min(64, len(data))):
            try:
                return zlib.decompress(data[offset:])
            except Exception:
                pass
        try:
            return zlib.decompress(data, -15)
        except Exception:
            pass
        for offset in range(1, min(64, len(data))):
            try:
                return zlib.decompress(data[offset:], -15)
            except Exception:
                pass
        return None

    @staticmethod
    def _extract_rules_from_text(text: str) -> List[str]:
        """Extract valid rule lines from decoded text."""
        rules = []
        prefixes = ('DOMAIN', 'IP-CIDR', 'USER-AGENT', 'URL-REGEX',
                    'GEOIP', 'PROCESS-NAME', 'DEST-PORT', 'SRC-PORT')
        for line in text.splitlines():
            line = line.strip()
            if not line or line.startswith('#') or line.startswith('//'):
                continue
            if any(line.startswith(p) for p in prefixes) or ',' not in line:
                if not _has_dangerous_chars(line):
                    rules.append(line)
        return rules

    def _decode_lsr_bytes(self, raw: bytes) -> str:
        """Decode .lsr binary content."""
        try:
            text = raw.decode('utf-8')
            rules = self._extract_rules_from_text(text)
            if rules:
                return "\n".join(rules)
        except Exception:
            pass
        decompressed = self._try_decompress(raw)
        if decompressed:
            try:
                text = decompressed.decode('utf-8')
                rules = self._extract_rules_from_text(text)
                if rules:
                    return "\n".join(rules)
            except Exception:
                pass
        return ""

    def _load_hashes(self) -> Dict[str, str]:
        hashes = {}
        if os.path.exists(CACHE_FILE):
            for line in read_file(CACHE_FILE):
                if ':' in line:
                    k, v = line.strip().split(':', 1)
                    hashes[k] = v
        return hashes

    def _save_hashes(self):
        content = "\n".join([f"{k}:{v}" for k, v in sorted(self.hashes.items())]) + "\n"
        write_file(CACHE_FILE, content)

    def _load_policy_map(self) -> Dict[str, Dict]:
        mapping = {}
        if os.path.exists(POLICY_MAP_FILE):
            for line in read_file(POLICY_MAP_FILE):
                line = line.strip()
                if not line or line.startswith('#'): continue
                parts = line.split('|')
                if len(parts) >= 2:
                    mapping[parts[0]] = {
                        "policy": parts[1],
                        "node": parts[2] if len(parts) > 2 else "",
                        "desc": parts[3] if len(parts) > 3 else ""
                    }
        return mapping

    @staticmethod
    def _surge_convert_domain_regex(rule: str) -> str:
        return convert_domain_regex_for_surge(rule)

    def clean_rule(self, rule: str, ruleset_name: str) -> str:
        """使用统一的 RuleProcessor 处理规则"""
        r = rule.strip()
        if not r or r.startswith('#'):
            return ""

        # 预处理：支持 Clash YAML payload 格式
        if r in {"payload:", "payload"}:
            return ""
        if r.startswith("-"):
            r = r[1:].strip().strip('"').strip("'")
            if not r:
                return ""

        # 预处理：转换无逗号格式（Domain Set）到标准格式
        if "," not in r:
            # IP 地址检测
            if ":" in r or (r.count(".") == 3 and all(c.isdigit() for c in r.split('/')[0].replace(".", ""))):
                if "/" in r:
                    r = f"IP-CIDR6,{r}" if ":" in r else f"IP-CIDR,{r}"
                else:
                    r = f"IP-CIDR6,{r}/128" if ":" in r else f"IP-CIDR,{r}/32"
            else:
                # 域名格式
                if r.startswith('.'):
                    r = f"DOMAIN-SUFFIX,{r[1:]}"
                elif r.startswith('+.'):
                    r = f"DOMAIN-SUFFIX,{r[2:]}"
                elif r.startswith('+'):
                    r = f"DOMAIN-SUFFIX,{r[1:]}"
                else:
                    r = f"DOMAIN,{r}"

        normalized = self._processor.normalize_rule(r)
        if not normalized:
            return ""

        if normalized.startswith("DOMAIN-REGEX,"):
            return self._surge_convert_domain_regex(normalized)

        # 1. Conflict Check (Match original bash behavior)
        if ruleset_name not in SKIP_CONFLICT_CHECK:
            for domain in CONFLICT_DOMAINS:
                if normalized.endswith(f",{domain}"):
                    return ""

        # 1.1 Hardening: Force Proxy for international services in Direct ruleset
        if ruleset_name == "Direct":
            r_lower = normalized.lower()
            if any(kw in r_lower for kw in MANDATORY_PROXY_KEYWORDS):
                return ""

        return normalized

    def generate_header(self, name: str, rule_count: int) -> str:
        base_name = name.replace("_ip", "")
        p_info = self.policy_map.get(name) or self.policy_map.get(base_name, {"policy": "PROXY", "node": "Any", "desc": "Auto-merged ruleset"})
        header = [
            f"# Ruleset: {name}",
            f"# Policy: {p_info['policy']}"
        ]
        if p_info.get('node'): header.append(f"# Node: {p_info['node']}")
        header.extend([
            f"# Description: {p_info['desc']}",
            f"# Rules: {rule_count}"
        ])
        return "\n".join(header) + "\n"

    def process_ruleset(self, name: str):
        target_file = os.path.join(SURGE_DIR, f"{name}.list")
        if name in DEPRECATED_RULESETS:
            if os.path.exists(target_file):
                if safe_remove(target_file):
                    self.stats["deleted"] += 1
                    Logger.warn(f"Removed deprecated: {name}")
                else:
                    Logger.error(f"Failed to remove deprecated: {name}")
            return

        hasher = hashlib.md5()
        found = False
        
        # 1. Hash the manifest lists themselves
        manifests = [
            os.path.join(SOURCES_DIR, f"{name}_sources.txt"), 
            self._find_case_insensitive_file(METACUBEX_DIR, f"MetaCubeX_{name}.list")
        ]
        for f in manifests:
            if not f:
                continue
            if os.path.exists(f):
                with open(f, 'rb') as f_obj: 
                    content = f_obj.read()
                    hasher.update(content)
                    # 2. Deep Dive: If it's a manifest, hash the local files it references
                    if f.endswith('_sources.txt'):
                        for line in content.decode('utf-8', errors='ignore').splitlines():
                            line = line.strip()
                            if line and not line.startswith(('#', 'http')):
                                local_ref = os.path.join(SOURCES_DIR, line.split('|')[0])
                                if os.path.exists(local_ref):
                                    with open(local_ref, 'rb') as lr_obj: hasher.update(lr_obj.read())
                found = True
        
        if name in PROTECTED_RULESETS and os.path.exists(target_file):
            content = "".join([l for l in read_file(target_file) if l.strip() and not l.strip().startswith('#')])
            hasher.update(content.encode('utf-8'))
            found = True
            
        current_hash = hasher.hexdigest() if found else ""
        # Detect if any source is remote (http)
        has_remote = False
        s_file = os.path.join(SOURCES_DIR, f"{name}_sources.txt")
        if os.path.exists(s_file):
            for line in read_file(s_file):
                if line.strip().startswith('http'):
                    has_remote = True
                    break

        if not self.force and os.path.exists(target_file) and self.hashes.get(name) == current_hash and not has_remote:
            self.stats["skipped"] += 1
            return

        all_rules: Set[str] = set()
        if name in PROTECTED_RULESETS:
            if os.path.exists(target_file):
                for line in read_file(target_file):
                    cleaned = self.clean_rule(line, name)
                    if cleaned: all_rules.add(cleaned)
        else:
            m_file = self._find_case_insensitive_file(METACUBEX_DIR, f"MetaCubeX_{name}.list")
            if m_file and os.path.exists(m_file):
                for line in read_file(m_file):
                    cleaned = self.clean_rule(line, name)
                    if cleaned: all_rules.add(cleaned)
            s_file = os.path.join(SOURCES_DIR, f"{name}_sources.txt")
            if os.path.exists(s_file):
                for line in read_file(s_file):
                    line = line.strip()
                    if not line or line.startswith('#'): continue
                    if line.startswith('http'):
                        remote_content = self._download(line)
                        if remote_content:
                            for r_line in remote_content.splitlines():
                                cleaned = self.clean_rule(r_line, name)
                                if cleaned: all_rules.add(cleaned)
                    else:
                        local_path = os.path.join(SOURCES_DIR, line.split('|')[0])
                        if os.path.exists(local_path):
                            for r_line in read_file(local_path):
                                cleaned = self.clean_rule(r_line, name)
                                if cleaned: all_rules.add(cleaned)

        if not all_rules and name not in PROTECTED_RULESETS: return

        non_ip_rules = set()
        ip_rules = set()
        for rule in all_rules:
            if rule.startswith(("IP-CIDR,", "IP-CIDR6,", "IP-ASN,", "GEOIP,")):
                ip_rules.add(rule)
            else:
                non_ip_rules.add(rule)

        target_ip_file = os.path.join(SURGE_DIR, f"{name}_ip.list")
        if ip_rules:
            write_file(target_ip_file, self.generate_header(f"{name}_ip", len(ip_rules)) + "\n".join(sorted(list(ip_rules))) + "\n")
            Logger.success(f"Processed IP Ruleset: {name}_ip ({len(ip_rules)} rules)")
        else:
            if os.path.exists(target_ip_file):
                safe_remove(target_ip_file)

        if non_ip_rules or name in PROTECTED_RULESETS:
            write_file(target_file, self.generate_header(name, len(non_ip_rules)) + "\n".join(sorted(list(non_ip_rules))) + "\n")
            Logger.success(f"Processed Domain Ruleset: {name} ({len(non_ip_rules)} rules)")
        else:
            if os.path.exists(target_file):
                safe_remove(target_file)

        self.hashes[name] = current_hash
        self.stats["merged"] += 1


    def run(self):
        Logger.section("Ruleset Processing (1:1 Logic)")
        os.makedirs(SURGE_DIR, exist_ok=True)
        # Exclude AdBlock, substore, and IP sub-lists
        all_files = [f[:-5] for f in os.listdir(SURGE_DIR) if f.endswith('.list') and not f[:-5].endswith('_ip')]
        all_to_process = (set(RULESETS) | set(DEPRECATED_RULESETS) | set(all_files)) - SPECIAL_MANAGED_RULESETS
        
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            list(executor.map(self.process_ruleset, sorted(list(all_to_process))))
        self._prune_empty_rulesets()
        self._save_hashes()
        Logger.info(
            f"Done: Merged {self.stats['merged']} | Skipped {self.stats['skipped']} "
            f"| Deleted {self.stats['deleted']}"
        )

    def _count_rules(self, path: str) -> int:
        n = 0
        for line in read_file(path):
            s = line.strip()
            if s and not s.startswith("#"):
                n += 1
        return n

    def _prune_empty_rulesets(self) -> None:
        """Remove empty deprecated .list files (legacy ruleset_cleaner.sh behavior)."""
        if not os.path.isdir(SURGE_DIR):
            return
        for fname in os.listdir(SURGE_DIR):
            if not fname.endswith(".list"):
                continue
            name = fname[:-5]
            if name in SPECIAL_MANAGED_RULESETS or name.endswith('_ip'):
                continue
            path = os.path.join(SURGE_DIR, fname)
            if self._count_rules(path) > 0:
                continue
            if name in DEPRECATED_RULESETS:
                if safe_remove(path):
                    src = os.path.join(SOURCES_DIR, f"{name}_sources.txt")
                    if os.path.exists(src):
                        safe_remove(src)
                    self.stats["deleted"] += 1
                    Logger.warn(f"Pruned empty deprecated ruleset: {name}")
            else:
                Logger.warn(f"Empty ruleset not in deprecated list: {name}")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()
    RulesetManager(force=args.force).run()
