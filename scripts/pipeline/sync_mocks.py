#!/usr/bin/env python3
"""
Sync Mocks Module.

Downloads standard mock resources (blank.txt, reject-200.txt, etc.) from
upstream repositories to ensure local copies are always up-to-date with upstream.
"""

import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from hub.project_paths import ROOT
from hub.common import safe_download_binary

def sync_mocks():
    MOCKS_DIR = os.path.join(ROOT, "modules/source/mocks")
    os.makedirs(MOCKS_DIR, exist_ok=True)
    
    # Authoritative upstream sources for standard mock responses
    mock_sources = {
        "reject-200.txt": "https://raw.githubusercontent.com/deezertidal/Surge_Module/master/files/reject-200.txt",
        "blank.txt": "https://raw.githubusercontent.com/NobyDa/Script/master/Surge/Mock/blank.txt",
        "reject-dict.json": "https://raw.githubusercontent.com/deezertidal/Surge_Module/master/files/reject-dict.json",
        "reject-img.gif": "https://raw.githubusercontent.com/deezertidal/Surge_Module/master/files/reject-img.gif",
        "blank_dict.json.js": "https://raw.githubusercontent.com/deezertidal/Surge_Module/master/files/blank_dict.json.js",
        "blank.gif": "https://raw.githubusercontent.com/NobyDa/Script/master/Surge/Mock/blank.gif",
        "blank_dict.json": "https://raw.githubusercontent.com/deezertidal/Surge_Module/master/files/blank_dict.json"
    }

    count = 0
    for filename, url in mock_sources.items():
        content = safe_download_binary(url, retries=2, timeout=15)
        if content is not None:
            path = os.path.join(MOCKS_DIR, filename)
            with open(path, "wb") as f:
                f.write(content)
            count += 1
            print(f"[✓] Synced mock: {filename}")
        else:
            print(f"[!] Failed to sync mock: {filename}")
            
    return count

if __name__ == "__main__":
    sync_mocks()
