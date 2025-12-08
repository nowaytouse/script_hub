#!/usr/bin/env python3
"""重新下载损坏的模块"""
import os
import urllib.request
import ssl

# 需要重新下载的模块及其源URL
MODULES_TO_FIX = {
    'iRingo.Location.sgmodule': 'https://github.com/NSRingo/GeoServices/releases/latest/download/iRingo.Location.sgmodule',
}

# 分类
CATEGORIES = {
    'amplify_nexus': '『 🛠️ Amplify Nexus › 增幅枢纽 』',
    'head_expanse': '『 🔝 Head Expanse › 首端扩域 』',
    'narrow_pierce': '『 🎯 Narrow Pierce › 窄域穿刺 』',
}

def download_module(url, filepath, category):
    """下载模块并添加category"""
    print(f'下载: {url}')
    
    # 创建SSL上下文
    ctx = ssl.create_default_context()
    ctx.check_hostname = False
    ctx.verify_mode = ssl.CERT_NONE
    
    try:
        req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
        with urllib.request.urlopen(req, context=ctx, timeout=30) as response:
            content = response.read().decode('utf-8')
        
        # 检查是否有效
        if 'Not Found' in content or '404' in content or len(content) < 100:
            print(f'  ❌ 下载失败或内容无效')
            return False
        
        # 添加 #!category= 字段
        lines = content.split('\n')
        new_lines = []
        category_added = False
        
        for line in lines:
            new_lines.append(line)
            # 在 #!name= 后添加 #!category=
            if line.startswith('#!name=') and not category_added:
                new_lines.append(f'#!category={category}')
                category_added = True
        
        # 如果没有 #!name=，在开头添加
        if not category_added:
            new_lines.insert(0, f'#!category={category}')
        
        # 写入文件
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write('\n'.join(new_lines))
        
        print(f'  ✓ 保存到: {filepath}')
        return True
        
    except Exception as e:
        print(f'  ❌ 错误: {e}')
        return False

def main():
    print('=== 重新下载损坏的模块 ===\n')
    
    success = 0
    failed = 0
    
    for filename, url in MODULES_TO_FIX.items():
        # 确定目录和分类
        if 'iRingo' in filename or 'DNS' in filename or 'BiliBili' in filename:
            subdir = 'amplify_nexus'
        elif 'Ad' in filename or 'Block' in filename:
            subdir = 'head_expanse'
        else:
            subdir = 'amplify_nexus'
        
        category = CATEGORIES[subdir]
        
        # Surge 路径
        surge_path = f'module/surge(main)/{subdir}/{filename}'
        sr_path = f'module/shadowrocket/{subdir}/{filename}'
        
        # 下载到 Surge
        if download_module(url, surge_path, category):
            success += 1
            # 复制到 Shadowrocket（注释掉 category）
            with open(surge_path, 'r') as f:
                content = f.read()
            content = content.replace('#!category=', '#!category (Surge only): ')
            os.makedirs(os.path.dirname(sr_path), exist_ok=True)
            with open(sr_path, 'w') as f:
                f.write(content)
            print(f'  ✓ 同步到 Shadowrocket')
        else:
            failed += 1
    
    print(f'\n=== 完成 ===')
    print(f'成功: {success}')
    print(f'失败: {failed}')

if __name__ == '__main__':
    main()
