package qa

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/hub"
)

func testSourceDirectories() (int, int) {
	hub.Section("Testing Source Directories")
	dirs := []struct {
		name string
		path string
	}{
		{"SOURCES_DIR", hub.SOURCES_DIR},
		{"DNS_SOURCE_DIR", hub.DNS_SOURCE_DIR},
		{"DNS_MAPPING_DIR", hub.DNS_MAPPING_DIR},
		{"SKK_UPSTREAM_DIR", hub.SKK_UPSTREAM_DIR},
		{"KELEE_DIR", hub.KELEE_DIR},
		{"METACUBEX_DIR", hub.METACUBEX_DIR},
	}
	passed := 0
	for _, d := range dirs {
		if hub.ValidateDirExists(d.path, d.name) {
			hub.Success(fmt.Sprintf("✓ %s: %s", d.name, d.path))
			passed++
		} else {
			hub.Error(fmt.Sprintf("✗ %s: %s", d.name, d.path))
		}
	}
	return passed, len(dirs)
}

func testOutputDirectories() (int, int) {
	hub.Section("Testing Output Directories")
	dirs := []struct {
		name string
		path string
	}{
		{"RULE_SET_DIR", hub.RULE_SET_DIR},
		{"ADBLOCK_DIR", hub.ADBLOCK_DIR},
		{"SINGBOX_DIR", hub.SINGBOX_DIR},
		{"SINGBOX_DNS_DIR", hub.SINGBOX_DNS_DIR},
	}
	passed := 0
	for _, d := range dirs {
		if hub.ValidateFileExists(d.path, "") || hub.ValidateFileExists(filepath.Dir(d.path), "") {
			hub.Success(fmt.Sprintf("✓ %s: %s", d.name, d.path))
			passed++
		} else {
			hub.Warn(fmt.Sprintf("⚠ %s parent missing: %s", d.name, d.path))
		}
	}
	return passed, len(dirs)
}

func testModuleDirectories() (int, int) {
	hub.Section("Testing Module Directories")
	dirs := []struct {
		name string
		path string
	}{
		{"SURGE_HEAD_EXPANSE_DIR", hub.SURGE_HEAD_EXPANSE_DIR},
		{"SURGE_AMPLIFY_NEXUS_DIR", hub.SURGE_AMPLIFY_NEXUS_DIR},
		{"MODULE_LOCAL_DIR", hub.MODULE_LOCAL_DIR},
	}
	passed := 0
	for _, d := range dirs {
		if hub.ValidateDirExists(d.path, d.name) {
			hub.Success(fmt.Sprintf("✓ %s: %s", d.name, d.path))
			passed++
		} else {
			hub.Error(fmt.Sprintf("✗ %s: %s", d.name, d.path))
		}
	}
	return passed, len(dirs)
}

func testPathConsistency() (int, int) {
	hub.Section("Testing Path Consistency")
	tests := []struct {
		desc   string
		result bool
	}{
		{"DNS mapping in Sources/dns", strings.HasPrefix(hub.DNS_MAPPING_DIR, hub.DNS_SOURCE_DIR)},
		{"SingBox DNS in Sources/dns", strings.HasPrefix(hub.SINGBOX_DNS_DIR, hub.DNS_SOURCE_DIR)},
		{"Sources in rulesets", strings.HasPrefix(hub.SOURCES_DIR, filepath.Join(hub.ROOT, "rulesets"))},
	}
	passed := 0
	for _, t := range tests {
		if t.result {
			hub.Success(fmt.Sprintf("✓ %s", t.desc))
			passed++
		} else {
			hub.Error(fmt.Sprintf("✗ %s", t.desc))
		}
	}
	return passed, len(tests)
}

func RunTestProjectPaths() int {
	hub.Info(fmt.Sprintf("Project Root: %s", hub.ROOT))
	fmt.Println()

	totalPassed := 0
	totalTests := 0

	p, t := testSourceDirectories()
	totalPassed += p
	totalTests += t
	fmt.Println()

	p, t = testOutputDirectories()
	totalPassed += p
	totalTests += t
	fmt.Println()

	p, t = testModuleDirectories()
	totalPassed += p
	totalTests += t
	fmt.Println()

	p, t = testPathConsistency()
	totalPassed += p
	totalTests += t
	fmt.Println()

	hub.Section("Test Summary")
	hub.Info(fmt.Sprintf("Passed: %d/%d", totalPassed, totalTests))

	if totalPassed == totalTests {
		hub.Success("✅ All tests passed!")
		return 0
	}

	hub.Error(fmt.Sprintf("❌ %d test(s) failed", totalTests-totalPassed))
	return 1
}
