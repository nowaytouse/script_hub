#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
同步 Sing-box 配置与 Surge 配置
"""

import json
import sys

def main():
    # 读取原始配置文件
    with open('substore/Singbox_substore_1.13.0+.json', 'r', encoding='utf-8') as f:
        config = json.load(f)

    changes_made = []

    # 1. 更新 FakeIP 范围
    for server in config['dns']['servers']:
        if server.get('tag') == 'fake_dns':
            server['inet4_range'] = '28.0.0.0/8'
            server['inet6_range'] = 'fc00::/18'
            changes_made.append("FakeIP范围已更新")

    # 2. 更新 DNS 服务器的 detour
    dns_updated = 0
    for server in config['dns']['servers']:
        if server.get('detour') == '♻️ 自动入口 🧠':
            server['detour'] = '🌍 海外通用 🌍'
            dns_updated += 1
    if dns_updated > 0:
        changes_made.append(f"DNS detour已更新 ({dns_updated}处)")

    # 3. 更新 route_exclude_address
    new_exclude_addresses = [
        "10.0.0.0/8", "192.168.0.0/16", "172.16.0.0/12", "127.0.0.0/8",
        "169.254.0.0/16", "224.0.0.0/4", "240.0.0.0/4", "255.255.255.255/32",
        "100.64.0.0/10", "fd6f:d1dc:e54f::/48", "2001:b28::/32", "fc00::/7",
        "fe80::/10", "ff00::/8", "::1/128", "1.1.1.1/32", "1.0.0.1/32",
        "8.8.8.8/32", "8.8.4.4/32", "9.9.9.9/32", "9.9.9.11/32",
        "149.112.112.112/32", "94.140.14.14/32", "94.140.15.15/32",
        "208.67.222.222/32", "208.67.220.220/32", "223.5.5.5/32", "223.6.6.6/32",
        "119.29.29.29/32", "180.76.76.76/32", "114.114.114.114/32",
        "45.90.28.0/24", "45.90.30.0/24", "2606:4700:4700::1111/128",
        "2606:4700:4700::1001/128", "2001:4860:4860::8888/128",
        "2001:4860:4860::8844/128", "2620:fe::fe/128", "2620:fe::9/128"
    ]

    for inbound in config['inbounds']:
        if 'route_exclude_address' in inbound:
            inbound['route_exclude_address'] = new_exclude_addresses
            changes_made.append("route_exclude_address已更新")

    # 4. 更新 rule_set 的 download_detour
    ruleset_updated = 0
    for rule_set in config['route']['rule_set']:
        if rule_set.get('download_detour') == '♻️ 自动入口 🧠':
            rule_set['download_detour'] = '🌍 海外通用 🌍'
            ruleset_updated += 1
    if ruleset_updated > 0:
        changes_made.append(f"rule_set download_detour已更新 ({ruleset_updated}处)")

    # 5. 更新策略组
    for outbound in config['outbounds']:
        tag = outbound.get('tag', '')
        
        if tag == '🐟 漏网之鱼 🕸️':
            outbound['outbounds'] = ['♻️ 自动入口 🧠', '🚫 漏网绝杀 🕸️', '🗺️ 直连通用 🌏', '🌍 海外通用 🌍', '🔗 自动回退 🏁']
            changes_made.append("🐟 漏网之鱼 🕸️ 已更新")
        
        elif tag == ' ▶️  YouTube 🔴' or tag == '▶️  YouTube 🔴':
            outbound['tag'] = '▶️  YouTube 🔴'
            outbound['outbounds'] = ['🇺🇸 西方 🇫🇷', '🇯🇵 JP 🇯🇵', '🇸🇬 亚洲 🇰🇷', '🇬🇧 UK 🇬🇧', '🇭🇰 港澳台 🇲🇴', '🇺🇸 美国 🇺🇸', '🇭🇰 香港 🇭🇰', '🇹🇼 台湾 🇹🇼', '🇸🇬 新加坡 🇸🇬', '🇰🇷 韩国 🇰🇷', '🇲🇴 澳门 🇲🇴', '🗺️ 直连通用 🌏']
            outbound['default'] = '🇺🇸 西方 🇫🇷'
            changes_made.append("▶️  YouTube 🔴 已更新")
        
        elif tag == '📱 TikTok 🧠':
            outbound['outbounds'] = ['🇰🇷 韩国 🇰🇷', '🇯🇵 JP 🇯🇵', '🇺🇸 西方 🇫🇷', '🇸🇬 亚洲 🇰🇷', '🇬🇧 UK 🇬🇧', '🇺🇸 美国 🇺🇸', '🇸🇬 新加坡 🇸🇬', '🇹🇼 台湾 🇹🇼', '🇭🇰 香港 🇭🇰', '🇲🇴 澳门 🇲🇴', '🗺️ 直连通用 🌏']
            outbound['default'] = '🇰🇷 韩国 🇰🇷'
            changes_made.append("📱 TikTok 🧠 已更新")
        
        elif tag == '🔊  Spotify  🟢':
            outbound['outbounds'] = ['🇺🇸 西方 🇫🇷', '🇯🇵 JP 🇯🇵', '🇸🇬 亚洲 🇰🇷', '🇬🇧 UK 🇬🇧', '🇭🇰 港澳台 🇲🇴', '🇺🇸 美国 🇺🇸', '🇭🇰 香港 🇭🇰', '🇹🇼 台湾 🇹🇼', '🇸🇬 新加坡 🇸🇬', '🇰🇷 韩国 🇰🇷', '🇲🇴 澳门 🇲🇴', '🗺️ 直连通用 🌏']
            outbound['default'] = '🇺🇸 西方 🇫🇷'
            changes_made.append("🔊  Spotify  🟢 已更新")
        
        elif tag == '🌍 海外通用 🌍':
            outbound['outbounds'] = ['🕳️ 落地节点 🔐 +', '🇭🇰 港澳台 🇲🇴', '🇺🇸 西方 🇫🇷', '🇸🇬 亚洲 🇰🇷', '🗺️ 中国大陆 🇨🇳', '🇯🇵 JP 🇯🇵', '🇬🇧 UK 🇬🇧', '🇺🇸 美国 🇺🇸', '🇭🇰 香港 🇭🇰', '🇲🇴 澳门 🇲🇴', '🇹🇼 台湾 🇹🇼', '🇸🇬 新加坡 🇸🇬', '🇰🇷 韩国 🇰🇷', '🇯🇵日本专线🧱', '🇺🇸美国专线🧱', '🇭🇰香港专线🧱', '🇸🇬新加坡专线🧱', '🇹🇼台湾专线🧱', '🇬🇧英国专线🧱', '🇰🇷韩国专线🧱', '🧱仅专线🧱']
            outbound['default'] = '🕳️ 落地节点 🔐 +'
            changes_made.append("🌍 海外通用 🌍 已更新")
        
        elif tag == '🤖AI平台🤖':
            outbound['type'] = 'urltest'
            outbound['outbounds'] = ['🇺🇸美国专线🧱', '🇺🇸 美国 🇺🇸']
            outbound['url'] = 'http://www.cloudflare.com/generate_204'
            outbound['interval'] = '10m'
            outbound['tolerance'] = 50
            if 'default' in outbound:
                del outbound['default']
            changes_made.append("🤖AI平台🤖 已更新")
        
        elif tag == '☎️telegram✈️':
            outbound['outbounds'] = ['🇯🇵 JP 🇯🇵', '🇺🇸 美国 🇺🇸']
            outbound['default'] = '🇯🇵 JP 🇯🇵'
            changes_made.append("☎️telegram✈️ 已更新")
        
        elif tag == '🌐 社交媒体 📱':
            outbound['outbounds'] = ['🇯🇵日本专线🧱', '🇺🇸美国专线🧱', '🇰🇷韩国专线🧱', '🇯🇵 JP 🇯🇵', '🇺🇸 美国 🇺🇸']
            outbound['default'] = '🇯🇵日本专线🧱'
            changes_made.append("🌐 社交媒体 📱 已更新")

    # 6. 添加 🔗 自动回退 🏁 策略组
    fallback_exists = any(o.get('tag') == '🔗 自动回退 🏁' for o in config['outbounds'])
    if not fallback_exists:
        fallback_group = {
            'type': 'urltest',
            'tag': '🔗 自动回退 🏁',
            'outbounds': ['🎯 全球直连'],
            'url': 'http://www.cloudflare.com/generate_204',
            'interval': '5m',
            'tolerance': 2
        }
        for i, outbound in enumerate(config['outbounds']):
            if outbound.get('tag') == '🧱仅专线🧱':
                config['outbounds'].insert(i + 1, fallback_group)
                changes_made.append("🔗 自动回退 🏁 已添加")
                break

    # 7. 更新路由规则
    for rule in config['route']['rules']:
        if rule.get('rule_set') == 'surge-github':
            rule['outbound'] = '🔗 自动回退 🏁'
            changes_made.append("surge-github 路由已更新")
        elif rule.get('rule_set') == 'surge-substore':
            rule['outbound'] = '🔗 自动回退 🏁'
            changes_made.append("surge-substore 路由已更新")

    # 保存配置文件
    with open('substore/Singbox_substore_1.13.0+.json', 'w', encoding='utf-8') as f:
        json.dump(config, f, ensure_ascii=False, indent=2)

    print("更新完成！")
    print("\n已完成的更改:")
    for change in changes_made:
        print(f"  ✓ {change}")

if __name__ == '__main__':
    main()
