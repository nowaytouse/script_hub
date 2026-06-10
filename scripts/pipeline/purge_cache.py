#!/usr/bin/env python3
"""
JSDelivr CDN Cache Purger.

This script detects which files were modified in the most recent git commits
(or in the current working directory) and sends a purge request to JSDelivr.
This instantly clears the CDN edge cache so updates are immediately available.
"""

import os
import sys
import json
import time
import urllib.request
import urllib.parse
import subprocess
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

# Adjust path to import hub modules
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from hub.project_paths import ROOT
from hub.common import Logger

REPO_OWNER = "nowaytouse"
REPO_NAME = "script_hub"
BRANCH = "master"

def get_all_relevant_files():
    """Get tracked files in the repository that were modified recently."""
    try:
        # Get files changed in the latest commit to avoid purging entire repo
        commit_files = subprocess.check_output(
            ["git", "diff", "--name-only", "HEAD~1", "HEAD"], 
            cwd=ROOT, text=True
        ).splitlines()
        
        # Also include any current working tree changes
        working_files = subprocess.check_output(
            ["git", "diff", "--name-only", "HEAD"], 
            cwd=ROOT, text=True
        ).splitlines()
        
        all_files = list(set(commit_files + working_files))
        
        # Fallback to all files if no recent changes found (e.g., initial or forced run)
        if not all_files:
            all_files = subprocess.check_output(
                ["git", "ls-files"], 
                cwd=ROOT, text=True
            ).splitlines()
    except subprocess.CalledProcessError:
        all_files = []
    
    # We include all possible config, script, rule, and text formats
    valid_exts = {".sgmodule", ".module", ".list", ".json", ".js", ".srs", ".md", ".txt", ".conf"}
    return [f for f in all_files if any(f.endswith(ext) for ext in valid_exts)]

def purge_url(filepath: str, max_retries: int = 4) -> bool:
    """Send purge request for a single file path with idempotent retry logic."""
    # JSDelivr API expects encoded paths for spaces and special chars, but MUST preserve slashes
    encoded_filepath = urllib.parse.quote(filepath, safe='/')
    url = f"https://purge.jsdelivr.net/gh/{REPO_OWNER}/{REPO_NAME}@{BRANCH}/{encoded_filepath}"
    
    for attempt in range(max_retries):
        try:
            req = urllib.request.Request(url, method="GET")
            req.add_header('User-Agent', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64)')
            with urllib.request.urlopen(req, timeout=8) as response:
                res_body = response.read().decode('utf-8')
                res_json = json.loads(res_body)
                if res_json.get("status") == "finished":
                    return True
        except urllib.error.HTTPError as e:
            if e.code == 429:
                retry_after = e.headers.get('Retry-After')
                delay = float(retry_after) if retry_after else (2.0 ** attempt)
                Logger.warn(f"Rate limited (429) for {filepath}, waiting {delay:.1f}s...")
                time.sleep(delay)
                continue
            Logger.error(f"HTTPError {e.code} for {filepath}: {e.reason}")
        except Exception as e:
            if attempt < max_retries - 1:
                time.sleep(1.5 * (attempt + 1))
            else:
                Logger.error(f"Failed to purge {filepath} after {max_retries} attempts: {e}")
    return False

def main():
    Logger.section("Deep CDN Cache Cleared")
    modified_files = get_all_relevant_files()
    
    if not modified_files:
        Logger.info("No relevant files found to purge.")
        return

    Logger.info(f"Attempting to instantly purge {len(modified_files)} files from JSDelivr CDN...")
    
    with ThreadPoolExecutor(max_workers=8) as executor:
        results = list(executor.map(purge_url, modified_files))
    
    success_count = sum(1 for r in results if r)
    failed_files = [f for f, r in zip(modified_files, results) if not r]
    
    if success_count > 0:
        Logger.success(f"Successfully purged {success_count}/{len(modified_files)} files from global CDN edge nodes!")
        Logger.info("You can now safely pull from JSDelivr without hitting cache.")
    
    if failed_files:
        Logger.warn(f"Atomicity constraint failed. Could not purge {len(failed_files)} files: {', '.join(failed_files[:5])}" + ("..." if len(failed_files) > 5 else ""))
        sys.exit(1)

if __name__ == "__main__":
    main()
