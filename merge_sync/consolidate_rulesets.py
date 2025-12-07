#!/usr/bin/env python3
"""Consolidate related rulesets into unified files"""
import os
from datetime import datetime

RULESET_DIR = "ruleset/Surge(Shadowkroket)"

def read_rules(filepath):
    rules = set()
    if not os.path.exists(filepath):
        return rules
    with open(filepath, 'r') as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith('#'):
                continue
            rules.add(line)
    return rules

def write_ruleset(filepath, rules, name):
    sorted_rules = sorted(rules)
    with open(filepath, 'w') as f:
        f.write(f"# Ruleset: {name}\n")
        f.write(f"# Updated: {datetime.now().strftime('%Y-%m-%d %H:%M')}\n")
        f.write(f"# Rules: {len(sorted_rules)}\n\n")
        for rule in sorted_rules:
            f.write(rule + '\n')

def merge(target, sources):
    target_path = os.path.join(RULESET_DIR, target)
    all_rules = read_rules(target_path)
    print(f"Target {target}: {len(all_rules)} rules")
    
    for src in sources:
        src_path = os.path.join(RULESET_DIR, src)
        src_rules = read_rules(src_path)
        before = len(all_rules)
        all_rules.update(src_rules)
        print(f"  + {src}: +{len(all_rules)-before} new")
    
    write_ruleset(target_path, all_rules, target.replace('.list',''))
    print(f"  = Total: {len(all_rules)} rules\n")
    return all_rules

if __name__ == '__main__':
    print("=== Consolidating Rulesets ===\n")
    
    # 1. Tencent = QQ + WeChat + Tencent
    merge("Tencent.list", ["QQ.list", "WeChat.list"])
    
    # 2. StreamUS = Netflix + Disney + StreamUS
    merge("StreamUS.list", ["Netflix.list", "Disney.list"])
    
    # 3. StreamTW = Bahamut + StreamTW
    merge("StreamTW.list", ["Bahamut.list"])
    
    print("Done!")
