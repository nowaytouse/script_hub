#!/usr/bin/env python3
"""
从Singbox中删除FirewallPorts规则（只能在Surge/小火箭模块中使用）
"""

import json
from pathlib import Path

PROJECT_ROOT = Path(__file__).parent.parent
SINGBOX_CONFIG = PROJECT_ROOT / "substore" / "Singbox_substore_1.13.0+.json"

def load_config():
    with open(SINGBOX_CONFIG, 'r', encoding='utf-8') as f:
        return json.load(f)

def save_config(config):
    with open(SINGBOX_CONFIG, 'w', encoding='utf-8') as f:
        json.dump(config, f, ensure_ascii=False, indent=2)

def remove_firewall():
    print("📖 加载Singbox配置...")
    config = load_config()
    
    # 删除FirewallPorts规则集定义
    print("\n🗑️  删除FirewallPorts规则集定义...")
    original_defs = len(config['route']['rule_set'])
    config['route']['rule_set'] = [
        rs for rs in config['route']['rule_set']
        if rs['tag'] != 'surge-firewallports'
    ]
    removed_defs = original_defs - len(config['route']['rule_set'])
    print(f"   ✅ 删除了 {removed_defs} 个定义")
    
    # 删除FirewallPorts规则引用
    print("\n🗑️  删除FirewallPorts规则引用...")
    original_rules = len(config['route']['rules'])
    config['route']['rules'] = [
        rule for rule in config['route']['rules']
        if not ('rule_set' in rule and rule['rule_set'] == 'surge-firewallports')
    ]
    removed_rules = original_rules - len(config['route']['rules'])
    print(f"   ✅ 删除了 {removed_rules} 个引用")
    
    print("\n💾 保存配置...")
    save_config(config)
    
    print("\n✅ FirewallPorts已从Singbox中删除（只能在Surge/小火箭模块中使用）")

if __name__ == '__main__':
    remove_firewall()
