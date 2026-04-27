import os
import re
import hashlib
import subprocess
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).parent.parent.parent
MODULE_DIRS = [ROOT / "module/surge(main)", ROOT / "module/shadowrocket"]
SCRIPTS_DIR = ROOT / "module/scripts"
LOCAL_URL_PREFIX = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/module/scripts/"

def download_script(url):
    """Download script and return local filename."""
    parsed = urlparse(url)
    # Create a unique but readable filename
    domain = parsed.netloc.replace(".", "_")
    path_parts = parsed.path.strip("/").split("/")
    filename = path_parts[-1]
    if not filename.endswith(".js"):
        filename += ".js"
    
    # Use path hash to prevent collisions for same filenames in different paths
    path_hash = hashlib.md5(parsed.path.encode()).hexdigest()[:6]
    local_filename = f"{domain}_{path_hash}_{filename}"
    local_path = SCRIPTS_DIR / local_filename
    
    try:
        print(f"  📥 Downloading: {url}")
        result = subprocess.run(
            ["curl", "-L", "-k", "-s", "-m", "30", "-f", url],
            capture_output=True, check=True
        )
        if result.stdout:
            with open(local_path, "wb") as f:
                f.write(result.stdout)
            return local_filename
    except Exception as e:
        print(f"  ❌ Failed to download {url}: {e}")
    return None

def localize_module(file_path):
    with open(file_path, "r", encoding="utf-8") as f:
        content = f.read()
    
    # Find all script-path patterns (Surge & Shadowrocket)
    # Patterns: script-path=http..., URL-REGEX,http..., etc.
    urls = re.findall(r'https?://[^\s,]+?\.js', content)
    if not urls:
        return

    print(f"🔍 Processing: {file_path.relative_to(ROOT)}")
    new_content = content
    changed = False
    
    for url in set(urls):
        # Skip if already localized
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
        print(f"  ✅ Localized and updated.")

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
