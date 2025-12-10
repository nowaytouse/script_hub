#!/usr/bin/env python3
"""
合并 Narrow Pierce 所有去广告模块为一个大合集
同时生成 Shadowrocket 兼容版本
"""
import os
import re
from pathlib import Path
from datetime import datetime

SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent
MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)" / "narrow_pierce"
OUTPUT_DIR = PROJECT_ROOT / "module" / "surge(main)" / "head_expanse"
SR_OUTPUT_DIR = PROJECT_ROOT / "module" / "shadowrocket" / "head_expanse"

# 合并后的模块名称
MERGED_NAME = "🎯 App去广告大合集"
MERGED_DESC = "整合所有App专项去广告规则（购物/云盘/社交/工具等）"

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

def convert_to_shadowrocket(content):
    """转换Surge模块为Shadowrocket兼容格式"""
    lines = content.split('\n')
    converted = []
    
    # 🔥 Surge参数占位符转换规则
    # Surge支持 {{{参数名}}} 语法，Shadowrocket不支持
    PARAMETER_PLACEHOLDER_RULES = {
        "{{{Proxy}}}": "PROXY",
        "{{{DIRECT}}}": "DIRECT",
        "{{{REJECT}}}": "REJECT",
        "{{{proxy}}}": "PROXY",
        "{{{direct}}}": "DIRECT",
        "{{{reject}}}": "REJECT",
    }
    
    for line in lines:
        # 移除 %APPEND% %INSERT%
        line = re.sub(r'%APPEND%\s*', '', line)
        line = re.sub(r'%INSERT%\s*', '', line)
        
        # 移除 extended-matching, pre-matching
        line = re.sub(r',extended-matching', '', line)
        line = re.sub(r',pre-matching', '', line)
        
        # REJECT-DROP -> REJECT
        line = re.sub(r'REJECT-DROP', 'REJECT', line)
        line = re.sub(r'REJECT-NO-DROP', 'REJECT', line)
        line = re.sub(r'REJECT-TINYGIF', 'REJECT', line)
        
        # 🔥 Surge参数占位符转换：{{{Proxy}}} → PROXY
        for placeholder, replacement in PARAMETER_PLACEHOLDER_RULES.items():
            line = line.replace(placeholder, replacement)
        
        # 通用占位符处理：任何未知的 {{{xxx}}} → PROXY
        line = re.sub(r'\{\{\{[^}]+\}\}\}', 'PROXY', line)
        
        # DoH/DoT DNS -> 普通DNS
        line = re.sub(r'server:h3://[^/]+/dns-query', 'server:223.5.5.5', line)
        line = re.sub(r'server:https://doh\.pub/dns-query', 'server:119.29.29.29', line)
        line = re.sub(r'server:https://doh\.360\.cn/dns-query', 'server:101.198.198.198', line)
        
        converted.append(line)
    
    return '\n'.join(converted)

def merge_all_modules():
    """合并所有narrow_pierce模块"""
    print("=== 合并所有 Narrow Pierce 模块 ===")
    
    rules, rewrites, scripts, mitm = set(), set(), set(), set()
    found_modules = []
    
    for f in sorted(MODULE_DIR.glob("*.sgmodule")):
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
        print("  未找到模块")
        return None
    
    # 生成Surge版本
    date_str = datetime.now().strftime('%Y-%m-%d')
    lines = [
        f"#!name={MERGED_NAME}",
        f"#!desc={MERGED_DESC} (合并自 {len(found_modules)} 个模块)",
        "#!author=nowaytouse (自动合并)",
        f"#!date={date_str}",
        "#!category=🔝 Head Expanse › 首端扩域",
        "",
        f"# 来源: {len(found_modules)} 个App专项去广告模块",
        f"# 包含: {', '.join(m.replace('.sgmodule','').replace('去广告','') for m in found_modules[:10])}...",
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
    
    surge_content = '\n'.join(lines)
    
    # 保存Surge版本
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    surge_file = OUTPUT_DIR / f"{MERGED_NAME}.sgmodule"
    surge_file.write_text(surge_content, encoding='utf-8')
    print(f"\n[Surge] 生成: {surge_file.name}")
    print(f"  规则: {len(rules)}, 重写: {len(rewrites)}, 脚本: {len(scripts)}, MITM: {len(mitm)}")
    
    # 生成Shadowrocket版本
    sr_content = convert_to_shadowrocket(surge_content)
    # 更新desc标记为SR版本
    sr_content = sr_content.replace(
        f"#!desc={MERGED_DESC}",
        f"#!desc=[🚀SR] {MERGED_DESC}"
    )
    
    SR_OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    sr_file = SR_OUTPUT_DIR / f"{MERGED_NAME}.sgmodule"
    sr_file.write_text(sr_content, encoding='utf-8')
    print(f"[Shadowrocket] 生成: {sr_file.name}")
    
    return {
        "name": MERGED_NAME,
        "modules_count": len(found_modules),
        "rules_count": len(rules),
        "source_modules": found_modules
    }

def cleanup_old_merged():
    """清理旧的分类合集"""
    old_files = [
        "购物平台去广告合集.sgmodule",
        "云盘应用去广告合集.sgmodule", 
        "社交媒体去广告合集.sgmodule"
    ]
    for name in old_files:
        for dir in [OUTPUT_DIR, SR_OUTPUT_DIR]:
            f = dir / name
            if f.exists():
                f.unlink()
                print(f"  删除旧文件: {f}")

if __name__ == "__main__":
    print("清理旧的分类合集...")
    cleanup_old_merged()
    print()
    
    result = merge_all_modules()
    
    if result:
        print(f"\n=== 合并完成 ===")
        print(f"合并了 {result['modules_count']} 个模块")
        print(f"总规则数: {result['rules_count']}")
