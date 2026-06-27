package qa

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
	"github.com/nyamiiko/script_hub/go_scripts/pkg/pipeline"
)

func auditPath(path string) {
	text := hub.ReadFileString(path)
	mode := hub.ModuleIngestMode(path, text)
	_, stats := hub.SplitModuleSections(text, path)

	if stats.SkippedLines == 0 && mode == "full" {
		return
	}

	rel, _ := filepath.Rel(hub.ROOT, path)
	fmt.Printf("%s\n  mode=%s  kept=%d/%d  dropped=%d\n",
		rel,
		mode,
		stats.KeptLines,
		stats.TotalLines,
		stats.SkippedLines)
}

func RunAuditPromaxFunctionalIngest() int {
	mgr := pipeline.NewAdBlockManager()
	paths, _ := mgr.LoadFunctionalSourcePaths()
	fmt.Println("=== Functional sources with drops (split preview) ===")
	for _, p := range paths {
		auditPath(p[0])
	}

	if hub.ValidateFileExists(hub.MODULE_LOCAL_DIR, "") {
		fmt.Println("\n=== LocalModules (all) ===")
		entries, _ := os.ReadDir(hub.MODULE_LOCAL_DIR)
		var names []string
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".sgmodule") {
				names = append(names, e.Name())
			}
		}
		sort.Strings(names)
		for _, name := range names {
			auditPath(filepath.Join(hub.MODULE_LOCAL_DIR, name))
		}
	}

	return 0
}
