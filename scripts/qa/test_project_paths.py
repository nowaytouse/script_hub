#!/usr/bin/env python3
"""Test script to validate project paths and structure."""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hub.common import Logger
from hub.project_paths import *
from hub.error_handling import validate_dir_exists, validate_file_exists

def test_source_directories():
    """Test that all source directories are properly configured."""
    Logger.section("Testing Source Directories")
    
    dirs = [
        ("SOURCES_DIR", SOURCES_DIR),
        ("DNS_SOURCE_DIR", DNS_SOURCE_DIR),
        ("DNS_MAPPING_DIR", DNS_MAPPING_DIR),
        ("SKK_UPSTREAM_DIR", SKK_UPSTREAM_DIR),
        ("KELEE_DIR", KELEE_DIR),
        ("METACUBEX_DIR", METACUBEX_DIR),
    ]
    
    passed = 0
    for name, path in dirs:
        if validate_dir_exists(path, name):
            Logger.success(f"✓ {name}: {path}")
            passed += 1
        else:
            Logger.error(f"✗ {name}: {path}")
    
    return passed, len(dirs)

def test_output_directories():
    """Test that all output directories exist or can be created."""
    Logger.section("Testing Output Directories")
    
    dirs = [
        ("RULE_SET_DIR", RULE_SET_DIR),
        ("ADBLOCK_DIR", ADBLOCK_DIR),
        ("SINGBOX_DIR", SINGBOX_DIR),
        ("SINGBOX_DNS_DIR", SINGBOX_DNS_DIR),
    ]
    
    passed = 0
    for name, path in dirs:
        if os.path.exists(path) or os.path.exists(os.path.dirname(path)):
            Logger.success(f"✓ {name}: {path}")
            passed += 1
        else:
            Logger.warn(f"⚠ {name} parent missing: {path}")
    
    return passed, len(dirs)

def test_module_directories():
    """Test that module directories are properly configured."""
    Logger.section("Testing Module Directories")
    
    dirs = [
        ("SURGE_HEAD_EXPANSE_DIR", SURGE_HEAD_EXPANSE_DIR),
        ("SURGE_AMPLIFY_NEXUS_DIR", SURGE_AMPLIFY_NEXUS_DIR),
        ("MODULE_LOCAL_DIR", MODULE_LOCAL_DIR),
    ]
    
    passed = 0
    for name, path in dirs:
        if validate_dir_exists(path, name):
            Logger.success(f"✓ {name}: {path}")
            passed += 1
        else:
            Logger.error(f"✗ {name}: {path}")
    
    return passed, len(dirs)

def test_path_consistency():
    """Test that paths are consistent and don't have conflicts."""
    Logger.section("Testing Path Consistency")
    
    tests = []
    passed = 0
    
    # DNS paths should be organized correctly
    tests.append(("DNS mapping in Sources/dns", DNS_MAPPING_DIR.startswith(DNS_SOURCE_DIR)))
    tests.append(("SingBox DNS in SingBox", SINGBOX_DNS_DIR.startswith(SINGBOX_DIR)))
    tests.append(("Sources in rulesets", SOURCES_DIR.startswith(os.path.join(ROOT, "rulesets"))))
    
    for description, result in tests:
        if result:
            Logger.success(f"✓ {description}")
            passed += 1
        else:
            Logger.error(f"✗ {description}")
    
    return passed, len(tests)

def main():
    Logger.info(f"Project Root: {ROOT}")
    print()
    
    total_passed = 0
    total_tests = 0
    
    passed, total = test_source_directories()
    total_passed += passed
    total_tests += total
    print()
    
    passed, total = test_output_directories()
    total_passed += passed
    total_tests += total
    print()
    
    passed, total = test_module_directories()
    total_passed += passed
    total_tests += total
    print()
    
    passed, total = test_path_consistency()
    total_passed += passed
    total_tests += total
    print()
    
    Logger.section("Test Summary")
    Logger.info(f"Passed: {total_passed}/{total_tests}")
    
    if total_passed == total_tests:
        Logger.success("✅ All tests passed!")
        return 0
    else:
        Logger.error(f"❌ {total_tests - total_passed} test(s) failed")
        return 1

if __name__ == "__main__":
    sys.exit(main())
