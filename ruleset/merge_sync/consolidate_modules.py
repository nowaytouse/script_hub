#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Surge模块整合脚本
功能：
1. 自动生成模块URL列表
2. 生成导入助手网页数据
3. 验证模块完整性
4. 检测重复/冲突模块
5. 显示Shadowrocket兼容性信息
"""

import os
import re
import json
from pathlib import Path
from datetime import datetime
from urllib.parse import quote

# 项目根目录
SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent
MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)"
OUTPUT_DIR = PROJECT_ROOT / "module"
COMPAT_FILE = OUTPUT_DIR / "modules_compatibility.json"

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


def sanitize_string(s: str) -> str:
    """清理字符串中的特殊字符，确保JSON安全"""
    if not s:
        return s
    # 移除字面 \n \r \t 字符串（不是真正的换行符）
    s = s.replace('\\n', ' ').replace('\\r', ' ').replace('\\t', ' ')
    # 移除真正的换行符、制表符等控制字符
    s = s.replace('\n', ' ').replace('\r', ' ').replace('\t', ' ')
    # 移除反斜杠（可能导致JSON转义问题）
    s = s.replace('\\', '')
    # 移除多余空格
    s = ' '.join(s.split())
    return s


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
                    info["name"] = sanitize_string(match.group(1).strip())
            elif line.startswith('#!desc'):
                match = re.search(r'#!desc\s*[=:]\s*(.+)', line)
                if match:
                    info["desc"] = sanitize_string(match.group(1).strip()[:60])
            elif line.startswith('#!author'):
                match = re.search(r'#!author\s*[=:]\s*(.+)', line)
                if match:
                    info["author"] = sanitize_string(match.group(1).strip())
            elif line.startswith('#!version'):
                match = re.search(r'#!version\s*[=:]\s*(.+)', line)
                if match:
                    info["version"] = sanitize_string(match.group(1).strip())
            elif line.startswith('#!date'):
                match = re.search(r'#!date\s*[=:]\s*(.+)', line)
                if match:
                    info["date"] = sanitize_string(match.group(1).strip())
                    
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


def load_compatibility_data() -> dict:
    """加载兼容性数据"""
    if not COMPAT_FILE.exists():
        print(f"  ⚠️ 兼容性数据文件不存在: {COMPAT_FILE}")
        return {}
    
    try:
        with open(COMPAT_FILE, 'r', encoding='utf-8') as f:
            data = json.load(f)
        
        # 构建快速查找字典: name -> {compatible: bool, issues: []}
        compat_map = {}
        
        # 兼容模块
        for m in data.get("modules", {}).get("compatible", []):
            compat_map[m["name"]] = {"compatible": True, "issues": []}
        
        # Surge专属模块
        for m in data.get("modules", {}).get("surge_only", []):
            compat_map[m["name"]] = {"compatible": False, "issues": m.get("issues", [])}
        
        print(f"  ✅ 加载兼容性数据: {len(compat_map)} 个模块")
        return compat_map
        
    except Exception as e:
        print(f"  ❌ 加载兼容性数据失败: {e}")
        return {}


def generate_helper_js(modules: dict, compat_data: dict) -> str:
    """生成助手网页的JavaScript数据（紧凑格式，避免IDE格式化破坏）"""
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
            
            # 添加兼容性信息
            compat_info = compat_data.get(item["name"], {})
            if compat_info:
                js_item["srCompat"] = compat_info.get("compatible", False)
                if not compat_info.get("compatible", False) and compat_info.get("issues"):
                    # 只保留前3个问题，避免数据过大
                    js_item["srIssues"] = compat_info["issues"][:3]
            
            js_modules[cat_key]["items"].append(js_item)
    
    # 使用紧凑格式，避免IDE自动格式化破坏JSON结构
    return json.dumps(js_modules, ensure_ascii=False, separators=(',', ':'))


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


def load_shadowrocket_modules() -> dict:
    """加载Shadowrocket模块数据"""
    sr_data_path = OUTPUT_DIR / "shadowrocket_modules_data.json"
    
    if not sr_data_path.exists():
        print(f"  ⚠️ Shadowrocket模块数据不存在: {sr_data_path}")
        return {}
    
    try:
        with open(sr_data_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
        
        # 提取categories部分
        categories = data.get("categories", {})
        print(f"  ✅ 加载Shadowrocket模块: {data.get('total', 0)} 个")
        return categories
        
    except Exception as e:
        print(f"  ❌ 加载Shadowrocket模块失败: {e}")
        return {}


def generate_sr_helper_js(sr_modules: dict) -> str:
    """生成Shadowrocket模块的JavaScript数据"""
    js_modules = {}
    
    for cat_key, cat_data in sr_modules.items():
        js_modules[cat_key] = {
            "name": cat_data["name"],
            "desc": cat_data["desc"],
            "items": []
        }
        
        for item in cat_data["items"]:
            js_item = {
                "name": sanitize_string(item["name"]),
                "desc": sanitize_string(item.get("desc", ""))[:60],
                "url": item["url"]
            }
            # Shadowrocket模块添加标签
            name_lower = (item["name"] + item.get("desc", "")).lower()
            if "bilibili" in name_lower or "bili" in name_lower:
                js_item["tag"] = "bilibili"
            elif "youtube" in name_lower:
                js_item["tag"] = "youtube"
            elif "iringo" in name_lower:
                js_item["tag"] = "iringo"
            elif any(x in name_lower for x in ["boxjs", "sub_info", "timecard", "net-lsp"]):
                js_item["tag"] = "tool"
            elif "dns" in name_lower:
                js_item["tag"] = "dns"
            elif any(x in name_lower for x in ["淘宝", "京东", "拼多多", "闲鱼"]):
                js_item["tag"] = "shopping"
            
            js_modules[cat_key]["items"].append(js_item)
    
    return json.dumps(js_modules, ensure_ascii=False, separators=(',', ':'))


def update_helper_html(modules: dict, compat_data: dict):
    """更新助手网页中的模块数据"""
    helper_path = OUTPUT_DIR / "surge_module_helper.html"
    
    if not helper_path.exists():
        print("  ⚠️ surge_module_helper.html 不存在，跳过更新")
        return
        
    try:
        with open(helper_path, 'r', encoding='utf-8') as f:
            content = f.read()
            
        # 生成Surge模块数据（包含兼容性信息）
        surge_js_data = generate_helper_js(modules, compat_data)
        
        # 加载并生成Shadowrocket模块数据
        sr_modules = load_shadowrocket_modules()
        sr_js_data = generate_sr_helper_js(sr_modules) if sr_modules else "{}"
        
        # 替换Surge模块数据 - 使用更精确的正则
        surge_pattern = r'const surgeModules = \{.*?\};\s*(?=\n(?:const srModules|let copiedModules))'
        surge_replacement = f'const surgeModules = {surge_js_data};\n'
        new_content = re.sub(surge_pattern, surge_replacement, content, flags=re.DOTALL)
        
        # 检查是否已有srModules定义
        if 'const srModules = ' not in new_content:
            # 在surgeModules后面添加srModules（在let copiedModules之前）
            new_content = new_content.replace(
                'let copiedModules = ',
                f'const srModules = {sr_js_data};\nlet copiedModules = '
            )
        else:
            # 替换现有的srModules
            sr_pattern = r'const srModules = \{.*?\};\s*(?=\nlet copiedModules)'
            sr_replacement = f'const srModules = {sr_js_data};\n'
            new_content = re.sub(sr_pattern, sr_replacement, new_content, flags=re.DOTALL)
        
        with open(helper_path, 'w', encoding='utf-8') as f:
            f.write(new_content)
            
        print(f"  ✅ 更新 surge_module_helper.html (Surge + Shadowrocket)")
        
    except Exception as e:
        import traceback
        print(f"  ❌ 更新失败: {e}")
        traceback.print_exc()


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
    
    # 加载兼容性数据
    print("📱 加载Shadowrocket兼容性数据...")
    compat_data = load_compatibility_data()
    print()
    
    # 更新助手网页（唯一输出）
    print("🌐 更新助手网页...")
    update_helper_html(modules, compat_data)
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
