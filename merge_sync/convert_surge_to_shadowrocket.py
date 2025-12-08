#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Surge模块转换为Shadowrocket兼容版本
功能：
1. 移除/转换Surge专属特性
2. 生成Shadowrocket专属模块目录
3. 更新网页端数据
"""

import os
import re
import json
import shutil
from pathlib import Path
from datetime import datetime

# 项目根目录
SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
SURGE_MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)"
SR_MODULE_DIR = PROJECT_ROOT / "module" / "shadowrocket"
OUTPUT_DIR = PROJECT_ROOT / "module"

# GitHub raw URL基础路径
GITHUB_RAW_BASE_SR = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/shadowrocket"

# Surge专属特性转换规则
CONVERSION_RULES = {
    # 规则类型转换
    "extended-matching": "",  # 移除extended-matching标记
    "pre-matching": "",       # 移除pre-matching标记
    
    # 拒绝类型转换
    "REJECT-DROP": "REJECT",           # 静默拒绝 → 普通拒绝
    "REJECT-TINYGIF": "REJECT",        # 返回GIF → 普通拒绝
    "REJECT-NO-DROP": "REJECT",        # 拒绝不丢弃 → 普通拒绝
    
    # 规则追加/插入 - 移除这些标记，保留规则本身
    "%APPEND%": "",
    "%INSERT%": "",
}

# 需要移除的Surge专属行
REMOVE_PATTERNS = [
    r'^#!update-interval\s*=.*$',      # 更新间隔
    r'^#!ability\s*=.*$',              # 能力声明
]

# 脚本路径转换（部分脚本可能需要特殊处理）
SCRIPT_CONVERSIONS = {
    # 如果有特定脚本需要转换，在这里添加
}


def convert_module_content(content: str, filename: str) -> tuple[str, list]:
    """
    转换模块内容为Shadowrocket兼容格式
    返回: (转换后内容, 转换记录列表)
    """
    changes = []
    lines = content.split('\n')
    new_lines = []
    
    for line in lines:
        original_line = line
        modified = False
        
        # 检查是否需要移除整行
        should_remove = False
        for pattern in REMOVE_PATTERNS:
            if re.match(pattern, line.strip(), re.IGNORECASE):
                should_remove = True
                changes.append(f"移除: {line.strip()[:50]}")
                break
        
        if should_remove:
            continue
        
        # 应用转换规则
        for surge_feature, sr_replacement in CONVERSION_RULES.items():
            if surge_feature in line:
                # 特殊处理规则类型
                if surge_feature in ["extended-matching", "pre-matching"]:
                    # 移除规则选项中的这些标记
                    line = re.sub(rf',\s*{surge_feature}', '', line)
                    line = re.sub(rf'{surge_feature}\s*,', '', line)
                    line = re.sub(rf'{surge_feature}', '', line)
                elif surge_feature in ["REJECT-DROP", "REJECT-TINYGIF", "REJECT-NO-DROP"]:
                    # 替换拒绝类型
                    line = line.replace(surge_feature, sr_replacement)
                elif surge_feature in ["%APPEND%", "%INSERT%"]:
                    # 移除追加/插入标记
                    line = line.replace(surge_feature, sr_replacement)
                
                if line != original_line:
                    modified = True
        
        # 清理多余的逗号和空格
        line = re.sub(r',\s*,', ',', line)
        line = re.sub(r',\s*$', '', line)
        line = re.sub(r'^\s*,', '', line)
        
        if modified and line != original_line:
            changes.append(f"转换: {original_line.strip()[:40]} → {line.strip()[:40]}")
        
        new_lines.append(line)
    
    # 修改模块描述，标记为Shadowrocket版本
    result = '\n'.join(new_lines)
    
    # 在#!desc后添加[SR]标记
    result = re.sub(
        r'(#!desc\s*[=:]\s*)(.+)',
        r'\1[🚀SR] \2',
        result
    )
    
    return result, changes


def process_all_modules():
    """处理所有模块，生成Shadowrocket版本"""
    
    # 创建Shadowrocket模块目录
    if SR_MODULE_DIR.exists():
        # 逐个删除子目录内容
        for item in SR_MODULE_DIR.iterdir():
            if item.is_dir():
                shutil.rmtree(item, ignore_errors=True)
            else:
                item.unlink(missing_ok=True)
    else:
        SR_MODULE_DIR.mkdir(parents=True)
    
    # 创建分类子目录
    categories = ["amplify_nexus", "head_expanse", "narrow_pierce"]
    for cat in categories:
        (SR_MODULE_DIR / cat).mkdir(exist_ok=True)
    
    stats = {
        "total": 0,
        "converted": 0,
        "skipped": 0,
        "categories": {}
    }
    
    conversion_log = []
    
    print("=" * 60)
    print("🚀 Surge → Shadowrocket 模块转换工具")
    print("=" * 60)
    print()
    
    for cat in categories:
        cat_path = SURGE_MODULE_DIR / cat
        if not cat_path.exists():
            continue
        
        stats["categories"][cat] = {"total": 0, "converted": 0}
        
        print(f"📁 处理分类: {cat}")
        
        for module_file in sorted(cat_path.glob("*.sgmodule")):
            stats["total"] += 1
            stats["categories"][cat]["total"] += 1
            
            try:
                with open(module_file, 'r', encoding='utf-8') as f:
                    content = f.read()
                
                # 转换内容
                converted_content, changes = convert_module_content(content, module_file.name)
                
                # 保存到Shadowrocket目录
                sr_file = SR_MODULE_DIR / cat / module_file.name
                with open(sr_file, 'w', encoding='utf-8') as f:
                    f.write(converted_content)
                
                stats["converted"] += 1
                stats["categories"][cat]["converted"] += 1
                
                if changes:
                    conversion_log.append({
                        "file": module_file.name,
                        "category": cat,
                        "changes": changes
                    })
                    print(f"  ✅ {module_file.name} ({len(changes)} 处转换)")
                else:
                    print(f"  ✅ {module_file.name} (无需转换)")
                    
            except Exception as e:
                stats["skipped"] += 1
                print(f"  ❌ {module_file.name}: {e}")
        
        print()
    
    # 保存转换日志
    log_file = OUTPUT_DIR / "shadowrocket_conversion_log.json"
    with open(log_file, 'w', encoding='utf-8') as f:
        json.dump({
            "generated": datetime.now().isoformat(),
            "stats": stats,
            "conversions": conversion_log
        }, f, ensure_ascii=False, indent=2)
    
    print("=" * 60)
    print(f"✅ 转换完成!")
    print(f"   总模块: {stats['total']}")
    print(f"   已转换: {stats['converted']}")
    print(f"   跳过: {stats['skipped']}")
    print(f"   日志: {log_file}")
    print("=" * 60)
    
    return stats


def generate_sr_module_data():
    """生成Shadowrocket模块数据（用于网页）"""
    
    modules = {}
    categories = {
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
    
    for cat_key, cat_info in categories.items():
        cat_path = SR_MODULE_DIR / cat_key
        if not cat_path.exists():
            continue
        
        modules[cat_key] = {
            "name": cat_info["name"],
            "desc": cat_info["desc"],
            "items": []
        }
        
        for module_file in sorted(cat_path.glob("*.sgmodule")):
            # 解析模块信息
            info = {"name": module_file.stem, "desc": ""}
            try:
                with open(module_file, 'r', encoding='utf-8') as f:
                    content = f.read()
                for line in content.split('\n')[:20]:
                    if line.startswith('#!name'):
                        match = re.search(r'#!name\s*[=:]\s*(.+)', line)
                        if match:
                            info["name"] = match.group(1).strip()
                    elif line.startswith('#!desc'):
                        match = re.search(r'#!desc\s*[=:]\s*(.+)', line)
                        if match:
                            info["desc"] = match.group(1).strip()[:60]
            except:
                pass
            
            # 生成URL
            from urllib.parse import quote
            encoded_filename = quote(module_file.name, safe='')
            url = f"{GITHUB_RAW_BASE_SR}/{cat_key}/{encoded_filename}"
            
            modules[cat_key]["items"].append({
                "name": info["name"],
                "desc": info["desc"],
                "url": url
            })
    
    return modules


def update_helper_html_with_sr(sr_modules: dict):
    """更新网页，添加Shadowrocket模块数据"""
    
    helper_path = OUTPUT_DIR / "surge_module_helper.html"
    if not helper_path.exists():
        print("  ⚠️ surge_module_helper.html 不存在")
        return
    
    # 生成SR模块的JS数据
    sr_js_data = json.dumps(sr_modules, ensure_ascii=False, separators=(',', ':'))
    
    with open(helper_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # 检查是否已有srModules变量
    if 'const srModules = ' in content:
        # 替换现有数据
        pattern = r'const srModules = \{[^;]*\};'
        replacement = f'const srModules = {sr_js_data};'
        content = re.sub(pattern, replacement, content, flags=re.DOTALL)
    else:
        # 在modules变量后添加srModules
        pattern = r'(const modules = \{[^;]*\};)'
        replacement = f'\\1\nconst srModules = {sr_js_data};'
        content = re.sub(pattern, replacement, content, flags=re.DOTALL)
    
    with open(helper_path, 'w', encoding='utf-8') as f:
        f.write(content)
    
    print(f"  ✅ 更新 surge_module_helper.html (添加SR模块数据)")


if __name__ == "__main__":
    # 1. 转换所有模块
    stats = process_all_modules()
    
    print()
    print("🌐 生成网页数据...")
    
    # 2. 生成SR模块数据
    sr_modules = generate_sr_module_data()
    
    # 3. 更新网页
    update_helper_html_with_sr(sr_modules)
    
    # 4. 保存SR模块数据
    sr_data_file = OUTPUT_DIR / "shadowrocket_modules_data.json"
    with open(sr_data_file, 'w', encoding='utf-8') as f:
        json.dump({
            "generated": datetime.now().isoformat(),
            "total": sum(len(cat["items"]) for cat in sr_modules.values()),
            "categories": sr_modules
        }, f, ensure_ascii=False, indent=2)
    print(f"  ✅ 保存 {sr_data_file}")
    
    print()
    print("🎉 全部完成!")
