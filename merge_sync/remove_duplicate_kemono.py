#!/usr/bin/env python3
"""
删除重复的Kemono规则集（已包含在NSFW中）
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

def remove_kemono():
    """删除Kemono规则集（重复，已在NSFW中）"""
    print("📖 加载Singbox配置...")
    config = load_singbox_config()
    
    # 删除Kemono规则集定义
    print("\n🗑️  删除Kemono规则集定义...")
    original_count = len(config['route']['rule_set'])
    config['route']['rule_set'] = [
        rs for rs in config['route']['rule_set']
        if rs['tag'] != 'surge-kemono'
    ]
    removed_defs = original_count - len(config['route']['rule_set'])
    if removed_defs > 0:
        print(f"   ✅ 删除了 {removed_defs} 个Kemono规则集定义")
    else:
        print("   ℹ️  未找到Kemono规则集定义")
    
    # 删除Kemono规则引用
    print("\n🗑️  删除Kemono规则引用...")
    original_rules = len(config['route']['rules'])
    config['route']['rules'] = [
        rule for rule in config['route']['rules']
        if not (
            'rule_set' in rule and 
            (rule['rule_set'] == 'surge-kemono' or 
             (isinstance(rule['rule_set'], list) and 'surge-kemono' in rule['rule_set']))
        )
    ]
    removed_rules = original_rules - len(config['route']['rules'])
    if removed_rules > 0:
        print(f"   ✅ 删除了 {removed_rules} 个Kemono规则引用")
    else:
        print("   ℹ️  未找到Kemono规则引用")
    
    # 保存配置
    print("\n💾 保存Singbox配置...")
    save_singbox_config(config)
    
    print("\n✅ Kemono规则集已删除（已包含在NSFW规则集中）")
    print(f"\n📊 更新后统计:")
    print(f"   规则集定义: {original_count} → {len(config['route']['rule_set'])}")
    print(f"   路由规则: {original_rules} → {len(config['route']['rules'])}")

if __name__ == '__main__':
    remove_kemono()
