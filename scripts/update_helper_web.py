#!/usr/bin/env python3
import json
import os
import re

ROOT = "/Users/nyamiiko/Downloads/GitHub/script_hub"
DATA_PATH = os.path.join(ROOT, "module/modules_data.json")
HTML_PATH = os.path.join(ROOT, "module/surge_module_helper.html")

def update():
    if not os.path.exists(DATA_PATH):
        print("Data file not found.")
        return

    with open(DATA_PATH, "r", encoding="utf-8") as f:
        data = json.load(f)

    # Categories definitions (matching HTML display)
    DISPLAY_NAMES = {
        "amplify_nexus": {"name": "🛠️ Amplify Nexus › 增幅枢纽", "desc": "功能增强类模块"},
        "head_expanse": {"name": "🔝 Head Expanse › 首端扩域", "desc": "广告拦截平台类"},
        "narrow_pierce": {"name": "🎯 Narrow Pierce › 窄域穿刺", "desc": "App专项去广告"}
    }

    # Grouping logic
    surge_grouped = {k: {"name": v["name"], "desc": v["desc"], "items": []} for k, v in DISPLAY_NAMES.items()}
    sr_grouped = {k: {"name": v["name"], "desc": v["desc"], "items": []} for k, v in DISPLAY_NAMES.items()}

    base_url = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/"

    # Pre-defined special attributes for common modules (to maintain UI richness)
    SPECIAL_ATTRS = {
        "🚫 Universal Ad-Blocking Rules (PROMAX)": {"essential": True, "tag": "merged"},
        "Script Hub: 重写 & 规则集转换": {"essential": True, "tag": "tool"},
        "BoxJs": {"tag": "tool"},
        "📺 BiliBili增强合集": {"tag": "bilibili"},
        "📺 YouTube增强合集": {"tag": "youtube"},
        "节假日信息": {"tag": "tool"},
        "网络信息 𝕏": {"tag": "tool"},
        "机场订阅信息": {"tag": "tool"}
    }

    for m in data["modules"]:
        cat = m["category"]
        if cat not in surge_grouped:
            continue
        
        name = m["name"]
        
        # Base item properties
        item_base = {
            "name": name,
            "desc": m["desc"],
        }
        
        # Apply special attributes
        if name in SPECIAL_ATTRS:
            item_base.update(SPECIAL_ATTRS[name])
        elif m.get("tags"):
            item_base["tag"] = m["tags"][0] # Just use first tag for UI color

        # Surge item
        surge_item = item_base.copy()
        surge_item["url"] = base_url + m["path"]
        surge_grouped[cat]["items"].append(surge_item)

        # Shadowrocket item (check if exists)
        sr_path = m["path"].replace("surge(main)", "shadowrocket").replace(".sgmodule", ".module")
        if os.path.exists(os.path.join(ROOT, sr_path)):
            sr_item = item_base.copy()
            sr_item["desc"] = f"[🚀SR] {m['desc']}"
            sr_item["url"] = base_url + sr_path
            sr_grouped[cat]["items"].append(sr_item)

    # Read HTML
    with open(HTML_PATH, "r", encoding="utf-8") as f:
        html = f.read()

    # Inject Surge data (ensure_ascii=False to keep Unicode, but this properly escapes quotes)
    surge_json = json.dumps(surge_grouped, ensure_ascii=False)
    html = re.sub(
        r'const surgeModules = \{.*?\};',
        f'const surgeModules = {surge_json};',
        html,
        flags=re.DOTALL
    )

    # Inject Shadowrocket data
    sr_json = json.dumps(sr_grouped, ensure_ascii=False)
    html = re.sub(
        r'const srModules = \{.*?\};',
        f'const srModules = {sr_json};',
        html,
        flags=re.DOTALL
    )

    with open(HTML_PATH, "w", encoding="utf-8") as f:
        f.write(html)

    print("Successfully updated surge_module_helper.html with latest module data.")

if __name__ == "__main__":
    update()
