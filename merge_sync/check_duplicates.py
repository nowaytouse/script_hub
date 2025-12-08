#!/usr/bin/env python3
"""检查可能重复或功能相似的模块"""
import os
import glob
import re

surge_dir = 'module/surge(main)'

# 收集所有模块信息
modules = []
for pattern in ['*/*.sgmodule', '*/*.module']:
    for f in glob.glob(os.path.join(surge_dir, pattern)):
        filename = os.path.basename(f)
        subdir = os.path.basename(os.path.dirname(f))
        
        with open(f, 'r', encoding='utf-8', errors='ignore') as file:
            content = file.read()
        
        name_match = re.search(r'#!name=(.+)', content)
        desc_match = re.search(r'#!desc=(.+)', content)
        
        modules.append({
            'file': filename,
            'name': name_match.group(1).strip() if name_match else filename,
            'desc': desc_match.group(1).strip()[:100] if desc_match else '',
            'subdir': subdir,
            'path': f,
            'size': os.path.getsize(f)
        })

# 按分类统计
print('=== 模块统计 ===\n')
categories = {}
for m in modules:
    cat = m['subdir']
    if cat not in categories:
        categories[cat] = []
    categories[cat].append(m)

names = {
    'amplify_nexus': '🛠️ 增幅枢纽 (功能增强)',
    'head_expanse': '🔝 首端扩域 (广告拦截平台)',
    'narrow_pierce': '🎯 窄域穿刺 (App专属去广告)'
}

for cat in ['amplify_nexus', 'head_expanse', 'narrow_pierce']:
    mods = categories.get(cat, [])
    print(f'【{names.get(cat, cat)}】({len(mods)}个)')
    for m in sorted(mods, key=lambda x: x['name']):
        print(f'  {m["name"]}')
    print()

# 检查可能功能重复的
print('=== 功能分析 ===\n')

# BiliBili相关
bili_mods = [m for m in modules if 'bili' in m['name'].lower() or '哔哩' in m['name']]
if bili_mods:
    print(f'BiliBili相关 ({len(bili_mods)}个):')
    for m in bili_mods:
        print(f'  [{m["subdir"]}] {m["name"]}')
    print('  说明: Enhanced/Global/Redirect是功能增强，ADBlock/Helper是去广告，漫画是独立App')
    print()

# YouTube相关
yt_mods = [m for m in modules if 'youtube' in m['name'].lower()]
if yt_mods:
    print(f'YouTube相关 ({len(yt_mods)}个):')
    for m in yt_mods:
        print(f'  [{m["subdir"]}] {m["name"]}')
    print('  说明: Enhance是功能增强，remove_ads是去广告')
    print()

# 广告拦截平台
ad_mods = [m for m in modules if m['subdir'] == 'head_expanse']
if ad_mods:
    print(f'广告拦截平台 ({len(ad_mods)}个):')
    for m in ad_mods:
        print(f'  {m["name"]} ({m["size"]} bytes)')
    print('  说明: 这些是不同来源的广告规则，可以叠加使用')
    print()

print(f'\n总计: {len(modules)} 个模块')
