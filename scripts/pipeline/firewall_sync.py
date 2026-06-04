#!/usr/bin/env python3
import os
import re
import argparse
from datetime import datetime
from hub.common import (
    Logger, read_file, write_file, 
    extract_section, clean_rules
)
from hub.project_paths import (
    ROOT,
    SURGE_HEAD_EXPANSE_DIR,
    SHADOWROCKET_HEAD_EXPANSE_DIR,
    SOURCES_DIR,
)

# CONFIGURATION

PORTS_SOURCE = os.path.join(SOURCES_DIR, "conf/SurgeConf_DirectPorts.list")
FIREWALL_MODULES = [
    os.path.join(SURGE_HEAD_EXPANSE_DIR, "🔥 Firewall Port Blocker 🛡️🚫.sgmodule"),
    os.path.join(SHADOWROCKET_HEAD_EXPANSE_DIR, "🔥 Firewall Port Blocker 🛡️🚫.module"),
]

# SYNC LOGIC

def sync_ports(execute: bool = False):
    Logger.section("Firewall Port Sync (English Script Interface)")

    if not os.path.exists(PORTS_SOURCE):
        Logger.warn(f"Port rules source not found: {PORTS_SOURCE}")
        return

    # 1. Extract port rules
    source_lines = read_file(PORTS_SOURCE)
    port_rules = []
    port_pattern = re.compile(r'^(IN-PORT|DEST-PORT|SRC-PORT)')
    invalid_rules = []
    
    for line in source_lines:
        line = line.strip()
        if not line or line.startswith('#'): continue
        if port_pattern.match(line):
            # Validate rule has action
            parts = line.split(',')
            if len(parts) < 3:
                invalid_rules.append(line)
                Logger.warn(f"  Invalid rule (missing action): {line}")
            else:
                port_rules.append(line)

    if invalid_rules:
        Logger.error(f"Found {len(invalid_rules)} invalid rules (missing action)")
        for rule in invalid_rules:
            Logger.error(f"  - {rule}")
        return

    if not port_rules:
        Logger.warn("No port rules found in source.")
        return

    Logger.info(f"Extracted {len(port_rules)} valid rules from source.")


    for module_path in FIREWALL_MODULES:
        if not os.path.exists(module_path):
            Logger.warn(f"Firewall module not found: {module_path}")
            continue

        module_lines = read_file(module_path)
        new_module_content = []
        START_MARKER = "# --- SYNCED PORT RULES START ---"
        END_MARKER = "# --- SYNCED PORT RULES END ---"
        managed_keys = set()
        for rule in port_rules:
            parts = [p.strip().upper() for p in rule.split(',')]
            if len(parts) >= 2:
                managed_keys.add(f"{parts[0]},{parts[1]}")
        
        skip = False
        in_rule_section = False
        for line in module_lines:
            stripped = line.strip()
            if stripped.startswith("[") and stripped.endswith("]"):
                in_rule_section = (stripped.lower() == "[rule]")
            if START_MARKER in line:
                new_module_content.append(line)
                new_module_content.append(f"# Automated update from ports source\n")
                for rule in sorted(port_rules): new_module_content.append(f"{rule}\n")
                skip = True
                continue
            if END_MARKER in line:
                skip = False
                new_module_content.append(line)
                continue
            if not skip:
                if in_rule_section and stripped and not stripped.startswith('#'):
                    parts = [p.strip().upper() for p in stripped.split(',')]
                    if len(parts) >= 2:
                        key = f"{parts[0]},{parts[1]}"
                        if key in managed_keys:
                            continue
                new_module_content.append(line)

        final_content = "".join(new_module_content)

        if execute:
            final_content = re.sub(r'#!version=.*', f'#!version={datetime.now().strftime("%Y.%m.%d")}', final_content)
            write_file(module_path, final_content)
            Logger.success(f"Firewall module updated successfully: {os.path.basename(module_path)}")
        else:
            Logger.info(f"Dry Run: Use --execute to apply changes to {os.path.basename(module_path)}.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    sync_ports(execute=args.execute)
