#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
从小火箭导入模块到Surge（增量更新 + 去重）
"""

import os
import re
import hashlib
from pathlib import Path
from urllib.parse import unquote

# 路径配置
SR_DIR = Path("/Users/nyamiiko/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules")
SURGE_DIR = Path(__file__).parent.parent / "module" / "surge(main)"

# 分类映射
CATEGORY_MAP = {
    "amplify_nexus": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "head_expanse": "『 🔝 Head Expanse › 首端扩域 』",
    "narrow_pierce": "『 🎯 Narrow Pierce › 窄域穿刺 』",
}

def get_module_name(content):
    """从模块内容提取 #!name"""
    match = re.search(r'^#!name\s*[=:]\s*(.+)$', content, re.MULTILINE)
    return match.group(1).strip() if match else None

def get_content_hash(content):
    """计算内容哈希（忽略 #!category 和 #!url）"""
    lines = content.split('\n')
    filtered = [l for l in lines if not l.startswith('#!category') and not l.startswith('#!url')]
    return hashlib.md5('\n'.join(filtered).encode()).hexdigest()[:8]

def classify_module(name, content):
    """根据模块名称和内容自动分类"""
    name_lower = name.lower()
    
    # 功能增强类
    if any(k in name_lower for k in ['wifi', 'calling', 'helper', 'enhanced', 'dns', 'iringo', 'dualsubs', 'tiktok', '助手']):
        return "amplify_nexus"
    
    # 广告拦截平台类
    if any(k in name_lower for k in ['ad-block', 'adblock', 'firewall', 'script hub', '广告平台', '广告联盟', 'universal']):
        return "head_expanse"
    
    # App专项去广告（默认）
    return "narrow_pierce"

def main():
    print("=" * 60)
    print("📦 小火箭模块导入工具（增量更新 + 去重）")
    print("=" * 60)
    
    if not SR_DIR.exists():
        print(f"❌ 小火箭目录不存在: {SR_DIR}")
        return
    
    # 收集现有模块信息
    existing = {}  # name_lower -> {path, hash, name}
    for cat in ["amplify_nexus", "head_expanse", "narrow_pierce"]:
        cat_path = SURGE_DIR / cat
        if not cat_path.exists():
            continue
        for f in cat_path.glob("*.sgmodule"):
            try:
                content = f.read_text(encoding='utf-8')
                name = get_module_name(content) or f.stem
                existing[name.lower()] = {
                    "path": f,
                    "hash": get_content_hash(content),
                    "name": name
                }
            except:
                pass
    
    print(f"现有模块数: {len(existing)}\n")
    
    # 统计
    added = updated = duplicate = skipped = 0
    
    # 处理小火箭模块
    for sr_file in sorted(SR_DIR.glob("*.*module")):
        filename = sr_file.name
        
        # 跳过以 __ 开头的（我们同步过去的）
        if filename.startswith("__"):
            continue
        
        # 跳过超大文件
        size = sr_file.stat().st_size
        if size > 100000:
            print(f"⏭️  跳过大文件: {filename} ({size//1024}KB)")
            skipped += 1
            continue
        
        try:
            content = sr_file.read_text(encoding='utf-8')
        except:
            print(f"❌ 读取失败: {filename}")
            continue
        
        # 获取模块名称
        module_name = get_module_name(content) or unquote(sr_file.stem)
        content_hash = get_content_hash(content)
        name_key = module_name.lower()
        
        # 检查是否已存在
        if name_key in existing:
            ex = existing[name_key]
            if ex["hash"] == content_hash:
                print(f"🔄 重复: {module_name}")
                duplicate += 1
            else:
                print(f"📝 已存在但内容不同: {module_name}")
                print(f"   → 保留现有: {ex['path'].name}")
                skipped += 1
            continue
        
        # 新模块
        category = classify_module(module_name, content)
        
        # 清理文件名
        safe_name = re.sub(r'[<>:"/\\|?*]', '', module_name)
        if not safe_name.endswith('.sgmodule'):
            safe_name += '.sgmodule'
        
        dst_path = SURGE_DIR / category / safe_name
        
        # 处理内容：添加 #!category，移除 #!url
        lines = content.split('\n')
        new_lines = []
        cat_added = False
        
        for line in lines:
            if line.startswith('#!url'):
                continue
            if line.startswith('#!category'):
                if not cat_added:
                    new_lines.append(f"#!category={CATEGORY_MAP[category]}")
                    cat_added = True
                continue
            if line.startswith('#!name') and not cat_added:
                new_lines.append(f"#!category={CATEGORY_MAP[category]}")
                cat_added = True
            new_lines.append(line)
        
        if not cat_added:
            new_lines.insert(0, f"#!category={CATEGORY_MAP[category]}")
        
        # 写入
        dst_path.write_text('\n'.join(new_lines), encoding='utf-8')
        print(f"✅ 新增: {module_name} → {category}/")
        added += 1
    
    print("\n" + "=" * 60)
    print(f"统计: 新增 {added}, 重复 {duplicate}, 跳过 {skipped}")
    print("=" * 60)

if __name__ == "__main__":
    main()
