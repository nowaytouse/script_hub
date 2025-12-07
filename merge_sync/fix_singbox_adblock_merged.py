#!/usr/bin/env python3
"""
Fix Singbox AdBlock-Merged References
Problem: Config references 'surge-adblock-merged' which doesn't exist
Solution: Remove the invalid rule reference
"""
import json
import sys
import os

def fix_singbox_config(filepath):
    """Remove invalid rule-set references from Singbox config"""
    print(f"Fixing: {filepath}")
    
    if not os.path.exists(filepath):
        print(f"  ⚠️  File not found, skipping")
        return
    
    with open(filepath, 'r', encoding='utf-8') as f:
        config = json.load(f)
    
    # Invalid rule-set references (not in Surge RULE-SET)
    invalid_rulesets = [
        'surge-adblock-merged',  # Merged into surge-adblock
        'surge-blockhttpdns',    # Not in Surge RULE-SET, standalone module
        'surge-chinaip'          # Used in route config, not RULE-SET
    ]
    
    # Check route.rules for invalid references
    if 'route' in config and 'rules' in config['route']:
        original_count = len(config['route']['rules'])
        
        # Remove rules that reference invalid rule-sets
        config['route']['rules'] = [
            rule for rule in config['route']['rules']
            if rule.get('rule_set') not in invalid_rulesets
        ]
        
        removed = original_count - len(config['route']['rules'])
        if removed > 0:
            print(f"  ✅ Removed {removed} invalid rule-set reference(s)")
            print(f"  📊 Remaining rules: {len(config['route']['rules'])}")
        else:
            print(f"  ℹ️  No invalid references found")
    
    # Write back
    with open(filepath, 'w', encoding='utf-8') as f:
        json.dump(config, f, ensure_ascii=False, indent=2)
    
    print(f"  ✅ Config fixed\n")

def main():
    """Fix all Singbox configs"""
    configs = [
        'substore/Singbox_substore_1.13.0+.json',
        '隐私🔏/singbox_config_生成后.json'
    ]
    
    print("=" * 60)
    print("Singbox AdBlock-Merged Reference Fix")
    print("=" * 60)
    print()
    
    for config in configs:
        try:
            fix_singbox_config(config)
        except Exception as e:
            print(f"  ❌ Error: {e}\n")
            sys.exit(1)
    
    print("=" * 60)
    print("✅ All Singbox configs fixed!")
    print("=" * 60)
    print()
    print("Removed invalid rule-set references:")
    print("  • surge-adblock-merged  → Merged into surge-adblock")
    print("  • surge-blockhttpdns    → Not in Surge RULE-SET (standalone)")
    print("  • surge-chinaip         → Used in route config, not RULE-SET")

if __name__ == '__main__':
    main()
