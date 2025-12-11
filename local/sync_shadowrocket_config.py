#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Shadowrocket Configuration Sync Script
Syncs RULE-SET entries from Surge config to Shadowrocket SQLite database

Database Schema (Shadowrocket):
  - Table: config_content (FTS3 virtual table)
  - Columns: c0section, c1name, c2value, c3option, c4ext, c5remarks, c6created

Created: 2025-12-07
"""

import argparse
import os
import re
import sqlite3
import sys
from datetime import datetime


def parse_surge_rules(surge_config_path):
    """Parse RULE-SET entries from Surge config file"""
    rules = []
    
    with open(surge_config_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
    
    in_rule_section = False
    
    for line in lines:
        line = line.strip()
        
        # Detect [Rule] section
        if line == '[Rule]':
            in_rule_section = True
            continue
        
        # Detect other sections
        if line.startswith('[') and line != '[Rule]':
            in_rule_section = False
            continue
        
        # Skip empty lines and comments
        if not in_rule_section or not line or line.startswith('#'):
            continue
        
        # Parse RULE-SET entries
        # Format: RULE-SET,URL,POLICY,options...
        if line.startswith('RULE-SET,'):
            parts = line.split(',')
            if len(parts) >= 3:
                url = parts[1].strip()
                policy = parts[2].strip()
                
                # Skip built-in rules (SYSTEM, LAN)
                if url in ['SYSTEM', 'LAN']:
                    continue
                
                # Collect options (everything after policy)
                options = []
                for opt in parts[3:]:
                    opt = opt.strip()
                    if opt:
                        options.append(opt)
                
                # Build option string for Shadowrocket
                # Format: POLICY,pre-matching,extended-matching,update-interval=86400,no-resolve
                option_str = policy
                if options:
                    option_str += ',' + ','.join(options)
                
                rules.append({
                    'url': url,
                    'policy': policy,
                    'options': option_str,
                    'raw': line
                })
    
    return rules


def get_shadowrocket_rules(db_path):
    """Get existing RULE-SET entries from Shadowrocket database"""
    rules = {}
    
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # Query config_content table for Rule entries
    cursor.execute("""
        SELECT docid, c0section, c1name, c2value, c3option, c5remarks
        FROM config_content
        WHERE c0section = 'Rule' AND c1name = 'RULE-SET'
    """)
    
    for row in cursor.fetchall():
        docid, section, name, url, option, remarks = row
        rules[url] = {
            'docid': docid,
            'section': section,
            'name': name,
            'url': url,
            'option': option,
            'remarks': remarks
        }
    
    conn.close()
    return rules


def get_max_docid(db_path):
    """Get maximum docid from config_content table"""
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    cursor.execute("SELECT MAX(docid) FROM config_content")
    result = cursor.fetchone()[0]
    conn.close()
    return result or 0


def sync_rules(surge_config_path, db_path, dry_run=False, verbose=False):
    """Sync rules from Surge config to Shadowrocket database"""
    
    print("=" * 60)
    print("Shadowrocket Configuration Sync")
    print("=" * 60)
    
    # Parse Surge rules
    print("\n[1/4] Reading Surge configuration...")
    surge_rules = parse_surge_rules(surge_config_path)
    print(f"      Found {len(surge_rules)} RULE-SET entries")
    
    # Get existing Shadowrocket rules
    print("\n[2/4] Reading Shadowrocket database...")
    sr_rules = get_shadowrocket_rules(db_path)
    print(f"      Found {len(sr_rules)} existing RULE-SET entries")
    
    # Calculate differences
    print("\n[3/4] Calculating differences...")
    
    surge_urls = {r['url'] for r in surge_rules}
    sr_urls = set(sr_rules.keys())
    
    to_add = []
    to_update = []
    to_delete = []
    
    # Find rules to add or update
    for rule in surge_rules:
        url = rule['url']
        if url not in sr_urls:
            to_add.append(rule)
        else:
            # Check if options changed
            existing = sr_rules[url]
            if existing['option'] != rule['options']:
                to_update.append({
                    'docid': existing['docid'],
                    'url': url,
                    'old_option': existing['option'],
                    'new_option': rule['options']
                })
    
    # Find rules to delete (in Shadowrocket but not in Surge)
    # Skip built-in rules and local rules
    skip_delete = {'SYSTEM', 'LAN'}
    for url in sr_urls:
        if url not in surge_urls and url not in skip_delete:
            to_delete.append(sr_rules[url])
    
    # Report differences
    print(f"\n      Rules to ADD:    {len(to_add)}")
    print(f"      Rules to UPDATE: {len(to_update)}")
    print(f"      Rules to DELETE: {len(to_delete)}")
    
    if verbose:
        if to_add:
            print("\n      [ADD]")
            for r in to_add[:10]:  # Show first 10
                print(f"        + {r['url'][:60]}...")
            if len(to_add) > 10:
                print(f"        ... and {len(to_add) - 10} more")
        
        if to_update:
            print("\n      [UPDATE]")
            for r in to_update[:5]:
                print(f"        ~ {r['url'][:60]}...")
        
        if to_delete:
            print("\n      [DELETE]")
            for r in to_delete[:5]:
                print(f"        - {r['url'][:60]}...")
    
    # Check if any changes needed
    if not to_add and not to_update and not to_delete:
        print("\n[4/4] No changes needed - already in sync!")
        return True
    
    # Apply changes
    print(f"\n[4/4] {'[DRY-RUN] Would apply' if dry_run else 'Applying'} changes...")
    
    if dry_run:
        print("      (No actual changes made)")
        return True
    
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    try:
        # Get next docid
        next_docid = get_max_docid(db_path) + 1
        
        # Add new rules
        for rule in to_add:
            cursor.execute("""
                INSERT INTO config_content 
                (docid, c0section, c1name, c2value, c3option, c4ext, c5remarks, c6created)
                VALUES (?, 'Rule', 'RULE-SET', ?, ?, '', '', ?)
            """, (next_docid, rule['url'], rule['options'], datetime.now().isoformat()))
            next_docid += 1
        
        # Update existing rules
        for rule in to_update:
            cursor.execute("""
                UPDATE config_content
                SET c3option = ?
                WHERE docid = ?
            """, (rule['new_option'], rule['docid']))
        
        # Delete removed rules
        for rule in to_delete:
            cursor.execute("""
                DELETE FROM config_content
                WHERE docid = ?
            """, (rule['docid'],))
        
        conn.commit()
        print(f"      Added:   {len(to_add)} rules")
        print(f"      Updated: {len(to_update)} rules")
        print(f"      Deleted: {len(to_delete)} rules")
        
    except Exception as e:
        conn.rollback()
        print(f"\n      ERROR: {e}")
        return False
    finally:
        conn.close()
    
    print("\n" + "=" * 60)
    print("Sync completed successfully!")
    print("=" * 60)
    
    return True


def find_shadowrocket_db():
    """Find Shadowrocket database in iCloud"""
    icloud_base = os.path.expanduser(
        "~/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents"
    )
    
    if not os.path.exists(icloud_base):
        return None
    
    # Find .db files
    for filename in os.listdir(icloud_base):
        if filename.endswith('.db') and 'SURGE' in filename.upper():
            return os.path.join(icloud_base, filename)
    
    # If no SURGE-related db found, look for any .db file
    for filename in os.listdir(icloud_base):
        if filename.endswith('.db'):
            return os.path.join(icloud_base, filename)
    
    return None


def get_default_paths():
    """Get default configuration file paths"""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    repo_root = os.path.dirname(os.path.dirname(script_dir))
    
    # Surge rules source
    surge_config = os.path.join(repo_root, "ruleset/Sources/surge_rules_complete.conf")
    
    # Shadowrocket database
    sr_db = find_shadowrocket_db()
    
    return surge_config, sr_db


if __name__ == '__main__':
    default_surge, default_sr_db = get_default_paths()
    
    parser = argparse.ArgumentParser(
        description='Sync RULE-SET entries from Surge config to Shadowrocket database',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog='''
Examples:
  %(prog)s                           # Use default paths
  %(prog)s --dry-run                 # Simulate, don't modify database
  %(prog)s -s surge.conf -d sr.db    # Specify paths
  %(prog)s --verbose                 # Show detailed changes
        '''
    )
    
    parser.add_argument('-s', '--surge',
                        default=default_surge,
                        help=f'Surge config file path (default: {default_surge})')
    parser.add_argument('-d', '--database',
                        default=default_sr_db,
                        help=f'Shadowrocket database path (default: auto-detect)')
    parser.add_argument('-n', '--dry-run',
                        action='store_true',
                        help='Simulate sync without modifying database')
    parser.add_argument('-v', '--verbose',
                        action='store_true',
                        help='Show detailed information')
    
    args = parser.parse_args()
    
    # Validate paths
    if not os.path.exists(args.surge):
        print(f"ERROR: Surge config not found: {args.surge}", file=sys.stderr)
        sys.exit(1)
    
    if not args.database:
        print("ERROR: Shadowrocket database not found in iCloud", file=sys.stderr)
        print("       Please specify path with -d option", file=sys.stderr)
        sys.exit(1)
    
    if not os.path.exists(args.database):
        print(f"ERROR: Shadowrocket database not found: {args.database}", file=sys.stderr)
        sys.exit(1)
    
    if args.verbose:
        print(f"Surge config:  {args.surge}")
        print(f"SR database:   {args.database}")
        if args.dry_run:
            print("Mode:          DRY-RUN (no changes)")
        print()
    
    try:
        success = sync_rules(args.surge, args.database, args.dry_run, args.verbose)
        sys.exit(0 if success else 1)
    except Exception as e:
        print(f"\nERROR: {e}", file=sys.stderr)
        sys.exit(1)
