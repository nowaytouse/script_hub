#!/usr/bin/env python3
"""Fix cnip ruleset reference in Singbox configs"""
import json
import sys

def fix_cnip_ruleset(filepath):
    """Add ChinaIP ruleset definition and fix cnip references"""
    print(f"Fixing: {filepath}")
    
    with open(filepath, 'r', encoding='utf-8') as f:
        config = json.load(f)
    
    # Add ChinaIP ruleset definition if not exists
    if 'route' in config and 'rule_set' in config['route']:
        # Check if ChinaIP already exists
        has_chinaip = any('ChinaIP' in rs.get('url', '') for rs in config['route']['rule_set'])
        
        if not has_chinaip:
            chinaip_ruleset = {
                "tag": "surge-chinaip",
                "type": "remote",
                "format": "binary",
                "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/ChinaIP_Singbox.srs",
                "download_detour": "♻️ 自动入口 🧠",
                "update_interval": "24h"
            }
            config['route']['rule_set'].append(chinaip_ruleset)
            print(f"  ✅ Added ChinaIP ruleset definition")
        else:
            print(f"  ℹ️  ChinaIP ruleset already exists")
    
    # Fix route_exclude_address_set reference
    if 'inbounds' in config:
        for inbound in config['inbounds']:
            if 'route_exclude_address_set' in inbound and inbound['route_exclude_address_set'] == 'cnip':
                inbound['route_exclude_address_set'] = 'surge-chinaip'
                print(f"  ✅ Fixed route_exclude_address_set: cnip → surge-chinaip")
    
    # Fix rules reference
    if 'route' in config and 'rules' in config['route']:
        for rule in config['route']['rules']:
            if 'rule_set' in rule and rule['rule_set'] == 'cnip':
                rule['rule_set'] = 'surge-chinaip'
                print(f"  ✅ Fixed rule reference: cnip → surge-chinaip")
    
    # Write back
    with open(filepath, 'w', encoding='utf-8') as f:
        json.dump(config, f, ensure_ascii=False, indent=2)
    
    print(f"  ✅ Config fixed\n")

if __name__ == '__main__':
    configs = [
        'substore/Singbox_substore_1.13.0+.json',
        '隐私🔏/singbox_config_生成后.json'
    ]
    
    for config in configs:
        try:
            fix_cnip_ruleset(config)
        except Exception as e:
            print(f"  ❌ Error: {e}\n")
            sys.exit(1)
    
    print("✅ All Singbox configs fixed!")
