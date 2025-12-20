#!/usr/bin/env python3
"""
DNS模块智能合并脚本 - 保留本地优化 + 追加上游新增规则
策略：本地[General][Host][URL Rewrite]保持不变，只追加上游[Rule]
上游: VirgilClyne/GetSomeFries (HTTPDNS.Block + ASN.China)
"""

import os
import re
import urllib.request
from datetime import datetime

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
OUTPUT_FILE = os.path.join(REPO_ROOT, "module/surge(main)/amplify_nexus/🌐 DNS & Host Enhanced.sgmodule")

# 上游URL
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

def extract_rules(content):
    """提取[Rule]段内容"""
    match = re.search(r'\[Rule\](.*?)(?=\[|$)', content, re.DOTALL)
    if match:
        rules = match.group(1).strip()
        # 过滤空行和纯注释行（保留带规则的注释）
        lines = []
        for line in rules.split('\n'):
            line = line.strip()
            if line and (line.startswith('#') or line.startswith('DOMAIN') or 
                        line.startswith('IP-') or line.startswith('RULE-SET')):
                lines.append(line)
        return '\n'.join(lines)
    return ""

def main():
    print("[INFO] 下载上游模块...")
    
    httpdns_content = download(HTTPDNS_URL, "HTTPDNS.Block")
    asn_content = download(ASN_URL, "ASN.China")
    
    if not httpdns_content or not asn_content:
        print("[✗] 下载失败，退出")
        return 1
    
    # 检查本地模块
    if not os.path.exists(OUTPUT_FILE):
        print(f"[✗] 本地DNS模块不存在: {OUTPUT_FILE}")
        return 1
    
    print("[INFO] 智能合并模块（保留本地优化配置）...")
    
    # 读取本地模块
    with open(OUTPUT_FILE, 'r', encoding='utf-8') as f:
        local_content = f.read()
    
    # 提取上游规则
    httpdns_rules = extract_rules(httpdns_content)
    asn_rules = extract_rules(asn_content)
    
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
    
    # 删除旧的上游规则（如果存在）
    # 匹配从 "FROM: GetSomeFries HTTPDNS" 到 [MITM] 或文件末尾
    local_content = re.sub(
        r'\n# ═+\n# FROM: GetSomeFries HTTPDNS.*?(?=\n\[MITM\]|\Z)',
        '',
        local_content,
        flags=re.DOTALL
    )
    
    # 检查是否有[Rule]段
    if '[Rule]' not in local_content:
        # 在[MITM]之前或文件末尾添加[Rule]段
        if '[MITM]' in local_content:
            local_content = local_content.replace('[MITM]', f'[Rule]{new_rules}\n\n[MITM]')
        else:
            local_content += f'\n[Rule]{new_rules}'
    else:
        # 在[MITM]之前插入新规则
        if '[MITM]' in local_content:
            local_content = local_content.replace('[MITM]', f'{new_rules}\n\n[MITM]')
        else:
            local_content += new_rules
    
    # 更新版本号
    local_content = re.sub(
        r'^#!version=.*$',
        f'#!version={datetime.now().strftime("%Y.%m.%d")}',
        local_content,
        flags=re.MULTILINE
    )
    
    # 写入文件
    with open(OUTPUT_FILE, 'w', encoding='utf-8') as f:
        f.write(local_content)
    
    # 统计
    httpdns_count = len([l for l in httpdns_rules.split('\n') if l.startswith(('DOMAIN', 'IP-'))])
    asn_count = len([l for l in asn_rules.split('\n') if l.startswith('IP-ASN')])
    
    print(f"[✓] DNS模块智能合并完成")
    print(f"    - HTTPDNS Block: {httpdns_count} 规则")
    print(f"    - ASN China: {asn_count} 规则")
    print(f"    - 本地优化配置: 已保留")
    
    return 0

if __name__ == "__main__":
    exit(main())
