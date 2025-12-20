#!/usr/bin/env python3
# DNS模块智能合并脚本 - 保留本地DoH优化 + 追加上游规则
# 上游: VirgilClyne/GetSomeFries (General + DNS + HTTPDNS.Block + ASN.China)

import os
import re
import urllib.request
from datetime import datetime

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
OUTPUT_FILE = os.path.join(REPO_ROOT, "module/surge(main)/amplify_nexus/🌐 DNS & Host Enhanced.sgmodule")

GENERAL_URL = "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/General.sgmodule"
DNS_URL = "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/DNS.sgmodule"
HTTPDNS_URL = "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/HTTPDNS.Block.sgmodule"
ASN_URL = "https://raw.githubusercontent.com/VirgilClyne/GetSomeFries/main/sgmodule/ASN.China.sgmodule"

def download(url, name):
    try:
        with urllib.request.urlopen(url, timeout=30) as resp:
            return resp.read().decode('utf-8')
    except Exception as e:
        print(f"[X] {name} failed: {e}")
        return None

def extract_rules(content):
    match = re.search(r'\[Rule\](.*?)(?=\n\[|$)', content, re.DOTALL)
    if not match:
        return ""
    lines = []
    for line in match.group(1).strip().split('\n'):
        line = line.strip()
        if line and (line.startswith('#') or line.startswith('DOMAIN') or 
                    line.startswith('IP-') or line.startswith('RULE-SET')):
            lines.append(line)
    return '\n'.join(lines)

def main():
    print("[INFO] Downloading 4 upstream modules...")
    
    general = download(GENERAL_URL, "General")
    dns = download(DNS_URL, "DNS")
    httpdns = download(HTTPDNS_URL, "HTTPDNS.Block")
    asn = download(ASN_URL, "ASN.China")
    
    if not all([general, dns, httpdns, asn]):
        print("[X] Download failed")
        return 1
    
    print("[OK] All 4 modules downloaded")
    
    if not os.path.exists(OUTPUT_FILE):
        print(f"[X] Local module not found: {OUTPUT_FILE}")
        return 1
    
    with open(OUTPUT_FILE, 'r', encoding='utf-8') as f:
        local = f.read()
    
    # Extract upstream rules
    httpdns_rules = extract_rules(httpdns)
    asn_rules = extract_rules(asn)
    
    # Remove old upstream rules if exist
    local = re.sub(r'\n\[Rule\].*?(?=\n\[MITM\])', '', local, flags=re.DOTALL)
    
    # Build new [Rule] section
    date_str = datetime.now().strftime('%Y.%m.%d')
    rule_section = f"""
[Rule]
# ═══════════════════════════════════════════════════════════════
# FROM: GetSomeFries HTTPDNS.Block (Block HTTPDNS hijacking)
# AUTO-MERGED: {date_str}
# ═══════════════════════════════════════════════════════════════
{httpdns_rules}

# ═══════════════════════════════════════════════════════════════
# FROM: GetSomeFries ASN.China (China ASN Direct)
# AUTO-MERGED: {date_str}
# ═══════════════════════════════════════════════════════════════
{asn_rules}
"""
    
    # Insert [Rule] before [MITM]
    if '[MITM]' in local:
        local = local.replace('[MITM]', rule_section + '\n[MITM]')
    else:
        local += rule_section
    
    # Update version
    local = re.sub(r'^#!version=.*$', f'#!version={date_str}', local, flags=re.MULTILINE)
    
    with open(OUTPUT_FILE, 'w', encoding='utf-8') as f:
        f.write(local)
    
    httpdns_count = len([l for l in httpdns_rules.split('\n') if l.startswith(('DOMAIN', 'IP-'))])
    asn_count = len([l for l in asn_rules.split('\n') if l.startswith('IP-ASN')])
    
    print(f"[OK] Merged successfully")
    print(f"    - HTTPDNS Block: {httpdns_count} rules")
    print(f"    - ASN China: {asn_count} rules")
    print(f"    - Local DoH config: preserved")
    return 0

if __name__ == "__main__":
    exit(main())
