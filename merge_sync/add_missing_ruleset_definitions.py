#!/usr/bin/env python3
"""
添加缺失的规则集定义到Singbox配置
"""

import json
from pathlib import Path

# 项目根目录
PROJECT_ROOT = Path(__file__).parent.parent

# Singbox配置文件
SINGBOX_CONFIG = PROJECT_ROOT / "substore" / "Singbox_substore_1.13.0+.json"

# 缺失的规则集定义
MISSING_RULESETS = [
    {
        "tag": "surge-manual-us",
        "type": "remote",
        "format": "binary",
        "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Manual_US_Singbox.srs",
        "download_detour": "♻️ 自动选择",
        "update_interval": "24h"
    },
    {
        "tag": "surge-manual-west",
        "type": "remote",
        "format": "binary",
        "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Manual_West_Singbox.srs",
        "download_detour": "♻️ 自动选择",
        "update_interval": "24h"
    },
    {
        "tag": "surge-manual-jp",
        "type": "remote",
        "format": "binary",
        "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Manual_JP_Singbox.srs",
        "download_detour": "♻️ 自动选择",
        "update_interval": "24h"
    },
    {
        "tag": "surge-manual_global",
        "type": "remote",
        "format": "binary",
        "url": "https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Manual_Global_Singbox.srs",
        "download_detour": "♻️ 自动选择",
        "update_interval": "24h"
    },
    {
        "tag": "surge-kemono",
        "type": "remote",
        "format": "source",
        "url": "https://whatshub.top/rule/Kemono.list",
        "download_detour": "♻️ 自动选择",
        "update_interval": "24h"
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

def add_missing_definitions():
    """添加缺失的规则集定义"""
    print("📖 加载Singbox配置...")
    config = load_singbox_config()
    
    # 获取现有的规则集定义
    existing_tags = {rs['tag'] for rs in config['route']['rule_set']}
    print(f"   现有 {len(existing_tags)} 个规则集定义")
    
    # 查找缺失的定义
    missing = []
    for ruleset in MISSING_RULESETS:
        if ruleset['tag'] not in existing_tags:
            missing.append(ruleset)
    
    if not missing:
        print("\n✅ 所有规则集定义都已存在！")
        return
    
    print(f"\n⚠️  发现 {len(missing)} 个缺失的规则集定义:")
    for rs in missing:
        print(f"   - {rs['tag']}")
    
    # 添加定义
    print("\n📝 添加缺失的定义...")
    for rs in missing:
        config['route']['rule_set'].append(rs)
        print(f"   ✅ 添加: {rs['tag']}")
    
    # 保存配置
    print("\n💾 保存Singbox配置...")
    save_singbox_config(config)
    
    print(f"\n✅ 成功添加 {len(missing)} 个规则集定义！")
    print(f"\n📊 更新后统计:")
    print(f"   规则集定义: {len(existing_tags)} → {len(existing_tags) + len(missing)}")

if __name__ == '__main__':
    add_missing_definitions()
