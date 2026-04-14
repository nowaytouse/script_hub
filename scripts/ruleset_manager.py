#!/usr/bin/env python3
import os
import re
import hashlib
import zlib
import gzip
import concurrent.futures
from datetime import datetime
from typing import List, Set, Dict, Optional
from lib.common import (
    Logger, get_project_root, read_file, write_file,
    safe_download, safe_download_binary, _has_dangerous_chars
)

ROOT = get_project_root()
SURGE_DIR = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)")
SOURCES_DIR = os.path.join(ROOT, "ruleset/Sources/Links")
METACUBEX_DIR = os.path.join(ROOT, "ruleset/MetaCubeX")
CACHE_FILE = os.path.join(ROOT, ".cache/merge_hashes.txt")
POLICY_MAP_FILE = os.path.join(ROOT, "ruleset/Sources/ruleset_policy_map.txt")

# ═══════════════════════════════════════════════════════════════════════════════
# CONFIGURATION
# ═══════════════════════════════════════════════════════════════════════════════

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
INVALID_DOMAIN_REGEX_VALUES = {"", "$", ",", "-", ".", "2", "6", "]", "["}

RULESETS = [
    "AI", "Gaming", "GlobalProxy", "Microsoft", "NSFW",
    "SocialMedia",
    "Netflix", "Disney", "Spotify", "Bahamut", "AppleNews",
    "Google", "Apple", "GitHub", "PayPal", "Binance",
    "Direct", "Bilibili",
    "CDN", "Speedtest",
    "StreamJP", "StreamUS", "StreamKR", "StreamHK", "StreamTW", "StreamEU"
]

# ═══════════════════════════════════════════════════════════════════════════════
# RULESET MANAGER CLASS
# ═══════════════════════════════════════════════════════════════════════════════

class RulesetManager:
    def __init__(self, force: bool = False):
        self.force = force
        self.hashes = self._load_hashes()
        self.policy_map = self._load_policy_map()
        self.stats = {"merged": 0, "skipped": 0, "deleted": 0}

    def _download(self, url: str) -> str:
        """Download remote content with hardened protections (via lib/common)."""
        is_lsr = url.lower().endswith('.lsr')
        if is_lsr:
            raw = safe_download_binary(url, retries=1)
            if raw is None:
                return ""
            return self._decode_lsr_bytes(raw)
        else:
            content = safe_download(url, retries=1)
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

    def clean_rule(self, rule: str, ruleset_name: str) -> str:
        r = rule.strip()
        if not r or r.startswith('#'): return ""
        
        # 0. Convert Domain Set formats (no commas) to classical syntax
        if "," not in r:
            # Check if it looks like an IP Address (basic heuristic)
            if ":" in r or (r.count(".") == 3 and all(c.isdigit() for c in r.split('/')[0].replace(".", ""))):
                if "/" in r:
                    r = f"IP-CIDR6,{r}" if ":" in r else f"IP-CIDR,{r}"
                else:
                    r = f"IP-CIDR6,{r}/128" if ":" in r else f"IP-CIDR,{r}/32"
            else:
                if r.startswith('.'):
                    r = f"DOMAIN-SUFFIX,{r[1:]}"
                elif r.startswith('+.'):
                    r = f"DOMAIN-SUFFIX,{r[2:]}"
                elif r.startswith('+'):
                    r = f"DOMAIN-SUFFIX,{r[1:]}"
                else:
                    r = f"DOMAIN,{r}"

        # 1. Conflict Check (Match original bash behavior)
        if ruleset_name not in SKIP_CONFLICT_CHECK:
            for domain in CONFLICT_DOMAINS:
                if r.endswith(f",{domain}"): return ""
        
        # 2. Cleanup features
        r = re.sub(r',(REJECT|DIRECT|PROXY|REJECT-DROP|REJECT-TINYGIF|REJECT-NO-DROP|REJECT-IMG)(,.*)*$', '', r)
        r = r.replace(',extended-matching', '').replace(',pre-matching', '').replace(',no-resolve', '')
        
        if r.startswith('IP-CIDR,') and '::' in r: r = r.replace('IP-CIDR,', 'IP-CIDR6,', 1)
        if any(k in r for k in ["AND,", "OR,", "NOT,"]): return ""
        if r.startswith("DOMAIN-REGEX,"):
            # Deep clean: Skip regex rules for Google as they are redundant and break Surge parsing
            if ruleset_name == "Google":
                return ""
            payload = r[13:].strip().strip('"')
            # Surge does not support spaces in DOMAIN-REGEX in .list files
            if not payload or " " in payload or payload in INVALID_DOMAIN_REGEX_VALUES or len(payload) < 2:
                return ""
            r = f'DOMAIN-REGEX,"{payload}"'
        elif "," in r:
            parts = r.split(",", 1)
            r = f"{parts[0].strip()},{parts[1].strip()}"
        
        if not r.startswith(("DOMAIN", "IP-CIDR", "PROCESS-NAME", "URL-REGEX", "USER-AGENT", "GEOIP")):
            return ""
        return r

    def generate_header(self, name: str, rule_count: int) -> str:
        p_info = self.policy_map.get(name, {"policy": "PROXY", "node": "Any", "desc": "Auto-merged ruleset"})
        header = [
            "# ═══════════════════════════════════════════════════════════════",
            f"# Ruleset: {name}",
            f"# Policy: {p_info['policy']}"
        ]
        if p_info.get('node'): header.append(f"# Node: {p_info['node']}")
        header.extend([
            f"# Description: {p_info['desc']}",
            f"# Rules: {rule_count}",
            f"# Updated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}",
            "# ═══════════════════════════════════════════════════════════════\n"
        ])
        return "\n".join(header) + "\n"

    def process_ruleset(self, name: str):
        target_file = os.path.join(SURGE_DIR, f"{name}.list")
        if name in DEPRECATED_RULESETS:
            if os.path.exists(target_file):
                os.remove(target_file)
                self.stats["deleted"] += 1
                Logger.warn(f"Removed deprecated: {name}")
            return

        hasher = hashlib.md5()
        found = False
        for f in [os.path.join(SOURCES_DIR, f"{name}_sources.txt"), os.path.join(METACUBEX_DIR, f"MetaCubeX_{name}.list")]:
            if os.path.exists(f):
                with open(f, 'rb') as f_obj: hasher.update(f_obj.read())
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
            m_file = os.path.join(METACUBEX_DIR, f"MetaCubeX_{name}.list")
            if os.path.exists(m_file):
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
        write_file(target_file, self.generate_header(name, len(all_rules)) + "\n".join(sorted(list(all_rules))) + "\n")
        self.hashes[name] = current_hash
        self.stats["merged"] += 1
        Logger.success(f"Processed Ruleset: {name} ({len(all_rules)} rules)")

    def run(self):
        Logger.section("Ruleset Processing (1:1 Logic)")
        os.makedirs(SURGE_DIR, exist_ok=True)
        # Exclude AdBlock and substore as they have specialized managers
        SPECIAL_MANAGED = ["AdBlock", "substore"]
        all_files = [f[:-5] for f in os.listdir(SURGE_DIR) if f.endswith('.list')]
        all_to_process = (set(RULESETS) | set(DEPRECATED_RULESETS) | set(all_files)) - set(SPECIAL_MANAGED)
        
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            executor.map(self.process_ruleset, sorted(list(all_to_process)))
        self._save_hashes()
        Logger.info(f"Done: Merged {self.stats['merged']} | Skipped {self.stats['skipped']}")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()
    RulesetManager(force=args.force).run()
