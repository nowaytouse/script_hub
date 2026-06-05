#!/usr/bin/env python3
"""
MITM (Man-in-the-Middle) Management Module.

This script scans all generated module files (.sgmodule) and removes specific domains
from the [MITM] hostname list. Instead of using Surge's `-` exclude prefix,
it directly removes the restricted domains from the list, making the configuration
cleaner and more compatible with other clients like Shadowrocket.
"""

import os
import sys
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from hub.project_paths import *

# ============================================================================
# MITM Exclusion List (Exact Domain Matches)
# ============================================================================
RESTRICTED_DOMAINS = {
    'github.com',
    'api.github.com',
    'raw.githubusercontent.com',
    'gist.githubusercontent.com',
    'objects.githubusercontent.com'
}

def _log(msg: str) -> None:
    print(msg, file=sys.stderr if msg.startswith("[!]") else sys.stdout)

def process_file(filepath: str, dry_run: bool = False) -> bool:
    """Remove positive MITM mappings for restricted domains."""
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
    except Exception as e:
        _log(f"[!] Error reading {filepath}: {e}")
        return False

    if '[MITM]' not in content:
        return False

    lines = content.splitlines()
    new_lines = []
    modified = False

    for line in lines:
        stripped = line.strip()
        # Only target hostname lines that are not disabled
        if stripped.startswith('hostname') and not stripped.startswith('hostname-disabled'):
            if '=' not in line:
                new_lines.append(line)
                continue
                
            parts = line.split('=', 1)
            prefix = parts[0]
            val_part = parts[1].strip()
            
            # Handle Surge/Shadowrocket tags
            tag = ""
            for t in ["%APPEND%", "%INSERT%", "%SET%"]:
                if val_part.startswith(t):
                    tag = f"{t} "
                    val_part = val_part[len(t):].strip()
                    break

            # Split domains by comma and clean up
            domains = [d.strip() for d in val_part.split(',') if d.strip()]
            
            # Remove any domains that match the restricted list
            new_domains = []
            for d in domains:
                # Remove Surge's negative prefix if it was already there for a restricted domain
                check_domain = d[1:] if d.startswith('-') else d
                if check_domain.lower() not in RESTRICTED_DOMAINS:
                    new_domains.append(d)
            
            if sorted(new_domains) != sorted(domains):
                modified = True
            
            # If all domains were removed and no tag exists, we could omit the line or leave it empty.
            # Usually, leaving it as 'hostname = %APPEND% ' or 'hostname = ' is safe, but skipping is cleaner if not %APPEND%.
            # Surge allows empty `hostname = ` but to be safe, we just rebuild it.
            if new_domains:
                new_line = f"{prefix}= {tag}{', '.join(new_domains)}"
                new_lines.append(new_line)
            else:
                # If there's a tag like %APPEND%, we must keep the line, else we can skip or write empty.
                if tag:
                    new_lines.append(f"{prefix}= {tag}")
                else:
                    new_lines.append(f"{prefix}=")
        else:
            new_lines.append(line)

    if modified:
        if dry_run:
            print(f"[DRY RUN] Would modify: {filepath}")
        else:
            with open(filepath, 'w', encoding='utf-8') as f:
                f.write('\n'.join(new_lines) + '\n')
        return True
    return False

def run_cleanup(directory: str = None, dry_run: bool = False) -> int:
    """Walk *directory* (default: MODULES_DIR) and harden MITM hostnames."""
    scan_dir = directory if directory else MODULES_DIR
    modified_count = 0
    
    for dirpath, _, files in os.walk(scan_dir):
        for file in files:
            if file.endswith(('.sgmodule', '.module')):
                path = os.path.join(dirpath, file)
                if process_file(path, dry_run):
                    if not dry_run:
                        _log(f"[✓] MITM Cleaned: {path}")
                    modified_count += 1
    return modified_count

def main():
    parser = argparse.ArgumentParser(description="Mass-cleanup of MITM hostnames by stripping exact domains.")
    parser.add_argument("--dir", default=MODULES_DIR, help="Directory to scan (default: modules)")
    parser.add_argument("--dry-run", action="store_true", help="Print changes without saving")
    args = parser.parse_args()

    count = run_cleanup(args.dir, args.dry_run)
    status = "Modification candidates found" if args.dry_run else "Modules modified"
    print(f"\nTotal: {count} {status}")

if __name__ == "__main__":
    main()
