#!/usr/bin/env python3
"""Module scan, sanitize, JSON catalog, and helper HTML (single source of truth)."""

from __future__ import annotations

import hashlib
import json
import re
import urllib.parse
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Tuple

from hub.module_sanitizer import parse_module, sanitize_file_content
from hub.common import write_file

PROMAX_FILENAME = "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule"
PROMAX_MERGE_LABEL = "🚫 Universal Ad-Blocking Rules (PROMAX)"
PROMAX_MERGED_NAME_HINTS = ("去广告", "AdBlock", "adblock", "ADBlock")
from hub.paths import REPO_RAW_PREFIX

CDN_BASE = REPO_RAW_PREFIX

CATEGORIES = {
    "amplify_nexus": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "head_expanse": "『 🔝 Head Expanse › 首端扩域 』",
    "narrow_pierce": "『 🎯 Narrow Pierce › 窄域穿刺 』",
}

CATEGORY_SHORT = {
    "amplify_nexus": "🛠️ Amplify Nexus › 增幅枢纽",
    "head_expanse": "🔝 Head Expanse › 首端扩域",
    "narrow_pierce": "🎯 Narrow Pierce › 窄域穿刺",
}

MERGED_ALIASES = {
    "bili.sgmodule": "📺 BiliBili增强合集",
    "BiliBili.Enhanced.sgmodule": "📺 BiliBili增强合集",
    "BiliBili.Global.sgmodule": "📺 BiliBili增强合集",
    "BiliBili.Redirect.sgmodule": "📺 BiliBili增强合集",
    "sukka_url_redirect.sgmodule": PROMAX_MERGE_LABEL,
    "Timecard.sgmodule": "📊 面板工具合集",
    "net-lsp-x.sgmodule": "📊 面板工具合集",
    "Sub_Info.sgmodule": "📊 面板工具合集",
    "YouTube.Enhance.sgmodule": "📺 YouTube增强合集",
    "iRingo.Maps.sgmodule": "🍎 Apple服务增强合集",
    "iRingo.WeatherKit.sgmodule": "🍎 Apple服务增强合集",
    "iRingo.News.sgmodule": "🍎 Apple服务增强合集",
    "iRingo.TV.sgmodule": "🍎 Apple服务增强合集",
    "DualSubs.Universal.sgmodule": "🍎 Apple服务增强合集",
    "boxjs.rewrite.surge.sgmodule": "🧰 Script Hub 配套工具合集",
    "Surge-Beta.sgmodule": "🧰 Script Hub 配套工具合集",
    "Script Hub 重写 & 规则集转换.sgmodule": "🧰 Script Hub 配套工具合集",
}

UI_BADGES = {
    "🧰 Script Hub 配套工具合集": "⭐",
    "🚫 Universal Ad-Blocking Rules (PROMAX)": "🔥",
}

META_CATEGORY_LINE = re.compile(r"^#!\s*category\s*[=:]\s*.*$", re.IGNORECASE)
SR_CATEGORY_LINE = re.compile(r"^#\s*category\s*:.*$", re.IGNORECASE)


def normalize_categories_tree(base_dir: Path, project_root: Path, pattern: str) -> int:
    """Align #!category with on-disk folder (Head Expanse only under head_expanse/)."""
    changed = 0
    if not base_dir.exists():
        return 0
    for path in sorted(base_dir.rglob(pattern)):
        try:
            rel = path.relative_to(base_dir)
        except ValueError:
            continue
        if len(rel.parts) < 2:
            continue
        cat_key = rel.parts[0]
        if cat_key not in CATEGORIES:
            continue

        expected = CATEGORIES[cat_key]

        new_text, modified = _apply_category_to_file(
            path.read_text(encoding="utf-8"),
            expected,
            suffix=path.suffix.lower(),
        )
        if modified:
            path.write_text(new_text, encoding="utf-8")
            changed += 1
            print(f"  📂 Category fixed: {path.relative_to(project_root)} → {expected}")
    return changed


def _apply_category_to_file(content: str, expected: str, *, suffix: str) -> Tuple[str, bool]:
    is_surge = suffix == ".sgmodule"
    category_line = f"#!category={expected}"
    sr_line = f"# category: {expected}"

    lines = content.splitlines()
    out: List[str] = []
    found = False
    modified = False
    name_idx = -1

    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.lower().startswith("#!name"):
            name_idx = len(out)

        if is_surge and META_CATEGORY_LINE.match(stripped):
            found = True
            if stripped != category_line:
                out.append(category_line)
                modified = True
            else:
                out.append(line)
            continue

        if not is_surge and SR_CATEGORY_LINE.match(stripped):
            found = True
            if stripped != sr_line:
                out.append(sr_line)
                modified = True
            else:
                out.append(line)
            continue

        if not is_surge and META_CATEGORY_LINE.match(stripped):
            found = True
            if stripped != category_line:
                out.append(category_line)
                modified = True
            else:
                out.append(line)
            continue

        out.append(line)

    if not found and is_surge:
        insert_at = name_idx + 1 if name_idx >= 0 else 0
        out.insert(insert_at, category_line)
        modified = True
    elif not found and not is_surge:
        out.insert(0, sr_line)
        modified = True

    text = "\n".join(out)
    if content.endswith("\n"):
        text += "\n"
    return text, modified


def scan_modules(project_root: Path, surge_dir: Path) -> List[Dict[str, Any]]:
    deduped: Dict[tuple, Dict[str, Any]] = {}
    for cat_key in CATEGORIES:
        cat_path = surge_dir / cat_key
        if not cat_path.exists():
            continue
        for module_file in cat_path.glob("*.sgmodule"):
            # URL encode the path for CDN compatibility (spaces, emoji, special chars)
            relative_path = str(module_file.relative_to(project_root))
            encoded_path = urllib.parse.quote(relative_path, safe='/')
            
            info: Dict[str, Any] = {
                "id": module_file.stem,
                "filename": module_file.name,
                "category": cat_key,
                "path": relative_path,
                "has_arguments": False,
                "merged_into": MERGED_ALIASES.get(module_file.name),
                "install_url": CDN_BASE + encoded_path,
            }
            try:
                meta, _ = parse_module(module_file.read_text(encoding="utf-8"))
                info["name"] = meta.get("name", module_file.stem)
                info["desc"] = meta.get("desc", "")
                info["tags"] = [t.strip() for t in meta.get("tag", "").split(",") if t.strip()]
                info["has_arguments"] = "arguments" in meta
                info["author"] = meta.get("author", "")
                info["date"] = meta.get("date", "")
                info["icon"] = meta.get(
                    "icon",
                    "",
                )
                if (
                    not info.get("merged_into")
                    and "Helper" not in module_file.name
                    and any(h in module_file.name for h in PROMAX_MERGED_NAME_HINTS)
                ):
                    info["merged_into"] = PROMAX_MERGE_LABEL
                if info["merged_into"]:
                    info["essential"] = False
                    info["note"] = f"已合并进「{info['merged_into']}」，请勿重复安装"
            except Exception as exc:
                print(f"  ❌ Error parsing {module_file.name}: {exc}")
                continue

            key = (cat_key, urllib.parse.unquote(module_file.name))
            existing = deduped.get(key)
            prefer = existing is None or (
                "%" in module_file.name and "%" not in existing["filename"]
            )
            if prefer:
                deduped[key] = info

    return sorted(deduped.values(), key=lambda x: (x["category"], x["filename"].lower()))


def sanitize_tree(base_dir: Path, project_root: Path, pattern: str) -> int:
    changed = 0
    if not base_dir.exists():
        return 0
    for path in sorted(base_dir.rglob(pattern)):
        if path.name == PROMAX_FILENAME:
            continue
        original = path.read_text(encoding="utf-8")
        cleaned = sanitize_file_content(original, dedupe=True)
        if cleaned != original:
            path.write_text(cleaned, encoding="utf-8")
            changed += 1
            print(f"  🧹 Sanitized: {path.relative_to(project_root)}")
    return changed


def write_modules_json(modules: List[Dict[str, Any]], output_path: Path) -> None:
    payload = {
        "generated": datetime.now().isoformat(),
        "total": len(modules),
        "policy": {
            "adblock": "仅安装 PROMAX（含各 App 去广告脚本；专项去广告模块勿重复安装）",
            "features": "解锁/增强/翻译等保留 Amplify Nexus 独立模块",
        },
        "modules": modules,
    }
    write_file(str(output_path), json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def _modules_to_helper_groups(
    modules: List[Dict[str, Any]],
    project_root: Path,
    *,
    shadowrocket: bool,
) -> Dict[str, Dict[str, Any]]:
    groups = {
        k: {"name": CATEGORY_SHORT[k], "items": []}
        for k in CATEGORIES
    }
    for m in modules:
        if m.get("merged_into"):
            continue
        cat = m["category"]
        if cat not in groups:
            continue
        path = m["path"]
        if shadowrocket:
            path = path.replace("modules/surge", "modules/shadowrocket", 1).replace(".sgmodule", ".module")
            if not (project_root / path).exists():
                continue
            desc = f"[🚀SR] {m.get('desc', '')}"
        else:
            desc = m.get("desc", "")
        # URL encode the path for CDN compatibility (spaces, emoji, special chars)
        encoded_path = urllib.parse.quote(path, safe='/')
        url = CDN_BASE + encoded_path
        name = m.get("name", m["filename"])
        badge = ""
        for key, icon in UI_BADGES.items():
            if key in name or key in m.get("filename", ""):
                badge = icon
                break
        groups[cat]["items"].append(
            {
                "id": hashlib.md5(url.encode()).hexdigest(),
                "name": name,
                "desc": desc,
                "author": m.get("author", ""),
                "date": m.get("date", ""),
                "icon": m.get("icon", ""),
                "url": url,
                "badge": badge,
                "filename": m.get("filename", ""),
            }
        )
    return groups


def build_helper_html(
    modules: List[Dict[str, Any]],
    project_root: Path,
    output_path: Path,
) -> None:
    """Generate modules/helper/surge_module_helper.html from modules_data scan."""
    surge_groups = _modules_to_helper_groups(modules, project_root, shadowrocket=False)
    sr_groups = _modules_to_helper_groups(modules, project_root, shadowrocket=True)
    surge_total = sum(len(c["items"]) for c in surge_groups.values())
    sr_total = sum(len(c["items"]) for c in sr_groups.values())
    catalog_generated = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    default_icon = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/docs/assets/default_icon.svg"

    html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no">
    <title>Script Hub | 模块快速复制中心</title>
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=Outfit:wght@500;700&display=swap" rel="stylesheet">
    <style>
        :root {{
            --bg-base: #f8fafc;
            --bg-list: rgba(255, 255, 255, 0.9);
            --bg-list-hover: rgba(255, 255, 255, 1);
            --text-main: #0f172a;
            --text-sub: #475569;
            --primary: #2563eb;
            --primary-hover: #1d4ed8;
            --accent: #10b981;
            --border: rgba(226, 232, 240, 0.8);
            --glass-blur: blur(20px);
            --shadow-sm: 0 2px 4px rgba(0, 0, 0, 0.04);
            --shadow-md: 0 4px 6px -1px rgba(0, 0, 0, 0.08), 0 2px 4px -2px rgba(0, 0, 0, 0.04);
        }}
        @media (prefers-color-scheme: dark) {{
            :root {{
                --bg-base: #020617;
                --bg-list: rgba(15, 23, 42, 0.7);
                --bg-list-hover: rgba(15, 23, 42, 0.95);
                --text-main: #f8fafc;
                --text-sub: #94a3b8;
                --primary: #3b82f6;
                --primary-hover: #60a5fa;
                --border: rgba(51, 65, 85, 0.6);
                --shadow-sm: 0 4px 6px rgba(0, 0, 0, 0.3);
                --shadow-md: 0 10px 15px -3px rgba(0, 0, 0, 0.4);
            }}
        }}

        * {{ margin: 0; padding: 0; box-sizing: border-box; font-family: 'Inter', -apple-system, sans-serif; }}
        body {{
            background: var(--bg-base);
            color: var(--text-main);
            min-height: 100vh;
            -webkit-font-smoothing: antialiased;
        }}

        .app-header {{
            text-align: center;
            padding: 40px 20px 20px;
            background: var(--bg-list);
            backdrop-filter: var(--glass-blur);
            -webkit-backdrop-filter: var(--glass-blur);
            border-bottom: 1px solid var(--border);
            position: sticky;
            top: 0;
            z-index: 1000;
        }}
        .app-title {{
            font-family: 'Outfit', sans-serif;
            font-size: 2.2rem;
            font-weight: 800;
            color: var(--primary);
            margin-bottom: 8px;
            letter-spacing: -0.5px;
        }}
        .app-subtitle {{ font-size: 0.95rem; color: var(--text-sub); }}
        
        .controls-row {{
            max-width: 900px;
            margin: 20px auto 0;
            display: flex;
            gap: 16px;
            align-items: center;
            justify-content: space-between;
        }}

        .nav-tabs {{
            display: flex;
            background: rgba(148, 163, 184, 0.1);
            padding: 4px;
            border-radius: 12px;
            border: 1px solid var(--border);
        }}
        .nav-btn {{
            padding: 10px 24px;
            border-radius: 8px;
            border: none;
            background: transparent;
            color: var(--text-sub);
            font-weight: 600;
            font-size: 0.95rem;
            cursor: pointer;
            transition: all 0.2s ease;
        }}
        .nav-btn:hover {{ color: var(--text-main); }}
        .nav-btn.active {{
            background: var(--bg-list-hover);
            color: var(--primary);
            box-shadow: var(--shadow-sm);
        }}

        .url-source-toggle {{
            display: flex;
            background: rgba(148, 163, 184, 0.1);
            padding: 4px;
            border-radius: 12px;
            border: 1px solid var(--border);
            white-space: nowrap;
        }}
        .url-source-btn {{
            padding: 8px 16px;
            border-radius: 8px;
            border: none;
            background: transparent;
            color: var(--text-sub);
            font-weight: 600;
            font-size: 0.85rem;
            cursor: pointer;
            transition: all 0.2s ease;
            display: flex;
            align-items: center;
            gap: 6px;
        }}
        .url-source-btn:hover {{ color: var(--text-main); }}
        .url-source-btn.active {{
            background: var(--bg-list-hover);
            color: var(--primary);
            box-shadow: var(--shadow-sm);
        }}
        .url-source-btn::before {{
            content: '';
            width: 8px;
            height: 8px;
            border-radius: 50%;
            background: currentColor;
        }}

        .search-input {{
            flex: 1;
            padding: 12px 20px;
            border-radius: 12px;
            border: 1px solid var(--border);
            background: var(--bg-list);
            color: var(--text-main);
            font-size: 1rem;
            transition: all 0.2s;
        }}
        .search-input:focus {{
            outline: none;
            border-color: var(--primary);
            box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.2);
        }}

        .content-container {{
            max-width: 900px;
            margin: 0 auto;
            padding: 30px 20px 80px;
        }}

        .category-group {{
            margin-bottom: 40px;
        }}
        .category-header {{
            font-size: 1.25rem;
            font-weight: 700;
            color: var(--text-main);
            margin-bottom: 16px;
            padding-bottom: 8px;
            border-bottom: 2px solid var(--border);
            display: flex;
            align-items: center;
        }}

        .list-layout {{
            display: flex;
            flex-direction: column;
            gap: 12px;
        }}

        .list-item {{
            background: var(--bg-list);
            border: 1px solid var(--border);
            border-radius: 16px;
            padding: 16px;
            display: flex;
            align-items: center;
            gap: 20px;
            transition: all 0.2s ease;
            box-shadow: var(--shadow-sm);
            overflow: hidden;
        }}
        .list-item:hover {{
            transform: translateY(-2px);
            box-shadow: var(--shadow-md);
            border-color: rgba(59, 130, 246, 0.3);
            background: var(--bg-list-hover);
        }}

        .item-icon-wrap {{
            width: 84px;
            height: 84px;
            border-radius: 18px;
            background: rgba(148, 163, 184, 0.1);
            overflow: hidden;
            flex-shrink: 0;
            display: flex;
            align-items: center;
            justify-content: center;
            border: 1px solid var(--border);
        }}
        .item-icon {{
            width: 100%;
            height: 100%;
            object-fit: cover;
        }}

        .item-content {{
            flex: 1;
            min-width: 0;
        }}
        .item-title {{
            font-size: 1.15rem;
            font-weight: 700;
            color: var(--text-main);
            margin-bottom: 6px;
            line-height: 1.3;
            word-wrap: break-word;
            word-break: break-word;
        }}
        .item-meta {{
            font-size: 0.85rem;
            color: var(--text-sub);
            margin-bottom: 6px;
            display: flex;
            flex-wrap: wrap;
            gap: 12px;
            word-wrap: break-word;
            word-break: break-all;
        }}
        .item-desc {{
            font-size: 0.95rem;
            line-height: 1.5;
            color: var(--text-sub);
            word-wrap: break-word;
            word-break: break-word;
            display: -webkit-box;
            -webkit-line-clamp: 2;
            -webkit-box-orient: vertical;
            overflow: hidden;
        }}

        .item-action {{
            flex-shrink: 0;
            padding-left: 10px;
        }}
        .copy-btn {{
            padding: 12px 24px;
            border-radius: 12px;
            border: none;
            background: rgba(59, 130, 246, 0.1);
            color: var(--primary);
            font-weight: 700;
            font-size: 1rem;
            cursor: pointer;
            transition: all 0.2s;
            min-width: 110px;
        }}
        .list-item:hover .copy-btn {{
            background: var(--primary);
            color: white;
            box-shadow: 0 4px 12px rgba(59, 130, 246, 0.3);
        }}
        .copy-btn.success {{
            background: var(--accent) !important;
            color: white !important;
            box-shadow: 0 4px 12px rgba(16, 185, 129, 0.3) !important;
        }}

        .toast {{
            position: fixed;
            bottom: 30px;
            left: 50%;
            transform: translateX(-50%) translateY(20px);
            background: var(--text-main);
            color: var(--bg-base);
            padding: 12px 24px;
            border-radius: 50px;
            font-weight: 600;
            box-shadow: 0 10px 25px rgba(0,0,0,0.2);
            opacity: 0;
            transition: all 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
            z-index: 1000;
        }}
        .toast.show {{
            opacity: 1;
            transform: translateX(-50%) translateY(0);
        }}

        @media (max-width: 600px) {{
            .controls-row {{ flex-direction: column; align-items: stretch; }}
            .nav-tabs {{ justify-content: stretch; }}
            .nav-btn {{ flex: 1; padding: 12px 16px; }}
            .url-source-toggle {{ justify-content: center; }}
            .url-source-btn {{ flex: 1; justify-content: center; }}
            .list-item {{ flex-direction: column; align-items: flex-start; gap: 16px; padding: 20px; }}
            .item-action {{ width: 100%; padding-left: 0; }}
            .copy-btn {{ width: 100%; padding: 14px; }}
            .item-icon-wrap {{ width: 72px; height: 72px; }}
        }}
        
    </style>
</head>
<body>
    <header class="app-header">
        <h1 class="app-title">Script Hub</h1>
        <p class="app-subtitle">为了便于代理软件导入，点击右侧按钮即可快速复制链接</p>
        
        <div class="controls-row">
            <div class="nav-tabs">
                <button class="nav-btn active" onclick="switchApp('surge')">Surge ({surge_total})</button>
                <button class="nav-btn" onclick="switchApp('shadowrocket')">Shadowrocket ({sr_total})</button>
            </div>
            <div class="url-source-toggle">
                <button class="url-source-btn active" onclick="switchUrlSource('cdn')" title="jsDelivr CDN 加速（推荐）">CDN</button>
                <button class="url-source-btn" onclick="switchUrlSource('github')" title="GitHub Raw 直连（CDN 缓存未更新时使用）">GitHub</button>
            </div>
            <input type="text" id="search" class="search-input" placeholder="输入关键字极速搜索..." oninput="render()" autocomplete="off">
        </div>
    </header>

    <main id="content" class="content-container"></main>
    <div id="toast" class="toast">复制成功 ✓</div>

    <script>
        const surgeData = {json.dumps(surge_groups, ensure_ascii=False)};
        const srData = {json.dumps(sr_groups, ensure_ascii=False)};
        const defaultIcon = "{default_icon}";
        
        let currentApp = 'surge';
        let urlSource = 'cdn'; // 'cdn' or 'github'
        const copiedSet = new Set();
        let renderTimeout;

        function switchApp(app) {{
            currentApp = app;
            document.querySelectorAll('.nav-btn').forEach(b => 
                b.classList.toggle('active', b.textContent.toLowerCase().includes(app))
            );
            copiedSet.clear(); // 清除复制状态
            render();
        }}

        function switchUrlSource(source) {{
            urlSource = source;
            document.querySelectorAll('.url-source-btn').forEach(b => 
                b.classList.toggle('active', b.textContent.toLowerCase() === source || (source === 'cdn' && b.textContent === 'CDN'))
            );
            copiedSet.clear(); // 切换 URL 源后清除复制状态
            render();
        }}

        function convertUrl(url) {{
            if (urlSource === 'github') {{
                // 将 cdn.jsdelivr.net/gh/owner/repo@branch/ 转换为 raw.githubusercontent.com/owner/repo/branch/
                return url.replace(
                    /https:\/\/cdn\.jsdelivr\.net\/gh\/([^\/]+)\/([^\/]+)@([^\/]+)\//,
                    'https://raw.githubusercontent.com/$1/$2/$3/'
                );
            }}
            return url; // 默认返回 CDN URL
        }}

        function escapeHTML(str) {{
            if (!str) return '';
            return String(str).replace(/[&<>'"]/g, tag => ({{
                '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;'
            }}[tag]));
        }}

        function render() {{
            clearTimeout(renderTimeout);
            const data = currentApp === 'surge' ? surgeData : srData;
            const term = document.getElementById('search').value.toLowerCase().trim();
            const container = document.getElementById('content');
            
            let html = '';

            for (const [k, cat] of Object.entries(data)) {{
                const items = cat.items.filter(i =>
                    (i.name && i.name.toLowerCase().includes(term)) || 
                    (i.desc && i.desc.toLowerCase().includes(term)));
                
                if (!items.length) continue;
                
                html += `
                    <div class="category-group">
                        <h3 class="category-header">${{escapeHTML(cat.name)}}</h3>
                        <div class="list-layout">
                `;
                
                items.forEach(i => {{
                    const actualUrl = convertUrl(i.url);
                    const isCopied = copiedSet.has(actualUrl);
                    const btnClass = isCopied ? 'copy-btn success' : 'copy-btn';
                    const btnText = isCopied ? '已复制 ✓' : '复制链接';
                    const iconSrc = (i.icon && !i.icon.includes('default_icon.png') && !i.icon.includes('default_icon.svg')) ? i.icon : defaultIcon;
                    
                    html += `
                        <div class="list-item">
                            <div class="item-icon-wrap">
                                <img class="item-icon" src="${{escapeHTML(iconSrc)}}" onerror="this.src='${{defaultIcon}}'">
                            </div>
                            <div class="item-content">
                                <h4 class="item-title">${{escapeHTML(i.badge || '')}} ${{escapeHTML(i.name)}}</h4>
                                <div class="item-meta">
                                    <span style="flex:1;">${{i.author ? `<span>@${{escapeHTML(i.author)}}</span>` : ''}}</span>
                                    ${{i.date ? `<span style="white-space:nowrap; color:var(--primary);">${{escapeHTML(i.date)}}</span>` : ''}}
                                </div>
                                <p class="item-desc">${{escapeHTML(i.desc)}}</p>
                            </div>
                            <div class="item-action">
                                <button class="${{btnClass}}" onclick="copy('${{actualUrl}}', this)">${{btnText}}</button>
                            </div>
                        </div>
                    `;
                }});
                html += '</div></div>';
            }}

            if (!html) {{
                html = `
                    <div style="text-align:center; padding: 60px 20px; color: var(--text-sub);">
                        <p style="font-size: 1.1rem;">未找到匹配的模块</p>
                    </div>
                `;
            }}
            
            container.innerHTML = html;
        }}

        async function copy(url, btn) {{
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
            
            copiedSet.add(url);
            btn.textContent = '已复制 ✓'; 
            btn.className = 'copy-btn success';
            
            const toast = document.getElementById('toast');
            toast.classList.remove('show');
            void toast.offsetWidth;
            toast.classList.add('show');
            setTimeout(() => toast.classList.remove('show'), 2000);
        }}

        requestAnimationFrame(render);
    </script>
</body>
</html>"""
    write_file(str(output_path), html)


def write_shadowrocket_modules_json(
    modules: List[Dict[str, Any]],
    output_path: Path,
    project_root: Path,
) -> None:
    """Generate modules/helper/shadowrocket_modules_data.json from modules catalog."""
    categories_data = {}
    total_sr_modules = 0
    cat_keys = ["amplify_nexus", "head_expanse"]
    category_descs = {
        "amplify_nexus": "功能增强类模块",
        "head_expanse": "核心拦截/重写",
    }
    for cat in cat_keys:
        categories_data[cat] = {
            "name": CATEGORY_SHORT[cat],
            "desc": category_descs[cat],
            "items": [],
        }

    for m in modules:
        if m.get("merged_into"):
            continue
        cat = m["category"]
        if cat not in categories_data:
            continue
        path = m["path"]
        sr_rel_path = path.replace("modules/surge", "modules/shadowrocket", 1).replace(".sgmodule", ".module")
        if not (project_root / sr_rel_path).exists():
            continue
        desc = m.get("desc", "")
        if desc and not desc.startswith("[🚀SR]"):
            desc = f"[🚀SR] {desc}"
        # URL encode the path for CDN compatibility (spaces, emoji, special chars)
        encoded_sr_path = urllib.parse.quote(sr_rel_path, safe='/')
        url = CDN_BASE + encoded_sr_path
        categories_data[cat]["items"].append(
            {
                "name": m.get("name", m["filename"]),
                "desc": desc,
                "url": url,
            }
        )
        total_sr_modules += 1

    payload = {
        "generated": datetime.now().isoformat(),
        "categories": categories_data,
        "total": total_sr_modules,
    }
    write_file(str(output_path), json.dumps(payload, ensure_ascii=False, indent=2) + "\n")

