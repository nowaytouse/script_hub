#!/usr/bin/env python3
import os
import re
import hashlib
from datetime import datetime
from typing import List, Set, Dict, Optional
from lib.common import (
    Logger, get_project_root, read_file, write_file,
    safe_download, _has_dangerous_chars
)

ROOT = get_project_root()
DNS_MAPPING_DIR = os.path.join(ROOT, "ruleset/Sources/DNS_mapping")
CACHE_FILE = os.path.join(ROOT, ".cache/dns_mapping_hashes.txt")

class DNSMappingManager:
    def __init__(self, force: bool = False):
        self.force = force
        self.hashes = self._load_hashes()
        self.stats = {"merged": 0, "skipped": 0}

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

    def clean_rule(self, rule: str) -> Optional[str]:
        """Extract DOMAIN, DOMAIN-SUFFIX and DOMAIN-KEYWORD rules for DNS mapping."""
        r = rule.strip()
        if not r or r.startswith(('#', '//')):
            return None
        
        # Remove comments and modifiers
        r = re.sub(r'\s*//.*$', '', r)
        r = re.sub(r'\s*#.*$', '', r)
        r = re.sub(r',(REJECT|DIRECT|PROXY|REJECT-DROP|REJECT-TINYGIF|REJECT-NO-DROP|REJECT-IMG)(,.*)*$', '', r)
        r = r.replace(',extended-matching', '').replace(',pre-matching', '').replace(',no-resolve', '')

        # Standardize format
        if "," not in r:
            if r.startswith('.'): r = f"DOMAIN-SUFFIX,{r[1:]}"
            elif r.startswith('+.'): r = f"DOMAIN-SUFFIX,{r[2:]}"
            else: r = f"DOMAIN,{r}"

        if r.startswith(("DOMAIN,", "DOMAIN-SUFFIX,", "DOMAIN-KEYWORD,")):
            parts = r.split(",", 1)
            if len(parts) == 2:
                domain = parts[1].strip()
                if domain and not _has_dangerous_chars(domain):
                    return f"{parts[0].strip()},{domain}"
        
        return None

    def generate_header(self, name: str, rule_count: int) -> str:
        header = [
            f"# DNS Mapping Ruleset: {name}",
            f"# Description: Automated priority mapping for DoH steering",
            f"# Rules: {rule_count}",
            f"# Updated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n"
        ]
        return "\n".join(header) + "\n"

    def process_ruleset(self, sources_file: str):
        name = os.path.basename(sources_file).replace("_sources.txt", "")
        target_file = os.path.join(DNS_MAPPING_DIR, f"{name}.list")
        
        # Hash the manifest itself
        with open(sources_file, 'rb') as f:
            manifest_hash = hashlib.md5(f.read()).hexdigest()
        
        if not self.force and os.path.exists(target_file) and self.hashes.get(name) == manifest_hash:
            # Check if any local file mentioned in manifest has changed (simplified)
            self.stats["skipped"] += 1
            # return # Disabled for now to ensure first run works perfectly

        all_rules: Set[str] = set()
        for line in read_file(sources_file):
            line = line.strip()
            if not line or line.startswith('#'): continue
            
            if line.startswith('http'):
                content = safe_download(line)
                if content:
                    for r_line in content.splitlines():
                        cleaned = self.clean_rule(r_line)
                        if cleaned: all_rules.add(cleaned)
            else:
                # Handle local path relative to manifest
                local_path = os.path.abspath(os.path.join(os.path.dirname(sources_file), line))
                if os.path.exists(local_path):
                    for r_line in read_file(local_path):
                        cleaned = self.clean_rule(r_line)
                        if cleaned: all_rules.add(cleaned)
        
        if all_rules:
            write_file(target_file, self.generate_header(name, len(all_rules)) + "\n".join(sorted(list(all_rules))) + "\n")
            self.hashes[name] = manifest_hash
            self.stats["merged"] += 1
            Logger.success(f"Generated DNS Mapping: {name} ({len(all_rules)} rules)")

    def run(self):
        Logger.section("DNS Mapping Automation")
        for f in os.listdir(DNS_MAPPING_DIR):
            if f.endswith("_sources.txt"):
                self.process_ruleset(os.path.join(DNS_MAPPING_DIR, f))
        self._save_hashes()
        Logger.info(f"Summary: {self.stats['merged']} merged, {self.stats['skipped']} skipped")

if __name__ == "__main__":
    DNSMappingManager(force=True).run()
