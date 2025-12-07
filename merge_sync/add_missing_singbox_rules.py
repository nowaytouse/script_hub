#!/usr/bin/env python3
"""
添加Surge中使用但Singbox中缺失的规则集
"""

import json
import re
from pathlib import Path

# 项目根目录
PROJECT_ROOT = Path(__file__).parent.parent

# Surge配置文件
SURGE_CONFIG = PROJECT_ROOT / "conf_template" / "surge_profile_template.conf"

# Singbox配置文件
SINGBOX_CONFIG = PROJECT_ROOT / "substore" / "Singbox_substore_1.13.0+.json"

# Surge规则集名称到Singbox规则集tag的映射
RULESET_MAPPING = {
    "AdBlock_Merged": "surge-adblock-merged",
    "AIProcess": "surge-aiprocess",
    "GamingProcess": "surge-gamingprocess",
    "DirectProcess": "surge-directprocess",
    "DownloadProcess": "surge-downloadprocess",
    "BlockHttpDNS": "surge-blockhttpdns",
    "FirewallPorts": "surge-firewallports",
    "AppleNews": "surge-applenews",
    "Bahamut": "surge-bahamut",
    "StreamEU": "surge-streameu",
    "Binance": "surge-binance",
    "PayPal": "surge-paypal",
    "NetEaseMusic": "surge-neteasemusic",
    "Tencent": "surge-tencent",
    "XiaoHongShu": "surge-xiaohongshu",
    "WeChat": "surge-wechat",
    "Tesla": "surge-tesla",
    "substore": "surge-substore",
    "QQ": "surge-qq",
    "GoogleCN": "surge-googlecn",
    "Reddit": "surge-reddit",
    "SocialMedia": "surge-socialmedia",
    "Epic": "surge-epic",
}

def extract_surge_rules():
    """提取Surge配置中使用的规则集"""
    with open(SURGE_CONFIG, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # 匹配RULE-SET行
    pattern = r'RULE-SET,https://[^,]+/([^/]+)\.list,([^,\n]+)'
    matches = re.findall(pattern, content)
    
    rules = []
    for ruleset_name, policy in matches:
        # 清理策略名称
        policy = policy.strip().strip('"')
        rules.append({
            'ruleset': ruleset_name,
            'policy': policy
        })
    
    return rules

def load_singbox_config():
    """加载Singbox配置"""
    with open(SINGBOX_CONFIG, 'r', encoding='utf-8') as f:
        return json.load(f)

def save_singbox_config(config):
    """保存Singbox配置"""
    with open(SINGBOX_CONFIG, 'w', encoding='utf-8') as f:
        json.dump(config, f, ensure_ascii=False, indent=2)

def get_singbox_tag(surge_ruleset):
    """获取Singbox规则集tag"""
    # 直接映射
    if surge_ruleset in RULESET_MAPPING:
        return RULESET_MAPPING[surge_ruleset]
    
    # 默认转换：小写 + surge- 前缀
    return f"surge-{surge_ruleset.lower()}"

def add_missing_rules():
    """添加缺失的规则到Singbox配置"""
    print("📖 分析Surge配置...")
    surge_rules = extract_surge_rules()
    print(f"   找到 {len(surge_rules)} 个Surge规则集引用")
    
    print("\n📖 加载Singbox配置...")
    config = load_singbox_config()
    
    # 获取现有的规则集引用
    existing_rules = set()
    for rule in config['route']['rules']:
        if 'rule_set' in rule:
            if isinstance(rule['rule_set'], list):
                existing_rules.update(rule['rule_set'])
            else:
                existing_rules.add(rule['rule_set'])
    
    print(f"   现有 {len(existing_rules)} 个规则集引用")
    
    # 查找缺失的规则
    missing_rules = []
    for surge_rule in surge_rules:
        singbox_tag = get_singbox_tag(surge_rule['ruleset'])
        if singbox_tag not in existing_rules:
            missing_rules.append({
                'surge_name': surge_rule['ruleset'],
                'singbox_tag': singbox_tag,
                'policy': surge_rule['policy']
            })
    
    if not missing_rules:
        print("\n✅ 所有Surge规则集都已在Singbox中使用！")
        return
    
    print(f"\n⚠️  发现 {len(missing_rules)} 个缺失的规则集:")
    for rule in missing_rules:
        print(f"   - {rule['surge_name']} → {rule['singbox_tag']} → {rule['policy']}")
    
    # 添加规则到Singbox配置
    print("\n📝 添加缺失的规则...")
    for rule in missing_rules:
        new_rule = {
            "rule_set": rule['singbox_tag'],
            "outbound": rule['policy']
        }
        # 在FINAL规则之前插入
        config['route']['rules'].insert(-1, new_rule)
        print(f"   ✅ 添加: {rule['singbox_tag']} → {rule['policy']}")
    
    # 保存配置
    print("\n💾 保存Singbox配置...")
    save_singbox_config(config)
    
    print(f"\n✅ 成功添加 {len(missing_rules)} 个规则集！")
    print("\n📊 更新后统计:")
    print(f"   规则集引用: {len(existing_rules)} → {len(existing_rules) + len(missing_rules)}")

if __name__ == '__main__':
    add_missing_rules()
