import os
import re
import hashlib
import subprocess
import random
import time
from pathlib import Path
from urllib.parse import urlparse

import sys

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS_DIR))
from lib.common import _BROWSER_UA, safe_download_binary, Logger

ROOT = Path(__file__).parent.parent.parent
MODULE_DIRS = [ROOT / "module/surge(main)", ROOT / "module/shadowrocket"]
SCRIPTS_DIR = ROOT / "module/scripts"
LOCAL_URL_PREFIX = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/scripts/"

# User-Agent Pool
USER_AGENTS = [_BROWSER_UA]

def is_valid_url(url):
    """Filter out regex patterns or invalid mock URLs."""
    if ".*" in url or "{{" in url or url.endswith(".js*"):
        return False
    parsed = urlparse(url)
    if not parsed.netloc or "." not in parsed.netloc:
        return False
    return True

def download_script(url):
    """Download script with centralized download utility."""
    if not is_valid_url(url):
        print(f"  ⏭️  Skipping invalid/regex URL: {url}")
        return None

    parsed = urlparse(url)
    domain = parsed.netloc.replace(".", "_")
    path_parts = parsed.path.strip("/").split("/")
    filename = path_parts[-1]
    if not filename.endswith(".js"):
        filename += ".js"
    
    path_hash = hashlib.md5(parsed.path.encode()).hexdigest()[:6]
    local_filename = f"{domain}_{path_hash}_{filename}"
    local_path = SCRIPTS_DIR / local_filename
    
    # If already downloaded, no need to re-download unless it's a force sync
    if local_path.exists():
        return local_filename

    print(f"  📥 Downloading script: {url}")
    content = safe_download_binary(url, retries=2, timeout=15)
    if content:
        with open(local_path, "wb") as f:
            f.write(content)
        return local_filename
    
    print(f"  ⚠️  Keep remote (download failed): {url}")
    return None

def localize_module(file_path):
    with open(file_path, "r", encoding="utf-8") as f:
        content = f.read()
    
    urls = re.findall(r'https?://[^\s,]+?\.js', content)
    if not urls:
        return

    new_content = content
    changed = False
    
    for url in set(urls):
        if LOCAL_URL_PREFIX in url:
            continue
            
        local_filename = download_script(url)
        if local_filename:
            local_url = LOCAL_URL_PREFIX + local_filename
            new_content = new_content.replace(url, local_url)
            changed = True
    
    if changed:
        with open(file_path, "w", encoding="utf-8") as f:
            f.write(new_content)
        print(f"🔍 Updated: {file_path.relative_to(ROOT)}")

def main():
    if not SCRIPTS_DIR.exists():
        os.makedirs(SCRIPTS_DIR)
        
    for root_dir in MODULE_DIRS:
        if not root_dir.exists():
            continue
        for ext in ["*.sgmodule", "*.module"]:
            for module_file in root_dir.rglob(ext):
                localize_module(module_file)

if __name__ == "__main__":
    main()
