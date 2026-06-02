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

from module_sanitizer import parse_module, sanitize_file_content
from common import write_file

PROMAX_FILENAME = "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule"
PROMAX_MERGE_LABEL = "🚫 Universal Ad-Blocking Rules (PROMAX)"
PROMAX_MERGED_NAME_HINTS = ("去广告", "AdBlock", "adblock", "ADBlock")
CDN_BASE = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/"

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
    "BiliBili.Enhanced.sgmodule": "📺 BiliBili增强合集",
    "BiliBili.Global.sgmodule": "📺 BiliBili增强合集",
    "BiliBili.Redirect.sgmodule": "📺 BiliBili增强合集",
    "YouTube.Enhance.sgmodule": "📺 YouTube增强合集",
    "iRingo.Maps.sgmodule": "🍎 Apple服务增强合集",
    "iRingo.WeatherKit.sgmodule": "🍎 Apple服务增强合集",
}

UI_BADGES = {
    "Script Hub: 重写 & 规则集转换": "⭐",
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
            info: Dict[str, Any] = {
                "id": module_file.stem,
                "filename": module_file.name,
                "category": cat_key,
                "path": str(module_file.relative_to(project_root)),
                "has_arguments": False,
                "merged_into": MERGED_ALIASES.get(module_file.name),
                "install_url": CDN_BASE + urllib.parse.quote(str(module_file.relative_to(project_root))),
            }
            try:
                meta, _ = parse_module(module_file.read_text(encoding="utf-8"))
                info["name"] = meta.get("name", module_file.stem)
                info["desc"] = meta.get("desc", "")
                info["tags"] = [t.strip() for t in meta.get("tag", "").split(",") if t.strip()]
                info["has_arguments"] = "arguments" in meta
                info["author"] = meta.get("author", "")
                info["icon"] = meta.get(
                    "icon",
                    f"{CDN_BASE}docs/assets/default_icon.png",
                )
                if (
                    not info.get("merged_into")
                    and cat_key == "narrow_pierce"
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
        url = CDN_BASE + urllib.parse.quote(path)
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
                "icon": m.get("icon", f"{CDN_BASE}docs/assets/default_icon.png"),
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
    """Generate modules/surge_module_helper.html from modules_data scan."""
    surge_groups = _modules_to_helper_groups(modules, project_root, shadowrocket=False)
    sr_groups = _modules_to_helper_groups(modules, project_root, shadowrocket=True)
    surge_total = sum(len(c["items"]) for c in surge_groups.values())
    sr_total = sum(len(c["items"]) for c in sr_groups.values())

    html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Script Hub | 模块列表</title>
    <style>
        :root {{
            --bg: #f8fafc; --text: #1e293b; --primary: #3b82f6; --accent: #10b981;
            --card: #ffffff; --border: #e2e8f0; --copied-bg: #f1f5f9;
        }}
        @media (prefers-color-scheme: dark) {{
            :root {{
                --bg: #0f172a; --text: #f1f5f9; --card: #1e293b;
                --border: #334155; --copied-bg: #020617;
            }}
        }}
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, system-ui, sans-serif; background: var(--bg); color: var(--text); }}
        .header {{ padding: 30px 20px; text-align: center; background: var(--card); border-bottom: 1px solid var(--border); }}
        .nav {{ position: sticky; top: 0; z-index: 100; background: var(--card); border-bottom: 1px solid var(--border);
            padding: 10px; display: flex; justify-content: center; gap: 10px; }}
        .btn {{ padding: 8px 24px; border-radius: 10px; border: 1px solid var(--border); background: var(--card);
            cursor: pointer; font-weight: 600; }}
        .btn.active {{ background: var(--primary); color: white; border-color: var(--primary); }}
        .search-bar {{ max-width: 800px; margin: 20px auto; padding: 0 20px; }}
        #search {{ width: 100%; padding: 14px; border-radius: 12px; border: 2px solid var(--border); background: var(--card); color: var(--text); }}
        .container {{ max-width: 1000px; margin: 0 auto; padding: 0 20px 40px; }}
        .category-title {{ font-size: 0.85rem; color: var(--primary); margin: 30px 0 12px 5px; font-weight: 800; }}
        .module-list {{ background: var(--card); border-radius: 16px; border: 1px solid var(--border); overflow: hidden; }}
        .module-item {{ display: flex; align-items: center; padding: 14px 20px; border-bottom: 1px solid var(--border); gap: 15px; }}
        .module-item:last-child {{ border-bottom: none; }}
        .module-item.is-copied {{ background: var(--copied-bg); opacity: 0.7; }}
        .icon {{ width: 44px; height: 44px; border-radius: 10px; object-fit: cover; flex-shrink: 0; }}
        .info {{ flex: 1; min-width: 0; }}
        .name {{ font-weight: 700; font-size: 1.05rem; }}
        .desc {{ font-size: 0.9rem; opacity: 0.6; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
        .author {{ font-size: 0.75rem; color: var(--primary); margin-left: 8px; }}
        .copy-btn {{ padding: 10px 20px; border-radius: 10px; border: none; background: var(--primary); color: white; font-weight: 600; cursor: pointer; min-width: 85px; }}
        .copy-btn.success {{ background: var(--accent); }}
        .toast {{ position: fixed; bottom: 30px; left: 50%; transform: translateX(-50%); background: #1e293b; color: #fff;
            padding: 12px 24px; border-radius: 12px; display: none; z-index: 1000; }}
    </style>
</head>
<body>
    <div class="header">
        <h2>Script Hub</h2>
        <p style="opacity:0.6">PROMAX = 广告规则；带配置的增强模块请单独安装</p>
    </div>
    <div class="nav">
        <button class="btn active" onclick="switchApp('surge')">⚡ Surge ({surge_total})</button>
        <button class="btn" onclick="switchApp('shadowrocket')">🚀 Shadowrocket ({sr_total})</button>
    </div>
    <div class="search-bar"><input type="text" id="search" placeholder="搜索模块…" oninput="render()"></div>
    <div id="content" class="container"></div>
    <div id="toast" class="toast">✓ 链接已复制</div>
    <script>
        const surgeData = {json.dumps(surge_groups, ensure_ascii=False)};
        const srData = {json.dumps(sr_groups, ensure_ascii=False)};
        let currentApp = 'surge';
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
                    i.name.toLowerCase().includes(term) || i.desc.toLowerCase().includes(term));
                if (!items.length) continue;
                html += `<div class="category-title">${{cat.name}}</div><div class="module-list">`;
                items.forEach(i => {{
                    const isCopied = copiedSet.has(i.url);
                    html += `<div class="module-item ${{isCopied ? 'is-copied' : ''}}" id="row-${{i.id}}">
                        <img class="icon" src="${{i.icon}}" onerror="this.src='{CDN_BASE}docs/assets/default_icon.png'">
                        <div class="info"><div class="name">${{i.badge}} ${{i.name}} ${{i.author ? `<span class="author">@${{i.author}}</span>` : ''}}</div>
                        <div class="desc">${{i.desc}}</div></div>
                        <button class="copy-btn ${{isCopied ? 'success' : ''}}" onclick="copy('${{i.url}}','${{i.id}}',this)">${{isCopied ? '✓ 已复制' : '复制'}}</button></div>`;
                }});
                html += '</div>';
            }}
            container.innerHTML = html || '<p style="text-align:center;padding:40px;opacity:0.5">无匹配</p>';
        }}
        async function copy(url, id, btn) {{
            try {{ await navigator.clipboard.writeText(url); }} catch(e) {{
                const t = document.createElement('textarea'); t.value = url; document.body.appendChild(t);
                t.select(); document.execCommand('copy'); document.body.removeChild(t);
            }}
            copiedSet.add(url);
            document.getElementById('row-' + id)?.classList.add('is-copied');
            btn.textContent = '✓ 已复制'; btn.classList.add('success');
            const toast = document.getElementById('toast');
            toast.style.display = 'block';
            setTimeout(() => toast.style.display = 'none', 1500);
        }}
        render();
    </script>
</body>
</html>"""
    write_file(str(output_path), html)


def write_shadowrocket_modules_json(
    modules: List[Dict[str, Any]],
    output_path: Path,
    project_root: Path,
) -> None:
    """Generate modules/shadowrocket_modules_data.json from modules catalog."""
    categories_data = {}
    total_sr_modules = 0
    cat_keys = ["amplify_nexus", "head_expanse", "narrow_pierce"]
    category_descs = {
        "amplify_nexus": "功能增强类模块",
        "head_expanse": "广告拦截平台类",
        "narrow_pierce": "App专项去广告",
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
        url = CDN_BASE + urllib.parse.quote(sr_rel_path)
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

