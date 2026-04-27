import os
import re
import json
from pathlib import Path
from datetime import datetime, timedelta

# 路径
ROOT = Path(__file__).parent.parent
SURGE_DIR = ROOT / "module" / "surge(main)"
SR_DIR = ROOT / "module" / "shadowrocket"
OUTPUT = ROOT / "module" / "surge_module_helper.html"

# 过时阈值
OUTDATED_DAYS = 365

# 分类定义
CATEGORIES = {
    "amplify_nexus": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "head_expanse": "『 🔝 Head Expanse › 首端扩域 』", 
    "narrow_pierce": "『 🎯 Narrow Pierce › 窄域穿刺 』"
}

# 特殊标记
SPECIAL = {
    "Script Hub: 重写 & 规则集转换": "⭐",
    "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style)": "🔥"
}

# 已合并排除
MERGED_MODULES = {
    "BiliBili.Enhanced.sgmodule": "📺 BiliBili增强合集",
    "BiliBili.Global.sgmodule": "📺 BiliBili增强合集",
    "BiliBili.Redirect.sgmodule": "📺 BiliBili增强合集",
    "YouTube.Enhance.sgmodule": "📺 YouTube增强合集",
    "iRingo.Maps.sgmodule": "🍎 Apple服务增强合集",
    "iRingo.WeatherKit.sgmodule": "🍎 Apple服务增强合集",
}

def parse_metadata(content, is_sr=False):
    """通用的元数据解析逻辑"""
    meta = {
        "name": "",
        "desc": "",
        "author": "",
        "icon": "",
        "date": "",
        "version": ""
    }
    
    patterns = {
        "name": r'^[#!][\s!]*(?:name)\s*[=:]\s*(.+)',
        "desc": r'^[#!][\s!]*(?:desc)\s*[=:]\s*(.+)',
        "author": r'^[#!][\s!]*(?:author)\s*[=:]\s*(.+)',
        "icon": r'^[#!][\s!]*(?:icon)\s*[=:]\s*(.+)',
        "date": r'^[#!][\s!]*(?:date)\s*[=:]\s*(.+)',
        "version": r'^[#!][\s!]*(?:version)\s*[=:]\s*(.+)'
    }
    
    for key, pattern in patterns.items():
        match = re.search(pattern, content, re.MULTILINE | re.IGNORECASE)
        if match:
            meta[key] = match.group(1).strip()
            
    return meta

def scan_modules(base_dir, is_shadowrocket=False):
    modules = {}
    now = datetime.now()
    
    for cat_key, cat_name in CATEGORIES.items():
        cat_dir = base_dir / cat_key
        if not cat_dir.exists():
            modules[cat_key] = {"name": cat_name, "items": []}
            continue
            
        items = []
        for file in sorted(cat_dir.glob("*.sgmodule" if not is_shadowrocket else "*.module")):
            if file.name in MERGED_MODULES:
                continue
                
            try:
                content = file.read_text(encoding='utf-8')
                meta = parse_metadata(content, is_shadowrocket)
                
                name = meta["name"] or file.stem
                desc = meta["desc"] or "暂无描述"
                
                # 检查过期
                is_outdated = False
                if meta["date"]:
                    try:
                        module_date = datetime.strptime(meta["date"], "%Y-%m-%d %H:%M:%S")
                        if (now - module_date).days > OUTDATED_DAYS:
                            is_outdated = True
                    except: pass
                
                if is_outdated: continue

                # 构建URL
                rel_path = file.relative_to(ROOT)
                url = f"https://raw.githubusercontent.com/nowaytouse/script_hub/master/{rel_path}"
                
                items.append({
                    "name": name,
                    "desc": desc,
                    "author": meta["author"],
                    "icon": meta["icon"] or "https://raw.githubusercontent.com/nowaytouse/script_hub/master/docs/assets/default_icon.png",
                    "url": url,
                    "badge": SPECIAL.get(name, ""),
                    "filename": file.name
                })
            except Exception as e:
                print(f"  ⚠️  Error parsing {file.name}: {e}")
        
        modules[cat_key] = {"name": cat_name, "items": items}
    
    return modules

def generate_html(surge_modules, sr_modules):
    surge_total = sum(len(cat["items"]) for cat in surge_modules.values())
    sr_total = sum(len(cat["items"]) for cat in sr_modules.values())
    
    html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Script Hub | 模块列表</title>
    <style>
        :root {{
            --bg: #f3f4f6;
            --text: #1f2937;
            --primary: #2563eb;
            --accent: #059669;
            --card: #ffffff;
            --border: #e5e7eb;
        }}
        @media (prefers-color-scheme: dark) {{
            :root {{
                --bg: #111827;
                --text: #f3f4f6;
                --card: #1f2937;
                --border: #374151;
            }}
        }}
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, system-ui, sans-serif; background: var(--bg); color: var(--text); line-height: 1.5; }}
        
        .header {{ padding: 30px 20px; text-align: center; border-bottom: 1px solid var(--border); background: var(--card); }}
        .nav {{ position: sticky; top: 0; z-index: 100; background: var(--card); border-bottom: 1px solid var(--border); padding: 10px; display: flex; justify-content: center; gap: 10px; }}
        
        .btn {{ padding: 8px 20px; border-radius: 8px; border: 1px solid var(--border); background: var(--card); color: var(--text); cursor: pointer; font-weight: 600; transition: all 0.2s; }}
        .btn.active {{ background: var(--primary); color: white; border-color: var(--primary); }}
        
        .search-bar {{ max-width: 800px; margin: 20px auto; padding: 0 20px; }}
        #search {{ width: 100%; padding: 12px; border-radius: 10px; border: 1px solid var(--border); background: var(--card); color: var(--text); font-size: 1rem; }}
        
        .container {{ max-width: 1000px; margin: 0 auto; padding: 20px; }}
        .category-title {{ font-size: 0.9rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--primary); margin: 30px 0 10px 5px; font-weight: 700; }}
        
        .module-list {{ background: var(--card); border-radius: 12px; border: 1px solid var(--border); overflow: hidden; }}
        .module-item {{ display: flex; align-items: center; padding: 12px 15px; border-bottom: 1px solid var(--border); gap: 15px; }}
        .module-item:last-child {{ border-bottom: none; }}
        .module-item:hover {{ background: rgba(0,0,0,0.02); }}
        
        .icon {{ width: 40px; height: 40px; border-radius: 8px; object-fit: cover; flex-shrink: 0; background: #eee; }}
        .info {{ flex: 1; min-width: 0; }}
        .name {{ font-weight: 600; font-size: 1rem; margin-bottom: 2px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
        .desc {{ font-size: 0.85rem; opacity: 0.7; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
        .author {{ font-size: 0.75rem; opacity: 0.5; font-weight: 400; }}
        
        .copy-btn {{ padding: 8px 16px; border-radius: 6px; border: none; background: var(--primary); color: white; font-size: 0.85rem; font-weight: 600; cursor: pointer; white-space: nowrap; }}
        .copy-btn.success {{ background: var(--accent); }}
        
        .toast {{ position: fixed; bottom: 20px; left: 50%; transform: translateX(-50%); background: #000; color: #fff; padding: 10px 20px; border-radius: 20px; font-size: 0.9rem; display: none; z-index: 1000; }}
        
        @media (max-width: 600px) {{
            .module-item {{ padding: 10px; gap: 10px; }}
            .icon {{ width: 32px; height: 32px; }}
            .copy-btn {{ padding: 6px 12px; font-size: 0.8rem; }}
        }}
    </style>
</head>
<body>
    <div class="header">
        <h2>Script Hub 模块列表</h2>
        <p style="opacity: 0.6; font-size: 0.9rem;">点击复制链接至 Surge / Shadowrocket</p>
    </div>

    <div class="nav">
        <button class="btn active" onclick="switchApp('surge')">Surge ({surge_total})</button>
        <button class="btn" onclick="switchApp('shadowrocket')">Shadowrocket ({sr_total})</button>
    </div>

    <div class="search-bar">
        <input type="text" id="search" placeholder="搜索模块..." oninput="render()">
    </div>

    <div id="content" class="container"></div>
    <div id="toast" class="toast">✓ 链接已复制</div>

    <script>
        const surgeData = {json.dumps(surge_modules, ensure_ascii=False)};
        const srData = {json.dumps(sr_modules, ensure_ascii=False)};
        let currentApp = 'surge';

        function switchApp(app) {{
            currentApp = app;
            document.querySelectorAll('.btn').forEach(b => b.classList.toggle('active', b.textContent.toLowerCase().includes(app)));
            render();
        }}

        function render() {{
            const data = currentApp === 'surge' ? surgeData : srData;
            const term = document.getElementById('search').value.toLowerCase();
            const container = document.getElementById('content');
            let html = '';

            for (const k in data) {{
                const cat = data[k];
                const items = cat.items.filter(i => i.name.toLowerCase().includes(term) || i.desc.toLowerCase().includes(term));
                if (items.length === 0) continue;

                html += `<div class="category-title">${{cat.name}}</div><div class="module-list">`;
                items.forEach(i => {{
                    html += `
                        <div class="module-item">
                            <img class="icon" src="${{i.icon}}" onerror="this.src='https://raw.githubusercontent.com/nowaytouse/script_hub/master/docs/assets/default_icon.png'">
                            <div class="info">
                                <div class="name">${{i.badge}} ${{i.name}} <span class="author">by ${{i.author || 'Anon'}}</span></div>
                                <div class="desc" title="${{i.desc}}">${{i.desc}}</div>
                            </div>
                            <button class="copy-btn" onclick="copy('${{i.url}}', this)">复制</button>
                        </div>`;
                }});
                html += `</div>`;
            }}
            container.innerHTML = html || '<p style="text-align:center; padding:40px; opacity:0.5;">未发现匹配模块</p>';
        }}

        async function copy(url, btn) {{
            try {{ await navigator.clipboard.writeText(url); }} 
            catch(e) {{
                const t = document.createElement('textarea'); t.value = url; document.body.appendChild(t);
                t.select(); document.execCommand('copy'); document.body.removeChild(t);
            }}
            const old = btn.textContent; btn.textContent = '✓'; btn.classList.add('success');
            const toast = document.getElementById('toast'); toast.style.display = 'block';
            setTimeout(() => {{ btn.textContent = old; btn.classList.remove('success'); toast.style.display = 'none'; }}, 1500);
        }}

        render();
    </script>
</body>
</html>"""
    with open(OUTPUT, "w", encoding="utf-8") as f:
        f.write(html)

def main():
    print("=" * 60)
    print("🚀 正在构建紧凑型模块导入助手...")
    surge_modules = scan_modules(SURGE_DIR, False)
    sr_modules = scan_modules(SR_DIR, True)
    generate_html(surge_modules, sr_modules)
    print(f"✅ 完成: {OUTPUT}")
    print("=" * 60)

if __name__ == "__main__":
    main()
