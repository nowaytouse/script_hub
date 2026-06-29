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
        r"https://raw\.githubusercontent\.com/([^/]+)/([^/]+)/([^/]+)/([^\"'\s]+\.[a-zA-Z0-9]+)": r"https://cdn.jsdelivr.net/gh/\1/\2@\3/\4",
        # Some gist urls are hard to map to cdn.jsdelivr.net directly, so we'll rewrite known internal ones to the localized paths if they exist
        r"(?<!/)https://github\.com/([^/]+)/([^/]+)/raw/([^/]+)/([^\"'\s]+\.[a-zA-Z0-9]+)": r"https://cdn.jsdelivr.net/gh/\1/\2@\3/\4",
        
        # Github Releases to gh-proxy
        r"(?<!/)https://github\.com/([^/]+)/([^/]+)/releases/download/([^/]+)/([^\"'\s]+\.[a-zA-Z0-9]+)": r"https://gh-proxy.com/https://github.com/\1/\2/releases/download/\3/\4",
    }

    # Normalise for consistent prefix matching
    directory = os.path.normpath(directory)
    # Paths that intentionally carry GitHub Raw URLs — must not be CDN-rewritten.
    _GITHUB_SOURCE_DIRS = {
        os.path.normpath(p) for p in [
            os.path.join(SURGE_HEAD_EXPANSE_GITHUB_DIR),
            os.path.join(SHADOWROCKET_HEAD_EXPANSE_GITHUB_DIR),
        ]
    }

    def _is_github_source(path: str) -> bool:
        p = os.path.normpath(path)
        return any(p == d or p.startswith(d + os.sep) for d in _GITHUB_SOURCE_DIRS)

    modified_count = 0
    for root, dirs, files in os.walk(directory):
        if '.git' in root:
            continue
        # Skip github/ variant folders — their raw.githubusercontent refs are intentional
        if _is_github_source(root):
            dirs.clear()
            continue
        for file in files:
            if file.endswith((".sgmodule", ".module", ".list", ".conf", ".json", ".html", ".md")):
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
    count += run_url_rewrites(ROOT)
    print(f"URL rewrite completed: {count} files modified.")
