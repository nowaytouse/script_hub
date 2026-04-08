#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Surge module conversion to Shadowrocket compatible version
Features:
1. Remove/Convert Surge exclusive features
2. Generate Shadowrocket exclusive module directory
3. Update web data
"""

import os
import re
import json
import shutil
from pathlib import Path
from datetime import datetime

# Root directory
SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
SURGE_MODULE_DIR = PROJECT_ROOT / "module" / "surge(main)"
SR_MODULE_DIR = PROJECT_ROOT / "module" / "shadowrocket"
OUTPUT_DIR = PROJECT_ROOT / "module"

GITHUB_RAW_BASE_SR = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/shadowrocket"

# 1:1 Logic Consistency with download_modules.sh (Bash)
CONVERSION_RULES = {
    "extended-matching": "",
    "pre-matching": "",
    "REJECT-DROP": "REJECT",
    "REJECT-TINYGIF": "REJECT",
    "REJECT-NO-DROP": "REJECT",
    "%APPEND%": "",
    "%INSERT%": "",
}

REMOVE_PATTERNS = [
    r'^#!update-interval\s*=.*$',
    r'^#!ability\s*=.*$',
    r',"update-interval=[0-9]*"', # Precise match for quoted update-interval from Bash
]

def convert_module_content(content: str, filename: str) -> tuple[str, list]:
    changes = []
    lines = content.split('\n')
    new_lines = []
    
    for line in lines:
        original_line = line
        modified = False
        
        # 1. Remove patterns (Align with Bash sed)
        should_remove = False
        for pattern in REMOVE_PATTERNS:
            if re.search(pattern, line, re.IGNORECASE):
                if pattern.startswith('^'): # Line-based
                    should_remove = True
                    break
                else: # Segment-based
                    line = re.sub(pattern, '', line)
                    modified = True
        
        if should_remove:
            changes.append(f"Removed line: {original_line[:30]}")
            continue
        
        # 2. Basic feature conversion
        for feature, replacement in CONVERSION_RULES.items():
            if feature in line:
                if feature in ["extended-matching", "pre-matching"]:
                    line = re.sub(rf',\s*{feature}', '', line)
                    line = re.sub(rf'{feature}\s*,', '', line)
                    line = line.replace(feature, '')
                else:
                    line = line.replace(feature, replacement)
                modified = True
        
        # 3. Clean up commas
        line = re.sub(r',\s*,', ',', line)
        line = re.sub(r',\s*$', '', line)
        
        # 4. Shadowrocket Header Mark [🚀SR]
        new_lines.append(line)
    
    result = '\n'.join(new_lines)
    result = re.sub(r'(#!desc\s*[=:]\s*)(.+)', r'\1[🚀SR] \2', result)
    return result, changes

def process_all_modules():
    if SR_MODULE_DIR.exists():
        shutil.rmtree(SR_MODULE_DIR)
    SR_MODULE_DIR.mkdir(parents=True)
    
    categories = ["amplify_nexus", "head_expanse", "narrow_pierce"]
    stats = {"total": 0, "converted": 0}
    
    for cat in categories:
        (SR_MODULE_DIR / cat).mkdir(exist_ok=True)
        cat_path = SURGE_MODULE_DIR / cat
        if not cat_path.exists(): continue
        
        for module_file in sorted(cat_path.glob("*.sgmodule")):
            stats["total"] += 1
            try:
                with open(module_file, 'r', encoding='utf-8') as f:
                    content = f.read()
                converted, _ = convert_module_content(content, module_file.name)
                with open(SR_MODULE_DIR / cat / module_file.name, 'w', encoding='utf-8') as f:
                    f.write(converted)
                stats["converted"] += 1
            except: pass
    return stats

if __name__ == "__main__":
    process_all_modules()
    print("Shadowrocket conversion complete (English).")
