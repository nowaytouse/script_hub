#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Surge模块整合脚本
功能：
1. 自动生成模块URL列表
2. 生成导入助手网页数据
3. 验证模块完整性
4. 检测重复/冲突模块
"""

import os
import re
import json
from pathlib import Path
from datetime import datetime
from urllib.parse import quote

# 项目根目录
SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)"
OUTPUT_DIR = PROJECT_ROOT / "module"

# GitHub raw URL基础路径
GITHUB_RAW_BASE = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/surge%28main%29"

# 分类定义
CATEGORIES = {
    "amplify_nexus": {
        "name": "🛠️ Amplify Nexus › 增幅枢纽",
        "desc": "功能增强类模块"
    },
    "head_expanse": {
        "name": "🔝 Head Expanse › 首端扩域", 
        "desc": "广告拦截平台类"
    },
    "narrow_pierce": {
        "name": "🎯 Narrow Pierce › 窄域穿刺",
        "desc": "App专项去广告"
    }
}

# 必装模块
ESSENTIAL_MODULES = [
    "Script Hub",
    "可莉广告过滤器"
]

# 标签映射
TAG_PATTERNS = {
    "bilibili": ["bilibili", "bili", "哔哩"],
    "youtube": ["youtube"],
    "iringo": ["iringo"],
    "tool": ["boxjs", "sub_info", "timecard", "surge-beta", "preview", "net-lsp"],
    "dns": ["dns"],
    "shopping": ["淘宝", "京东", "拼多多", "闲鱼"]
}


def get_module_info(filepath: Path) -> dict:
    """解析模块文件获取信息"""
    info = {
        "name": filepath.stem,
        "filename": filepath.name,
        "desc": "",
        "category": "",
        "author": "",
        "version": "",
        "date": ""
    }
    
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
            
        for line in content.split('\n')[:30]:
            line = line.strip()
            if line.startswith('#!name'):
                # 提取名称
                match = re.search(r'#!name\s*[=:]\s*(.+)', line)
                if match:
                    info["name"] = match.group(1).strip()
            elif line.startswith('#!desc'):
                match = re.search(r'#!desc\s*[=:]\s*(.+)', line)
                if match:
                    info["desc"] = match.group(1).strip()[:60]
            elif line.startswith('#!author'):
                match = re.search(r'#!author\s*[=:]\s*(.+)', line)
                if match:
                    info["author"] = match.group(1).strip()
            elif line.startswith('#!version'):
                match = re.search(r'#!version\s*[=:]\s*(.+)', line)
                if match:
                    info["version"] = match.group(1).strip()
            elif line.startswith('#!date'):
                match = re.search(r'#!date\s*[=:]\s*(.+)', line)
                if match:
                    info["date"] = match.group(1).strip()
                    
    except Exception as e:
        print(f"  ⚠️ 解析失败: {filepath.name} - {e}")
        
    return info


def get_tag(name: str, filename: str) -> str:
    """根据名称获取标签"""
    combined = (name + filename).lower()
    for tag, patterns in TAG_PATTERNS.items():
        for pattern in patterns:
            if pattern in combined:
                return tag
    return ""


def is_essential(name: str) -> bool:
    """判断是否为必装模块"""
    for essential in ESSENTIAL_MODULES:
        if essential.lower() in name.lower():
            return True
    return False


def generate_url(category: str, filename: str) -> str:
    """生成GitHub raw URL"""
    encoded_filename = quote(filename, safe='')
    return f"{GITHUB_RAW_BASE}/{category}/{encoded_filename}"


def scan_modules() -> dict:
    """扫描所有模块"""
    modules = {}
    
    for cat_dir in CATEGORIES.keys():
        cat_path = MODULE_DIR / cat_dir
        if not cat_path.exists():
            print(f"  ⚠️ 分类目录不存在: {cat_dir}")
            continue
            
        modules[cat_dir] = {
            "name": CATEGORIES[cat_dir]["name"],
            "desc": CATEGORIES[cat_dir]["desc"],
            "items": []
        }
        
        for module_file in sorted(cat_path.glob("*.sgmodule")):
            info = get_module_info(module_file)
            tag = get_tag(info["name"], module_file.name)
            essential = is_essential(info["name"])
            url = generate_url(cat_dir, module_file.name)
            
            modules[cat_dir]["items"].append({
                "name": info["name"],
                "filename": module_file.name,
                "desc": info["desc"] or info["name"],
                "url": url,
                "tag": tag,
                "essential": essential,
                "author": info["author"],
                "version": info["version"],
                "date": info["date"]
            })
            
    return modules


# 已删除 generate_url_list 函数 - 用户要求仅更新网页，不再生成URL列表文件


def generate_helper_js(modules: dict) -> str:
    """生成助手网页的JavaScript数据"""
    js_modules = {}
    
    for cat_key, cat_data in modules.items():
        js_modules[cat_key] = {
            "name": cat_data["name"],
            "desc": cat_data["desc"],
            "items": []
        }
        
        for item in cat_data["items"]:
            js_item = {
                "name": item["name"],
                "desc": item["desc"],
                "url": item["url"]
            }
            if item["tag"]:
                js_item["tag"] = item["tag"]
            if item["essential"]:
                js_item["essential"] = True
            js_modules[cat_key]["items"].append(js_item)
            
    return json.dumps(js_modules, ensure_ascii=False, indent=4)


def check_duplicates(modules: dict) -> list:
    """检测重复模块（基于文件名完全匹配）"""
    duplicates = []
    all_items = []
    
    # 收集所有模块
    for cat_key, cat_data in modules.items():
        for item in cat_data["items"]:
            all_items.append({
                "name": item["name"],
                "filename": item["filename"],
                "cat": cat_key
            })
    
    # 检测完全同名文件（不同分类）
    seen_filenames = {}
    for item in all_items:
        filename = item["filename"].lower()
        
        if filename in seen_filenames:
            prev = seen_filenames[filename]
            if prev["cat"] != item["cat"]:
                duplicates.append({
                    "name1": f"{prev['name']} ({prev['cat']})",
                    "name2": f"{item['name']} ({item['cat']})",
                    "reason": "完全同名文件"
                })
        else:
            seen_filenames[filename] = item
    
    # 统计相关模块组
    groups = {
        "B站": [i for i in all_items if 'bilibili' in i["filename"].lower() or 'bili' in i["filename"].lower()],
        "YouTube": [i for i in all_items if 'youtube' in i["filename"].lower()],
        "iRingo": [i for i in all_items if 'iringo' in i["filename"].lower()],
        "DNS": [i for i in all_items if 'dns' in i["filename"].lower()]
    }
    
    for group_name, items in groups.items():
        if len(items) >= 3:
            duplicates.append({
                "name1": f"{group_name}相关模块",
                "name2": f"共 {len(items)} 个",
                "reason": "可考虑整合"
            })
                
    return duplicates


def update_helper_html(modules: dict):
    """更新助手网页中的模块数据"""
    helper_path = OUTPUT_DIR / "surge_module_helper.html"
    
    if not helper_path.exists():
        print("  ⚠️ surge_module_helper.html 不存在，跳过更新")
        return
        
    try:
        with open(helper_path, 'r', encoding='utf-8') as f:
            content = f.read()
            
        # 生成新的模块数据
        js_data = generate_helper_js(modules)
        
        # 替换模块数据 - 支持空对象 {} 和多行对象
        pattern = r'const modules = \{[^;]*\};'
        replacement = f'const modules = {js_data};'
        
        new_content = re.sub(pattern, replacement, content, flags=re.DOTALL)
        
        with open(helper_path, 'w', encoding='utf-8') as f:
            f.write(new_content)
            
        print(f"  ✅ 更新 surge_module_helper.html")
        
    except Exception as e:
        print(f"  ❌ 更新失败: {e}")


def main():
    print("=" * 60)
    print("📦 Surge模块整合工具")
    print("=" * 60)
    print()
    
    # 扫描模块
    print("🔍 扫描模块...")
    modules = scan_modules()
    
    total = sum(len(cat["items"]) for cat in modules.values())
    print(f"  找到 {total} 个模块")
    print()
    
    # 统计各分类
    print("📊 分类统计:")
    for cat_key, cat_data in modules.items():
        print(f"  {cat_data['name']}: {len(cat_data['items'])} 个")
    print()
    
    # 检测重复
    print("🔄 检测重复模块...")
    duplicates = check_duplicates(modules)
    if duplicates:
        print(f"  ⚠️ 发现 {len(duplicates)} 组可能重复的模块:")
        for dup in duplicates:
            print(f"    - {dup['name1']} vs {dup['name2']}")
    else:
        print("  ✅ 未发现重复模块")
    print()
    
    # 更新助手网页（唯一输出）
    print("🌐 更新助手网页...")
    update_helper_html(modules)
    print()
    
    # 生成JSON数据
    print("💾 生成JSON数据...")
    json_path = OUTPUT_DIR / "modules_data.json"
    with open(json_path, 'w', encoding='utf-8') as f:
        json.dump({
            "generated": datetime.now().isoformat(),
            "total": total,
            "categories": modules
        }, f, ensure_ascii=False, indent=2)
    print(f"  ✅ 保存到 {json_path}")
    print()
    
    # 统计标签
    print("🏷️ 标签统计:")
    tag_counts = {}
    essential_count = 0
    for cat_data in modules.values():
        for item in cat_data["items"]:
            if item["tag"]:
                tag_counts[item["tag"]] = tag_counts.get(item["tag"], 0) + 1
            if item["essential"]:
                essential_count += 1
                
    for tag, count in sorted(tag_counts.items(), key=lambda x: -x[1]):
        print(f"  {tag}: {count}")
    print(f"  ⭐ 必装: {essential_count}")
    print()
    
    print("=" * 60)
    print(f"✅ 整合完成! 共 {total} 个模块")
    print("=" * 60)


if __name__ == "__main__":
    main()
