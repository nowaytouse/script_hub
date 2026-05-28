import os
import hashlib
import re
import sys
import subprocess
import time
from datetime import datetime
from typing import List, Optional, Tuple

# COLORS & LOGGING

class Logger:
    BLUE = '\033[0;34m'
    CYAN = '\033[0;36m'
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    RED = '\033[0;31m'
    PURPLE = '\033[1;35m'
    NC = '\033[0m'

    @staticmethod
    def info(msg: str):
        print(f"{Logger.BLUE}[INFO]{Logger.NC} {msg}")

    @staticmethod
    def success(msg: str):
        print(f"{Logger.GREEN}[✓]{Logger.NC} {msg}")

    @staticmethod
    def warn(msg: str):
        print(f"{Logger.YELLOW}[⚠]{Logger.NC} {msg}")

    @staticmethod
    def error(msg: str, exit_code: Optional[int] = None):
        print(f"{Logger.RED}[✗ ERROR]{Logger.NC} {msg}", file=sys.stderr)
        if exit_code is not None:
            sys.exit(exit_code)

    @staticmethod
    def section(msg: str):
        print(f"\n{Logger.PURPLE}═══ {msg} ═══{Logger.NC}")

# FILE & PATH UTILITIES

def get_project_root() -> str:
    """Gets the project root based on the script location.
    Current location: scripts/lib/common.py
    Root is 2 levels up.
    """
    script_dir = os.path.dirname(os.path.abspath(__file__))
    return os.path.abspath(os.path.join(script_dir, "../.."))

def get_file_hash(file_path: str) -> str:
    """Calculates MD5 hash of a file."""
    if not os.path.exists(file_path):
        return ""
    hash_md5 = hashlib.md5()
    with open(file_path, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_md5.update(chunk)
    return hash_md5.hexdigest()

def read_file(file_path: str) -> List[str]:
    """Reads file and returns list of lines, handling encoding issues."""
    if not os.path.exists(file_path):
        return []
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            return f.readlines()
    except UnicodeDecodeError:
        with open(file_path, 'r', encoding='latin-1') as f:
            return f.readlines()

def write_file(file_path: str, content: str):
    """Atomically write *content* to *file_path* (tmp → rename), creating parent dirs.
    Only writes if the actual content (excluding dynamic comments like timestamps/dates/versions) changed.
    """
    if os.path.exists(file_path):
        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                old_content = f.read()
            
            def strip_dynamic(text: str) -> str:
                lines = text.splitlines()
                filtered = []
                for line in lines:
                    stripped = line.strip()
                    # Skip lines with date, version or update timestamps
                    if (stripped.startswith("#") and any(k in stripped.lower() for k in ("updated", "date"))):
                        continue
                    if stripped.startswith("#!date") or stripped.startswith("#!version"):
                        continue
                    if '"generated":' in stripped or '"date":' in stripped:
                        continue
                    filtered.append(line)
                return "\n".join(filtered)
            
            old_stripped = strip_dynamic(old_content)
            new_stripped = strip_dynamic(content)
            if old_stripped == new_stripped:
                # Functionally identical, skip write
                print(f"[SKIP WRITE] {file_path}")
                return
            else:
                print(f"[ACTUAL WRITE] {file_path}")
                import difflib
                diff = list(difflib.unified_diff(
                    old_stripped.splitlines(),
                    new_stripped.splitlines(),
                    fromfile='old_stripped',
                    tofile='new_stripped',
                    lineterm=''
                ))
                if diff:
                    print(f"--- STRIPPED DIFF for {file_path} ---")
                    print("\n".join(diff[:30]))
                    print("-------------------------------------")
        except Exception as e:
            print(f"Error checking write: {e}")
            pass

    dir_path = os.path.dirname(file_path) or "."
    os.makedirs(dir_path, exist_ok=True)
    tmp = file_path + ".tmp~"
    try:
        with open(tmp, 'w', encoding='utf-8') as f:
            f.write(content)
        os.replace(tmp, file_path)
    except Exception:
        if os.path.exists(tmp):
            os.remove(tmp)
        raise

# SURGE/SINGBOX SPECIFIC PARSERS

def extract_section(lines: List[str], section_name: str) -> List[str]:
    """Extracts a section (e.g., [Rule]) from a list of lines."""
    result = []
    in_section = False
    section_pattern = re.compile(rf'^\[{re.escape(section_name)}\]', re.IGNORECASE)
    
    for line in lines:
        stripped = line.strip()
        if section_pattern.match(stripped):
            in_section = True
            continue
        if stripped.startswith('[') and in_section:
            break
        if in_section and stripped and not stripped.startswith('#'):
            result.append(line.rstrip())
    return result

def clean_rules(rules: List[str]) -> List[str]:
    """Deduplicates and cleans rules while preserving comments optionally."""
    seen = set()
    cleaned = []
    for r in rules:
        r_strip = r.strip()
        if r_strip and not r_strip.startswith('#') and r_strip not in seen:
            seen.add(r_strip)
            cleaned.append(r_strip)
    return sorted(cleaned)

# HARDENED DOWNLOAD UTILITIES
#
# Kelee / Loon ecosystem expects a Loon UA with version.
LOON_VERSION = "3.3.9"
_BROWSER_UA = f"Loon/{LOON_VERSION}"

def _is_html_content(data) -> bool:
    """Detect if downloaded content is HTML (Cloudflare challenge, 404 page, etc.)."""
    if isinstance(data, bytes):
        sample = data[:500].lower()
        return b"<!doctype html" in sample or b"<html" in sample
    if isinstance(data, str):
        sample = data[:500].lower()
        return "<!doctype html" in sample or "<html" in sample
    return False

def _has_dangerous_chars(line: str) -> bool:
    """Reject lines with HTML/JS artifacts that should never appear in rule files."""
    return any(c in line for c in ('<', '>', '{', '}', 'function(', 'window.', 'document.'))

def safe_download(url: str, binary: bool = False, retries: int = 1,
                  timeout: int = 30) -> Optional[str]:
    """Download text content from *url*.  Returns None on failure / HTML response."""
    raw = _curl_fetch(url, timeout=timeout, retries=retries)
    if raw is None or _is_html_content(raw):
        return None
    return raw.decode("utf-8", errors="replace") if not binary else raw  # type: ignore[return-value]


def safe_download_binary(url: str, retries: int = 1,
                         timeout: int = 30) -> Optional[bytes]:
    """Download binary content from *url*.  Returns None on failure / HTML response."""
    raw = _curl_fetch(url, timeout=timeout, retries=retries)
    if raw is None or _is_html_content(raw):
        return None
    return raw


# ── internal ──────────────────────────────────────────────────────────────────

def _curl_fetch(url: str, *, timeout: int = 30, retries: int = 1) -> Optional[bytes]:
    """Low-level curl wrapper shared by safe_download* helpers."""
    cmd = ["curl", "-L", "-s", "-m", str(timeout), "-f",
           "-H", f"User-Agent: {_BROWSER_UA}", url]
    for attempt in range(retries + 1):
        try:
            result = subprocess.run(cmd, capture_output=True)
            if result.returncode == 0:
                return result.stdout
        except Exception:
            pass
        if attempt < retries:
            time.sleep(2 ** attempt)
    return None
