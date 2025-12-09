#!/usr/bin/env python3
"""合并 Narrow Pierce 小型去广告模块为分类模块"""
import os
import re
from pathlib import Path
from datetime import datetime

SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent
MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)" / "narrow_pierce"
OUTPUT_DIR = PROJECT_ROOT / "module" / "surge(main)" / "head_expanse"

MERGE_GROUPS = {
    "购物平台去广告合集": {
        "desc": "整合京东、淘宝、拼多多、闲鱼、菜鸟等购物App去广告规则",
        "keywords": ["京东", "淘宝", "拼多多", "闲鱼", "菜鸟"]
    },
    "云盘应用去广告合集": {
        "desc": "整合123云盘、阿里云盘、百度网盘、夸克等云盘App去广告规则",
        "keywords": ["123云盘", "阿里云盘", "百度网盘", "夸克"]
    },
    "社交媒体去广告合集": {
        "desc": "整合微博、小红书、知乎等社交媒体App去广告规则",
        "keywords": ["微博", "小红书", "知乎", "RedNote"]
    }
}

def extract_section(content, section_name):
    """提取模块文件中的指定section"""
    pattern = rf'^\[{re.escape(section_name)}\]\s*\n(.*?)(?=^\[|\Z)'
    match = re.search(pattern, content, re.MULTILINE | re.DOTALL)
    if match:
        lines = match.group(1).strip().split('\n')
        return [l.strip() for l in lines if l.strip() and not l.strip().startswith('#')]
    return []

def extract_hostname(content):
    """提取MITM hostname"""
    hostnames = set()
    for match in re.finditer(r'hostname\s*=\s*(?:%APPEND%\s*)?(.*)', content):
        hosts = match.group(1).strip()
        for h in hosts.split(','):
            h = h.strip()
            if h:
                hostnames.add(h)
    return hostnames

def merge_group(name, config):
    """合并一组模块"""
    print(f"[INFO] 处理: {name}")
    
    rules, rewrites, scripts, mitm = set(), set(), set(), set()
    found_modules = []
    
    for kw in config["keywords"]:
        for f in MODULE_DIR.glob(f"*{kw}*.sgmodule"):
            if not f.is_file():
                continue
            found_modules.append(f.name)
            print(f"  + {f.name}")
            
            content = f.read_text(encoding='utf-8')
            rules.update(extract_section(content, 'Rule'))
            rewrites.update(extract_section(content, 'URL Rewrite'))
            scripts.update(extract_section(content, 'Script'))
            mitm.update(extract_hostname(content))
    
    if not found_modules:
        print("  跳过(无匹配)")
        return
    
    # 生成合并后的模块
    output_file = OUTPUT_DIR / f"{name}.sgmodule"
    lines = [
        f"#!name={name}",
        f"#!desc={config['desc']} (合并自 {len(found_modules)} 个模块)",
        "#!author=nowaytouse (自动合并)",
        f"#!date={datetime.now().strftime('%Y-%m-%d')}",
        "#!category=🔝 Head Expanse › 首端扩域",
        "",
        f"# 来源模块: {', '.join(found_modules)}",
        ""
    ]
    
    if rules:
        lines.append("[Rule]")
        lines.extend(sorted(rules))
        lines.append("")
    
    if rewrites:
        lines.append("[URL Rewrite]")
        lines.extend(sorted(rewrites))
        lines.append("")
    
    if scripts:
        lines.append("[Script]")
        lines.extend(sorted(scripts))
        lines.append("")
    
    if mitm:
        lines.append("[MITM]")
        lines.append(f"hostname = %APPEND% {','.join(sorted(mitm))}")
    
    output_file.write_text('\n'.join(lines), encoding='utf-8')
    print(f"[OK] 生成: {output_file.name} ({len(rules)} 规则)")

def main():
    print("=== 合并 Narrow Pierce 模块 ===")
    for name, config in MERGE_GROUPS.items():
        merge_group(name, config)
    print("=== 合并完成 ===")

if __name__ == "__main__":
    main()
