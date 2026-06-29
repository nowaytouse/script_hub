#!/usr/bin/env python3
import os
import json
import subprocess
import concurrent.futures
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import re
from hub.common import Logger, safe_remove
from hub.project_paths import *
import hub.project_paths as project_paths

EXTRA_SOURCE_FILES = [
    os.path.join(ADBLOCK_DIR, "reject-drop.list"),
    os.path.join(ADBLOCK_DIR, "reject-no-drop.list"),
    os.path.join(project_paths.CUSTOM_DIR, "substore.list"),
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
        local_path = project_paths.SINGBOX_LOCAL_BIN
        if os.path.exists(local_path):
            return local_path
        # Also check project root
        root_path = project_paths.SINGBOX_ROOT_BIN
        if os.path.exists(root_path):
            return root_path
        return None

    def compile_srs(self, list_file: str):
        name = os.path.splitext(os.path.basename(list_file))[0]
        
        # Determine output directory based on source location
        if "dns/mapping" in list_file or "/dns/" in list_file:
            out_dir = SINGBOX_DNS_DIR
            os.makedirs(out_dir, exist_ok=True)
        elif "rulesets/AdBlock" in list_file:
            out_dir = project_paths.ADBLOCK_DIR
            os.makedirs(out_dir, exist_ok=True)
        else:
            out_dir = SINGBOX_DIR
            
        srs_file = os.path.join(out_dir, f"{name}_Singbox.srs")
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
            json_tmp_write = json_tmp + ".write"
            if os.path.exists(json_tmp_write):
                safe_remove(json_tmp_write)

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

        source_dirs = [RULE_SET_DIR, DNS_MAPPING_DIR, ADBLOCK_DIR]
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

        # Prune main SingBox directory
        for fname in sorted(os.listdir(SINGBOX_DIR)):
            if not fname.endswith(".srs"):
                continue
            if fname not in expected_srs:
                path = os.path.join(SINGBOX_DIR, fname)
                if safe_remove(path):
                    Logger.warn(f"Pruned stale SRS: {fname}")
                else:
                    Logger.error(f"Failed to prune stale SRS: {fname}")
        
        # Prune DNS directory if it exists
        if os.path.isdir(SINGBOX_DNS_DIR):
            for fname in sorted(os.listdir(SINGBOX_DNS_DIR)):
                if not fname.endswith(".srs"):
                    continue
                if fname not in expected_srs:
                    path = os.path.join(SINGBOX_DNS_DIR, fname)
                    if safe_remove(path):
                        Logger.warn(f"Pruned stale DNS SRS: {fname}")
                    else:
                        Logger.error(f"Failed to prune stale DNS SRS: {fname}")

if __name__ == "__main__":
    generator = SRSGenerator()
    generator.run()
