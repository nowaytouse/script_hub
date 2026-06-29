package tools

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/hub"
)

var (
	gistUrl    = "https://gist.githubusercontent.com/ddgksf2013/4f53b7c6083678df25fecc8ff68b52c4/raw/netease.adblock.js"
	outPath    = filepath.Join(hub.MODULE_SOURCE_SCRIPTS_DIR, "gist_githubusercontent_com_4f53b7_netease.adblock.js")
	adultFiles = []string{
		filepath.Join(hub.ROOT, "modules/source/local/adultraplus.sgmodule"),
		filepath.Join(hub.ROOT, "rulesets/Sources/vendor/adultraplus.sgmodule"),
	}
	localScriptUrl = hub.SCRIPT_RAW_PREFIX + "gist_githubusercontent_com_4f53b7_netease.adblock.js"
)

func wrapForSurge(raw string) string {
	lines := strings.Split(raw, "\n")
	header := "const version = 'unknown';"
	if len(lines) > 0 {
		header = lines[0]
	}
	body := raw
	if len(lines) > 2 {
		body = strings.Join(lines[2:], "\n")
	}

	return fmt.Sprintf(`%s
// Patched for Surge JSC (module.exports UMD + obfuscator) — ScriptHub
(() => {
  var module = { exports: {} };
  var exports = module.exports;
  var require = function() { return {}; };
%s
})();
`, header, body)
}

func patchAdultraplusModules() int {
	n := 0
	for _, path := range adultFiles {
		if !hub.ValidateFileExists(path, "") {
			continue
		}
		text := hub.ReadFileString(path)
		if !strings.Contains(text, gistUrl) {
			continue
		}
		hub.SafeWriteFile(path, strings.ReplaceAll(text, gistUrl, localScriptUrl), true)
		n++
		rel, _ := filepath.Rel(hub.ROOT, path)
		fmt.Printf("  Updated script-path: %s\n", rel)
	}
	return n
}

func RunPatchNeteaseAdblockForSurge() int {
	fmt.Println("Patch netease.adblock.js for Surge")
	raw := hub.SafeDownload(gistUrl, 2, 60)
	if raw == "" || len(raw) < 1000 {
		fmt.Printf("Failed to download upstream: %s\n", gistUrl)
		return 1
	}
	hub.SafeWriteFile(outPath, wrapForSurge(raw), true)
	rel, _ := filepath.Rel(hub.ROOT, outPath)
	fmt.Printf("✅ Wrote %s\n", rel)

	patched := patchAdultraplusModules()
	fmt.Printf("Patched %d adultraplus module file(s)\n", patched)
	fmt.Println("Next: run main_update adblock or equivalent pipeline")
	return 0
}
