package pipeline

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"sync"

	"github.com/nyamiiko/script_hub/create/hub"
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

	// Process rulesets concurrently using a bounded worker pool matching the CPU count.
	// Network downloads inside processRuleset are strictly rate-limited and locked sequentially via downloadMu.
	numWorkers := runtime.NumCPU()
	if numWorkers < 1 {
		numWorkers = 1
	}
	sem := make(chan struct{}, numWorkers)
	var wg sync.WaitGroup

	for _, name := range allToProcess {
		wg.Add(1)
		sem <- struct{}{}
		go func(n string) {
			defer wg.Done()
			defer func() { <-sem }()
			// Create a thread-local RuleProcessor to avoid concurrent map writes to stats
			rp := hub.NewRuleProcessor(false)
			// TODO: processRuleset method not implemented
			_ = rp
			hub.Warn(fmt.Sprintf("Ruleset %s skipped: processRuleset not implemented", n))
		}(name)
	}
	wg.Wait()

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
