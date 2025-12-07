#!/usr/bin/env python3
"""
修复Manual规则集的tag命名不一致问题
统一使用下划线格式：surge-manual_us, surge-manual_jp, surge-manual_west
"""

import json
from pathlib import Path

# 项目根目录
PROJECT_ROOT = Path(__file__).parent.parent

# Singbox配置文件
SINGBOX_CONFIG = PROJECT_ROOT / "substore" / "Singbox_substore_1.13.0+.json"

def load_singbox_config():
    """加载Singbox配置"""
    with open(SINGBOX_CONFIG, 'r', encoding='utf-8') as f:
        return json.load(f)

def save_singbox_config(config):
    """保存Singbox配置"""
    with open(SINGBOX_CONFIG, 'w', encoding='utf-8') as f:
        json.dump(config, f, ensure_ascii=False, indent=2)

def fix_manual_tags():
    """修复Manual规则集tag"""
    print("📖 加载Singbox配置...")
    config = load_singbox_config()
    
    # Tag映射：连字符 → 下划线
    tag_mapping = {
        'surge-manual-us': 'surge-manual_us',
        'surge-manual-jp': 'surge-manual_jp',
        'surge-manual-west': 'surge-manual_west',
        'surge-manual-global': 'surge-manual_global'
    }
    
    # 修复规则集定义中的tag
    print("\n🔧 修复规则集定义中的tag...")
    fixed_defs = 0
    for rs in config['route']['rule_set']:
        if rs['tag'] in tag_mapping:
            old_tag = rs['tag']
            new_tag = tag_mapping[old_tag]
            rs['tag'] = new_tag
            print(f"   ✅ {old_tag} → {new_tag}")
            fixed_defs += 1
    
    if fixed_defs == 0:
        print("   ℹ️  规则集定义中的tag已正确")
    
    # 修复规则引用中的tag
    print("\n🔧 修复规则引用中的tag...")
    fixed_rules = 0
    for rule in config['route']['rules']:
        if 'rule_set' in rule:
            if isinstance(rule['rule_set'], str):
                if rule['rule_set'] in tag_mapping:
                    old_tag = rule['rule_set']
                    new_tag = tag_mapping[old_tag]
                    rule['rule_set'] = new_tag
                    print(f"   ✅ {old_tag} → {new_tag}")
                    fixed_rules += 1
            elif isinstance(rule['rule_set'], list):
                for i, tag in enumerate(rule['rule_set']):
                    if tag in tag_mapping:
                        old_tag = tag
                        new_tag = tag_mapping[old_tag]
                        rule['rule_set'][i] = new_tag
                        print(f"   ✅ {old_tag} → {new_tag}")
                        fixed_rules += 1
    
    if fixed_rules == 0:
        print("   ℹ️  规则引用中的tag已正确")
    
    # 删除重复的定义
    print("\n🗑️  删除重复的规则集定义...")
    seen_tags = set()
    unique_rulesets = []
    removed = 0
    for rs in config['route']['rule_set']:
        if rs['tag'] not in seen_tags:
            seen_tags.add(rs['tag'])
            unique_rulesets.append(rs)
        else:
            print(f"   ✅ 删除重复: {rs['tag']}")
            removed += 1
    
    config['route']['rule_set'] = unique_rulesets
    
    if removed == 0:
        print("   ℹ️  无重复定义")
    
    # 保存配置
    print("\n💾 保存Singbox配置...")
    save_singbox_config(config)
    
    print("\n✅ Manual规则集tag已修复！")
    print(f"\n📊 修复统计:")
    print(f"   规则集定义修复: {fixed_defs}")
    print(f"   规则引用修复: {fixed_rules}")
    print(f"   删除重复定义: {removed}")

if __name__ == '__main__':
    fix_manual_tags()
