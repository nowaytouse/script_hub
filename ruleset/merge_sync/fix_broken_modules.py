#!/usr/bin/env python3
"""检查并修复损坏的模块文件"""
import os
import glob

surge_dir = 'module/surge(main)'
shadowrocket_dir = 'module/shadowrocket'

broken_modules = []

def check_module(filepath):
    """检查模块是否损坏"""
    filesize = os.path.getsize(filepath)
    
    with open(filepath, 'r', encoding='utf-8', errors='ignore') as f:
        content = f.read()
    
    issues = []
    
    # 1. 文件太小（小于200字节通常是损坏的）
    if filesize < 200:
        issues.append(f'文件过小({filesize}字节)')
    
    # 2. 内容只有 "Not Found" 或类似错误
    content_lower = content.lower().strip()
    if content_lower in ['not found', '404', '404 not found']:
        issues.append('内容为404错误')
    elif 'not found' in content_lower and filesize < 500:
        issues.append('可能是404错误页面')
    
    # 3. 是HTML错误页面
    if '<!doctype html>' in content_lower or '<html' in content_lower:
        issues.append('HTML错误页面')
    
    # 4. 没有任何有效的模块元数据或内容
    has_metadata = '#!name=' in content or '#!desc=' in content
    has_content = any(s in content for s in ['[Rule]', '[Script]', '[MITM]', '[URL Rewrite]', '[Header Rewrite]', '[General]', '[Map Local]', '[Host]'])
    
    if not has_metadata and not has_content and filesize < 500:
        issues.append('缺少有效模块内容')
    
    return issues, filesize

def scan_all_modules():
    """扫描所有模块"""
    print('=== 扫描损坏的模块 ===\n')
    
    total = 0
    for base_dir in [surge_dir, shadowrocket_dir]:
        for pattern in ['*/*.sgmodule', '*/*.module', '*.sgmodule', '*.module']:
            for filepath in glob.glob(os.path.join(base_dir, pattern)):
                total += 1
                issues, filesize = check_module(filepath)
                if issues:
                    broken_modules.append((filepath, issues, filesize))
                    print(f'❌ {os.path.relpath(filepath)} ({filesize}字节)')
                    for issue in issues:
                        print(f'   - {issue}')
    
    print(f'\n=== 扫描结果 ===')
    print(f'总模块数: {total}')
    print(f'损坏模块: {len(broken_modules)} 个')
    
    if broken_modules:
        print('\n建议操作:')
        for filepath, issues, filesize in broken_modules:
            filename = os.path.basename(filepath)
            print(f'  rm -f "{filepath}"  # {", ".join(issues)}')

if __name__ == '__main__':
    scan_all_modules()
