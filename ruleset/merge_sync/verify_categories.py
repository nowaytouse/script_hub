#!/usr/bin/env python3
"""验证模块分类字段修复结果"""
import os
import glob

surge_dir = 'module/surge(main)'
group_count = 0
category_count = 0

for pattern in ['*/*.sgmodule', '*/*.module']:
    for f in glob.glob(os.path.join(surge_dir, pattern)):
        with open(f, 'r') as file:
            content = file.read()
            if '#!group=' in content:
                group_count += 1
                print(f'还有 #!group=: {os.path.basename(f)}')
            if '#!category=' in content:
                category_count += 1

print('')
print('=== 验证结果 ===')
print(f'#!group= 字段: {group_count} 个')
print(f'#!category= 字段: {category_count} 个')
if group_count == 0:
    print('✓ 修复成功！所有模块已使用 #!category=')
else:
    print('✗ 还需修复')
