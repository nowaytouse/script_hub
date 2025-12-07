#!/usr/bin/env python3
"""
将未使用的规则集添加到Surge配置中
"""

from pathlib import Path

# 项目根目录
PROJECT_ROOT = Path(__file__).parent.parent

# Surge配置文件
SURGE_CONFIG = PROJECT_ROOT / "conf_template" / "surge_profile_template.conf"

# 未使用的规则集及其应该使用的策略
UNUSED_RULESETS = {
    'BlockHttpDNS': {
        'policy': 'REJECT',
        'position': 'after_adblock',  # 在AdBlock之后
        'description': 'HTTP DNS劫持屏蔽'
    },
    'FirewallPorts': {
        'policy': '❌ 拒绝屏蔽',
        'position': 'after_adblock',  # 在AdBlock之后
        'description': '防火墙端口屏蔽'
    },
    'Reddit': {
        'policy': '🌍 海外通用 🌍',
        'position': 'social_media',  # 社交媒体区域
        'description': 'Reddit社交平台'
    },
    'SocialMedia': {
        'policy': '🌍 海外通用 🌍',
        'position': 'social_media',  # 社交媒体区域
        'description': '社交媒体通用规则'
    }
}

def add_rules_to_surge():
    """添加规则到Surge配置"""
    print("📖 读取Surge配置...")
    with open(SURGE_CONFIG, 'r', encoding='utf-8') as f:
        lines = f.readlines()
    
    # 查找插入位置
    adblock_line = -1
    social_media_line = -1
    
    for i, line in enumerate(lines):
        if 'AdBlock_Merged.list' in line:
            adblock_line = i
        if 'Instagram.list' in line or 'Twitter.list' in line:
            if social_media_line == -1:
                social_media_line = i
    
    print(f"   AdBlock位置: 第{adblock_line + 1}行")
    print(f"   社交媒体位置: 第{social_media_line + 1}行")
    
    # 准备要添加的规则
    rules_to_add = []
    
    # 1. BlockHttpDNS - 在AdBlock之后
    if adblock_line > 0:
        rules_to_add.append({
            'line': adblock_line + 1,
            'content': 'RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/BlockHttpDNS.list,REJECT,extended-matching,no-resolve\n',
            'name': 'BlockHttpDNS'
        })
    
    # 2. FirewallPorts - 在BlockHttpDNS之后
    if adblock_line > 0:
        rules_to_add.append({
            'line': adblock_line + 2,
            'content': 'RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/FirewallPorts.list,"❌ 拒绝屏蔽",extended-matching,no-resolve\n',
            'name': 'FirewallPorts'
        })
    
    # 3. Reddit - 在社交媒体区域
    if social_media_line > 0:
        rules_to_add.append({
            'line': social_media_line,
            'content': 'RULE-SET,https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Reddit/Reddit.list,"🌍 海外通用 🌍",extended-matching,no-resolve\n',
            'name': 'Reddit'
        })
    
    # 4. SocialMedia - 在Reddit之后
    if social_media_line > 0:
        rules_to_add.append({
            'line': social_media_line + 1,
            'content': 'RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/SocialMedia.list,"🌍 海外通用 🌍",extended-matching,no-resolve\n',
            'name': 'SocialMedia'
        })
    
    # 按行号倒序插入（避免行号变化）
    rules_to_add.sort(key=lambda x: x['line'], reverse=True)
    
    print(f"\n📝 添加 {len(rules_to_add)} 个规则到Surge配置...")
    for rule in rules_to_add:
        lines.insert(rule['line'], rule['content'])
        print(f"   ✅ 添加: {rule['name']} (第{rule['line'] + 1}行)")
    
    # 保存配置
    print("\n💾 保存Surge配置...")
    with open(SURGE_CONFIG, 'w', encoding='utf-8') as f:
        f.writelines(lines)
    
    print("\n✅ 规则已添加到Surge配置！")
    print("\n📋 添加的规则:")
    for ruleset, info in UNUSED_RULESETS.items():
        print(f"   - {ruleset}: {info['description']} → {info['policy']}")

if __name__ == '__main__':
    add_rules_to_surge()
