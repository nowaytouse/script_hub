#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Surge Module Consolidation Tool
Features:
1. Scan modules across categories
2. Detect duplicate modules
3. Load Shadowrocket compatibility data
4. Update helper web data
5. Generate unified JSON metadata
"""

import os
import re
import json
import urllib.parse
from pathlib import Path
from datetime import datetime

# Root directory
SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)"
OUTPUT_DIR = PROJECT_ROOT / "module"
COMPATIBILITY_FILE = OUTPUT_DIR / "modules_compatibility.json"
HELPER_HTML = OUTPUT_DIR / "surge_module_helper.html"

# Category mapping
CATEGORIES = {
    "amplify_nexus": "Amplify Nexus",
    "head_expanse": "Head Expanse",
    "narrow_pierce": "Narrow Pierce"
}

def scan_modules():
    print("=" * 60)
    print("📦 Surge Module Consolidation Tool")
    print("=" * 60)
    
    deduped_modules = {}
    category_stats = {cat: 0 for cat in CATEGORIES}
    
    for cat_key in CATEGORIES:
        cat_path = MODULE_DIR / cat_key
        if not cat_path.exists():
            continue
            
        for module_file in cat_path.glob("*.sgmodule"):
            # Parse basic info
            info = {
                "id": module_file.stem,
                "filename": module_file.name,
                "category": cat_key,
                "path": str(module_file.relative_to(PROJECT_ROOT))
            }
            
            try:
                with open(module_file, 'r', encoding='utf-8') as f:
                    content = f.read()
                    
                # Extract metadata
                name_match = re.search(r'#!name\s*[=:]\s*(.+)', content)
                desc_match = re.search(r'#!desc\s*[=:]\s*(.+)', content)
                tag_match = re.search(r'#!tag\s*[=:]\s*(.+)', content)
                
                info["name"] = name_match.group(1).strip() if name_match else module_file.stem
                info["desc"] = desc_match.group(1).strip() if desc_match else ""
                info["tags"] = [t.strip() for t in tag_match.group(1).split(',')] if tag_match else []
                
            except Exception as e:
                print(f"  ❌ Error parsing {module_file.name}: {e}")
                continue
                
            canonical_name = urllib.parse.unquote(module_file.name)
            dedupe_key = (cat_key, canonical_name)
            existing = deduped_modules.get(dedupe_key)
            prefer_current = existing is None or ("%" in module_file.name and "%" not in existing["filename"])
            if prefer_current:
                deduped_modules[dedupe_key] = info

    all_modules = sorted(
        deduped_modules.values(),
        key=lambda item: (item["category"], item["filename"].lower()),
    )
    for item in all_modules:
        category_stats[item["category"]] += 1
            
    print(f"🔍 Scanned {len(all_modules)} modules")
    for cat, count in category_stats.items():
        print(f"  - {CATEGORIES[cat]}: {count}")
        
    return all_modules

def generate_json_data(modules):
    output_file = OUTPUT_DIR / "modules_data.json"
    data = {
        "generated": datetime.now().isoformat(),
        "total": len(modules),
        "modules": modules
    }
    
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
    
    print(f"✅ Metadata saved to {output_file.name}")

if __name__ == "__main__":
    modules = scan_modules()
    if modules:
        generate_json_data(modules)
    print("=" * 60)
    print("✅ Consolidation Complete!")
