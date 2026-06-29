package tools

import (
	"fmt"
	"path/filepath"
	"sort"
	"strings"

	"github.com/nowaytouse/script_hub/create/hub"
)

var requiredModuleStems = map[string]bool{
	"WeChat_Enhance":      true,
	"📊 面板工具合集":            true,
	"🧰 Script Hub 配套工具合集": true,
}

func verifyHtmlCoverage(modules []hub.ModuleInfo) int {
	scannedStems := make(map[string]bool)
	for _, m := range modules {
		scannedStems[strings.TrimSuffix(filepath.Base(m.Path), filepath.Ext(m.Path))] = true
	}

	var missing []string
	for req := range requiredModuleStems {
		if !scannedStems[req] {
			missing = append(missing, req)
		}
	}

	if len(missing) > 0 {
		sort.Strings(missing)
		fmt.Printf("  ⚠️  WARN: Required modules not found in scan: %v\n", missing)
		fmt.Println("       → Check that .sgmodule file exists under modules/surge/<category>/")
	}
	return len(missing)
}

func checkSrGaps(modules []hub.ModuleInfo) int {
	var gaps []string
	for _, m := range modules {
		if m.MergedInto != "" {
			continue
		}
		srPath := filepath.Join(hub.ROOT, strings.Replace(strings.Replace(m.Path, "surge", "shadowrocket", 1), ".sgmodule", ".module", 1))
		if !hub.ValidateFileExists(srPath, "") {
			gaps = append(gaps, filepath.Base(m.Path))
		}
	}
	if len(gaps) > 0 {
		sort.Strings(gaps)
		fmt.Printf("  ⚠️  SR sync gaps (%d missing):\n", len(gaps))
		for _, g := range gaps {
			fmt.Printf("       - %s\n", g)
		}
	}
	return len(gaps)
}

func RunConsolidateModules() int {
	fmt.Println(strings.Repeat("=", 60))
	fmt.Println("📦 Module Catalog (sanitize · scan · JSON · HTML)")
	fmt.Printf("   Helper URL: %s\n", hub.SURGE_MODULE_HELPER_URL)
	fmt.Println(strings.Repeat("=", 60))

	errors := 0
	hub.EnsureDir(hub.MODULES_HELPER_DIR)

	fmt.Println("\n📂 Normalizing module categories (folder = source of truth)...")
	moduleDir := filepath.Join(hub.ROOT, "modules/surge")
	srModuleDir := filepath.Join(hub.ROOT, "modules/shadowrocket")

	catSurge := hub.NormalizeCategoriesTree(moduleDir, hub.ROOT, "*.sgmodule")
	catSr := hub.NormalizeCategoriesTree(srModuleDir, hub.ROOT, "*.module")
	fmt.Printf("   Fixed %d file(s)\n", catSurge+catSr)

	fmt.Println("\n🧹 Sanitizing Surge modules...")
	surgeChanged := hub.SanitizeTree(moduleDir, hub.ROOT, "*.sgmodule")

	fmt.Println("\n🧹 Sanitizing Shadowrocket modules...")
	srChanged := hub.SanitizeTree(srModuleDir, hub.ROOT, "*.module")

	modules := hub.ScanModules(hub.ROOT, moduleDir)
	total := len(modules)
	if total == 0 {
		fmt.Println("  ❌ FATAL: scan_modules returned 0 modules — aborting.")
		return 1
	}

	fmt.Printf("\n🔍 Scanned %d Surge modules (%d files cleaned)\n", total, surgeChanged)

	// Create ordered list of categories to print
	catOrder := []string{"system", "adblock", "media", "tool", "ai", "network", "dev", "other"}
	for _, cat := range catOrder {
		label := hub.CATEGORIES[cat]
		if label == "" {
			continue
		}
		count := 0
		for _, m := range modules {
			if m.Category == cat {
				count++
			}
		}
		fmt.Printf("  - %s: %d\n", label, count)
	}

	fmt.Println("\n🔒 Integrity checks...")
	errors += verifyHtmlCoverage(modules)
	srGaps := checkSrGaps(modules)
	if srGaps > 0 {
		fmt.Println("  ℹ️  Run tools/convert_surge_to_shadowrocket.py to regenerate missing SR modules.")
	}

	hub.WriteModulesJson(modules, hub.MODULES_DATA_JSON)
	rel1, _ := filepath.Rel(hub.ROOT, hub.MODULES_DATA_JSON)
	fmt.Printf("\n✅ Wrote %s\n", rel1)

	hub.WriteShadowrocketModulesJson(modules, hub.SHADOWROCKET_MODULES_JSON, hub.ROOT)
	rel2, _ := filepath.Rel(hub.ROOT, hub.SHADOWROCKET_MODULES_JSON)
	fmt.Printf("✅ Wrote %s\n", rel2)

	hub.BuildHelperHtml(modules, hub.ROOT, hub.SURGE_MODULE_HELPER_HTML)
	rel3, _ := filepath.Rel(hub.ROOT, hub.SURGE_MODULE_HELPER_HTML)
	fmt.Printf("✅ Wrote %s\n", rel3)

	fmt.Printf("✅ Shadowrocket sanitized: %d files\n", srChanged)
	fmt.Println(strings.Repeat("=", 60))

	if errors > 0 {
		fmt.Printf("⚠️  Pipeline completed with %d warning(s) — review output above.\n", errors)
	}
	return 0
}
