#!/usr/bin/env python3
"""
URL Rewriter Module.

This script scans all generated module and ruleset files to replace
blocked Github raw URLs (raw.githubusercontent.com) with the accelerated
jsDelivr CDN URLs. It also localizes specific mock resources.
"""

import os
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from hub.project_paths import *

def _log(msg: str) -> None:
    print(msg, file=sys.stderr if msg.startswith("[!]") else sys.stdout)

def run_url_rewrites(directory: str) -> int:
    """Scan and rewrite URLs in the given directory."""
    if not os.path.isdir(directory):
        return 0

    mock_replacements = {
        # Specific mock files to localized CDN
        r"https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/reject-200\.txt": "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-200.txt",
        r"https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/blank\.txt": "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank.txt",
        r"https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/reject-dict\.json": "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-dict.json",
        r"https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/reject-img\.gif": "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-img.gif",
        r"https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/blank_dict\.json\.js": "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank_dict.json.js",
        r"https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/blank\.gif": "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank.gif",
        r"https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/blank_dict\.json": "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank_dict.json",
        
        # Generic Github raw and Gist rewrites to CDN
        r"https://raw\.githubusercontent\.com/([^/]+)/([^/]+)/([^/]+)/(.+\.(js|json|png|gif|txt|list|module|sgmodule|srs))": r"https://cdn.jsdelivr.net/gh/\1/\2@\3/\4",
        # Some gist urls are hard to map to cdn.jsdelivr.net directly, so we'll rewrite known internal ones to the localized paths if they exist
    }

    modified_count = 0
    for root, _, files in os.walk(directory):
        for file in files:
            if file.endswith((".sgmodule", ".module", ".list", ".conf")):
                path = os.path.join(root, file)
                try:
                    with open(path, "r", encoding="utf-8") as f:
                        content = f.read()
                except Exception as e:
                    _log(f"[!] Error reading {path}: {e}")
                    continue

                orig_content = content
                for pattern, repl in mock_replacements.items():
                    content = re.sub(pattern, repl, content, flags=re.IGNORECASE)

                if content != orig_content:
                    with open(path, "w", encoding="utf-8") as f:
                        f.write(content)
                    modified_count += 1
    return modified_count

if __name__ == "__main__":
    count = 0
    count += run_url_rewrites(MODULES_DIR)
    count += run_url_rewrites(RULESETS_DIR)
    print(f"URL rewrite completed: {count} files modified.")
