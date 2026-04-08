#!/usr/bin/env python3
import os
import re
import argparse
import sys
from datetime import datetime
from typing import Dict, List, Set, Tuple
from lib.common import (
    Logger, get_project_root, read_file, write_file, get_file_hash
)

# ═══════════════════════════════════════════════════════════════════════════════
# CONFIGURATION
# ═══════════════════════════════════════════════════════════════════════════════

ROOT = get_project_root()
HEAD_EXPANSE_DIR = os.path.join(ROOT, "module/surge(main)/head_expanse")
CACHE_DIR = os.path.join(ROOT, ".cache")
HASH_FILE = os.path.join(CACHE_DIR, "adblock_hashes.txt")
WHITELIST_FILE = os.path.join(ROOT, "ruleset/Sources/adblock_whitelist.txt")
TARGET_MODULE = os.path.join(HEAD_EXPANSE_DIR, "🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule")
ADBLOCK_LIST = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)/AdBlock.list")
FIREWALL_MODULE = os.path.join(HEAD_EXPANSE_DIR, "🔥 Firewall Port Blocker 🛡️🚫.sgmodule")

# Group names (Preserve Chinese as per instruction)
GROUP_HEAD_EXPANSE = "『 🔝 Head Expanse › 首端扩域 』"

SOURCE_MODULES = [
    "小程序和应用懒人去广告合集.official.sgmodule",
    "All-in-One-2.x.sgmodule",
    "AllInOne_Mock.sgmodule",
    "可莉广告过滤器.beta.sgmodule",
    "广告平台拦截器.sgmodule",
    "Adblock4limbo.sgmodule"
]

# ═══════════════════════════════════════════════════════════════════════════════
# ADBlock MANAGER CLASS
# ═══════════════════════════════════════════════════════════════════════════════

class AdBlockManager:
    def __init__(self):
        self.whitelist: Set[str] = set()
        self.rules: Dict[str, Set[str]] = {
            "REJECT": set(),
            "REJECT-DROP": set(),
            "REJECT-NO-DROP": set(),
            "DIRECT": set()
        }
        self.sections: Dict[str, List[str]] = {
            "URL Rewrite": [],
            "Map Local": [],
            "Script": []
        }
        self.mitm_hosts: Set[str] = set()
        self.hashes: Dict[str, str] = {}
        self.needs_update = False

    def load_whitelist(self):
        if os.path.exists(WHITELIST_FILE):
            lines = read_file(WHITELIST_FILE)
            for line in lines:
                line = line.strip()
                if not line or line.startswith('#'): continue
                cleaned = re.sub(r'^(DOMAIN-SUFFIX|DOMAIN-KEYWORD|DOMAIN),', '', line)
                self.whitelist.add(cleaned)
            Logger.info(f"Loaded {len(self.whitelist)} whitelist patterns")

    def load_hashes(self):
        if os.path.exists(HASH_FILE):
            lines = read_file(HASH_FILE)
            for line in lines:
                if '|' in line:
                    name, h = line.strip().split('|', 1)
                    self.hashes[name] = h

    def save_hashes(self, new_hashes: Dict[str, str]):
        content = "\n".join([f"{k}|{v}" for k, v in sorted(new_hashes.items())]) + "\n"
        write_file(HASH_FILE, content)

    def expand_complex_rule(self, line: str) -> List[str]:
        """Restore AND/OR expansion logic from original bash script."""
        extracted = []
        if line.startswith("AND,((") or line.startswith("OR,(("):
            for m in re.findall(r"DOMAIN-SUFFIX,([^,\)\]]+)", line):
                s = m.strip()
                if s and "." in s: extracted.append(f"DOMAIN-SUFFIX,{s}")
            for m in re.findall(r"DOMAIN-KEYWORD,-?([^,\)\]]+)", line):
                s = m.strip()
                if re.search(r"\.(com|net|org|io|cn|jp|kr|tw|hk|sg|uk|de|fr|ru|br|in|au|co|me|tv|cc|xyz|top|app|dev)$", s, re.I):
                    extracted.append(f"DOMAIN-SUFFIX,{s}")
            return extracted
        if line.count("(") != line.count(")") or line.endswith(("DOMAIN-KEYWORD", "DOMAIN-SUFFIX", "DOMAIN")):
            return []
        return [line]

    def extract_from_file(self, file_path: str):
        if not os.path.exists(file_path): return
        lines = read_file(file_path)
        current_section = None
        for line in lines:
            line_strip = line.strip()
            if not line_strip: continue
            if line_strip.startswith('[') and line_strip.endswith(']'):
                current_section = line_strip[1:-1]
                continue
            
            if current_section == "Rule":
                if line_strip.startswith('#') or "RULE-SET" in line_strip: continue
                expanded = self.expand_complex_rule(line_strip)
                for r in expanded:
                    if "REJECT-DROP" in r: self.rules["REJECT-DROP"].add(r)
                    elif "REJECT-NO-DROP" in r: self.rules["REJECT-NO-DROP"].add(r)
                    elif "REJECT" in r: self.rules["REJECT"].add(r)
                    elif "DIRECT" in r: self.rules["DIRECT"].add(r)
            
            elif current_section in self.sections:
                if not line_strip.startswith('#'): self.sections[current_section].append(line_strip)
            
            elif current_section == "MITM":
                if line_strip.startswith("hostname"):
                    hosts_str = re.sub(r'^hostname\s*=\s*(%APPEND%\s*)?', '', line_strip)
                    hosts = [h.strip() for h in hosts_str.split(',') if h.strip()]
                    self.mitm_hosts.update(hosts)

    def filter_rules(self, rules_set: Set[str]) -> List[str]:
        filtered = []
        for r in rules_set:
            is_whitelisted = False
            for w in self.whitelist:
                if w in r:
                    is_whitelisted = True
                    break
            if not is_whitelisted: filtered.append(r)
        return sorted(filtered)

    def merge(self, execute: bool = False):
        Logger.section("AdBlock Module Consolidation (Logic Sync)")
        self.load_whitelist()
        self.load_hashes()
        
        new_hashes = {}
        all_modules = []
        for m in SOURCE_MODULES:
            path = os.path.join(HEAD_EXPANSE_DIR, m)
            if os.path.exists(path): all_modules.append(path)
        
        for f in os.listdir(HEAD_EXPANSE_DIR):
            if not f.endswith('.sgmodule'): continue
            path = os.path.join(HEAD_EXPANSE_DIR, f)
            if path in all_modules or f in [os.path.basename(TARGET_MODULE), os.path.basename(FIREWALL_MODULE)]: continue
            content = "".join(read_file(path)).lower()
            if any(k in content for k in ["ad", "reject", "block"]): all_modules.append(path)

        for path in all_modules:
            name = os.path.basename(path)
            h = get_file_hash(path)
            new_hashes[name] = h
            if self.hashes.get(name) != h:
                self.needs_update = True
                Logger.info(f"Update detected in: {name}")

        if not self.needs_update and os.path.exists(TARGET_MODULE) and not execute:
            Logger.success("AdBlock modules are up to date.")
            return

        for path in all_modules: self.extract_from_file(path)

        Logger.section("Generating Targets (English Script Interface)")
        # Module Header (Category kept in Chinese)
        header = f"""#!name=🚫 Universal Ad-Blocking Rules (LITE)
#!desc=Consolidated ad-blocking logic. Automated merge from multiple sources including Kelee, AllInOne, and App-specific blockers. Fully self-contained.
#!author=ScriptHub-Automated
#!icon=https://raw.githubusercontent.com/luestr/IconResource/main/Other_icon/120px/KeLee.png
#!category={GROUP_HEAD_EXPANSE}
#!tag=AdBlock, Dependency, HTTPDNS
#!date={datetime.now().strftime('%Y-%m-%d %H:%M:%S')}

[Rule]
# REJECT Rules (Self-hosted)
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/AdBlock.list,REJECT,extended-matching,pre-matching,update-interval=86400,no-resolve
# REJECT-NO-DROP Rules (Self-hosted)
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/skk_upstream/reject-no-drop.conf,REJECT-NO-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve
# REJECT-DROP Rules (Self-hosted)
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/skk_upstream/reject-drop.conf,REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve
# BlockHttpDNS (Self-hosted)
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/skk_upstream/BlockHttpDNS.list,REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve

"""
        content = [header]
        for rtype in ["REJECT-DROP", "REJECT-NO-DROP"]:
            rules = sorted(list(self.rules[rtype]))
            if rules:
                content.append(f"# Merged {rtype} Rules ({len(rules)})\n")
                content.extend([f"{r}\n" for r in rules])
                content.append("\n")

        for sec, items in self.sections.items():
            if items:
                content.append(f"[{sec}]\n")
                content.extend([f"{i}\n" for i in sorted(list(set(items)))])
                content.append("\n")

        if self.mitm_hosts:
            content.append("[MITM]\n")
            content.append(f"hostname = %APPEND% {', '.join(sorted(list(self.mitm_hosts)))}\n")

        write_file(TARGET_MODULE, "".join(content))
        Logger.success(f"Module generated: {os.path.basename(TARGET_MODULE)}")

        # Ruleset File
        clean_rules = self.filter_rules(self.rules["REJECT"])
        if os.path.exists(ADBLOCK_LIST):
            for l in read_file(ADBLOCK_LIST):
                if l.strip() and not l.startswith(('#', 'RULE-SET')): clean_rules.append(l.strip())
        
        adblock_content = [
            f"# Ruleset: AdBlock\n",
            f"# Updated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n",
            f"# Total Rules: {len(clean_rules)}\n",
            f"# Whitelist applied\n",
            f"# ═══════════════════════════════════════════════════════════════\n\n"
        ]
        adblock_content.extend([f"{r}\n" for r in sorted(list(set(clean_rules)))])
        write_file(ADBLOCK_LIST, "".join(adblock_content))
        Logger.success(f"Ruleset generated: AdBlock.list ({len(clean_rules)} rules)")
        self.save_hashes(new_hashes)

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    AdBlockManager().merge(execute=args.execute)
