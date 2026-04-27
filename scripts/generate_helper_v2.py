#!/usr/bin/env python3
"""
生成 Surge/Shadowrocket 模块导入助手 v2
"""
import json
import os
from datetime import datetime
from pathlib import Path

# 路径配置
PROJECT_ROOT = Path(__file__).parent.parent
MODULES_DATA_FILE = PROJECT_ROOT / "module" / "modules_data.json"
HTML_TEMPLATE = PROJECT_ROOT / "module" / "surge_module_helper_v2.html"
HTML_OUTPUT = PROJECT_ROOT / "module" / "surge_module_helper.html"
SHADOWROCKET_DIR = PROJECT_ROOT / "module" / "shadowrocket"

# 分类显示名称
CATEGORY_NAMES = {
    "amplify_nexus": "🛠️ Amplify Nexus › 增幅枢纽",
    "head_expanse": "🔝 Head Expanse › 首端扩域",
    "narrow_pierce": "🎯 Narrow Pierce › 窄域穿刺"
}

# 特殊标记
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

def load_modules_data():
    """加载模块数据"""
    with open(MODULES_DATA_FILE, 'r', encoding='utf-8') as f:
        return json.load(f)

def build_modules_structure(data):
    """构建模块结构"""
    base_url = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/"
    
    # Surge 模块
    surge_modules = {}
    for cat_key, cat_name in CATEGORY_NAMES.items():
        surge_modules[cat_key] = {
            "name": cat_name,
            "items": []
        }
    
    # Shadowrocket 模块
    sr_modules = {}
    for cat_key, cat_name in CATEGORY_NAMES.items():
        sr_modules[cat_key] = {
            "name": cat_name,
            "items": []
        }
    
    # 处理每个模块
    for module in data["modules"]:
        cat = module["category"]
        if cat not in surge_modules:
            continue
        
        name = module["name"]
        
        # 基础属性
        item = {
            "name": name,
            "desc": module["desc"]
        }
        
        # 应用特殊属性
        if name in SPECIAL_ATTRS:
            item.update(SPECIAL_ATTRS[name])
        elif module.get("tags"):
            item["tag"] = module["tags"][0]
        
        # Surge 版本
        surge_item = item.copy()
        surge_item["url"] = base_url + module["path"]
        surge_modules[cat]["items"].append(surge_item)
        
        # Shadowrocket 版本（检查是否存在）
        sr_path = module["path"].replace("surge(main)", "shadowrocket").replace(".sgmodule", ".module")
        sr_full_path = PROJECT_ROOT / sr_path
        if sr_full_path.exists():
            sr_item = item.copy()
            sr_item["desc"] = f"[🚀SR] {module['desc']}"
            sr_item["url"] = base_url + sr_path
            sr_modules[cat]["items"].append(sr_item)
    
    return surge_modules, sr_modules

def generate_html(surge_modules, sr_modules):
    """生成HTML文件"""
    # 读取模板
    with open(HTML_TEMPLATE, 'r', encoding='utf-8') as f:
        html = f.read()
    
    # 构建数据对象
    modules_data = {
        "surge": surge_modules,
        "shadowrocket": sr_modules,
        "generated": datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    }
    
    # 注入数据
    data_json = json.dumps(modules_data, ensure_ascii=False, indent=2)
    html = html.replace('__MODULES_DATA__', data_json)
    
    # 写入文件
    with open(HTML_OUTPUT, 'w', encoding='utf-8') as f:
        f.write(html)
    
    print(f"✅ 已生成: {HTML_OUTPUT.name}")
    print(f"   Surge 模块: {sum(len(cat['items']) for cat in surge_modules.values())}")
    print(f"   Shadowrocket 模块: {sum(len(cat['items']) for cat in sr_modules.values())}")

def main():
    print("=" * 60)
    print("🚀 生成 Surge/Shadowrocket 模块导入助手 v2")
    print("=" * 60)
    
    # 加载数据
    data = load_modules_data()
    print(f"📦 加载了 {data['total']} 个模块")
    
    # 构建结构
    surge_modules, sr_modules = build_modules_structure(data)
    
    # 生成HTML
    generate_html(surge_modules, sr_modules)
    
    print("=" * 60)
    print("✅ 完成！")
    print("=" * 60)

if __name__ == "__main__":
    main()
