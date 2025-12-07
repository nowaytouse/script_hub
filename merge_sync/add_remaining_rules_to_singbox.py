#!/usr/bin/env python3
"""
将剩余4个规则集添加到Singbox配置中
"""

import json
from pathlib import Path

# 项目根目录
PROJECT_ROOT = Path(__file__).parent.parent

# Singbox配置文件
SINGBOX_CONFIG = PROJECT_ROOT / "substore" / "Singbox_substore_1.13.0+.json"

# 要添加的规则
RULES_TO_ADD = [
    {
        'rule_set': 'surge-blockhttpdns',
        'outbound': '❌ 拒绝屏蔽',
        'position': 'after_adblock'
    },
    {
        'rule_set': 'surge-firewallports',
        'outbound': '❌ 拒绝屏蔽',
        'position': 'after_adblock'
    },
    {
        'rule_set': 'surge-reddit',
        'outbound': '🌍 海外通用 🌍',
        'position': 'social_media'
    },
    {
        'rule_set': 'surge-socialmedia',
        'outbound': '🌍 海外通用 🌍',
        'position': 'social_media'
    }
]

def load_singbox_config():
    """加载Singbox配置"""
    with open(SINGBOX_CONFIG, 'r', encoding='utf-8') as f:
        return json.load(f)

def save_singbox_config(config):
    """保存Singbox配置"""
    with open(SINGBOX_CONFIG, 'w', encoding='utf-8') as f:
        json.dump(config, f, ensure_ascii=False, indent=2)

def add_rules():
    """添加规则到Singbox配置"""
    print("📖 加载Singbox配置...")
    config = load_singbox_config()
    
    rules = config['route']['rules']
    
    # 查找插入位置
    adblock_index = -1
    instagram_index = -1
    
    for i, rule in enumerate(rules):
        if 'rule_set' in rule:
            rule_set = rule['rule_set']
            if isinstance(rule_set, list):
                if 'surge-adblock' in rule_set:
                    adblock_index = i
                if 'surge-instagram' in rule_set and instagram_index == -1:
                    instagram_index = i
            elif rule_set == 'surge-adblock':
                adblock_index = i
            elif rule_set == 'surge-instagram' and instagram_index == -1:
                instagram_index = i
    
    print(f"   AdBlock位置: 索引{adblock_index}")
    print(f"   Instagram位置: 索引{instagram_index}")
    
    # 添加规则
    print(f"\n📝 添加 {len(RULES_TO_ADD)} 个规则...")
    added = 0
    
    # 1. 在AdBlock之后添加BlockHttpDNS和FirewallPorts
    if adblock_index >= 0:
        # BlockHttpDNS
        rules.insert(adblock_index + 1, {
            'rule_set': 'surge-blockhttpdns',
            'outbound': '❌ 拒绝屏蔽'
        })
        print(f"   ✅ 添加: surge-blockhttpdns → ❌ 拒绝屏蔽")
        added += 1
        
        # FirewallPorts
        rules.insert(adblock_index + 2, {
            'rule_set': 'surge-firewallports',
            'outbound': '❌ 拒绝屏蔽'
        })
        print(f"   ✅ 添加: surge-firewallports → ❌ 拒绝屏蔽")
        added += 1
    
    # 2. 在Instagram之前添加Reddit和SocialMedia
    if instagram_index >= 0:
        # 调整索引（因为前面添加了2个规则）
        instagram_index += 2
        
        # Reddit
        rules.insert(instagram_index, {
            'rule_set': 'surge-reddit',
            'outbound': '🌍 海外通用 🌍'
        })
        print(f"   ✅ 添加: surge-reddit → 🌍 海外通用 🌍")
        added += 1
        
        # SocialMedia
        rules.insert(instagram_index + 1, {
            'rule_set': 'surge-socialmedia',
            'outbound': '🌍 海外通用 🌍'
        })
        print(f"   ✅ 添加: surge-socialmedia → 🌍 海外通用 🌍")
        added += 1
    
    # 保存配置
    print("\n💾 保存Singbox配置...")
    save_singbox_config(config)
    
    print(f"\n✅ 成功添加 {added} 个规则！")
    print(f"\n📊 更新后统计:")
    print(f"   路由规则数: {len(rules) - added} → {len(rules)}")

if __name__ == '__main__':
    add_rules()
