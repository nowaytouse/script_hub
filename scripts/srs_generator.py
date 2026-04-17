#!/usr/bin/env python3
import os
import json
import subprocess
import concurrent.futures
from lib.common import Logger, get_project_root

ROOT = get_project_root()
SURGE_DIR = os.path.join(ROOT, "ruleset/Surge(Shadowkroket)")
SINGBOX_DIR = os.path.join(ROOT, "ruleset/SingBox")
CACHE_DIR = os.path.join(ROOT, ".cache")

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
            with open(json_tmp, 'w') as f:
                json.dump(srs_json, f)

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
            if os.path.exists(json_tmp): os.remove(json_tmp)

    def run(self):
        Logger.section("Sing-box SRS Generation (English Interface)")
        if not self.singbox_path:
            Logger.error("sing-box binary not found. Skipping SRS generation.")
            return

        list_files = [os.path.join(SURGE_DIR, f) for f in os.listdir(SURGE_DIR) if f.endswith('.list')]
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            executor.map(self.compile_srs, list_files)

if __name__ == "__main__":
    generator = SRSGenerator()
    generator.run()
