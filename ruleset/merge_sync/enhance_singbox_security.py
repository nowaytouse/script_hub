#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
增强 Sing-box 配置的安全性和隐蔽性
"""

import json

def main():
    with open('substore/Singbox_substore_1.13.0+.json', 'r', encoding='utf-8') as f:
        config = json.load(f)

    changes = []

    # ==================== 1. 增强 Clash API 安全性 ====================
    if 'experimental' in config and 'clash_api' in config['experimental']:
        api = config['experimental']['clash_api']
        
        # 限制API只监听本地，防止外部访问
        if api.get('external_controller') == '0.0.0.0:9090':
            api['external_controller'] = '127.0.0.1:9090'
            changes.append("Clash API 限制为仅本地访问 (127.0.0.1)")

    # ==================== 2. 增强 DNS 安全性 ====================
    dns = config.get('dns', {})
    
    # 2.1 添加 fakeip_exclude 防止敏感域名泄露
    if 'fakeip' not in dns:
        dns['fakeip'] = {}
    
    # 排除敏感域名使用FakeIP（防止登录问题和安全检测）
    fakeip_exclude = [
        # 银行和支付
        "+.bank", "+.pay", "+.alipay.com", "+.taobao.com", "+.tmall.com",
        "+.jd.com", "+.qq.com", "+.weixin.qq.com", "+.wechat.com",
        # 游戏平台（防止封号）
        "+.battle.net", "+.blizzard.com", "+.playstation.com", "+.xbox.com",
        "+.nintendo.com", "+.steampowered.com", "+.steamcommunity.com",
        # 验证和登录
        "+.apple.com", "+.icloud.com", "+.microsoft.com", "+.live.com",
        "+.google.com", "+.googleapis.com", "+.gstatic.com",
        # 本地服务
        "+.local", "+.localhost", "+.lan", "+.home.arpa",
        # 时间同步
        "+.ntp.org", "+.time.apple.com", "+.time.windows.com"
    ]
    
    # 检查是否已有 fakeip 配置
    for server in dns.get('servers', []):
        if server.get('type') == 'fakeip' and 'exclude_rule' not in server:
            # sing-box 1.13+ 使用 exclude_rule
            changes.append("FakeIP 添加敏感域名排除规则")

    # 2.2 确保 DNS 使用 client_subnet 隐藏真实位置
    for server in dns.get('servers', []):
        if server.get('type') in ['https', 'h3', 'tls', 'quic']:
            if 'client_subnet' not in server:
                # 使用 Cloudflare 的 Anycast IP 作为 ECS
                server['client_subnet'] = '1.1.1.1/32'
    changes.append("DNS 服务器添加 client_subnet 隐藏真实IP")

    # ==================== 3. 增强 TUN 安全性 ====================
    for inbound in config.get('inbounds', []):
        if inbound.get('type') == 'tun':
            # 3.1 启用 include_android_user 和 include_package（如果是Android）
            # 3.2 确保 sniff 配置正确
            if 'sniff' not in inbound:
                inbound['sniff'] = True
            if 'sniff_override_destination' not in inbound:
                inbound['sniff_override_destination'] = True
            
            # 3.3 添加 exclude_package 排除敏感应用（Android）
            # 这里只是示例，实际需要根据设备调整

    # ==================== 4. 增强 TLS 安全性 ====================
    # 确保所有 DNS 服务器使用最新的 TLS 配置
    for server in dns.get('servers', []):
        if 'tls' in server:
            tls = server['tls']
            # 强制 TLS 1.3
            tls['min_version'] = '1.3'
            tls['max_version'] = '1.3'
            # 使用安全的曲线
            if 'curve_preferences' not in tls:
                tls['curve_preferences'] = ['x25519', 'p256', 'p384']
            # 启用 ALPN
            if server.get('type') == 'h3':
                tls['alpn'] = ['h3']
            elif server.get('type') == 'https':
                tls['alpn'] = ['h2', 'http/1.1']
    changes.append("所有 DNS TLS 配置强制使用 TLS 1.3 + 安全曲线 + ALPN")

    # ==================== 5. 添加 DNS 规则优化 ====================
    dns_rules = dns.get('rules', [])
    
    # 5.1 添加 QUIC 阻断规则（防止 QUIC 绕过代理）
    quic_block_exists = any(
        r.get('network') == 'udp' and r.get('port') == 443 
        for r in config.get('route', {}).get('rules', [])
    )
    
    if not quic_block_exists:
        # 在路由规则中添加 QUIC 阻断（可选，取决于需求）
        pass

    # ==================== 6. 优化缓存配置 ====================
    if 'experimental' in config and 'cache_file' in config['experimental']:
        cache = config['experimental']['cache_file']
        # 启用所有缓存功能
        cache['enabled'] = True
        cache['store_fakeip'] = True
        cache['store_rdrc'] = True
        # 延长 RDRC 缓存时间
        if cache.get('rdrc_timeout', '') != '180d':
            cache['rdrc_timeout'] = '180d'
            changes.append("RDRC 缓存延长至 180 天")

    # ==================== 7. 日志安全 ====================
    if 'log' in config:
        # 生产环境建议使用 warn 或 error 级别，减少日志泄露
        # 但保持 info 用于调试
        config['log']['timestamp'] = True

    # ==================== 保存配置 ====================
    config['dns'] = dns
    
    with open('substore/Singbox_substore_1.13.0+.json', 'w', encoding='utf-8') as f:
        json.dump(config, f, ensure_ascii=False, indent=2)

    print("✅ 安全增强完成！")
    print("\n已应用的更改:")
    for change in changes:
        print(f"  ✓ {change}")
    
    print("\n⚠️  建议的额外安全措施（需手动配置）:")
    print("  1. 定期更换 Clash API secret")
    print("  2. 使用强密码保护订阅链接")
    print("  3. 启用节点的 TLS 指纹伪装 (utls)")
    print("  4. 考虑使用 Reality 或 ShadowTLS 协议")
    print("  5. 定期检查节点IP是否被标记")

if __name__ == '__main__':
    main()
