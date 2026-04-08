import os
import hashlib
import re
import sys
from datetime import datetime
from typing import List, Optional

# ═══════════════════════════════════════════════════════════════════════════════
# COLORS & LOGGING
# ═══════════════════════════════════════════════════════════════════════════════

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

# ═══════════════════════════════════════════════════════════════════════════════
# FILE & PATH UTILITIES
# ═══════════════════════════════════════════════════════════════════════════════

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
    """Writes content to file, ensuring parent directories exist."""
    os.makedirs(os.path.dirname(file_path), exist_ok=True)
    with open(file_path, 'w', encoding='utf-8') as f:
        f.write(content)

# ═══════════════════════════════════════════════════════════════════════════════
# SURGE/SINGBOX SPECIFIC PARSERS
# ═══════════════════════════════════════════════════════════════════════════════

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
