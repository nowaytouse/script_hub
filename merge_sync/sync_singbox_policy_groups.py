#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Singbox策略组同步脚本
功能: 从Surge配置同步策略组到Singbox配置
创建: 2025-12-07
"""

import json
import re
import sys

def parse_surge_policy_groups(surge_config_path):
    """解析Surge配置文件中的策略组"""
    policy_groups = {}
    
    with open(surge_config_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
    
    in_proxy_group = False
    
    for line in lines:
        line = line.strip()
        
        # 检测[Proxy Group]部分
        if line == '[Proxy Group]':
            in_proxy_group = True
            continue
        
        # 检测其他section开始
        if line.startswith('[') and line != '[Proxy Group]':
            in_proxy_group = False
            continue
        
        # 跳过空行和注释
        if not in_proxy_group or not line or line.startswith('#'):
            continue
        
        # 解析策略组定义 - 更宽松的匹配
        if '=' in line:
            parts = line.split('=', 1)
            if len(parts) == 2:
                group_name = parts[0].strip()
                config = parts[1].strip()
                
                # 检测策略组类型
                for group_type in ['select', 'url-test', 'fallback', 'load-balance', 'smart']:
                    if config.startswith(group_type):
                        policy_groups[group_name] = group_type
                        break
    
    return policy_groups

def create_singbox_outbound(name, group_type, default_outbound="🎯 全球直连"):
    """创建Singbox outbound配置"""
    
    # 映射Surge类型到Singbox类型
    type_mapping = {
        'select': 'selector',
        'url-test': 'urltest',
        'fallback': 'urltest',  # Singbox用urltest代替fallback
        'load-balance': 'urltest',
        'smart': 'urltest'
    }
    
    singbox_type = type_mapping.get(group_type, 'selector')
    
    outbound = {
        "type": singbox_type,
        "tag": name,
        "outbounds": [default_outbound]
    }
    
    # 为urltest类型添加测试参数
    if singbox_type == 'urltest':
        outbound.update({
            "url": "http://www.cloudflare.com/generate_204",
            "interval": "3m",
            "tolerance": 30
        })
    else:
        # selector类型添加默认选项
        outbound["default"] = default_outbound
    
    return outbound

def sync_policy_groups(surge_config_path, singbox_config_path, output_path=None):
    """同步策略组"""
    
    print("📖 读取Surge配置...")
    surge_groups = parse_surge_policy_groups(surge_config_path)
    print(f"   找到 {len(surge_groups)} 个Surge策略组")
    
    print("\n📖 读取Singbox配置...")
    with open(singbox_config_path, 'r', encoding='utf-8') as f:
        singbox_config = json.load(f)
    
    # 提取现有的Singbox策略组
    existing_groups = {}
    for outbound in singbox_config.get('outbounds', []):
        if outbound.get('type') in ['selector', 'urltest']:
            existing_groups[outbound['tag']] = outbound
    
    print(f"   找到 {len(existing_groups)} 个Singbox策略组")
    
    # 找出缺失的策略组
    missing_groups = []
    for name, group_type in surge_groups.items():
        if name not in existing_groups:
            missing_groups.append((name, group_type))
    
    if not missing_groups:
        print("\n✅ 所有策略组已同步！")
        return
    
    print(f"\n🔍 发现 {len(missing_groups)} 个缺失的策略组:")
    for name, group_type in missing_groups:
        print(f"   - {name} ({group_type})")
    
    # 添加缺失的策略组
    print("\n➕ 添加缺失的策略组...")
    for name, group_type in missing_groups:
        new_outbound = create_singbox_outbound(name, group_type)
        singbox_config['outbounds'].append(new_outbound)
        print(f"   ✅ 添加: {name}")
    
    # 保存配置
    output_file = output_path or singbox_config_path
    print(f"\n💾 保存配置到: {output_file}")
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(singbox_config, f, ensure_ascii=False, indent=2)
    
    print("\n✅ 策略组同步完成！")
    print(f"   总策略组数: {len(singbox_config['outbounds'])}")

if __name__ == '__main__':
    surge_config = "conf_template/surge_profile_template.conf"
    singbox_config = "substore/Singbox_substore_1.13.0+.json"
    
    if len(sys.argv) > 1:
        surge_config = sys.argv[1]
    if len(sys.argv) > 2:
        singbox_config = sys.argv[2]
    
    try:
        sync_policy_groups(surge_config, singbox_config)
    except Exception as e:
        print(f"\n❌ 错误: {e}", file=sys.stderr)
        sys.exit(1)
