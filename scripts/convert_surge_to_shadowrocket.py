#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Surge → Shadowrocket module sync with Rule-Set Expansion
"""

import os
import re
import json
import shutil
import urllib.request
import urllib.error
from pathlib import Path
from datetime import datetime

SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
SURGE_MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)"
SR_MODULE_DIR = PROJECT_ROOT / "module" / "shadowrocket"

# 缓存已下载的规则集
RULESET_CACHE = {}

SURGE_ONLY_KEYS = {
    "use-local-host-item-for-proxy", "encrypted-dns-follow-outbound-mode",
    "encrypted-dns-skip-cert-verification", "force-http-engine-hosts",
    "always-raw-tcp-hosts", "always-raw-tcp-keywords", "tun-included-routes",
}

GENERAL_KEY_MAP = {
    "encrypted-dns-server": "doh-server",
    "tun-excluded-routes":  "bypass-tun",
}

SR_GENERAL_DEFAULTS = """\
bypass-system = true
ipv6 = true
prefer-ipv6 = true
hijack-dns = *:53
dns-direct-fallback-proxy = false"""

RULE_REPLACEMENTS = {
    "REJECT-DROP": "REJECT", "REJECT-TINYGIF": "REJECT", "REJECT-NO-DROP": "REJECT",
}

# Keep remote RULE-SET references for purpose-split AdBlock shards (avoid 50MB+ inline expansion)
PRESERVE_RULESET_MARKERS = (
    "/ruleset/AdBlock/AdBlock_",
    "ruleset%2FAdBlock%2FAdBlock_",
)

REWRITE_MODIFIER_RE = re.compile(r',\s*(extended-matching|pre-matching)\b|'
                                  r'\b(extended-matching|pre-matching)\s*,?')

def fetch_ruleset(url_or_path):
    """抓取并解析规则集"""
    if url_or_path in RULESET_CACHE:
        return RULESET_CACHE[url_or_path]
    
    content = ""
    try:
        # 转换 jsdelivr 链接到本地路径
        if "fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/" in url_or_path:
            rel_path = url_or_path.split("@master/")[-1]
            local_path = PROJECT_ROOT / rel_path
            if local_path.exists():
                print(f"  🏠 Using local file for {url_or_path}")
                content = local_path.read_text(encoding='utf-8')
        
        if not content:
            # 如果是相对路径
            if url_or_path.startswith(".."):
                path = (PROJECT_ROOT / "module/surge(main)/amplify_nexus" / url_or_path).resolve()
                if path.exists():
                    content = path.read_text(encoding='utf-8')
            # 如果是远程URL
            elif url_or_path.startswith("http"):
                print(f"  🌐 Fetching ruleset: {url_or_path}")
                req = urllib.request.Request(url_or_path, headers={'User-Agent': 'ClashMeta/1.18.1'})
                with urllib.request.urlopen(req, timeout=10) as response:
                    content = response.read().decode('utf-8')
        
        # 解析规则
        rules = []
        for line in content.splitlines():
            line = line.strip()
            if not line or line.startswith(("#", "//", ";")):
                continue
            parts = line.split(',')
            rtype = parts[0].upper()
            if rtype in ("DOMAIN", "DOMAIN-SUFFIX", "DOMAIN-KEYWORD", "IP-CIDR", "IP-CIDR6", "GEOIP"):
                rules.append(",".join(parts[:2]))
        
        RULESET_CACHE[url_or_path] = rules
        return rules
    except Exception as e:
        print(f"  ❌ Failed to fetch ruleset {url_or_path}: {e}")
        return []

def convert_content(content: str) -> str:
    lines = content.split('\n')
    out = []
    section = None
    general_defaults_added = False

    for line in lines:
        stripped = line.strip()

        if stripped.startswith('[') and stripped.endswith(']'):
            section = stripped[1:-1]
            out.append(line)
            if section == "General" and not general_defaults_added:
                out.append(SR_GENERAL_DEFAULTS)
                general_defaults_added = True
            continue

        if re.match(r'^#!', line):
            if re.match(r'^#!\s*(update-interval|ability)\s*=', line, re.IGNORECASE):
                continue
            m = re.match(r'^#!\s*(\S+?)\s*=\s*(.*)$', line)
            if m:
                key, val = m.group(1).strip(), m.group(2)
                if key == "desc":
                    val = f"[🚀SR] {val}" if "[🚀SR]" not in val else val
                out.append(f"#!{key}={val}")
            continue

        if section == "General" and not stripped.startswith('#') and stripped:
            if any(k in line for k in SURGE_ONLY_KEYS):
                out.append(f"# [SR不支持] {line.lstrip()}")
                continue
            for k, v in GENERAL_KEY_MAP.items():
                if line.startswith(k):
                    line = line.replace(k, v)
            out.append(line)
            continue

        if section == "Rule" and not stripped.startswith('#') and stripped:
            if stripped.startswith('RULE-SET,'):
                parts = stripped.split(',')
                url_or_path = parts[1].strip()
                policy = parts[2].strip() if len(parts) > 2 else "REJECT"
                policy = RULE_REPLACEMENTS.get(policy, policy)

                if any(marker in url_or_path for marker in PRESERVE_RULESET_MARKERS):
                    cleaned = REWRITE_MODIFIER_RE.sub('', line)
                    cleaned = re.sub(r',update-interval=\d+', '', cleaned)
                    cleaned = re.sub(r',no-resolve', '', cleaned)
                    for old, new in RULE_REPLACEMENTS.items():
                        cleaned = cleaned.replace(old, new)
                    out.append(cleaned.strip())
                    continue

                print(f"  📦 Expanding RULE-SET: {url_or_path}")
                expanded_rules = fetch_ruleset(url_or_path)
                if expanded_rules:
                    out.append(f"# --- Expanded from {url_or_path} ---")
                    for r in expanded_rules:
                        out.append(f"{r},{policy}")
                    out.append(f"# --- End expansion ---")
                else:
                    out.append(f"# [无法展开] {line}")
                continue
            
            if stripped.startswith('PROTOCOL,'):
                out.append(f"# [SR不支持PROTOCOL] {line}")
                continue

        line = re.sub(r'%(?:INSERT|APPEND)%\s*', '', line)
        for old, new in RULE_REPLACEMENTS.items():
            line = line.replace(old, new)
        line = REWRITE_MODIFIER_RE.sub('', line)
        line = re.sub(r',"update-interval=\d+"', '', line)
        
        out.append(line)

    return '\n'.join(out)

def process_all_modules():
    if not SR_MODULE_DIR.exists():
        SR_MODULE_DIR.mkdir(parents=True)

    categories = ["amplify_nexus", "head_expanse", "narrow_pierce"]
    stats = {"total": 0, "converted": 0, "failed": 0}
    
    for cat in categories:
        (SR_MODULE_DIR / cat).mkdir(exist_ok=True)
        cat_path = SURGE_MODULE_DIR / cat
        if not cat_path.exists(): continue

        for module_file in sorted(cat_path.glob("*.sgmodule")):
            stats["total"] += 1
            print(f"🔄 Converting: {module_file.name}")
            try:
                content = module_file.read_text(encoding="utf-8")
                converted = convert_content(content)
                out_path = SR_MODULE_DIR / cat / (module_file.stem + ".module")
                out_path.write_text(converted, encoding="utf-8")
                stats["converted"] += 1
            except Exception as e:
                print(f"  ❌ Failed {module_file.name}: {e}")
                stats["failed"] += 1

    return stats

if __name__ == "__main__":
    s = process_all_modules()
    print(f"\n✅ All modules converted: {s['converted']}/{s['total']}")
