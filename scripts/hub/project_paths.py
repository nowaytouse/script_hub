#!/usr/bin/env python3
"""
Centralized path management for script_hub project.
All path definitions should be imported from this module to ensure consistency.

Updates after task refactoring:
- DNS_mapping moved to Sources/dns/mapping/
- Compiled DNS .srs files output to SingBox/dns/
- Source files organized under Sources/
"""

import os
from pathlib import Path

# ============================================================================
# PROJECT ROOT
# ============================================================================

def get_project_root() -> str:
    """Get absolute path to project root (two levels up from this file)."""
    return str(Path(__file__).resolve().parents[2])

ROOT = get_project_root()

# ============================================================================
# SOURCE DIRECTORIES (Input - never auto-generated)
# ============================================================================

# Main source directory
SOURCES_DIR = os.path.join(ROOT, "rulesets/Sources")

# DNS sources
DNS_SOURCE_DIR = os.path.join(SOURCES_DIR, "dns")
DNS_MAPPING_DIR = os.path.join(DNS_SOURCE_DIR, "mapping")
DNS_CATEGORIES_DIR = os.path.join(DNS_SOURCE_DIR, "categories")
DNS_STEER_DIR = os.path.join(DNS_SOURCE_DIR, "steer")
DNS_VENDORS_DIR = os.path.join(DNS_SOURCE_DIR, "vendors")

# Upstream sources
SKK_UPSTREAM_DIR = os.path.join(SOURCES_DIR, "skk_upstream")
KELEE_DIR = os.path.join(SOURCES_DIR, "kelee")
METACUBEX_DIR = os.path.join(SOURCES_DIR, "MetaCubeX")

# Custom sources
CUSTOM_DIR = os.path.join(SOURCES_DIR, "custom")
CONF_DIR = os.path.join(SOURCES_DIR, "conf")
DECO_DIR = os.path.join(SOURCES_DIR, "deco")
VENDOR_DIR = os.path.join(SOURCES_DIR, "vendor")

# Module sources
LOCAL_MODULES_DIR = os.path.join(SOURCES_DIR, "LocalModules")
SGMODULE_DIR = os.path.join(SOURCES_DIR, "sgmodule")
LINKS_DIR = os.path.join(SOURCES_DIR, "Links")

# Smart Config Kit
SMART_CONFIG_KIT_DIR = os.path.join(CUSTOM_DIR, "SmartConfigKit")
SCK_SHUNT_RULES_DIR = os.path.join(SMART_CONFIG_KIT_DIR, "shunt-rules")

# ============================================================================
# OUTPUT DIRECTORIES (Final compiled/generated files)
# ============================================================================

# Main rulesets output
RULESETS_DIR = os.path.join(ROOT, "rulesets")
RULE_SET_DIR = os.path.join(RULESETS_DIR, "RULE-SET")

# AdBlock outputs
ADBLOCK_DIR = os.path.join(RULESETS_DIR, "AdBlock")
ADBLOCK_LIST = os.path.join(RULE_SET_DIR, "AdBlock.list")
ADBLOCK_CATALOG_JSON = os.path.join(ADBLOCK_DIR, "catalog.json")

# SingBox compiled outputs
SINGBOX_DIR = os.path.join(RULESETS_DIR, "SingBox")
SINGBOX_DNS_DIR = os.path.join(SOURCES_DIR, "dns/mapping")

# ============================================================================
# MODULE DIRECTORIES
# ============================================================================

MODULES_DIR = os.path.join(ROOT, "modules")

# Surge modules
SURGE_MODULE_DIR = os.path.join(MODULES_DIR, "surge")
SURGE_HEAD_EXPANSE_DIR = os.path.join(SURGE_MODULE_DIR, "head_expanse")
SURGE_AMPLIFY_NEXUS_DIR = os.path.join(SURGE_MODULE_DIR, "amplify_nexus")
SURGE_NARROW_PIERCE_DIR = os.path.join(SURGE_MODULE_DIR, "narrow_pierce")

# Shadowrocket modules
SHADOWROCKET_MODULE_DIR = os.path.join(MODULES_DIR, "shadowrocket")
SHADOWROCKET_HEAD_EXPANSE_DIR = os.path.join(SHADOWROCKET_MODULE_DIR, "head_expanse")
SHADOWROCKET_AMPLIFY_NEXUS_DIR = os.path.join(SHADOWROCKET_MODULE_DIR, "amplify_nexus")
SHADOWROCKET_NARROW_PIERCE_DIR = os.path.join(SHADOWROCKET_MODULE_DIR, "narrow_pierce")

# Module source build directory
MODULE_SOURCE_DIR = os.path.join(MODULES_DIR, "source")
MODULE_LOCAL_DIR = os.path.join(MODULE_SOURCE_DIR, "local")
MODULE_BUILD_DIR = os.path.join(MODULE_SOURCE_DIR, "build")
PROMAX_SPLITS_DIR = os.path.join(MODULE_BUILD_DIR, "promax_splits")

# ============================================================================
# SCRIPT & CONFIG DIRECTORIES
# ============================================================================

SCRIPTS_DIR = os.path.join(ROOT, "scripts")
SCRIPT_RAW_PREFIX = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/scripts/"

# SingBox binary paths
SINGBOX_LOCAL_BIN = os.path.join(SCRIPTS_DIR, "config-manager-auto-update/bin/sing-box")
SINGBOX_ROOT_BIN = os.path.join(ROOT, "sing-box")
SINGBOX_TAR_GZ = os.path.join(ROOT, "sing-box.tar.gz")

# Cache
CACHE_DIR = os.path.join(ROOT, ".cache")
CACHE_HTTP_DIR = os.path.join(CACHE_DIR, "http")

# Config
CLAUDE_DIR = os.path.join(ROOT, ".claude")
CONF_FILE = os.path.join(ROOT, ".conf/NyaMiiKo.conf.conf")

# ============================================================================
# SPECIFIC FILES (Commonly referenced)
# ============================================================================

# AdBlock
# Whitelist for AdBlock domains that shouldn't be blocked (e.g., googlevideo, analytics used for login)
WHITELIST_FILE = os.path.join(SOURCES_DIR, "adblock_whitelist.list")
ADBLOCK_SOURCES_FILE = os.path.join(LINKS_DIR, "AdBlock_sources.list")
ADBLOCK_FUNCTIONAL_SOURCES_FILE = os.path.join(LINKS_DIR, "AdBlock_functional_sources.list")

# DNS
HTTPDNS_HIJACK_LIST = os.path.join(ADBLOCK_DIR, "HTTPDNS_Hijack.list")

# SKK
SKK_REJECT = os.path.join(SKK_UPSTREAM_DIR, "reject.list")
SKK_HTTPDNS = os.path.join(SKK_UPSTREAM_DIR, "BlockHttpDNS.list")

# Cache hashes
ADBLOCK_HASH_FILE = os.path.join(CACHE_DIR, "adblock_hashes.list")
MERGE_HASH_FILE = os.path.join(CACHE_DIR, "merge_hashes.list")

# PROMAX modules
PROMAX_MODULE = os.path.join(
    SURGE_HEAD_EXPANSE_DIR,
    "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule"
)
PROMAX_LITE_MODULE = os.path.join(
    SURGE_HEAD_EXPANSE_DIR,
    "📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule"
)

# Docs and scripts
ADBLOCK_CALLCHAIN_DOC = os.path.join(ROOT, "docs", "AdBlock_callchain.doc")
SURGECONF_DIRECTPORTS_LIST = os.path.join(CONF_DIR, "SurgeConf_DirectPorts.list")
CONVERT_SURGE_TO_SHADOWROCKET_SCRIPT = os.path.join(SCRIPTS_DIR, "tools", "convert_surge_to_shadowrocket.py")

# ============================================================================
# URL PREFIXES
# ============================================================================

CDN_BASE_URL = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/"
REPO_RAW_PREFIX = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/"
LIST_RAW_PREFIX = f"{REPO_RAW_PREFIX}rulesets/RULE-SET/"
LEGACY_SCRIPT_RAW_PREFIX = f"{REPO_RAW_PREFIX}modules/scripts/"
LEGACY_LIST_RAW_PREFIX = f"{REPO_RAW_PREFIX}ruleset/Surge(Shadowkroket)/"

# Module helpers
MODULES_HELPER_DIR = os.path.join(ROOT, "modules/helper")
MODULES_DATA_JSON = os.path.join(MODULES_HELPER_DIR, "modules_data.json")
SHADOWROCKET_MODULES_JSON = os.path.join(MODULES_HELPER_DIR, "shadowrocket_modules_data.json")
SURGE_MODULE_HELPER_HTML = os.path.join(MODULES_HELPER_DIR, "surge_module_helper.html")
MODULES_COMPATIBILITY_JSON = os.path.join(MODULES_HELPER_DIR, "modules_compatibility.json")
SURGE_MODULE_HELPER_URL = f"{REPO_RAW_PREFIX}modules/helper/surge_module_helper.html"

# Maasea upstream
MAASEA_SGMODULE_BASE = "https://cdn.jsdelivr.net/gh/Maasea/sgmodule@master"
YOUTUBE_ENHANCE_URL = f"{MAASEA_SGMODULE_BASE}/YouTube.Enhance.sgmodule"
BILIBILI_HELPER_URL = f"{MAASEA_SGMODULE_BASE}/Bilibili.Helper.sgmodule"

# ============================================================================
# HELPER FUNCTIONS
# ============================================================================

def ensure_dir(path: str) -> None:
    """Create directory if it doesn't exist."""
    os.makedirs(path, exist_ok=True)

def ensure_all_output_dirs() -> None:
    """Create all output directories if they don't exist."""
    dirs = [
        RULE_SET_DIR,
        ADBLOCK_DIR,
        SINGBOX_DIR,
        SINGBOX_DNS_DIR,
        CACHE_DIR,
        CACHE_HTTP_DIR,
        MODULE_BUILD_DIR,
        PROMAX_SPLITS_DIR,
    ]
    for d in dirs:
        ensure_dir(d)

def ensure_all_source_dirs() -> None:
    """Create all source directories if they don't exist."""
    dirs = [
        SOURCES_DIR,
        DNS_SOURCE_DIR,
        DNS_MAPPING_DIR,
        DNS_CATEGORIES_DIR,
        DNS_STEER_DIR,
        DNS_VENDORS_DIR,
        SKK_UPSTREAM_DIR,
        KELEE_DIR,
        METACUBEX_DIR,
        CUSTOM_DIR,
        CONF_DIR,
        DECO_DIR,
        VENDOR_DIR,
        LOCAL_MODULES_DIR,
        SGMODULE_DIR,
        LINKS_DIR,
    ]
    for d in dirs:
        ensure_dir(d)

if __name__ == "__main__":
    print("Script Hub Project Paths")
    print("=" * 60)
    print(f"ROOT: {ROOT}")
    print("\nSource Directories:")
    print(f"  SOURCES_DIR: {SOURCES_DIR}")
    print(f"  DNS_MAPPING_DIR: {DNS_MAPPING_DIR}")
    print(f"  SKK_UPSTREAM_DIR: {SKK_UPSTREAM_DIR}")
    print(f"  KELEE_DIR: {KELEE_DIR}")
    print("\nOutput Directories:")
    print(f"  RULE_SET_DIR: {RULE_SET_DIR}")
    print(f"  ADBLOCK_DIR: {ADBLOCK_DIR}")
    print(f"  SINGBOX_DIR: {SINGBOX_DIR}")
    print(f"  SINGBOX_DNS_DIR: {SINGBOX_DNS_DIR}")
    print("\nModule Directories:")
    print(f"  SURGE_HEAD_EXPANSE_DIR: {SURGE_HEAD_EXPANSE_DIR}")
    print(f"  MODULE_LOCAL_DIR: {MODULE_LOCAL_DIR}")
