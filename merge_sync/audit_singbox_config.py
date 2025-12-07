#!/usr/bin/env python3
"""
Singbox Configuration Audit Tool
Comprehensive review of Singbox config logic and structure
"""
import json
import sys
from collections import defaultdict

def audit_singbox_config(filepath):
    """Audit Singbox configuration for logic and consistency"""
    print(f"\n{'='*70}")
    print(f"Auditing: {filepath}")
    print(f"{'='*70}\n")
    
    with open(filepath, 'r', encoding='utf-8') as f:
        config = json.load(f)
    
    # 1. Rule-set definitions audit
    print("📋 1. RULE-SET DEFINITIONS AUDIT")
    print("-" * 70)
    
    if 'route' not in config or 'rule_set' not in config['route']:
        print("❌ No rule-set definitions found!")
        return
    
    rulesets = config['route']['rule_set']
    print(f"Total rule-sets defined: {len(rulesets)}")
    
    # Group by category
    categories = defaultdict(list)
    for rs in rulesets:
        tag = rs.get('tag', '')
        if 'adblock' in tag.lower():
            categories['Ad Blocking'].append(tag)
        elif any(x in tag.lower() for x in ['stream', 'youtube', 'spotify', 'netflix', 'disney']):
            categories['Streaming'].append(tag)
        elif any(x in tag.lower() for x in ['ai', 'telegram', 'tiktok', 'socialmedia', 'twitter', 'reddit']):
            categories['Social & AI'].append(tag)
        elif any(x in tag.lower() for x in ['apple', 'google', 'microsoft', 'github']):
            categories['Tech Giants'].append(tag)
        elif any(x in tag.lower() for x in ['gaming', 'steam', 'epic']):
            categories['Gaming'].append(tag)
        elif any(x in tag.lower() for x in ['china', 'tencent', 'bilibili', 'xiaohongshu', 'neteasemusic']):
            categories['China Services'].append(tag)
        elif any(x in tag.lower() for x in ['lan', 'cdn', 'manual', 'direct', 'download']):
            categories['Network & Direct'].append(tag)
        else:
            categories['Others'].append(tag)
    
    for cat, tags in sorted(categories.items()):
        print(f"\n  {cat}: {len(tags)} rule-sets")
        for tag in sorted(tags):
            print(f"    • {tag}")
    
    # 2. Rule references audit
    print(f"\n\n📋 2. RULE REFERENCES AUDIT")
    print("-" * 70)
    
    if 'rules' not in config['route']:
        print("❌ No rules found!")
        return
    
    rules = config['route']['rules']
    print(f"Total rules: {len(rules)}")
    
    # Count rule types
    rule_types = defaultdict(int)
    outbound_usage = defaultdict(int)
    ruleset_usage = defaultdict(int)
    
    for rule in rules:
        # Determine rule type
        if 'rule_set' in rule:
            rule_types['rule_set'] += 1
            ruleset_usage[rule.get('rule_set', 'unknown')] += 1
        elif 'domain' in rule:
            rule_types['domain'] += 1
        elif 'domain_suffix' in rule:
            rule_types['domain_suffix'] += 1
        elif 'ip_cidr' in rule:
            rule_types['ip_cidr'] += 1
        elif 'protocol' in rule:
            rule_types['protocol'] += 1
        elif 'clash_mode' in rule:
            rule_types['clash_mode'] += 1
        elif 'network' in rule:
            rule_types['network'] += 1
        elif 'ip_is_private' in rule:
            rule_types['ip_is_private'] += 1
        else:
            rule_types['other'] += 1
        
        # Count outbound usage
        outbound = rule.get('outbound', 'unknown')
        outbound_usage[outbound] += 1
    
    print("\n  Rule Types:")
    for rtype, count in sorted(rule_types.items(), key=lambda x: -x[1]):
        print(f"    • {rtype}: {count}")
    
    print("\n  Top 10 Outbound Usage:")
    for outbound, count in sorted(outbound_usage.items(), key=lambda x: -x[1])[:10]:
        print(f"    • {outbound}: {count} rules")
    
    # 3. Rule-set usage analysis
    print(f"\n\n📋 3. RULE-SET USAGE ANALYSIS")
    print("-" * 70)
    
    defined_rulesets = {rs.get('tag') for rs in rulesets}
    used_rulesets = set(ruleset_usage.keys())
    
    unused = defined_rulesets - used_rulesets
    if unused:
        print(f"\n  ⚠️  Unused rule-sets ({len(unused)}):")
        for rs in sorted(unused):
            print(f"    • {rs}")
    else:
        print(f"\n  ✅ All {len(defined_rulesets)} rule-sets are used")
    
    # 4. Outbound definitions audit
    print(f"\n\n📋 4. OUTBOUND DEFINITIONS AUDIT")
    print("-" * 70)
    
    if 'outbounds' not in config:
        print("❌ No outbounds found!")
        return
    
    outbounds = config['outbounds']
    print(f"Total outbounds: {len(outbounds)}")
    
    outbound_types = defaultdict(int)
    outbound_tags = []
    
    for ob in outbounds:
        ob_type = ob.get('type', 'unknown')
        outbound_types[ob_type] += 1
        outbound_tags.append(ob.get('tag', 'unknown'))
    
    print("\n  Outbound Types:")
    for otype, count in sorted(outbound_types.items(), key=lambda x: -x[1]):
        print(f"    • {otype}: {count}")
    
    # Check for unused outbounds
    defined_outbounds = set(outbound_tags)
    used_outbounds = set(outbound_usage.keys())
    
    unused_outbounds = defined_outbounds - used_outbounds
    if unused_outbounds:
        print(f"\n  ⚠️  Unused outbounds ({len(unused_outbounds)}):")
        for ob in sorted(unused_outbounds):
            print(f"    • {ob}")
    
    undefined_outbounds = used_outbounds - defined_outbounds
    if undefined_outbounds:
        print(f"\n  ❌ Undefined outbounds referenced in rules ({len(undefined_outbounds)}):")
        for ob in sorted(undefined_outbounds):
            print(f"    • {ob}")
    
    # 5. DNS configuration audit
    print(f"\n\n📋 5. DNS CONFIGURATION AUDIT")
    print("-" * 70)
    
    if 'dns' not in config:
        print("❌ No DNS config found!")
        return
    
    dns = config['dns']
    
    if 'servers' in dns:
        print(f"DNS servers: {len(dns['servers'])}")
        for server in dns['servers']:
            tag = server.get('tag', 'unknown')
            stype = server.get('type', 'unknown')
            addr = server.get('server', 'N/A')
            print(f"    • {tag} ({stype}): {addr}")
    
    if 'rules' in dns:
        print(f"\nDNS rules: {len(dns['rules'])}")
    
    # 6. Logic consistency check
    print(f"\n\n📋 6. LOGIC CONSISTENCY CHECK")
    print("-" * 70)
    
    issues = []
    
    # Check rule order logic
    adblock_index = None
    globalproxy_index = None
    chinadirect_index = None
    
    for i, rule in enumerate(rules):
        rs = rule.get('rule_set', '')
        if 'adblock' in rs.lower():
            adblock_index = i
        elif 'globalproxy' in rs.lower():
            globalproxy_index = i
        elif 'chinadirect' in rs.lower():
            chinadirect_index = i
    
    # AdBlock should come before other rules
    if adblock_index is not None and globalproxy_index is not None:
        if adblock_index > globalproxy_index:
            issues.append("⚠️  AdBlock rules should come before GlobalProxy rules")
    
    # Regional rules should come before global rules
    if chinadirect_index is not None and globalproxy_index is not None:
        if chinadirect_index > globalproxy_index:
            issues.append("⚠️  ChinaDirect rules should come before GlobalProxy rules")
    
    # Check for duplicate rule-set references
    ruleset_refs = [r.get('rule_set') for r in rules if 'rule_set' in r]
    duplicates = {rs: ruleset_refs.count(rs) for rs in set(ruleset_refs) if ruleset_refs.count(rs) > 1}
    if duplicates:
        issues.append(f"⚠️  Duplicate rule-set references found: {len(duplicates)}")
        for rs, count in sorted(duplicates.items(), key=lambda x: -x[1])[:5]:
            issues.append(f"    • {rs}: {count} times")
    
    if issues:
        print("\n  Issues found:")
        for issue in issues:
            print(f"    {issue}")
    else:
        print("\n  ✅ No logic issues found")
    
    # 7. Summary
    print(f"\n\n{'='*70}")
    print("📊 AUDIT SUMMARY")
    print(f"{'='*70}")
    print(f"  Rule-sets defined: {len(rulesets)}")
    print(f"  Rule-sets used: {len(used_rulesets)}")
    print(f"  Total rules: {len(rules)}")
    print(f"  Outbounds defined: {len(outbounds)}")
    print(f"  Outbounds used: {len(used_outbounds)}")
    print(f"  DNS servers: {len(dns.get('servers', []))}")
    print(f"  Logic issues: {len(issues)}")
    print(f"{'='*70}\n")

def main():
    """Audit all Singbox configs"""
    configs = [
        'substore/Singbox_substore_1.13.0+.json'
    ]
    
    for config in configs:
        try:
            audit_singbox_config(config)
        except Exception as e:
            print(f"❌ Error auditing {config}: {e}")
            import traceback
            traceback.print_exc()
            sys.exit(1)
    
    print("✅ Audit complete!")

if __name__ == '__main__':
    main()
