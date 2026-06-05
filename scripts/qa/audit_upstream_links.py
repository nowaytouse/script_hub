#!/usr/bin/env python3
import os
import re
import ssl
import urllib.request
import urllib.error
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
PIPELINE_DIR = SCRIPTS_DIR / "pipeline"

def extract_urls(file_path):
    with open(file_path, "r", encoding="utf-8") as f:
        content = f.read()
    # Simple regex for http/https URLs inside quotes
    urls = re.findall(r'https?://[^\s\'"<>]+', content)
    return urls

def check_url(url):
    try:
        req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
        ctx = ssl.create_default_context()
        ctx.check_hostname = False
        ctx.verify_mode = ssl.CERT_NONE
        # Using GET because some CDNs/GitHub block HEAD or return 403 for HEAD
        with urllib.request.urlopen(req, context=ctx, timeout=10) as response:
            return url, response.status, None
    except urllib.error.HTTPError as e:
        return url, e.code, str(e)
    except urllib.error.URLError as e:
        return url, 0, str(e.reason)
    except Exception as e:
        return url, 0, str(e)

def main():
    print(f"Scanning for URLs in {PIPELINE_DIR}...")
    all_urls = set()
    for root, _, files in os.walk(PIPELINE_DIR):
        for file in files:
            if file.endswith(".py"):
                path = Path(root) / file
                urls = extract_urls(path)
                for u in urls:
                    if "github.com" in u or "raw.githubusercontent" in u or "moe" in u or "jsdelivr" in u or "kelee" in u or "yfamilys" in u:
                        all_urls.add(u)
                        
    urls = sorted(list(all_urls))
    print(f"Found {len(urls)} unique upstream URLs to check.")
    
    failed = []
    with ThreadPoolExecutor(max_workers=10) as executor:
        results = executor.map(check_url, urls)
        for url, status, error in results:
            if status not in (200, 301, 302, 304):
                print(f"[FAIL] {status} | {url} | {error}")
                failed.append((url, status, error))
            else:
                # print(f"[OK] {status} | {url}")
                pass
                
    if failed:
        print(f"\n{len(failed)} URLs failed.")
        exit(1)
    else:
        print("\nAll URLs are accessible (200 OK)!")

if __name__ == "__main__":
    main()
