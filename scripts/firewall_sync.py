#!/usr/bin/env python3
import os
import re
import argparse
from datetime import datetime
from core.common import (
    Logger, get_project_root, read_file, write_file, 
    extract_section, clean_rules
)

# CONFIGURATION

ROOT = get_project_root()
PORTS_SOURCE = os.path.join(ROOT, "rulesets/Sources/conf/SurgeConf_DirectPorts.list")
FIREWALL_MODULE = os.path.join(ROOT, "modules/surge(main)/head_expanse/🔥 Firewall Port Blocker 🛡️🚫.sgmodule")

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

    # 2. Process module
    module_lines = read_file(FIREWALL_MODULE)
    new_module_content = []
    START_MARKER = "# --- SYNCED PORT RULES START ---"
    END_MARKER = "# --- SYNCED PORT RULES END ---"
    managed_keys = set()
    for rule in port_rules:
        parts = [p.strip().upper() for p in rule.split(',')]
        if len(parts) >= 2:
            managed_keys.add(f"{parts[0]},{parts[1]}")
    
    module_content_str = "".join(module_lines)
    if START_MARKER not in module_content_str:
        Logger.info("Initializing markers at end of [Rule] section.")
        rule_found = False
        in_rule_section = False
        for line in module_lines:
            stripped = line.strip()
            if stripped.startswith("[") and stripped.endswith("]"):
                in_rule_section = (stripped.lower() == "[rule]")
            if stripped.lower() == "[rule]":
                rule_found = True
                new_module_content.append(line)
                new_module_content.append(f"\n{START_MARKER}\n")
                new_module_content.append(f"{END_MARKER}\n")
                continue

            # Remove legacy copies of managed rules so marker sync becomes single source of truth.
            if in_rule_section and stripped and not stripped.startswith('#'):
                parts = [p.strip().upper() for p in stripped.split(',')]
                if len(parts) >= 2:
                    key = f"{parts[0]},{parts[1]}"
                    if key in managed_keys:
                        continue
            new_module_content.append(line)
        if not rule_found:
            Logger.error("Failed to find [Rule] section.")
            return
        module_lines = new_module_content
        new_module_content = []

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
            # Enforce single source of truth: remove managed port rules outside sync markers.
            if in_rule_section and stripped and not stripped.startswith('#'):
                parts = [p.strip().upper() for p in stripped.split(',')]
                if len(parts) >= 2:
                    key = f"{parts[0]},{parts[1]}"
                    if key in managed_keys:
                        continue
            new_module_content.append(line)

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
