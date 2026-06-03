#!/usr/bin/env python3
import os
import json
import subprocess
import concurrent.futures
import re
from hub.common import Logger, get_project_root, safe_remove

ROOT = get_project_root()
SURGE_DIR = os.path.join(ROOT, "rulesets/list")
DNS_MAPPING_DIR = os.path.join(ROOT, "rulesets/Sources/DNS_mapping")
DNS_RULES_DIR = os.path.join(ROOT, "rulesets/dns")
SKK_UPSTREAM_DIR = os.path.join(SURGE_DIR, "skk_upstream")
SINGBOX_DIR = os.path.join(ROOT, "rulesets/SingBox")
CACHE_DIR = os.path.join(ROOT, ".cache")
EXTRA_SOURCE_FILES = [
    os.path.join(SKK_UPSTREAM_DIR, "reject-drop.conf"),
    os.path.join(SKK_UPSTREAM_DIR, "reject-no-drop.conf"),
]

class SRSGenerator:
    def __init__(self):
        self.singbox_path = self._find_singbox()
        os.makedirs(SINGBOX_DIR, exist_ok=True)
        os.makedirs(CACHE_DIR, exist_ok=True)

    def _find_singbox(self):
        try:
            result = subprocess.run(["which", "sing-box"], capture_output=True, text=True)
            if result.returncode == 0:
                return result.stdout.strip()
        except:
            pass
        local_path = os.path.join(ROOT, "scripts/config-manager-auto-update/bin/sing-box")
        if os.path.exists(local_path):
            return local_path
        # Also check project root
        root_path = os.path.join(ROOT, "sing-box")
        if os.path.exists(root_path):
            return root_path
        return None

    def compile_srs(self, list_file: str):
        name = os.path.splitext(os.path.basename(list_file))[0]
        srs_file = os.path.join(SINGBOX_DIR, f"{name}_Singbox.srs")
        json_tmp = os.path.join(CACHE_DIR, f"{name}.json")
        
        try:
            rules = []
            with open(list_file, 'r') as f:
                for line in f:
                    line = line.strip()
                    if not line or line.startswith('#'): continue
                    
                    parts = line.split(',')
                    if len(parts) < 2: continue
                    rtype = parts[0]
                    val = parts[1]
                    
                    if rtype == "DOMAIN": rules.append({"domain": [val]})
                    elif rtype == "DOMAIN-SUFFIX": rules.append({"domain_suffix": [val]})
                    elif rtype == "DOMAIN-KEYWORD": rules.append({"domain_keyword": [val]})
                    elif rtype == "DOMAIN-REGEX": rules.append({"domain_regex": [val]})
                    elif rtype == "DOMAIN-WILDCARD": rules.append({"domain_regex": [self._wildcard_to_regex(val)]})
                    elif rtype in ["IP-CIDR", "IP-CIDR6"]: rules.append({"ip_cidr": [val]})
                    elif rtype == "PROCESS-NAME": rules.append({"process_name": [val]})

            merged_rules = {
                "domain": [], "domain_suffix": [], "domain_keyword": [], 
                "domain_regex": [], "ip_cidr": [], "process_name": []
            }
            for r in rules:
                for k, v in r.items(): merged_rules[k].extend(v)
            
            final_rules = {k: sorted(list(set(v))) for k, v in merged_rules.items() if v}

            srs_json = {"version": 1, "rules": [final_rules]}

            # 原子写入 JSON
            json_tmp_write = json_tmp + ".write"
            with open(json_tmp_write, 'w') as f:
                json.dump(srs_json, f)
            os.replace(json_tmp_write, json_tmp)

            result = subprocess.run([
                self.singbox_path, "rule-set", "compile", json_tmp, "-o", srs_file
            ], capture_output=True)

            if result.returncode == 0:
                Logger.success(f"Compiled SRS: {name}")
            else:
                Logger.error(f"Failed to compile {name}: {result.stderr.decode()}")

        except Exception as e:
            Logger.error(f"Error processing {name}: {e}")
        finally:
            if os.path.exists(json_tmp):
                safe_remove(json_tmp)

    @staticmethod
    def _wildcard_to_regex(value: str) -> str:
        escaped = re.escape(value)
        escaped = escaped.replace(r"\*", ".*").replace(r"\?", ".")
        return f"^{escaped}$"

    def run(self):
        Logger.section("Sing-box SRS Generation (English Interface)")
        if not self.singbox_path:
            Logger.error("sing-box binary not found. Skipping SRS generation.")
            return

        source_dirs = [SURGE_DIR, DNS_MAPPING_DIR, os.path.join(ROOT, "rulesets/AdBlock")]
        list_files = []
        for source_dir in source_dirs:
            if not os.path.isdir(source_dir):
                continue
            list_files.extend(
                os.path.join(source_dir, f)
                for f in os.listdir(source_dir)
                if f.endswith(".list")
            )
        list_files.extend(path for path in EXTRA_SOURCE_FILES if os.path.isfile(path))

        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            list(executor.map(self.compile_srs, list_files))
        self.prune_stale_srs(list_files)

    def prune_stale_srs(self, list_files: list):
        expected_srs = {f"{os.path.splitext(os.path.basename(lf))[0]}_Singbox.srs" for lf in list_files}
        
        # Explicitly exempt GeoIP files as they are pre-compiled and tracked
        for fname in os.listdir(SINGBOX_DIR):
            if fname.startswith("GeoIP_") and fname.endswith(".srs"):
                expected_srs.add(fname)

        for fname in sorted(os.listdir(SINGBOX_DIR)):
            if not fname.endswith(".srs"):
                continue
            if fname not in expected_srs:
                path = os.path.join(SINGBOX_DIR, fname)
                if safe_remove(path):
                    Logger.warn(f"Pruned stale SRS: {fname}")
                else:
                    Logger.error(f"Failed to prune stale SRS: {fname}")

if __name__ == "__main__":
    generator = SRSGenerator()
    generator.run()
