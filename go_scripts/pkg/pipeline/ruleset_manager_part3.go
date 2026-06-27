package pipeline

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

func (rm *RulesetManager) Run() {
	hub.Section("Ruleset Processing (1:1 Logic)")
	hub.EnsureDir(surgeDir)

	var allFiles []string
	entries, _ := os.ReadDir(surgeDir)
	for _, entry := range entries {
		if !entry.IsDir() && strings.HasSuffix(entry.Name(), ".list") && !strings.HasSuffix(entry.Name(), "_ip.list") {
			allFiles = append(allFiles, strings.TrimSuffix(entry.Name(), ".list"))
		}
	}

	allToProcessMap := make(map[string]bool)
	for _, r := range rulesets {
		allToProcessMap[r] = true
	}
	for _, r := range deprecatedRulesets {
		allToProcessMap[r] = true
	}
	for _, r := range allFiles {
		allToProcessMap[r] = true
	}
	for r := range specialManagedRulesets {
		delete(allToProcessMap, r)
	}

	var allToProcess []string
	for r := range allToProcessMap {
		allToProcess = append(allToProcess, r)
	}
	sort.Strings(allToProcess)

	for _, name := range allToProcess {
		rm.processRuleset(name)
	}

	rm.pruneEmptyRulesets()
	rm.saveHashes()
	hub.Info(fmt.Sprintf("Done: Merged %d | Skipped %d | Deleted %d", rm.stats["merged"], rm.stats["skipped"], rm.stats["deleted"]))
}

func (rm *RulesetManager) countRules(path string) int {
	n := 0
	content := hub.ReadFileString(path)
	for _, line := range strings.Split(content, "\n") {
		s := strings.TrimSpace(line)
		if s != "" && !strings.HasPrefix(s, "#") {
			n++
		}
	}
	return n
}

func (rm *RulesetManager) pruneEmptyRulesets() {
	if !hub.ValidateDirExists(surgeDir, "") {
		return
	}
	entries, _ := os.ReadDir(surgeDir)
	for _, entry := range entries {
		if !strings.HasSuffix(entry.Name(), ".list") {
			continue
		}
		name := strings.TrimSuffix(entry.Name(), ".list")
		if specialManagedRulesets[name] || strings.HasSuffix(name, "_ip") {
			continue
		}
		path := filepath.Join(surgeDir, entry.Name())
		if rm.countRules(path) > 0 {
			continue
		}
		isDeprecated := false
		for _, dep := range deprecatedRulesets {
			if name == dep {
				isDeprecated = true
				break
			}
		}
		if isDeprecated {
			if err := os.Remove(path); err == nil {
				src := filepath.Join(hub.LINKS_DIR, fmt.Sprintf("%s_sources.list", name))
				if hub.ValidateFileExists(src, "") {
					os.Remove(src)
				}
				rm.stats["deleted"]++
				hub.Warn(fmt.Sprintf("Pruned empty deprecated ruleset: %s", name))
			}
		} else {
			hub.Warn(fmt.Sprintf("Empty ruleset not in deprecated list: %s", name))
		}
	}
}

func RunRulesetManager(force bool) {
	rm := NewRulesetManager(force)
	rm.Run()
}
