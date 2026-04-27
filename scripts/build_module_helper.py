#!/usr/bin/env python3
"""
从零构建 Surge/Shadowrocket 模块导入助手
完全独立，不依赖任何模板或外部JSON
"""
import os
import re
from pathlib import Path
from datetime import datetime

# 路径
ROOT = Path(__file__).parent.parent
SURGE_DIR = ROOT / "module" / "surge(main)"
SR_DIR = ROOT / "module" / "shadowrocket"
OUTPUT = ROOT / "module" / "surge_module_helper.html"

# 分类定义
CATEGORIES = {
    "amplify_nexus": "🛠️ Amplify Nexus › 增幅枢纽",
    "head_expanse": "🔝 Head Expanse › 首端扩域", 
    "narrow_pierce": "🎯 Narrow Pierce › 窄域穿刺"
}

# 特殊标记
SPECIAL = {
    "Script Hub: 重写 & 规则集转换": "⭐",
    "🚫 Universal Ad-Blocking Rules (PROMAX)": "⭐"
}

# 已合并到合集的模块（应该被排除）
# 这些单独模块的功能已经完全包含在对应的合集模块中
# 为避免用户重复安装，脚本会自动排除这些模块
MERGED_MODULES = {
    # BiliBili 单独模块已合并到 BiliBili增强合集
    # 合集包含: Enhanced(UI自定义) + Global(全区搜索) + Redirect(CDN重定向) + ADBlock(去广告) + Helper(禁P2P)
    "BiliBili.Enhanced.sgmodule": "📺 BiliBili增强合集",
    "BiliBili.Global.sgmodule": "📺 BiliBili增强合集",
    "BiliBili.Redirect.sgmodule": "📺 BiliBili增强合集",
    
    # YouTube 单独模块已合并到 YouTube增强合集
    # 合集包含: Enhance(画中画/后台播放/字幕翻译) + ADBlock(去广告)
    "YouTube.Enhance.sgmodule": "📺 YouTube增强合集",
}

def scan_modules(base_dir, is_shadowrocket=False):
    """扫描模块目录"""
    modules = {}
    
    for cat_key, cat_name in CATEGORIES.items():
        cat_dir = base_dir / cat_key
        if not cat_dir.exists():
            modules[cat_key] = {"name": cat_name, "items": []}
            continue
            
        items = []
        skipped = []
        for file in sorted(cat_dir.glob("*.sgmodule" if not is_shadowrocket else "*.module")):
            # 检查是否已合并到合集
            if file.name in MERGED_MODULES:
                skipped.append(f"{file.name} → {MERGED_MODULES[file.name]}")
                continue
                
            try:
                with open(file, 'r', encoding='utf-8') as f:
                    content = f.read()
                
                # 提取元数据
                name_match = re.search(r'#!name\s*[=:]\s*(.+)', content)
                desc_match = re.search(r'#!desc\s*[=:]\s*(.+)', content)
                
                name = name_match.group(1).strip() if name_match else file.stem
                desc = desc_match.group(1).strip() if desc_match else ""
                
                # 清理特殊字符
                name = name.replace('"', '&quot;').replace("'", '&#39;')
                desc = desc.replace('"', '&quot;').replace("'", '&#39;').replace('\n', ' ')
                
                # 构建URL
                rel_path = file.relative_to(ROOT)
                url = f"https://raw.githubusercontent.com/nowaytouse/script_hub/master/{rel_path}"
                
                items.append({
                    "name": name,
                    "desc": desc,
                    "url": url,
                    "special": SPECIAL.get(name, "")
                })
            except Exception as e:
                print(f"  ⚠️  跳过 {file.name}: {e}")
        
        if skipped:
            print(f"  ℹ️  已排除 {len(skipped)} 个已合并模块:")
            for s in skipped:
                print(f"     - {s}")
        
        modules[cat_key] = {"name": cat_name, "items": items}
    
    return modules

def generate_html(surge_modules, sr_modules):
    """生成HTML"""
    
    # 统计
    surge_total = sum(len(cat["items"]) for cat in surge_modules.values())
    sr_total = sum(len(cat["items"]) for cat in sr_modules.values())
    
    html = f'''<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Surge/Shadowrocket 模块导入助手</title>
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{ font-family: -apple-system, BlinkMacSystemFont, sans-serif; background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: #fff; min-height: 100vh; padding: 20px; }}
.container {{ max-width: 1200px; margin: 0 auto; }}
h1 {{ text-align: center; font-size: 2em; margin-bottom: 10px; }}
.subtitle {{ text-align: center; opacity: 0.9; margin-bottom: 30px; }}
.app-switch {{ display: flex; justify-content: center; gap: 15px; margin-bottom: 25px; }}
.app-btn {{ background: rgba(255,255,255,0.1); border: 2px solid rgba(255,255,255,0.3); color: #fff; padding: 12px 30px; border-radius: 25px; cursor: pointer; font-size: 1em; font-weight: 600; transition: all 0.3s; }}
.app-btn:hover {{ background: rgba(255,255,255,0.2); }}
.app-btn.active {{ background: rgba(255,255,255,0.3); border-color: rgba(255,255,255,0.6); }}
.stats {{ display: flex; justify-content: center; gap: 15px; margin-bottom: 25px; flex-wrap: wrap; }}
.stat {{ background: rgba(255,255,255,0.15); padding: 15px 25px; border-radius: 15px; text-align: center; }}
.stat-num {{ font-size: 2em; font-weight: bold; }}
.stat-label {{ font-size: 0.85em; margin-top: 5px; }}
.category {{ background: rgba(255,255,255,0.1); border-radius: 20px; margin-bottom: 20px; overflow: hidden; }}
.category-header {{ padding: 20px; cursor: pointer; display: flex; align-items: center; }}
.category-header:hover {{ background: rgba(255,255,255,0.05); }}
.category-title {{ flex: 1; font-size: 1.3em; font-weight: 600; }}
.category-count {{ background: rgba(255,255,255,0.2); padding: 5px 15px; border-radius: 15px; margin-right: 10px; }}
.category-toggle {{ transition: transform 0.3s; }}
.category.collapsed .category-toggle {{ transform: rotate(-90deg); }}
.category.collapsed .modules {{ display: none; }}
.modules {{ padding: 0 20px 20px; }}
.module {{ display: flex; align-items: center; gap: 15px; padding: 15px; background: rgba(255,255,255,0.08); border-radius: 15px; margin-bottom: 10px; transition: all 0.3s; }}
.module:hover {{ background: rgba(255,255,255,0.15); transform: translateX(5px); }}
.module.copied {{ background: rgba(76,175,80,0.2); }}
.module-info {{ flex: 1; }}
.module-name {{ font-weight: 600; margin-bottom: 5px; }}
.module-desc {{ font-size: 0.85em; opacity: 0.8; }}
.copy-btn {{ background: rgba(33,150,243,0.5); border: none; color: #fff; padding: 10px 20px; border-radius: 20px; cursor: pointer; font-weight: 600; white-space: nowrap; transition: all 0.3s; }}
.copy-btn:hover {{ background: rgba(33,150,243,0.7); transform: scale(1.05); }}
.copy-btn.copied {{ background: rgba(76,175,80,0.5); }}
.toast {{ position: fixed; bottom: 30px; left: 50%; transform: translateX(-50%); background: rgba(0,0,0,0.9); padding: 15px 30px; border-radius: 25px; z-index: 1000; }}
.footer {{ text-align: center; margin-top: 40px; opacity: 0.6; font-size: 0.85em; }}
.sr-only {{ display: none; }}
</style>
</head>
<body>
<div class="container">
<h1 id="title">🚀 Surge 模块导入助手</h1>
<p class="subtitle" id="subtitle">点击复制按钮 → 粘贴到 Surge「从URL安装模块」</p>

<div class="app-switch">
<button class="app-btn active" onclick="switchApp('surge')">⚡ Surge</button>
<button class="app-btn" onclick="switchApp('shadowrocket')">🚀 Shadowrocket</button>
</div>

<div class="stats">
<div class="stat"><div class="stat-num" id="total">0</div><div class="stat-label">总模块</div></div>
<div class="stat"><div class="stat-num" id="copied">0</div><div class="stat-label">已复制</div></div>
</div>

<div id="surge-modules">
'''
    
    # 生成Surge模块
    for cat_key, cat in surge_modules.items():
        if not cat["items"]:
            continue
        html += f'''
<div class="category">
<div class="category-header" onclick="toggleCat(this)">
<div class="category-title">{cat["name"]}</div>
<div class="category-count">{len(cat["items"])}</div>
<div class="category-toggle">▼</div>
</div>
<div class="modules">
'''
        for m in cat["items"]:
            html += f'''
<div class="module">
<div class="module-info">
<div class="module-name">{m["special"]}{m["name"]}</div>
<div class="module-desc">{m["desc"]}</div>
</div>
<button class="copy-btn" onclick="copy('{m["url"]}', this)">复制</button>
</div>
'''
        html += '</div></div>\n'
    
    html += '</div>\n<div id="sr-modules" class="sr-only">\n'
    
    # 生成Shadowrocket模块
    for cat_key, cat in sr_modules.items():
        if not cat["items"]:
            continue
        html += f'''
<div class="category">
<div class="category-header" onclick="toggleCat(this)">
<div class="category-title">{cat["name"]}</div>
<div class="category-count">{len(cat["items"])}</div>
<div class="category-toggle">▼</div>
</div>
<div class="modules">
'''
        for m in cat["items"]:
            html += f'''
<div class="module">
<div class="module-info">
<div class="module-name">{m["special"]}{m["name"]}</div>
<div class="module-desc">{m["desc"]}</div>
</div>
<button class="copy-btn" onclick="copy('{m["url"]}', this)">复制</button>
</div>
'''
        html += '</div></div>\n'
    
    html += f'''</div>

<div class="footer">
生成时间: {datetime.now().strftime("%Y-%m-%d %H:%M:%S")} | Surge: {surge_total}个 | Shadowrocket: {sr_total}个
</div>
</div>

<script>
let copied = {{}};
try {{ copied = JSON.parse(localStorage.getItem('copied') || '{{}}'); }} catch(e) {{}}

function switchApp(app) {{
  document.querySelectorAll('.app-btn').forEach(b => b.classList.toggle('active', b.textContent.toLowerCase().includes(app)));
  document.getElementById('surge-modules').classList.toggle('sr-only', app !== 'surge');
  document.getElementById('sr-modules').classList.toggle('sr-only', app !== 'shadowrocket');
  document.getElementById('title').textContent = app === 'surge' ? '⚡ Surge 模块导入助手' : '🚀 Shadowrocket 模块导入助手';
  document.getElementById('subtitle').textContent = app === 'surge' ? '点击复制按钮 → 粘贴到 Surge「从URL安装模块」' : '点击复制按钮 → 粘贴到 Shadowrocket「配置-模块-添加模块」';
  updateStats();
}}

async function copy(url, btn) {{
  try {{
    await navigator.clipboard.writeText(url);
  }} catch(e) {{
    const t = document.createElement('textarea');
    t.value = url;
    t.style.position = 'fixed';
    t.style.opacity = '0';
    document.body.appendChild(t);
    t.select();
    document.execCommand('copy');
    document.body.removeChild(t);
  }}
  copied[url] = Date.now();
  localStorage.setItem('copied', JSON.stringify(copied));
  btn.textContent = '✓ 已复制';
  btn.classList.add('copied');
  btn.closest('.module').classList.add('copied');
  showToast('✓ 已复制到剪贴板');
  updateStats();
}}

function showToast(msg) {{
  const t = document.createElement('div');
  t.className = 'toast';
  t.textContent = msg;
  document.body.appendChild(t);
  setTimeout(() => t.remove(), 2000);
}}

function toggleCat(el) {{
  el.closest('.category').classList.toggle('collapsed');
}}

function updateStats() {{
  const visible = document.getElementById('surge-modules').classList.contains('sr-only') ? 'sr-modules' : 'surge-modules';
  const modules = document.querySelectorAll(`#${{visible}} .module`);
  const copiedCount = Array.from(modules).filter(m => m.classList.contains('copied')).length;
  document.getElementById('total').textContent = modules.length;
  document.getElementById('copied').textContent = copiedCount;
}}

// 初始化
document.querySelectorAll('.module').forEach(m => {{
  const btn = m.querySelector('.copy-btn');
  const url = btn.getAttribute('onclick').match(/'([^']+)'/)[1];
  if (copied[url]) {{
    btn.textContent = '✓ 已复制';
    btn.classList.add('copied');
    m.classList.add('copied');
  }}
}});
updateStats();
</script>
</body>
</html>'''
    
    return html

def main():
    print("=" * 60)
    print("🚀 从零构建模块导入助手")
    print("=" * 60)
    
    # 扫描Surge模块
    print("\n📦 扫描 Surge 模块...")
    surge_modules = scan_modules(SURGE_DIR, False)
    surge_total = sum(len(cat["items"]) for cat in surge_modules.values())
    print(f"   找到 {surge_total} 个模块")
    
    # 扫描Shadowrocket模块
    print("\n📦 扫描 Shadowrocket 模块...")
    sr_modules = scan_modules(SR_DIR, True)
    sr_total = sum(len(cat["items"]) for cat in sr_modules.values())
    print(f"   找到 {sr_total} 个模块")
    
    # 生成HTML
    print("\n🔨 生成 HTML...")
    html = generate_html(surge_modules, sr_modules)
    
    # 写入文件
    with open(OUTPUT, 'w', encoding='utf-8') as f:
        f.write(html)
    
    print(f"\n✅ 完成！")
    print(f"   输出: {OUTPUT}")
    print(f"   大小: {len(html)} 字节")
    print("=" * 60)

if __name__ == "__main__":
    main()
