#!/usr/bin/env python3
import os
import re
import hashlib
import concurrent.futures
from datetime import datetime
from typing import List, Set, Dict, Optional
from lib.common import Logger, get_project_root, read_file, write_file

ROOT = get_project_root()
SURGE_DIR = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)")
SOURCES_DIR = os.path.join(ROOT, "ruleset/Sources/Links")
METACUBEX_DIR = os.path.join(ROOT, "ruleset/MetaCubeX")
CACHE_FILE = os.path.join(ROOT, ".cache/merge_hashes.txt")
POLICY_MAP_FILE = os.path.join(ROOT, "ruleset/Sources/ruleset_policy_map.txt")

# ═══════════════════════════════════════════════════════════════════════════════
# CONFIGURATION
# ═══════════════════════════════════════════════════════════════════════════════

PROTECTED_RULESETS = ["DownloadDirect", "Manual", "Manual_JP", "Manual_US", "Manual_West", "Manual_Global"]

SKIP_CONFLICT_CHECK = [
    "SocialMedia", "Gaming", "Steam", "Epic", "GlobalMedia", "YouTube", "Spotify", 
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

DEPRECATED_RULESETS = ["BlockHttpDNS", "FirewallPorts"]

RULESETS = [
    "GlobalMedia", "AI", "Gaming", "GlobalProxy", "Microsoft", "Discord", "Fediverse", "NSFW", "LAN",
    "SocialMedia", "Telegram", "TikTok", "Twitter", "Instagram", "Reddit", "WeChat",
    "YouTube", "Netflix", "Disney", "Spotify", "Bahamut", "AppleNews",
    "Google", "Bing", "Apple", "GitHub", "PayPal", "Tesla", "Binance",
    "Steam", "Epic",
    "ChinaDirect", "Bilibili", "QQ", "Tencent", "XiaoHongShu", "NetEaseMusic", "GoogleCN",
    "CDN", "Speedtest",
    "StreamJP", "StreamUS", "StreamKR", "StreamHK", "StreamTW", "StreamEU",
    "AIProcess", "DirectProcess", "DownloadProcess", "GamingProcess",
    "AdBlock", "substore"
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
        
        # 1. Conflict Check (Match original bash behavior)
        if ruleset_name not in SKIP_CONFLICT_CHECK:
            for domain in CONFLICT_DOMAINS:
                if r.endswith(f",{domain}"): return ""
        
        # 2. Cleanup features
        r = re.sub(r',(REJECT|DIRECT|PROXY|REJECT-DROP|REJECT-TINYGIF|REJECT-NO-DROP|REJECT-IMG)(,.*)*$', '', r)
        r = r.replace(',extended-matching', '').replace(',pre-matching', '').replace(',no-resolve', '')
        
        if r.startswith('IP-CIDR,') and '::' in r: r = r.replace('IP-CIDR,', 'IP-CIDR6,', 1)
        if any(k in r for k in ["AND,", "OR,", "NOT,"]): return ""
        if r.startswith("DOMAIN-REGEX,") and ("," not in r[13:] or not r[13:].strip()): return ""
        
        if not r.startswith(("DOMAIN", "IP-CIDR", "PROCESS-NAME", "URL-REGEX", "USER-AGENT")):
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
        if not self.force and os.path.exists(target_file) and self.hashes.get(name) == current_hash:
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
                    if not line or line.startswith(('#', 'http')): continue
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
        all_to_process = set(RULESETS) | set(DEPRECATED_RULESETS) | set([f[:-5] for f in os.listdir(SURGE_DIR) if f.endswith('.list')])
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
