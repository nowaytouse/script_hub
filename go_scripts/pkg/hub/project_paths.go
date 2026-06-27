package hub

import (
	"os"
	"path/filepath"
)

var (
	// PROJECT ROOT
	ROOT = GetProjectRoot()

	// SOURCE DIRECTORIES (Input - never auto-generated)
	SOURCES_DIR = filepath.Join(ROOT, "rulesets", "Sources")

	// DNS sources
	DNS_SOURCE_DIR     = filepath.Join(SOURCES_DIR, "dns")
	DNS_MAPPING_DIR    = filepath.Join(DNS_SOURCE_DIR, "mapping")
	DNS_CATEGORIES_DIR = filepath.Join(DNS_SOURCE_DIR, "categories")
	DNS_STEER_DIR      = filepath.Join(DNS_SOURCE_DIR, "steer")
	DNS_VENDORS_DIR    = filepath.Join(DNS_SOURCE_DIR, "vendors")

	// Upstream sources
	SKK_UPSTREAM_DIR = filepath.Join(SOURCES_DIR, "skk_upstream")
	KELEE_DIR        = filepath.Join(SOURCES_DIR, "kelee")
	METACUBEX_DIR    = filepath.Join(SOURCES_DIR, "MetaCubeX")

	// Custom sources
	CUSTOM_DIR = filepath.Join(SOURCES_DIR, "custom")
	CONF_DIR   = filepath.Join(SOURCES_DIR, "conf")
	DECO_DIR   = filepath.Join(SOURCES_DIR, "deco")
	VENDOR_DIR = filepath.Join(SOURCES_DIR, "vendor")

	// Module sources
	LOCAL_MODULES_DIR = filepath.Join(SOURCES_DIR, "LocalModules")
	SGMODULE_DIR      = filepath.Join(SOURCES_DIR, "sgmodule")
	LINKS_DIR         = filepath.Join(SOURCES_DIR, "Links")

	// Smart Config Kit
	SMART_CONFIG_KIT_DIR = filepath.Join(CUSTOM_DIR, "SmartConfigKit")
	SCK_SHUNT_RULES_DIR  = filepath.Join(SMART_CONFIG_KIT_DIR, "shunt-rules")

	// OUTPUT DIRECTORIES (Final compiled/generated files)
	RULESETS_DIR = filepath.Join(ROOT, "rulesets")
	RULE_SET_DIR = filepath.Join(RULESETS_DIR, "RULE-SET")

	// AdBlock outputs
	ADBLOCK_DIR          = filepath.Join(RULESETS_DIR, "AdBlock")
	ADBLOCK_LIST         = filepath.Join(RULE_SET_DIR, "AdBlock.list")
	ADBLOCK_CATALOG_JSON = filepath.Join(ADBLOCK_DIR, "catalog.json")

	// SingBox compiled outputs
	SINGBOX_DIR     = filepath.Join(RULESETS_DIR, "SingBox")
	SINGBOX_DNS_DIR = filepath.Join(SOURCES_DIR, "dns", "mapping")

	// MODULE DIRECTORIES
	MODULES_DIR = filepath.Join(ROOT, "modules")

	// Surge modules
	SURGE_MODULE_DIR              = filepath.Join(MODULES_DIR, "surge")
	SURGE_HEAD_EXPANSE_DIR        = filepath.Join(SURGE_MODULE_DIR, "head_expanse")
	SURGE_HEAD_EXPANSE_GITHUB_DIR = filepath.Join(SURGE_HEAD_EXPANSE_DIR, "github")
	SURGE_AMPLIFY_NEXUS_DIR       = filepath.Join(SURGE_MODULE_DIR, "amplify_nexus")

	// Shadowrocket modules
	SHADOWROCKET_MODULE_DIR              = filepath.Join(MODULES_DIR, "shadowrocket")
	SHADOWROCKET_HEAD_EXPANSE_DIR        = filepath.Join(SHADOWROCKET_MODULE_DIR, "head_expanse")
	SHADOWROCKET_HEAD_EXPANSE_GITHUB_DIR = filepath.Join(SHADOWROCKET_HEAD_EXPANSE_DIR, "github")
	SHADOWROCKET_AMPLIFY_NEXUS_DIR       = filepath.Join(SHADOWROCKET_MODULE_DIR, "amplify_nexus")

	// Module source build directory
	MODULE_SOURCE_DIR = filepath.Join(MODULES_DIR, "source")
	MODULE_LOCAL_DIR  = filepath.Join(MODULE_SOURCE_DIR, "local")
	MODULE_BUILD_DIR  = filepath.Join(MODULE_SOURCE_DIR, "build")
	PROMAX_SPLITS_DIR = filepath.Join(MODULE_BUILD_DIR, "promax_splits")

	// SCRIPT & CONFIG DIRECTORIES
	SCRIPTS_DIR               = filepath.Join(ROOT, "scripts")
	MODULE_SOURCE_SCRIPTS_DIR = filepath.Join(MODULES_DIR, "source", "scripts")
	SCRIPT_RAW_PREFIX         = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/scripts/"

	// SingBox binary paths
	SINGBOX_LOCAL_BIN = filepath.Join(SCRIPTS_DIR, "config-manager-auto-update", "bin", "sing-box")
	SINGBOX_ROOT_BIN  = filepath.Join(ROOT, "sing-box")
	SINGBOX_TAR_GZ    = filepath.Join(ROOT, "sing-box.tar.gz")

	// Cache
	CACHE_DIR      = filepath.Join(ROOT, ".cache")
	CACHE_HTTP_DIR = filepath.Join(CACHE_DIR, "http")

	// Config
	CLAUDE_DIR = filepath.Join(ROOT, ".claude")
	CONF_FILE  = filepath.Join(ROOT, ".conf", "NyaMiiKo.conf.conf")

	// SPECIFIC FILES (Commonly referenced)
	WHITELIST_FILE                  = filepath.Join(SOURCES_DIR, "adblock_whitelist.list")
	ADBLOCK_SOURCES_FILE            = filepath.Join(LINKS_DIR, "AdBlock_sources.list")
	ADBLOCK_FUNCTIONAL_SOURCES_FILE = filepath.Join(LINKS_DIR, "AdBlock_functional_sources.list")

	// DNS
	HTTPDNS_HIJACK_LIST = filepath.Join(ADBLOCK_DIR, "HTTPDNS_Hijack.list")

	// SKK
	SKK_REJECT  = filepath.Join(SKK_UPSTREAM_DIR, "reject.list")
	SKK_HTTPDNS = filepath.Join(SKK_UPSTREAM_DIR, "BlockHttpDNS.list")

	// Cache hashes
	ADBLOCK_HASH_FILE = filepath.Join(CACHE_DIR, "adblock_hashes.list")
	MERGE_HASH_FILE   = filepath.Join(CACHE_DIR, "merge_hashes.list")

	// PROMAX modules
	PROMAX_MODULE = filepath.Join(
		SURGE_HEAD_EXPANSE_DIR,
		"🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
	)
	PROMAX_LITE_MODULE = filepath.Join(
		SURGE_HEAD_EXPANSE_DIR,
		"📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule",
	)

	// Docs and scripts
	ADBLOCK_CALLCHAIN_DOC                = filepath.Join(ROOT, "docs", "AdBlock_callchain.doc")
	SURGECONF_DIRECTPORTS_LIST           = filepath.Join(CONF_DIR, "SurgeConf_DirectPorts.list")
	CONVERT_SURGE_TO_SHADOWROCKET_SCRIPT = filepath.Join(SCRIPTS_DIR, "tools", "convert_surge_to_shadowrocket.py")

	// URL PREFIXES
	CDN_BASE_URL             = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/"
	GITHUB_RAW_PREFIX        = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/"
	REPO_RAW_PREFIX          = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/"
	LIST_RAW_PREFIX          = REPO_RAW_PREFIX + "rulesets/RULE-SET/"
	LEGACY_SCRIPT_RAW_PREFIX = REPO_RAW_PREFIX + "modules/scripts/"
	LEGACY_LIST_RAW_PREFIX   = REPO_RAW_PREFIX + "ruleset/Surge(Shadowkroket)/"

	// Module helpers
	MODULES_HELPER_DIR         = filepath.Join(ROOT, "modules", "helper")
	MODULES_DATA_JSON          = filepath.Join(MODULES_HELPER_DIR, "modules_data.json")
	SHADOWROCKET_MODULES_JSON  = filepath.Join(MODULES_HELPER_DIR, "shadowrocket_modules_data.json")
	SURGE_MODULE_HELPER_HTML   = filepath.Join(MODULES_HELPER_DIR, "surge_module_helper.html")
	MODULES_COMPATIBILITY_JSON = filepath.Join(MODULES_HELPER_DIR, "modules_compatibility.json")
	SURGE_MODULE_HELPER_URL    = REPO_RAW_PREFIX + "modules/helper/surge_module_helper.html"

	// Maasea upstream
	MAASEA_SGMODULE_BASE = "https://cdn.jsdelivr.net/gh/Maasea/sgmodule@master"
	YOUTUBE_ENHANCE_URL  = MAASEA_SGMODULE_BASE + "/YouTube.Enhance.sgmodule"
	BILIBILI_HELPER_URL  = MAASEA_SGMODULE_BASE + "/Bilibili.Helper.sgmodule"
)

func EnsureDir(path string) {
	os.MkdirAll(path, 0755)
}

func EnsureAllOutputDirs() {
	dirs := []string{
		RULE_SET_DIR,
		ADBLOCK_DIR,
		SINGBOX_DIR,
		SINGBOX_DNS_DIR,
		CACHE_DIR,
		CACHE_HTTP_DIR,
		MODULE_BUILD_DIR,
		PROMAX_SPLITS_DIR,
	}
	for _, d := range dirs {
		EnsureDir(d)
	}
}

func EnsureAllSourceDirs() {
	dirs := []string{
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
	}
	for _, d := range dirs {
		EnsureDir(d)
	}
}
