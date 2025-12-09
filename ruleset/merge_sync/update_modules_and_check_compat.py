#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
模块更新 + 兼容性检查脚本
1. 下载并更新微博模块
2. 检查所有模块的Surge/Shadowrocket兼容性
3. 更新网页数据
"""

import os
import re
import json
import urllib.request
from pathlib import Path
from datetime import datetime

SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent
MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)"
OUTPUT_DIR = PROJECT_ROOT / "module"

# Surge特有语法（小火箭不支持）
SURGE_ONLY_FEATURES = [
    ("extended-matching", "扩展匹配"),
    ("pre-matching", "预匹配"),
    ("REJECT-DROP", "静默拒绝"),
    ("REJECT-NO-DROP", "拒绝不丢弃"),
    ("REJECT-TINYGIF", "拒绝返回GIF"),
    ("update-interval", "更新间隔"),
    ("%INSERT%", "插入规则"),
    ("%APPEND%", "追加规则"),
    ("ability:", "能力声明"),
    ("script-path", "脚本路径(部分)"),
]

def download_module(url, dest_path):
    """下载模块"""
    try:
        req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
        with urllib.request.urlopen(req, timeout=30) as resp:
            content = resp.read().decode('utf-8')
        with open(dest_path, 'w', encoding='utf-8') as f:
            f.write(content)
        return True
    except Exception as e:
        print(f"  ❌ 下载失败: {e}")
        return False

def check_compatibility(content):
    """检查模块兼容性，返回不兼容特性列表"""
    issues = []
    for feature, desc in SURGE_ONLY_FEATURES:
        if feature in content:
            count = content.count(feature)
            issues.append({"feature": feature, "desc": desc, "count": count})
    return issues

def get_module_info(filepath):
    """获取模块信息"""
    try:
        content = filepath.read_text(encoding='utf-8')
    except:
        return None
    
    info = {
        "name": filepath.stem,
        "path": str(filepath),
        "size": filepath.stat().st_size,
        "surge_only": False,
        "issues": [],
    }
    
    # 提取元数据
    for line in content.split('\n')[:20]:
        if line.startswith('#!name'):
            match = re.search(r'#!name\s*[=:]\s*(.+)', line)
            if match:
                info["name"] = match.group(1).strip()
        elif line.startswith('#!desc'):
            match = re.search(r'#!desc\s*[=:]\s*(.+)', line)
            if match:
                info["desc"] = match.group(1).strip()[:60]
    
    # 检查兼容性
    issues = check_compatibility(content)
    if issues:
        info["surge_only"] = True
        info["issues"] = issues
    
    return info

def main():
    print("=" * 60)
    print("📦 模块更新 + 兼容性检查")
    print("=" * 60)
    
    # 1. 下载微博模块
    print("\n📥 下载微博模块...")
    weibo_url = "https://github.com/fmz200/wool_scripts/raw/main/Surge/module/weibo.module"
    weibo_path = MODULE_DIR / "narrow_pierce" / "微博去广告_fmz200.sgmodule"
    
    if download_module(weibo_url, weibo_path):
        # 添加 #!category
        content = weibo_path.read_text(encoding='utf-8')
        if '#!category=『' not in content:
            lines = content.split('\n')
            new_lines = ["#!category=『 🎯 Narrow Pierce › 窄域穿刺 』"]
            for line in lines:
                if not line.startswith('#!category'):
                    new_lines.append(line)
            weibo_path.write_text('\n'.join(new_lines), encoding='utf-8')
        print(f"  ✅ 已保存: {weibo_path.name}")
    
    # 2. 检查所有模块兼容性
    print("\n🔍 检查模块兼容性...")
    
    all_modules = []
    surge_only_modules = []
    compatible_modules = []
    
    for cat in ["amplify_nexus", "head_expanse", "narrow_pierce"]:
        cat_path = MODULE_DIR / cat
        if not cat_path.exists():
            continue
        
        for f in sorted(cat_path.glob("*.sgmodule")):
            info = get_module_info(f)
            if info:
                info["category"] = cat
                all_modules.append(info)
                if info["surge_only"]:
                    surge_only_modules.append(info)
                else:
                    compatible_modules.append(info)
    
    # 3. 输出报告
    print(f"\n📊 兼容性统计:")
    print(f"  总模块数: {len(all_modules)}")
    print(f"  ✅ Surge+小火箭兼容: {len(compatible_modules)}")
    print(f"  ⚠️ 仅Surge: {len(surge_only_modules)}")
    
    if surge_only_modules:
        print(f"\n⚠️ 仅Surge支持的模块 ({len(surge_only_modules)}个):")
        for m in surge_only_modules:
            issues_str = ", ".join([f"{i['feature']}({i['count']})" for i in m["issues"][:3]])
            print(f"  - {m['name']}: {issues_str}")
    
    # 4. 保存兼容性数据
    compat_data = {
        "generated": datetime.now().isoformat(),
        "total": len(all_modules),
        "compatible": len(compatible_modules),
        "surge_only": len(surge_only_modules),
        "modules": {
            "compatible": [{"name": m["name"], "category": m["category"]} for m in compatible_modules],
            "surge_only": [{"name": m["name"], "category": m["category"], "issues": [i["desc"] for i in m["issues"]]} for m in surge_only_modules]
        }
    }
    
    compat_path = OUTPUT_DIR / "modules_compatibility.json"
    with open(compat_path, 'w', encoding='utf-8') as f:
        json.dump(compat_data, f, ensure_ascii=False, indent=2)
    print(f"\n💾 兼容性数据已保存: {compat_path}")
    
    print("\n" + "=" * 60)
    print("✅ 完成!")

if __name__ == "__main__":
    main()
