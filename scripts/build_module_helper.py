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
    
    # 针对不同平台的正则模式
    # Surge: #!name=...  Shadowrocket: # name: ...
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
    
    with open(ROOT / "module/surge_module_helper.html", "w", encoding="utf-8") as f:
        # 这里我将写入一整个非常精美的HTML
        html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Script Hub | 模块导入助手</title>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;600;700&display=swap" rel="stylesheet">
    <style>
        :root {{
            --primary: #6366f1;
            --primary-hover: #4f46e5;
            --bg: #0f172a;
            --card-bg: #1e293b;
            --text-main: #f8fafc;
            --text-dim: #94a3b8;
            --accent: #10b981;
        }}
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ 
            font-family: 'Inter', -apple-system, system-ui, sans-serif; 
            background-color: var(--bg); 
            color: var(--text-main); 
            line-height: 1.5;
            padding-bottom: 50px;
        }}
        .header {{
            background: linear-gradient(to right, #1e293b, #0f172a);
            padding: 40px 20px;
            text-align: center;
            border-bottom: 1px solid rgba(255,255,255,0.1);
        }}
        h1 {{ font-size: 2.5rem; margin-bottom: 10px; background: linear-gradient(to right, #818cf8, #34d399); -webkit-background-clip: text; -webkit-text-fill-color: transparent; }}
        .nav {{
            display: flex;
            justify-content: center;
            gap: 15px;
            margin: 25px 0;
            position: sticky;
            top: 0;
            z-index: 100;
            background: rgba(15, 23, 42, 0.8);
            backdrop-filter: blur(10px);
            padding: 15px;
        }}
        .nav-btn {{
            background: var(--card-bg);
            border: 1px solid rgba(255,255,255,0.1);
            color: var(--text-dim);
            padding: 10px 25px;
            border-radius: 12px;
            cursor: pointer;
            font-weight: 600;
            transition: all 0.2s;
        }}
        .nav-btn.active {{
            background: var(--primary);
            color: white;
            border-color: var(--primary);
            box-shadow: 0 4px 12px rgba(99, 102, 241, 0.3);
        }}
        .search-container {{
            max-width: 600px;
            margin: 0 auto 30px;
            padding: 0 20px;
        }}
        #search {{
            width: 100%;
            padding: 12px 20px;
            border-radius: 12px;
            border: 1px solid rgba(255,255,255,0.1);
            background: var(--card-bg);
            color: white;
            font-size: 1rem;
        }}
        .container {{ max-width: 1200px; margin: 0 auto; padding: 0 20px; }}
        .category-section {{ margin-bottom: 40px; }}
        .category-title {{ 
            font-size: 1.2rem; 
            color: var(--accent); 
            margin-bottom: 20px; 
            padding-left: 10px;
            border-left: 4px solid var(--accent);
        }}
        .module-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
            gap: 20px;
        }}
        .module-card {{
            background: var(--card-bg);
            border-radius: 16px;
            padding: 20px;
            border: 1px solid rgba(255,255,255,0.05);
            transition: transform 0.2s, border-color 0.2s;
            display: flex;
            flex-direction: column;
        }}
        .module-card:hover {{
            transform: translateY(-4px);
            border-color: var(--primary);
        }}
        .module-header {{ display: flex; gap: 15px; margin-bottom: 15px; align-items: center; }}
        .module-icon {{ width: 48px; height: 48px; border-radius: 12px; object-fit: cover; background: #334155; }}
        .module-name-wrapper {{ flex: 1; min-width: 0; }}
        .module-name {{ font-weight: 700; font-size: 1.1rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
        .module-author {{ font-size: 0.8rem; color: var(--text-dim); }}
        .module-desc {{ 
            font-size: 0.9rem; 
            color: var(--text-dim); 
            margin-bottom: 20px; 
            flex: 1;
            display: -webkit-box;
            -webkit-line-clamp: 3;
            -webkit-box-orient: vertical;
            overflow: hidden;
        }}
        .copy-btn {{
            width: 100%;
            padding: 12px;
            border-radius: 10px;
            border: none;
            background: var(--primary);
            color: white;
            font-weight: 600;
            cursor: pointer;
            transition: background 0.2s;
        }}
        .copy-btn:hover {{ background: var(--primary-hover); }}
        .copy-btn.success {{ background: var(--accent); }}
        
        .toast {{
            position: fixed;
            bottom: 30px;
            left: 50%;
            transform: translateX(-50%);
            background: var(--accent);
            color: white;
            padding: 12px 30px;
            border-radius: 50px;
            font-weight: 600;
            box-shadow: 0 10px 25px rgba(0,0,0,0.3);
            display: none;
            z-index: 1000;
        }}
        
        .hidden {{ display: none !important; }}
        
        @media (max-width: 600px) {{
            .module-grid {{ grid-template-columns: 1fr; }}
            h1 {{ font-size: 1.8rem; }}
        }}
    </style>
</head>
<body>
    <div class="header">
        <h1>Script Hub</h1>
        <p style="color: var(--text-dim)">一站式 Surge & Shadowrocket 模块增强库</p>
    </div>

    <div class="nav">
        <button class="nav-btn active" onclick="switchApp('surge')">⚡ Surge ({surge_total})</button>
        <button class="nav-btn" onclick="switchApp('shadowrocket')">🚀 Shadowrocket ({sr_total})</button>
    </div>

    <div class="search-container">
        <input type="text" id="search" placeholder="搜索模块名称或描述..." oninput="filterModules()">
    </div>

    <div id="main-content" class="container">
        <!-- 内容由 JS 动态切换 -->
    </div>

    <div id="toast" class="toast">✓ 链接已复制，请在 App 中粘贴安装</div>

    <script>
        const surgeData = {json.dumps(surge_modules, ensure_ascii=False)};
        const srData = {json.dumps(sr_modules, ensure_ascii=False)};
        let currentApp = 'surge';

        function switchApp(app) {{
            currentApp = app;
            document.querySelectorAll('.nav-btn').forEach(btn => {{
                btn.classList.toggle('active', btn.textContent.toLowerCase().includes(app));
            }});
            render();
        }}

        function render() {{
            const container = document.getElementById('main-content');
            const data = currentApp === 'surge' ? surgeData : srData;
            const searchTerm = document.getElementById('search').value.toLowerCase();
            
            let html = '';
            for (const catKey in data) {{
                const cat = data[catKey];
                const filteredItems = cat.items.filter(item => 
                    item.name.toLowerCase().includes(searchTerm) || 
                    item.desc.toLowerCase().includes(searchTerm)
                );

                if (filteredItems.length === 0) continue;

                html += `
                    <div class="category-section">
                        <div class="category-title">${{cat.name}}</div>
                        <div class="module-grid">
                            ${{filteredItems.map(item => `
                                <div class="module-card">
                                    <div class="module-header">
                                        <img class="module-icon" src="${{item.icon}}" onerror="this.src='https://raw.githubusercontent.com/nowaytouse/script_hub/master/docs/assets/default_icon.png'">
                                        <div class="module-name-wrapper">
                                            <div class="module-name">${{item.badge}} ${{item.name}}</div>
                                            <div class="module-author">by ${{item.author || 'Anonymous'}}</div>
                                        </div>
                                    </div>
                                    <div class="module-desc" title="${{item.desc}}">${{item.desc}}</div>
                                    <button class="copy-btn" onclick="copyUrl('${{item.url}}', this)">复制模块链接</button>
                                </div>
                            `).join('')}}
                        </div>
                    </div>
                `;
            }}
            container.innerHTML = html || '<div style="text-align:center; padding:50px; color:var(--text-dim)">未找到匹配模块</div>';
        }}

        async function copyUrl(url, btn) {{
            try {{
                await navigator.clipboard.writeText(url);
            }} catch(e) {{
                const t = document.createElement('textarea');
                t.value = url;
                document.body.appendChild(t);
                t.select();
                document.execCommand('copy');
                document.body.removeChild(t);
            }}
            
            const originalText = btn.textContent;
            btn.textContent = '✓ 已复制';
            btn.classList.add('success');
            
            const toast = document.getElementById('toast');
            toast.style.display = 'block';
            
            setTimeout(() => {{
                btn.textContent = originalText;
                btn.classList.remove('success');
                toast.style.display = 'none';
            }}, 2000);
        }}

        function filterModules() {{
            render();
        }}

        // 初始渲染
        render();
    </script>
</body>
</html>"""
        f.write(html)

def main():
    print("=" * 60)
    print("🚀 正在构建全新的模块导入助手...")
    
    surge_modules = scan_modules(SURGE_DIR, False)
    sr_modules = scan_modules(SR_DIR, True)
    
    generate_html(surge_modules, sr_modules)
    
    print(f"✅ 构建成功: {OUTPUT}")
    print("=" * 60)

if __name__ == "__main__":
    main()
