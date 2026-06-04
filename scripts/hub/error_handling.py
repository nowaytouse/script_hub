#!/usr/bin/env python3
"""
Unified error handling, retry logic, and validation for script_hub.
All scripts should use these utilities for robustness.
"""

import os
import time
import functools
from typing import Callable, Optional, Any, TypeVar
from pathlib import Path

from hub.common import Logger

T = TypeVar('T')

# ============================================================================
# PATH VALIDATION
# ============================================================================

def validate_file_exists(path: str, description: str = "File") -> bool:
    """Validate that a file exists and is readable."""
    if not path:
        Logger.error(f"{description} path is empty")
        return False
    
    if not os.path.exists(path):
        Logger.error(f"{description} not found: {path}")
        return False
    
    if not os.path.isfile(path):
        Logger.error(f"{description} is not a file: {path}")
        return False
    
    if not os.access(path, os.R_OK):
        Logger.error(f"{description} is not readable: {path}")
        return False
    
    return True


def validate_dir_exists(path: str, description: str = "Directory") -> bool:
    """Validate that a directory exists and is accessible."""
    if not path:
        Logger.error(f"{description} path is empty")
        return False
    
    if not os.path.exists(path):
        Logger.error(f"{description} not found: {path}")
        return False
    
    if not os.path.isdir(path):
        Logger.error(f"{description} is not a directory: {path}")
        return False
    
    if not os.access(path, os.R_OK | os.X_OK):
        Logger.error(f"{description} is not accessible: {path}")
        return False
    
    return True


def ensure_parent_dir(file_path: str) -> bool:
    """Ensure parent directory exists for a file path."""
    try:
        parent = os.path.dirname(file_path)
        if parent and not os.path.exists(parent):
            os.makedirs(parent, exist_ok=True)
            Logger.info(f"Created directory: {parent}")
        return True
    except OSError as e:
        Logger.error(f"Failed to create parent directory for {file_path}: {e}")
        return False


def validate_output_writable(path: str, description: str = "Output file") -> bool:
    """Validate that output path is writable."""
    if not path:
        Logger.error(f"{description} path is empty")
        return False
    
    # Check if parent directory is writable
    parent = os.path.dirname(path) or "."
    if not os.path.exists(parent):
        return ensure_parent_dir(path)
    
    if not os.access(parent, os.W_OK):
        Logger.error(f"Cannot write to directory: {parent}")
        return False
    
    # If file exists, check if writable
    if os.path.exists(path):
        if not os.access(path, os.W_OK):
            Logger.error(f"{description} is not writable: {path}")
            return False
    
    return True


# ============================================================================
# RETRY LOGIC
# ============================================================================

def retry_on_failure(
    max_attempts: int = 3,
    delay: float = 1.0,
    backoff: float = 2.0,
    exceptions: tuple = (Exception,),
    on_retry: Optional[Callable[[int, Exception], None]] = None,
) -> Callable:
    """
    Decorator to retry a function on failure with exponential backoff.
    
    Args:
        max_attempts: Maximum number of attempts
        delay: Initial delay between retries (seconds)
        backoff: Multiplier for delay after each failure
        exceptions: Tuple of exception types to catch
        on_retry: Optional callback called on each retry with (attempt, exception)
    """
    def decorator(func: Callable[..., T]) -> Callable[..., T]:
        @functools.wraps(func)
        def wrapper(*args, **kwargs) -> T:
            current_delay = delay
            last_exception = None
            
            for attempt in range(1, max_attempts + 1):
                try:
                    return func(*args, **kwargs)
                except exceptions as e:
                    last_exception = e
                    
                    if attempt == max_attempts:
                        Logger.error(
                            f"{func.__name__} failed after {max_attempts} attempts: {e}"
                        )
                        raise
                    
                    if on_retry:
                        on_retry(attempt, e)
                    else:
                        Logger.warn(
                            f"{func.__name__} attempt {attempt}/{max_attempts} failed: {e}. "
                            f"Retrying in {current_delay:.1f}s..."
                        )
                    
                    time.sleep(current_delay)
                    current_delay *= backoff
            
            # Should never reach here, but just in case
            raise last_exception or RuntimeError(f"{func.__name__} failed unexpectedly")
        
        return wrapper
    return decorator


# ============================================================================
# SAFE OPERATIONS
# ============================================================================

def safe_read_file(path: str, encoding: str = 'utf-8', fallback_encoding: str = 'latin-1') -> Optional[str]:
    """Safely read a file with encoding fallback."""
    if not validate_file_exists(path):
        return None
    
    try:
        with open(path, 'r', encoding=encoding) as f:
            return f.read()
    except UnicodeDecodeError:
        Logger.warn(f"Failed to decode {path} with {encoding}, trying {fallback_encoding}")
        try:
            with open(path, 'r', encoding=fallback_encoding) as f:
                return f.read()
        except Exception as e:
            Logger.error(f"Failed to read {path}: {e}")
            return None
    except Exception as e:
        Logger.error(f"Failed to read {path}: {e}")
        return None


def safe_write_file(path: str, content: str, atomic: bool = True) -> bool:
    """Safely write content to a file with atomic write option."""
    if not validate_output_writable(path):
        return False
    
    try:
        if atomic:
            import tempfile
            import shutil
            
            # Write to temp file first
            parent = os.path.dirname(path) or "."
            fd, tmp_path = tempfile.mkstemp(dir=parent, prefix=".tmp_", suffix=".writing")
            try:
                with os.fdopen(fd, 'w', encoding='utf-8') as f:
                    f.write(content)
                # Atomic rename
                shutil.move(tmp_path, path)
                return True
            except Exception as e:
                if os.path.exists(tmp_path):
                    os.remove(tmp_path)
                raise
        else:
            with open(path, 'w', encoding='utf-8') as f:
                f.write(content)
            return True
    except Exception as e:
        Logger.error(f"Failed to write {path}: {e}")
        return False


def safe_remove_file(path: str, missing_ok: bool = True) -> bool:
    """Safely remove a file."""
    if not os.path.exists(path):
        if missing_ok:
            return True
        Logger.warn(f"File does not exist: {path}")
        return False
    
    if not os.path.isfile(path):
        Logger.error(f"Not a file: {path}")
        return False
    
    try:
        os.remove(path)
        return True
    except PermissionError:
        Logger.error(f"Permission denied: {path}")
        return False
    except Exception as e:
        Logger.error(f"Failed to remove {path}: {e}")
        return False


# ============================================================================
# INPUT VALIDATION
# ============================================================================

def validate_not_empty(value: Any, name: str) -> bool:
    """Validate that a value is not empty."""
    if value is None:
        Logger.error(f"{name} is None")
        return False
    
    if isinstance(value, (str, list, dict, set)) and len(value) == 0:
        Logger.error(f"{name} is empty")
        return False
    
    return True


def validate_url(url: str) -> bool:
    """Basic URL validation."""
    if not url:
        return False
    
    url_lower = url.lower()
    if not (url_lower.startswith('http://') or url_lower.startswith('https://')):
        Logger.error(f"Invalid URL scheme: {url}")
        return False
    
    if ' ' in url or '\n' in url or '\t' in url:
        Logger.error(f"URL contains whitespace: {url}")
        return False
    
    return True


# ============================================================================
# CONTEXT MANAGERS
# ============================================================================

class safe_operation:
    """Context manager for safe operations with cleanup."""
    
    def __init__(self, operation_name: str, cleanup_func: Optional[Callable] = None):
        self.operation_name = operation_name
        self.cleanup_func = cleanup_func
        self.success = False
    
    def __enter__(self):
        Logger.info(f"Starting: {self.operation_name}")
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb):
        if exc_type is not None:
            Logger.error(f"Failed: {self.operation_name} - {exc_val}")
            if self.cleanup_func:
                try:
                    self.cleanup_func()
                except Exception as cleanup_error:
                    Logger.error(f"Cleanup failed: {cleanup_error}")
            return False
        else:
            self.success = True
            Logger.success(f"Completed: {self.operation_name}")
            return True


# ============================================================================
# HELPER FUNCTIONS
# ============================================================================

def check_network_connectivity(test_url: str = "https://www.google.com", timeout: int = 5) -> bool:
    """Check if network is accessible."""
    try:
        import urllib.request
        urllib.request.urlopen(test_url, timeout=timeout)
        return True
    except Exception:
        Logger.warn("Network connectivity check failed")
        return False


def check_disk_space(path: str, required_mb: int = 100) -> bool:
    """Check if sufficient disk space is available."""
    try:
        import shutil
        stat = shutil.disk_usage(path)
        available_mb = stat.free / (1024 * 1024)
        
        if available_mb < required_mb:
            Logger.error(
                f"Insufficient disk space: {available_mb:.1f}MB available, "
                f"{required_mb}MB required"
            )
            return False
        return True
    except Exception as e:
        Logger.warn(f"Could not check disk space: {e}")
        return True  # Don't block operation if check fails


if __name__ == "__main__":
    # Self-test
    print("Error Handling Utilities - Self Test")
    print("=" * 60)
    
    # Test path validation
    print("\n1. Path Validation:")
    print(f"   Valid file: {validate_file_exists(__file__, 'This script')}")
    print(f"   Invalid file: {validate_file_exists('/nonexistent/file.txt', 'Test file')}")
    
    # Test retry decorator
    print("\n2. Retry Logic:")
    
    @retry_on_failure(max_attempts=3, delay=0.1)
    def flaky_function(fail_count: int):
        flaky_function.attempt = getattr(flaky_function, 'attempt', 0) + 1
        if flaky_function.attempt < fail_count:
            raise ValueError(f"Attempt {flaky_function.attempt} failed")
        return f"Success on attempt {flaky_function.attempt}"
    
    try:
        result = flaky_function(2)
        print(f"   {result}")
    except Exception as e:
        print(f"   Failed: {e}")
    
    print("\n3. Safe File Operations:")
    import tempfile
    with tempfile.NamedTemporaryFile(mode='w', delete=False, suffix='.txt') as tf:
        temp_path = tf.name
    
    success = safe_write_file(temp_path, "Test content\n")
    print(f"   Write: {'Success' if success else 'Failed'}")
    
    content = safe_read_file(temp_path)
    print(f"   Read: {content.strip() if content else 'Failed'}")
    
    success = safe_remove_file(temp_path)
    print(f"   Remove: {'Success' if success else 'Failed'}")
    
    print("\n✅ Self-test completed")
