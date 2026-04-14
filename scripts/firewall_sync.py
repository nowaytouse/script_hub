#!/usr/bin/env python3
import os
import re
import argparse
from datetime import datetime
from lib.common import (
    Logger, get_project_root, read_file, write_file, 
    extract_section, clean_rules
)

# CONFIGURATION

ROOT = get_project_root()
PORTS_SOURCE = os.path.join(ROOT, "ruleset/Sources/conf/SurgeConf_DirectPorts.list")
FIREWALL_MODULE = os.path.join(ROOT, "module/surge(main)/head_expanse/🔥 Firewall Port Blocker 🛡️🚫.sgmodule")

# SYNC LOGIC

def sync_ports(execute: bool = False):
    Logger.section("Firewall Port Sync (English Script Interface)")

    if not os.path.exists(PORTS_SOURCE):
        Logger.warn(f"Port rules source not found: {PORTS_SOURCE}")
        return

    if not os.path.exists(FIREWALL_MODULE):
        Logger.error(f"Firewall module not found: {FIREWALL_MODULE}")
        return

    # 1. Extract port rules
    source_lines = read_file(PORTS_SOURCE)
    port_rules = []
    port_pattern = re.compile(r'^(IN-PORT|DEST-PORT|SRC-PORT)')
    
    for line in source_lines:
        line = line.strip()
        if not line or line.startswith('#'): continue
        if port_pattern.match(line): port_rules.append(line)

    if not port_rules:
        Logger.warn("No port rules found in source.")
        return

    Logger.info(f"Extracted {len(port_rules)} rules from source.")

    # 2. Process module
    module_lines = read_file(FIREWALL_MODULE)
    new_module_content = []
    START_MARKER = "# --- SYNCED PORT RULES START ---"
    END_MARKER = "# --- SYNCED PORT RULES END ---"
    
    module_content_str = "".join(module_lines)
    if START_MARKER not in module_content_str:
        Logger.info("Initializing markers at end of [Rule] section.")
        rule_found = False
        for line in module_lines:
            new_module_content.append(line)
            if line.strip().lower() == "[rule]":
                rule_found = True
                new_module_content.append(f"\n{START_MARKER}\n")
                new_module_content.append(f"{END_MARKER}\n")
        if not rule_found:
            Logger.error("Failed to find [Rule] section.")
            return
        module_lines = new_module_content
        new_module_content = []

    skip = False
    for line in module_lines:
        if START_MARKER in line:
            new_module_content.append(line)
            new_module_content.append(f"# Automated update from ports source\n")
            new_module_content.append(f"# Updated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")
            for rule in sorted(port_rules): new_module_content.append(f"{rule}\n")
            skip = True
            continue
        if END_MARKER in line:
            skip = False
            new_module_content.append(line)
            continue
        if not skip: new_module_content.append(line)

    final_content = "".join(new_module_content)

    # 3. Apply metadata (Keep Category in Chinese per instructions)
    if execute:
        # Note: We do NOT translate the module's internal comments anymore.
        # We only update the metadata headers to stay professional while keeping the Chinese category.
        final_content = re.sub(r'#!version=.*', f'#!version={datetime.now().strftime("%Y.%m.%d")}', final_content)
        write_file(FIREWALL_MODULE, final_content)
        Logger.success("Firewall module updated successfully (Content preserved).")
    else:
        Logger.info("Dry Run: Use --execute to apply changes.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    sync_ports(execute=args.execute)
