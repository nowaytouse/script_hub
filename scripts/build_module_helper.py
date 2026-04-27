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
    meta = {"name": "", "desc": "", "author": "", "icon": "", "date": "", "version": ""}
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
        if match: meta[key] = match.group(1).strip()
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
            if file.name in MERGED_MODULES: continue
            try:
                content = file.read_text(encoding='utf-8')
                meta = parse_metadata(content, is_shadowrocket)
                name = meta["name"] or file.stem
                desc = meta["desc"] or "暂无描述"
                is_outdated = False
                if meta["date"]:
                    try:
                        module_date = datetime.strptime(meta["date"], "%Y-%m-%d %H:%M:%S")
                        if (now - module_date).days > OUTDATED_DAYS: is_outdated = True
                    except: pass
                if is_outdated: continue
                rel_path = file.relative_to(ROOT)
                url = f"https://raw.githubusercontent.com/nowaytouse/script_hub/master/{rel_path}"
                items.append({
                    "id": hashlib.md5(url.encode()).hexdigest(), # 唯一ID用于跟踪
                    "name": name, "desc": desc, "author": meta["author"],
                    "icon": meta["icon"] or "https://raw.githubusercontent.com/nowaytouse/script_hub/master/docs/assets/default_icon.png",
                    "url": url, "badge": SPECIAL.get(name, ""), "filename": file.name
                })
            except Exception as e: print(f"  ⚠️ Error parsing {file.name}: {e}")
        modules[cat_key] = {"name": cat_name, "items": items}
    return modules

def generate_html(surge_modules, sr_modules):
    import hashlib
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
            --bg: #f8fafc;
            --text: #1e293b;
            --primary: #3b82f6;
            --accent: #10b981;
            --card: #ffffff;
            --border: #e2e8f0;
            --copied-bg: #f1f5f9;
        }}
        @media (prefers-color-scheme: dark) {{
            :root {{
                --bg: #0f172a;
                --text: #f1f5f9;
                --card: #1e293b;
                --border: #334155;
                --copied-bg: #020617;
            }}
        }}
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, system-ui, sans-serif; background: var(--bg); color: var(--text); line-height: 1.5; }}
        
        .header {{ padding: 30px 20px; text-align: center; background: var(--card); border-bottom: 1px solid var(--border); }}
        .nav {{ position: sticky; top: 0; z-index: 100; background: var(--card); border-bottom: 1px solid var(--border); padding: 10px; display: flex; justify-content: center; gap: 10px; box-shadow: 0 4px 6px -1px rgba(0,0,0,0.1); }}
        
        .btn {{ padding: 8px 24px; border-radius: 10px; border: 1px solid var(--border); background: var(--card); color: var(--text); cursor: pointer; font-weight: 600; transition: all 0.2s; }}
        .btn.active {{ background: var(--primary); color: white; border-color: var(--primary); }}
        
        .search-bar {{ max-width: 800px; margin: 20px auto; padding: 0 20px; }}
        #search {{ width: 100%; padding: 14px; border-radius: 12px; border: 2px solid var(--border); background: var(--card); color: var(--text); font-size: 1rem; transition: border-color 0.2s; }}
        #search:focus {{ outline: none; border-color: var(--primary); }}
        
        .container {{ max-width: 1000px; margin: 0 auto; padding: 0 20px 40px; }}
        .category-title {{ font-size: 0.85rem; text-transform: uppercase; letter-spacing: 0.1em; color: var(--primary); margin: 30px 0 12px 5px; font-weight: 800; }}
        
        .module-list {{ background: var(--card); border-radius: 16px; border: 1px solid var(--border); overflow: hidden; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }}
        .module-item {{ display: flex; align-items: center; padding: 14px 20px; border-bottom: 1px solid var(--border); gap: 15px; transition: background 0.2s; }}
        .module-item:last-child {{ border-bottom: none; }}
        .module-item:hover {{ background: rgba(0,0,0,0.02); }}
        
        /* 已处理样式 */
        .module-item.is-copied {{ background: var(--copied-bg); opacity: 0.7; }}
        .module-item.is-copied .name {{ text-decoration: line-through; opacity: 0.5; }}
        
        .icon {{ width: 44px; height: 44px; border-radius: 10px; object-fit: cover; flex-shrink: 0; background: #f1f5f9; border: 1px solid var(--border); }}
        .info {{ flex: 1; min-width: 0; }}
        .name {{ font-weight: 700; font-size: 1.05rem; margin-bottom: 2px; color: var(--text); display: flex; align-items: center; gap: 8px; }}
        .desc {{ font-size: 0.9rem; opacity: 0.6; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
        .author {{ font-size: 0.75rem; color: var(--primary); font-weight: 500; background: rgba(59, 130, 246, 0.1); padding: 2px 8px; border-radius: 6px; }}
        
        .copy-btn {{ padding: 10px 20px; border-radius: 10px; border: none; background: var(--primary); color: white; font-size: 0.9rem; font-weight: 600; cursor: pointer; transition: all 0.2s; min-width: 85px; }}
        .copy-btn:hover {{ filter: brightness(1.1); }}
        .copy-btn.success {{ background: var(--accent); }}
        
        .toast {{ position: fixed; bottom: 30px; left: 50%; transform: translateX(-50%); background: #1e293b; color: #fff; padding: 12px 24px; border-radius: 12px; font-size: 0.95rem; font-weight: 600; display: none; z-index: 1000; box-shadow: 0 10px 15px -3px rgba(0,0,0,0.2); }}
    </style>
</head>
<body>
    <div class="header">
        <h2 style="font-size: 1.8rem; margin-bottom: 5px;">Script Hub</h2>
        <p style="opacity: 0.6; font-size: 0.95rem;">点击“复制”标记已完成的项目</p>
    </div>

    <div class="nav">
        <button class="btn active" onclick="switchApp('surge')">⚡ Surge ({surge_total})</button>
        <button class="btn" onclick="switchApp('shadowrocket')">🚀 Shadowrocket ({sr_total})</button>
    </div>

    <div class="search-bar">
        <input type="text" id="search" placeholder="搜索模块名称、描述、作者..." oninput="render()">
    </div>

    <div id="content" class="container"></div>
    <div id="toast" class="toast">✓ 链接已复制</div>

    <script>
        const surgeData = {json.dumps(surge_modules, ensure_ascii=False)};
        const srData = {json.dumps(sr_modules, ensure_ascii=False)};
        let currentApp = 'surge';
        
        // 核心：使用 Set 记录已复制的 URL
        const copiedSet = new Set();

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
                const items = cat.items.filter(i => 
                    i.name.toLowerCase().includes(term) || 
                    i.desc.toLowerCase().includes(term) ||
                    (i.author && i.author.toLowerCase().includes(term))
                );
                if (items.length === 0) continue;

                html += `<div class="category-title">${{cat.name}}</div><div class="module-list">`;
                items.forEach(i => {{
                    const isCopied = copiedSet.has(i.url);
                    html += `
                        <div class="module-item ${{isCopied ? 'is-copied' : ''}}" id="row-${{i.id}}">
                            <img class="icon" src="${{i.icon}}" onerror="this.src='https://raw.githubusercontent.com/nowaytouse/script_hub/master/docs/assets/default_icon.png'">
                            <div class="info">
                                <div class="name">
                                    ${{i.badge}} ${{i.name}} 
                                    ${{i.author ? `<span class="author">@${{i.author}}</span>` : ''}}
                                </div>
                                <div class="desc" title="${{i.desc}}">${{i.desc}}</div>
                            </div>
                            <button class="copy-btn ${{isCopied ? 'success' : ''}}" onclick="copy('${{i.url}}', '${{i.id}}', this)">
                                ${{isCopied ? '✓ 已复制' : '复制'}}
                            </button>
                        </div>`;
                }});
                html += `</div>`;
            }}
            container.innerHTML = html || '<p style="text-align:center; padding:60px; opacity:0.5; font-size:1.1rem;">未发现匹配模块</p>';
        }}

        async function copy(url, id, btn) {{
            try {{ await navigator.clipboard.writeText(url); }} 
            catch(e) {{
                const t = document.createElement('textarea'); t.value = url; document.body.appendChild(t);
                t.select(); document.execCommand('copy'); document.body.removeChild(t);
            }}
            
            // 标记已复制
            copiedSet.add(url);
            
            // 立即应用样式
            const row = document.getElementById('row-' + id);
            if(row) row.classList.add('is-copied');
            btn.textContent = '✓ 已复制';
            btn.classList.add('success');

            const toast = document.getElementById('toast');
            toast.style.display = 'block';
            setTimeout(() => {{ toast.style.display = 'none'; }}, 1500);
        }}

        render();
    </script>
</body>
</html>"""
    import hashlib
    with open(OUTPUT, "w", encoding="utf-8") as f:
        f.write(html)

def main():
    import hashlib
    print("=" * 60)
    print("🚀 正在构建具备“状态标记”功能的模块导入助手...")
    surge_modules = scan_modules(SURGE_DIR, False)
    sr_modules = scan_modules(SR_DIR, True)
    generate_html(surge_modules, sr_modules)
    print(f"✅ 完成: {OUTPUT}")
    print("=" * 60)

if __name__ == "__main__":
    import hashlib
    main()
