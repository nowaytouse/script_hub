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

def is_ci() -> bool:
    """True when running under GitHub Actions or explicit CI=1."""
    if os.environ.get("GITHUB_ACTIONS", "").lower() == "true":
        return True
    return os.environ.get("CI", "").lower() in ("1", "true", "yes")


def get_project_root() -> str:
    """Repository root (scripts/hub/common.py → two levels up)."""
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
            
            import re as _re
            # Pre-compiled patterns for efficiency
            _DATE_BRACKET = _re.compile(r'\[\d{4}-\d{2}-\d{2}(?:\s+\d{2}:\d{2}(?::\d{2})?)?\]')

            def strip_dynamic(text: str) -> str:
                lines = text.splitlines()
                filtered = []
                for line in lines:
                    stripped = line.strip()
                    # Skip pure timestamp/date comment lines
                    if (stripped.startswith("#") and any(k in stripped.lower() for k in ("updated", "date", "生成于"))):
                        continue
                    if stripped.startswith("#!date") or stripped.startswith("#!version"):
                        continue
                    # Skip JSON lines with dynamic timestamp fields
                    if '\"generated\":' in stripped or '\"date\":' in stripped or '\"updated\":' in stripped:
                        continue
                    if "自动生成于" in stripped or "generated at" in stripped.lower():
                        continue
                    # Strip inline [YYYY-MM-DD] date tokens EXCEPT in #!name (module name should update daily)
                    if stripped.startswith("#") and _DATE_BRACKET.search(stripped) and not stripped.startswith("#!name"):
                        line = _DATE_BRACKET.sub("[DATE]", line)
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

# 统一的下载配置
DEFAULT_DOWNLOAD_TIMEOUT = 60  # 秒
DEFAULT_DOWNLOAD_RETRIES = 3   # 重试次数
MAX_DOWNLOAD_SIZE = 50 * 1024 * 1024  # 50MB

def atomic_write(file_path: str, content: str) -> bool:
    """原子写入文件（先写临时文件，再重命名）

    Args:
        file_path: 目标文件路径
        content: 文件内容

    Returns:
        是否成功
    """
    import tempfile
    import shutil

    try:
        # 确保目录存在
        dir_path = os.path.dirname(file_path) or "."
        os.makedirs(dir_path, exist_ok=True)

        # 写入临时文件
        fd, tmp_path = tempfile.mkstemp(dir=dir_path, prefix=".tmp_", suffix=".writing")
        try:
            with os.fdopen(fd, 'w', encoding='utf-8') as f:
                f.write(content)
            # 原子重命名
            shutil.move(tmp_path, file_path)
            return True
        except Exception as e:
            # 清理临时文件
            if os.path.exists(tmp_path):
                os.remove(tmp_path)
            raise
    except Exception as e:
        Logger.error(f"原子写入失败 {file_path}: {e}")
        return False

def safe_remove(file_path: str, missing_ok: bool = True) -> bool:
    """安全删除文件

    Args:
        file_path: 文件路径
        missing_ok: 文件不存在时是否视为成功

    Returns:
        是否成功
    """
    try:
        if not os.path.exists(file_path):
            if missing_ok:
                return True
            Logger.warn(f"文件不存在: {file_path}")
            return False

        os.remove(file_path)
        return True
    except PermissionError:
        Logger.error(f"权限不足，无法删除: {file_path}")
        return False
    except OSError as e:
        Logger.error(f"删除文件失败 {file_path}: {e}")
        return False

def safe_remove_tree(dir_path: str, missing_ok: bool = True) -> bool:
    """安全删除目录树

    Args:
        dir_path: 目录路径
        missing_ok: 目录不存在时是否视为成功

    Returns:
        是否成功
    """
    import shutil

    try:
        if not os.path.exists(dir_path):
            if missing_ok:
                return True
            Logger.warn(f"目录不存在: {dir_path}")
            return False

        shutil.rmtree(dir_path)
        return True
    except PermissionError:
        Logger.error(f"权限不足，无法删除目录: {dir_path}")
        return False
    except OSError as e:
        Logger.error(f"删除目录失败 {dir_path}: {e}")
        return False

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

# Committed snapshots for hosts that often block CI/datacenter IPs (yfamilys.com).
_VENDOR_DIR = os.path.join(get_project_root(), "rulesets", "Sources", "vendor")
VENDOR_SNAPSHOT_BY_URL: dict[str, str] = {
    "https://yfamilys.com/module/adultraplus.sgmodule": "adultraplus.sgmodule",
    "https://yfamilys.com/module/bili.module": "bili.module",
    "https://yfamilys.com/rule/Kemono.list": "yfamilys_Kemono.list",
    "https://yfamilys.com/rule/Cloudflare.list": "yfamilys_Cloudflare.list",
}

_CURL_UA = _BROWSER_UA


def _http_cache_path(url: str) -> str:
    digest = hashlib.sha256(url.encode("utf-8")).hexdigest()[:32]
    return os.path.join(get_project_root(), ".cache", "http", digest)


def _read_http_cache(url: str) -> Optional[bytes]:
    path = _http_cache_path(url)
    if not os.path.isfile(path):
        return None
    try:
        with open(path, "rb") as f:
            return f.read()
    except OSError:
        return None


def _write_http_cache(url: str, data: bytes) -> None:
    path = _http_cache_path(url)
    try:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "wb") as f:
            f.write(data)
    except OSError:
        pass


def _read_vendor_snapshot(url: str) -> Optional[bytes]:
    rel = VENDOR_SNAPSHOT_BY_URL.get(url)
    if not rel:
        return None
    path = os.path.join(_VENDOR_DIR, rel)
    if not os.path.isfile(path):
        return None
    try:
        with open(path, "rb") as f:
            return f.read()
    except OSError:
        return None


def safe_download(url: str, binary: bool = False, retries: int = 1,
                  timeout: int = 30) -> Optional[str]:
    """Download text content from *url*.  Returns None on failure / HTML response."""
    raw = _curl_fetch(url, timeout=timeout, retries=retries)
    if raw is None:
        return None
    if _is_html_content(raw):
        Logger.warn(
            f"Download rejected: {url} returned HTML instead of raw data "
            "(blocked or redirected)."
        )
        return None
    return raw.decode("utf-8", errors="replace") if not binary else raw  # type: ignore[return-value]


def safe_download_binary(url: str, retries: int = 1,
                         timeout: int = 30) -> Optional[bytes]:
    """Download binary content from *url*.  Returns None on failure / HTML response."""
    raw = _curl_fetch(url, timeout=timeout, retries=retries)
    if raw is None:
        return None
    if _is_html_content(raw):
        Logger.warn(f"Download rejected: {url} returned HTML instead of binary data.")
        return None
    return raw


# ── internal ──────────────────────────────────────────────────────────────────

def _curl_fetch(url: str, *, timeout: int = 30, retries: int = 1) -> Optional[bytes]:
    """curl download with retry, HTTP cache, and vendor snapshot fallback."""
    cmd_base = ["curl", "-L", "-s", "-m", str(timeout), "-f",
                "-H", f"User-Agent: {_CURL_UA}",
                "-H", "Accept: text/plain, application/octet-stream, */*"]
    last_err = None
    for attempt in range(retries + 1):
        try:
            result = subprocess.run([*cmd_base, url], capture_output=True)
            if result.returncode == 0 and result.stdout:
                _write_http_cache(url, result.stdout)
                return result.stdout
            stderr_msg = result.stderr.decode("utf-8", errors="ignore").strip()
            http_hint = ""
            if result.returncode == 22:
                http_hint = " (HTTP 4xx/5xx — upstream may block datacenter IPs)"
            last_err = (
                f"curl exit {result.returncode}{http_hint}. "
                f"Stderr: {stderr_msg or 'empty'}"
            )
        except Exception as e:
            last_err = f"Exception: {e}"

        if attempt < retries:
            time.sleep(2 ** attempt)

    cached = _read_http_cache(url)
    if cached and not _is_html_content(cached):
        Logger.info(f"Using cached copy (upstream unavailable): {url}")
        return cached

    vendor = _read_vendor_snapshot(url)
    if vendor and not _is_html_content(vendor):
        rel = os.path.relpath(
            os.path.join(_VENDOR_DIR, VENDOR_SNAPSHOT_BY_URL[url]),
            get_project_root(),
        )
        Logger.info(f"Using vendor snapshot: {rel}")
        return vendor

    Logger.warn(f"Download failed: {url} | Reason: {last_err}")
    return None
