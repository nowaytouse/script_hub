#!/usr/bin/env python3
"""
Sync Mocks Module.

Generates standard mock resources (blank.txt, reject-200.txt, etc.) locally
to avoid upstream 404s and reduce build dependencies.
"""

import os
import sys
from pathlib import Path
import base64

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from hub.project_paths import ROOT

def sync_mocks():
    MOCKS_DIR = os.path.join(ROOT, "modules/source/mocks")
    os.makedirs(MOCKS_DIR, exist_ok=True)
    
    # 1x1 Transparent GIF base64
    b64_gif = b"R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7"
    gif_bytes = base64.b64decode(b64_gif)
    
    mocks_data = {
        "reject-200.txt": b"",
        "blank.txt": b"",
        "reject-dict.json": b"{}",
        "blank_dict.json": b"{}",
        "blank_dict.json.js": b"let body = JSON.stringify({});\n$done({response: {status: 200, body: body}});",
        "reject-img.gif": gif_bytes,
        "blank.gif": gif_bytes
    }

    count = 0
    for filename, content in mocks_data.items():
        path = os.path.join(MOCKS_DIR, filename)
        with open(path, "wb") as f:
            f.write(content)
        count += 1
        print(f"[✓] Generated mock locally: {filename}")
            
    return count

if __name__ == "__main__":
    sync_mocks()
