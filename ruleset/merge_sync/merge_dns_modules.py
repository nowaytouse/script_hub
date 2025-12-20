#!/usr/bin/env python3
"""
DNS模块智能合并脚本 - 保留本地DoH优化 + 追加上游新增内容
策略：
1. [General] - 合并上游skip-proxy/always-real-ip，保留本地DoH配置
2. [Host] - 保留本地DoH优化，不用上游传统DNS
3. [Rule] - 追加上游HTTPDNS.Block + ASN.China
4. [MITM] - 合并上游hostname配置
上游: VirgilClyne/GetSomeFries (General + DNS + HTTPDNS.Block + ASN.China)
"""

import os
import re
import urllib.request
from datetime import datetime

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
OUTPUT_FILE = os.path.join(REPO_ROOT, "module/surge(main)/amplify_nexus/🌐 DNS & Host Enhanced.sgmodule")

# 上游URL - 4个模块
GENERAL_URL = "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/General.sgmodule"
DNS_URL = "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/DNS.sgmodule"
HTTPDNS_URL = "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/HTTPDNS.Block.sgmodule"
ASN_URL = "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/ASN.China.sgmodule"

def download(url, name):
    """下载上游模块"""
    try:
        with urllib.request.urlopen(url, timeout=30) as resp:
            content = resp.read().decode('utf-8')
        print(f"[✓] {name} 下载成功")
        return content
    except Exception as e:
        print(f"[✗] {name} 下载失败: {e}")
        return None

def extract_section(content, section):
    """提取指定段内容"""
    pattern = rf'\[{section}\](.*?)(?=\n\[|$)'
    match = re.search(pattern, content, re.DOTALL)
    if match:
        return match.group(1).strip()
    return ""

def extract_rules(content):
    """提取[Rule]段内容"""
    rules = extract_section(content, 'Rule')
    if rules:
        lines = []
        for line in rules.split('\n'):
            line = line.strip()
            if line and (line.startswith('#') or line.startswith('DOMAIN') or 
                        line.startswith('IP-') or line.startswith('RULE-SET')):
                lines.append(line)
        return '\n'.join(lines)
    return ""

def extract_general_values(content, key):
    """提取General段中指定key的值"""
    pattern = rf'^{re.escape(key)}\s*=\s*(.+)$'
    match = re.search(pattern, content, re.MULTILINE)
    if match:
        return match.group(1).strip()
    return ""

def extract_mitm_hostname(content):
    """提取MITM段的hostname配置"""
    mitm = extract_section(content, 'MITM')
    hostnames = []
    for line in mitm.split('\n'):
        line = line.strip()
        if line.startswith('hostname') and '=' in line:
            hostnames.append(line)
    return hostnames

def main():
    print("[INFO] 下载上游4个模块...")
    
    general_content = download(GENERAL_URL, "General")
    dns_content = download(DNS_URL, "DNS")
    httpdns_content = download(HTTPDNS_URL, "HTTPDNS.Block")
    asn_content = download(ASN_URL, "ASN.China")
    
    if not all([general_content, dns_content, httpdns_content, asn_content]):
        print("[✗] 部分模块下载失败，退出")
        return 1
    
    if not os.path.exists(OUTPUT_FILE):
        print(f"[✗] 本地DNS模块不存在: {OUTPUT_FILE}")
        return 1
    
    print("[INFO] 智能合并模块（保留本地DoH优化配置）...")
    
    with open(OUTPUT_FILE, 'r', encoding='utf-8') as f:
        local_content = f.read()
    
    # 1. 提取上游General的skip-proxy和always-real-ip（用于对比，但本地已有则不覆盖）
    upstream_skip_proxy = extract_general_values(general_content, 'skip-proxy')
    upstream_always_real_ip = extract_general_values(general_content, 'always-real-ip')
    
    # 2. 提取上游MITM hostname
    upstream_mitm = extract_mitm_hostname(general_content)
    
    # 3. 提取上游HTTPDNS的force-http-engine-hosts
    upstream_force_http = extract_general_values(httpdns_content, 'force-http-engine-hosts')
    
    # 4. 提取上游规则
    httpdns_rules = extract_rules(httpdns_content)
    asn_rules = extract_rules(asn_content)
    
    # 删除旧的上游规则
    local_content = re.sub(
        r'\n# ═+\n# FROM: GetSomeFries.*?(?=\n\[MITM\]|\Z)',
        '',
        local_content,
        flags=re.DOTALL
    )
    
    # 构建新规则段
    new_rules = f"""
# ═══════════════════════════════════════════════════════════════
# FROM: GetSomeFries HTTPDNS.Block (阻止HTTPDNS劫持)
# ⚠️ AUTO-MERGED - 自动从上游同步 {datetime.now().strftime('%Y.%m.%d')}
# ═══════════════════════════════════════════════════════════════
{httpdns_rules}

# ═══════════════════════════════════════════════════════════════
# FROM: GetSomeFries ASN.China (中国大陆ASN直连)
# ⚠️ AUTO-MERGED - 自动从上游同步 {datetime.now().strftime('%Y.%m.%d')}
# ═══════════════════════════════════════════════════════════════
{asn_rules}
"""
    
    # 检查是否有[MITM]段，在其前插入规则
    if '[MITM]' in local_content:
        # 检查并合并MITM hostname
        mitm_section = extract_section(local_content, 'MITM')
        for hostname_line in upstream_mitm:
            # 检查是否已存在
            key = hostname_line.split('=')[0].strip()
            if key not in mitm_section:
                # 在[MITM]后添加
                local_content = re.sub(
                    r'(\[MITM\])',
                    f'\\1\n# FROM: GetSomeFries General\n{hostname_line}',
                    local_content
                )
        
        local_content = local_content.replace('[MITM]', f'{new_rules}\n\n[MITM]')
    else:
        local_content += new_rules
    
    # 更新版本号和描述
    local_content = re.sub(
        r'^#!version=.*$',
        f'#!version={datetime.now().strftime("%Y.%m.%d")}',
        local_content,
        flags=re.MULTILINE
    )
    local_content = re.sub(
        r'^#!desc=.*$',
        '#!desc=🔒 全量DoH加密DNS + Host分流增强 + URL重写 + GetSomeFries(General/DNS/HTTPDNS/ASN) | 🔧 AUTO-MERGED',
        local_content,
        flags=re.MULTILINE
    )
    
    with open(OUTPUT_FILE, 'w', encoding='utf-8') as f:
        f.write(local_content)
    
    # 统计
    httpdns_count = len([l for l in httpdns_rules.split('\n') if l.startswith(('DOMAIN', 'IP-'))])
    asn_count = len([l for l in asn_rules.split('\n') if l.startswith('IP-ASN')])
    
    print(f"[✓] DNS模块智能合并完成 (4个上游模块)")
    print(f"    - General: skip-proxy/always-real-ip/MITM (本地已有，保留DoH优化)")
    print(f"    - DNS: Host映射 (本地已优化为DoH，不覆盖)")
    print(f"    - HTTPDNS Block: {httpdns_count} 规则")
    print(f"    - ASN China: {asn_count} 规则")
    
    return 0

if __name__ == "__main__":
    exit(main())
