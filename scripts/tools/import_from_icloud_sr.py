#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Import modules from Shadowrocket to Surge (Incremental + Dedup)
Features:
1. Sync modules from iCloud Shadowrocket directory
2. Deduplicate based on content hash
3. Preserve original category and naming
"""

import os
import re
import hashlib
from pathlib import Path
from urllib.parse import unquote

# Project Paths
SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent
SR_DIR = Path(
    os.environ.get(
        "SHADOWROCKET_MODULES_DIR",
        "~/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules",
    )
).expanduser()
SURGE_DIR = PROJECT_ROOT / "modules" / "surge"

# Category Mapping (Preserve original Chinese group names as per user instruction)
CATEGORY_MAP = {
    "amplify_nexus": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
    "head_expanse": "『 🔝 Head Expanse › 首端扩域 』",
}

def get_module_name(content):
    """Extract #!name from module content."""
    match = re.search(r'^#!name\s*[=:]\s*(.+)$', content, re.MULTILINE)
    return match.group(1).strip() if match else None

def get_content_hash(content):
    """Calculate content hash (ignore metadata like category/url)."""
    lines = content.split('\n')
    filtered = [line_ for line_ in lines if not line_.startswith('#!category') and not line_.startswith('#!url')]
    return hashlib.md5('\n'.join(filtered).encode()).hexdigest()[:8]

def classify_module(name, content):
    """Classify module based on name keywords."""
    name_lower = name.lower()
    if any(k in name_lower for k in ['wifi', 'calling', 'helper', 'enhanced', 'dns', 'iringo', 'dualsubs', 'tiktok', '助手']):
        return "amplify_nexus"
    if any(k in name_lower for k in ['ad-block', 'adblock', 'firewall', 'script hub', '广告平台', '广告联盟', 'universal']):
        return "head_expanse"
    return "narrow_pierce"

def main():
    print("=" * 60)
    print("🚀 Shadowrocket to Surge Module Importer")
    print("=" * 60)

    if not SR_DIR.exists():
        print(f"❌ Shadowrocket directory not found: {SR_DIR}")
        return

    # Scan existing modules
    existing = {}  # name_lower -> {path, hash, name}
    for cat in ["amplify_nexus", "head_expanse"]:
        cat_path = SURGE_DIR / cat
        if not cat_path.exists():
            continue
        for f in cat_path.glob("*.sgmodule"):
            try:
                content = f.read_text(encoding='utf-8')
                name = get_module_name(content) or f.stem
                existing[name.lower()] = {
                    "path": f,
                    "hash": get_content_hash(content),
                    "name": name
                }
            except Exception:
                pass

    print(f"Existing modules found: {len(existing)}\n")

    added = duplicate = skipped = 0

    # Process SR modules
    for sr_file in sorted(SR_DIR.glob("*.*module")):
        filename = sr_file.name
        if filename.startswith("__"):
            continue

        size = sr_file.stat().st_size
        if size > 100000:
            print(f"⏭️  Skipping large file: {filename} ({size//1024}KB)")
            skipped += 1
            continue

        try:
            content = sr_file.read_text(encoding='utf-8')
        except Exception:
            print(f"❌ Failed to read: {filename}")
            continue

        module_name = get_module_name(content) or unquote(sr_file.stem)
        content_hash = get_content_hash(content)
        name_key = module_name.lower()

        if name_key in existing:
            ex = existing[name_key]
            if ex["hash"] == content_hash:
                print(f"🔄 Duplicate: {module_name}")
                duplicate += 1
            else:
                print(f"📝 Exists with different content: {module_name}")
                print(f"   → Keeping existing: {ex['path'].name}")
                skipped += 1
            continue

        category = classify_module(module_name, content)
        safe_name = re.sub(r'[<>:"/\\|?*]', '', module_name)
        if not safe_name.endswith('.sgmodule'):
            safe_name += '.sgmodule'
        dst_path = SURGE_DIR / category / safe_name

        # Process content
        lines = content.split('\n')
        new_lines = []
        cat_added = False
        for line in lines:
            if line.startswith('#!url'):
                continue
            if line.startswith('#!category'):
                if not cat_added:
                    new_lines.append(f"#!category={CATEGORY_MAP[category]}")
                    cat_added = True
                continue
            if line.startswith('#!name') and not cat_added:
                new_lines.append(f"#!category={CATEGORY_MAP[category]}")
                cat_added = True
            new_lines.append(line)

        if not cat_added:
            new_lines.insert(0, f"#!category={CATEGORY_MAP[category]}")

        dst_path.write_text('\n'.join(new_lines), encoding='utf-8')
        print(f"✅ Added: {module_name} → {category}/")
        added += 1

    print("\n" + "=" * 60)
    print(f"Stats: Added {added}, Duplicate {duplicate}, Skipped {skipped}")
    print("=" * 60)

if __name__ == "__main__":
    main()
