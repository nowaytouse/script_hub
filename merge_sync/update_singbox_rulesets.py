#!/usr/bin/env python3
"""Update Singbox config to remove deleted rulesets"""
import json
import sys

def update_singbox_config(filepath):
    """Remove deleted ruleset references from Singbox config"""
    print(f"Updating: {filepath}")
    
    with open(filepath, 'r', encoding='utf-8') as f:
        config = json.load(f)
    
    # Deleted rulesets (merged into other rulesets)
    deleted = [
        'QQ_Singbox.srs',
        'WeChat_Singbox.srs', 
        'Netflix_Singbox.srs',
        'Disney_Singbox.srs',
        'Bahamut_Singbox.srs',
        'Reddit_Singbox.srs',
        'Discord_Singbox.srs',
        'Fediverse_Singbox.srs',
        'GlobalMedia_Singbox.srs',
        'Twitter_Singbox.srs',
        'Instagram_Singbox.srs',
        'AdBlock_Merged_Singbox.srs',  # Merged into AdBlock
        'BlockHttpDNS_Singbox.srs',    # Standalone, not in Surge RULE-SET
        'ChinaIP_Singbox.srs'          # Used in route config, not RULE-SET
    ]
    
    # Filter out deleted rulesets
    if 'route' in config and 'rule_set' in config['route']:
        original_count = len(config['route']['rule_set'])
        config['route']['rule_set'] = [
            rs for rs in config['route']['rule_set']
            if not any(d in rs.get('url', '') for d in deleted)
        ]
        removed = original_count - len(config['route']['rule_set'])
        print(f"  Removed {removed} deleted ruleset references")
        print(f"  Remaining: {len(config['route']['rule_set'])} rulesets")
    
    # Write back
    with open(filepath, 'w', encoding='utf-8') as f:
        json.dump(config, f, ensure_ascii=False, indent=2)
    
    print(f"  ✅ Updated\n")

if __name__ == '__main__':
    configs = [
        'substore/Singbox_substore_1.13.0+.json',
        '隐私🔏/singbox_config_生成后.json'
    ]
    
    for config in configs:
        try:
            update_singbox_config(config)
        except Exception as e:
            print(f"  ❌ Error: {e}\n")
            sys.exit(1)
    
    print("✅ All Singbox configs updated!")
