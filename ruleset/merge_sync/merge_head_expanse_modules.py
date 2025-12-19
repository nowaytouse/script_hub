#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
合并 head_expanse 广告拦截模块为一个大合集

功能：
1. 合并所有广告拦截相关模块
2. 去重规则
3. 生成Shadowrocket兼容版本
"""

import os
import re
from pathlib import Path
from datetime import datetime
from collections import OrderedDict

# 路径配置
SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent
HEAD_EXPANSE_DIR = PROJECT_ROOT / "module" / "surge(main)" / "head_expanse"
SR_HEAD_EXPANSE_DIR = PROJECT_ROOT / "module" / "shadowrocket" / "head_expanse"

# 要合并的模块列表（广告拦截相关）
# 注意: AWAvenue-Ads-Rule 已被吸收到 AdBlock.list，模块已删除 (commit 4644020)
# 注意: 广告联盟.official 规则已被吸收，仅保留 URL 编码版本的脚本部分
MODULES_TO_MERGE = [
    "Adblock4limbo.sgmodule",
    "All-in-One-2.x.sgmodule",
    "AllInOne_Mock.sgmodule",
    "adultraplus.sgmodule",
    # 中文文件名 (优先使用)
    "可莉广告过滤器.beta.sgmodule",
    "广告平台拦截器.sgmodule",
    "新手友好の去广告集合.official.sgmodule",
    "小程序和应用懒人去广告合集.official.sgmodule",
    # URL编码版本 (备用，避免重复)
    "%E5%8F%AF%E8%8E%89%E5%B9%BF%E5%91%8A%E8%BF%87%E6%BB%A4%E5%99%A8.beta.sgmodule",
    "%E5%B9%BF%E5%91%8A%E5%B9%B3%E5%8F%B0%E6%8B%A6%E6%88%AA%E5%99%A8.sgmodule",
    "%E5%B9%BF%E5%91%8A%E8%81%94%E7%9B%9F.official.sgmodule",
]

# Sukka 上游规则集 (自动跟随更新)
SUKKA_UPSTREAM_RULESETS = [
    "https://ruleset.skk.moe/List/non_ip/reject.conf",
    "https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf",
    "https://ruleset.skk.moe/List/non_ip/reject-drop.conf",
    "https://ruleset.skk.moe/List/domainset/reject.conf",
]

# 不合并的模块（功能性/工具类）
EXCLUDED_MODULES = [
    "⭐️ Script Hub.official.sgmodule",  # 脚本转换工具
    "🔥 Firewall Port Blocker 🛡️🚫.sgmodule",  # 端口防火墙
    "🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule",  # 规则依赖
    "🎯 App去广告大合集.sgmodule",  # narrow_pierce合集
    "blockHTTPDNS.module",  # 单独的HTTPDNS模块
]

# 输出文件名
OUTPUT_NAME = "🛡️ 广告拦截大合集.sgmodule"


def parse_module(filepath: Path) -> dict:
    """解析模块文件"""
    sections = {
        "meta": {},
        "General": [],
        "Rule": [],
        "URL Rewrite": [],
        "Map Local": [],
        "Script": [],
        "MITM": {"hostname": set()},
    }
    
    current_section = None
    
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
    except Exception as e:
        print(f"  ⚠️ 读取失败: {filepath.name} - {e}")
        return sections
    
    for line in content.split('\n'):
        line_stripped = line.strip()
        
        # 跳过空行
        if not line_stripped:
            continue
        
        # 解析元数据
        if line_stripped.startswith('#!'):
            match = re.match(r'#!(\w+)\s*[=:]\s*(.+)', line_stripped)
            if match:
                key, value = match.groups()
                sections["meta"][key] = value.strip()
            continue
        
        # 跳过普通注释
        if line_stripped.startswith('#'):
            continue
        
        # 检测section
        if line_stripped.startswith('[') and line_stripped.endswith(']'):
            section_name = line_stripped[1:-1]
            current_section = section_name
            continue
        
        # 添加内容到对应section
        if current_section == "Rule":
            sections["Rule"].append(line_stripped)
        elif current_section == "URL Rewrite":
            sections["URL Rewrite"].append(line_stripped)
        elif current_section == "Map Local":
            sections["Map Local"].append(line_stripped)
        elif current_section == "Script":
            sections["Script"].append(line_stripped)
        elif current_section == "General":
            sections["General"].append(line_stripped)
        elif current_section == "MITM":
            # 解析hostname
            if line_stripped.startswith("hostname"):
                match = re.search(r'hostname\s*=\s*%APPEND%\s*(.+)', line_stripped)
                if match:
                    hosts = [h.strip() for h in match.group(1).split(',') if h.strip()]
                    sections["MITM"]["hostname"].update(hosts)
    
    return sections


def convert_to_shadowrocket(content: str) -> str:
    """转换为Shadowrocket兼容格式"""
    lines = content.split('\n')
    converted = []
    
    # 🔥 Surge参数占位符转换规则
    PARAMETER_PLACEHOLDER_RULES = {
        "{{{Proxy}}}": "PROXY",
        "{{{DIRECT}}}": "DIRECT",
        "{{{REJECT}}}": "REJECT",
        "{{{proxy}}}": "PROXY",
        "{{{direct}}}": "DIRECT",
        "{{{reject}}}": "REJECT",
    }
    
    for line in lines:
        # 移除 %APPEND%
        line = line.replace('%APPEND%', '')
        
        # 移除 extended-matching 和 pre-matching
        line = re.sub(r',\s*extended-matching', '', line)
        line = re.sub(r',\s*pre-matching', '', line)
        
        # 转换 REJECT-TINYGIF 为 REJECT
        line = line.replace('REJECT-TINYGIF', 'REJECT')
        line = line.replace('REJECT-DROP', 'REJECT')
        line = line.replace('REJECT-NO-DROP', 'REJECT')
        
        # 🔥 Surge参数占位符转换：{{{Proxy}}} → PROXY
        for placeholder, replacement in PARAMETER_PLACEHOLDER_RULES.items():
            line = line.replace(placeholder, replacement)
        
        # 通用占位符处理：任何未知的 {{{xxx}}} → PROXY
        line = re.sub(r'\{\{\{[^}]+\}\}\}', 'PROXY', line)
        
        # 移除 update-interval
        if 'update-interval' in line.lower():
            continue
        
        # 移除 ability 声明
        if line.strip().startswith('#!') and 'ability' in line.lower():
            continue
        
        # 转换DoH为普通DNS（Shadowrocket不完全支持）
        if 'doh-server' in line.lower():
            line = re.sub(r'doh-server\s*=\s*https://[^\s,]+', '', line)
        
        converted.append(line)
    
    return '\n'.join(converted)


def merge_modules():
    """合并所有模块"""
    print("=" * 60)
    print("🛡️ 合并 Head Expanse 广告拦截模块")
    print("=" * 60)
    print()
    
    # 收集所有内容
    all_rules = OrderedDict()
    all_rewrites = OrderedDict()
    all_map_local = OrderedDict()
    all_scripts = OrderedDict()
    all_general = OrderedDict()
    all_hostnames = set()
    merged_count = 0
    source_modules = []
    
    print("📦 合并以下模块:")
    for module_name in MODULES_TO_MERGE:
        module_path = HEAD_EXPANSE_DIR / module_name
        if not module_path.exists():
            print(f"  ⚠️ 跳过不存在: {module_name}")
            continue
        
        print(f"  ✅ {module_name}")
        sections = parse_module(module_path)
        source_modules.append(module_name)
        merged_count += 1
        
        # 合并各section（使用OrderedDict去重）
        for rule in sections["Rule"]:
            all_rules[rule] = True
        for rewrite in sections["URL Rewrite"]:
            all_rewrites[rewrite] = True
        for map_local in sections["Map Local"]:
            all_map_local[map_local] = True
        for script in sections["Script"]:
            all_scripts[script] = True
        for general in sections["General"]:
            all_general[general] = True
        all_hostnames.update(sections["MITM"]["hostname"])
    
    print()
    print(f"📊 合并统计:")
    print(f"  模块数: {merged_count}")
    print(f"  规则数: {len(all_rules)}")
    print(f"  重写数: {len(all_rewrites)}")
    print(f"  Map Local: {len(all_map_local)}")
    print(f"  脚本数: {len(all_scripts)}")
    print(f"  MITM域名: {len(all_hostnames)}")
    print()
    
    # 生成合并后的模块内容
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    
    content_lines = [
        f"#!name=🛡️ 广告拦截大合集",
        f"#!desc=整合所有广告拦截平台规则（AWAvenue/毒奶/可莉/Sukka等） (合并自 {merged_count} 个模块)",
        f"#!category=『 🔝 Head Expanse › 首端扩域 』",
        f"#!author=Multiple Authors (合并版)",
        f"#!icon=https://raw.githubusercontent.com/Koolson/Qure/master/IconSet/Color/Advertising.png",
        f"#!date={now}",
        f"#!source={', '.join(source_modules[:5])}...",
        "",
    ]
    
    # General section
    if all_general:
        content_lines.append("[General]")
        for item in all_general.keys():
            content_lines.append(item)
        content_lines.append("")
    
    # Rule section
    if all_rules:
        content_lines.append("[Rule]")
        for rule in all_rules.keys():
            content_lines.append(rule)
        content_lines.append("")
    
    # URL Rewrite section
    if all_rewrites:
        content_lines.append("[URL Rewrite]")
        for rewrite in all_rewrites.keys():
            content_lines.append(rewrite)
        content_lines.append("")
    
    # Map Local section
    if all_map_local:
        content_lines.append("[Map Local]")
        for item in all_map_local.keys():
            content_lines.append(item)
        content_lines.append("")
    
    # Script section
    if all_scripts:
        content_lines.append("[Script]")
        for script in all_scripts.keys():
            content_lines.append(script)
        content_lines.append("")
    
    # MITM section
    if all_hostnames:
        content_lines.append("[MITM]")
        hostname_str = ", ".join(sorted(all_hostnames))
        content_lines.append(f"hostname = %APPEND% {hostname_str}")
        content_lines.append("")
    
    content = '\n'.join(content_lines)
    
    # 保存Surge版本
    surge_output = HEAD_EXPANSE_DIR / OUTPUT_NAME
    with open(surge_output, 'w', encoding='utf-8') as f:
        f.write(content)
    print(f"✅ 生成Surge版本: {surge_output.name}")
    
    # 生成Shadowrocket版本
    sr_content = convert_to_shadowrocket(content)
    # 更新描述
    sr_content = sr_content.replace(
        "#!desc=整合所有广告拦截平台规则",
        "#!desc=[🚀SR] 整合所有广告拦截平台规则"
    )
    
    SR_HEAD_EXPANSE_DIR.mkdir(parents=True, exist_ok=True)
    sr_output = SR_HEAD_EXPANSE_DIR / OUTPUT_NAME
    with open(sr_output, 'w', encoding='utf-8') as f:
        f.write(sr_content)
    print(f"✅ 生成Shadowrocket版本: {sr_output.name}")
    
    print()
    print("=" * 60)
    print(f"✅ 合并完成!")
    print(f"   Surge: {len(all_rules)} 规则, {len(all_rewrites)} 重写, {len(all_scripts)} 脚本")
    print("=" * 60)
    
    return {
        "merged_count": merged_count,
        "rules": len(all_rules),
        "rewrites": len(all_rewrites),
        "scripts": len(all_scripts),
        "hostnames": len(all_hostnames),
    }


if __name__ == "__main__":
    merge_modules()
