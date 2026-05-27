#!/usr/bin/env python3
"""
合并 iRingo 系列模块为 Apple 服务增强合集
"""
import re
from pathlib import Path
from datetime import datetime

ROOT = Path(__file__).resolve().parent.parent.parent
AMPLIFY_DIR = ROOT / "module" / "surge(main)" / "amplify_nexus"

# 要合并的模块
MODULES_TO_MERGE = [
    "iRingo.Maps.sgmodule",
    "iRingo.WeatherKit.sgmodule"
]

OUTPUT_FILE = AMPLIFY_DIR / "🍎 Apple服务增强合集.sgmodule"

def extract_sections(content):
    """提取模块的各个部分"""
    sections = {
        'rules': [],
        'scripts': [],
        'mitm': []
    }
    
    # 提取 [Rule] 部分
    rule_match = re.search(r'\[Rule\](.*?)(?=\n\[|\Z)', content, re.DOTALL)
    if rule_match:
        rules = [line.strip() for line in rule_match.group(1).strip().split('\n') if line.strip() and not line.strip().startswith('#')]
        sections['rules'].extend(rules)
    
    # 提取 [Script] 部分
    script_match = re.search(r'\[Script\](.*?)(?=\n\[|\Z)', content, re.DOTALL)
    if script_match:
        scripts = [line.strip() for line in script_match.group(1).strip().split('\n') if line.strip()]
        sections['scripts'].extend(scripts)
    
    # 提取 [MITM] hostname
    mitm_match = re.search(r'\[MITM\]\s*hostname\s*=\s*%APPEND%\s*(.+)', content)
    if mitm_match:
        hosts = [h.strip() for h in mitm_match.group(1).split(',') if h.strip()]
        sections['mitm'].extend(hosts)
    
    return sections

def merge_modules():
    """合并模块"""
    print("=" * 60)
    print("🍎 合并 Apple 服务增强模块")
    print("=" * 60)
    
    all_rules = []
    all_scripts = []
    all_mitm = []
    module_names = []
    
    for module_file in MODULES_TO_MERGE:
        file_path = AMPLIFY_DIR / module_file
        if not file_path.exists():
            print(f"⚠️  模块不存在: {module_file}")
            continue
        
        print(f"\n📦 读取: {module_file}")
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # 提取模块名称
        name_match = re.search(r'#!name\s*=\s*(.+)', content)
        if name_match:
            module_names.append(name_match.group(1).strip())
        
        # 提取各部分
        sections = extract_sections(content)
        all_rules.extend(sections['rules'])
        all_scripts.extend(sections['scripts'])
        all_mitm.extend(sections['mitm'])
        
        print(f"   Rules: {len(sections['rules'])}")
        print(f"   Scripts: {len(sections['scripts'])}")
        print(f"   MITM: {len(sections['mitm'])}")
    
    # 去重
    all_rules = list(dict.fromkeys(all_rules))  # 保持顺序去重
    all_mitm = list(dict.fromkeys(all_mitm))
    
    # 生成合并后的模块
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    
    merged_content = f'''#!category=『 🛠️ Amplify Nexus › 增幅枢纽 』
#!name = 🍎 Apple服务增强合集
#!desc = 整合 iRingo 系列模块\\n包含: Maps(地图增强) + WeatherKit(天气增强)\\n解锁Apple服务的国际版功能
#!author = VirgilClyne[https://github.com/VirgilClyne]
#!homepage = https://NSRingo.github.io
#!icon = https://developer.apple.com/assets/elements/icons/sf-symbols/sf-symbols-128x128.png
#!category = iRingo
#!date = {now}

# 本模块整合了以下 iRingo 模块:
# - {' | '.join(module_names)}

[Rule]
'''
    
    # 添加 Rules
    if all_rules:
        merged_content += '\n'.join(all_rules) + '\n'
    
    # 添加 Scripts
    if all_scripts:
        merged_content += '\n[Script]\n'
        merged_content += '\n'.join(all_scripts) + '\n'
    
    # 添加 MITM
    if all_mitm:
        merged_content += '\n[MITM]\n'
        merged_content += 'hostname = %APPEND% ' + ', '.join(all_mitm) + '\n'
    
    # 写入文件
    with open(OUTPUT_FILE, 'w', encoding='utf-8') as f:
        f.write(merged_content)
    
    print(f"\n✅ 合并完成!")
    print(f"   输出: {OUTPUT_FILE.name}")
    print(f"   总 Rules: {len(all_rules)}")
    print(f"   总 Scripts: {len(all_scripts)}")
    print(f"   总 MITM: {len(all_mitm)}")
    
    # 删除原始模块
    print(f"\n🗑️  删除原始模块...")
    for module_file in MODULES_TO_MERGE:
        file_path = AMPLIFY_DIR / module_file
        if file_path.exists():
            file_path.unlink()
            print(f"   ✓ 已删除: {module_file}")
    
    print("\n" + "=" * 60)

if __name__ == "__main__":
    merge_modules()
