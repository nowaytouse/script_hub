package qa

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	"github.com/nowaytouse/script_hub/create/hub"
)

func validateDirectory(directory string, allowDomainRegex bool) []string {
	var allIssues []string
	if !hub.ValidateDirExists(directory, "") {
		return []string{fmt.Sprintf("missing directory: %s", directory)}
	}

	filepath.WalkDir(directory, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		if d.IsDir() {
			return nil
		}
		if !strings.HasSuffix(d.Name(), ".list") {
			return nil
		}
		if strings.Contains(filepath.ToSlash(path), "skk_upstream") {
			return nil
		}

		text := hub.ReadFileString(path)
		lines := strings.Split(text, "\n")
		for i, line := range lines {
			errStr := hub.ValidateSurgeRulesetLine(line, allowDomainRegex)
			if errStr != nil {
				allIssues = append(allIssues, fmt.Sprintf("%s:%d: %s", d.Name(), i+1, *errStr))
			}
		}
		return nil
	})

	return allIssues
}

func RunValidateSurgeRulesets() int {
	surgeDir := filepath.Join(hub.ROOT, "rulesets/list")
	adblockDir := filepath.Join(hub.ROOT, "rulesets/AdBlock")

	var issues []string

	if info, err := os.Stat(surgeDir); err == nil && info.IsDir() {
		issues = append(issues, validateDirectory(surgeDir, false)...)
	}
	if info, err := os.Stat(adblockDir); err == nil && info.IsDir() {
		issues = append(issues, validateDirectory(adblockDir, true)...)
	}

	if len(issues) > 0 {
		fmt.Fprintln(os.Stderr, "Surge ruleset validation failed:")
		for i, item := range issues {
			if i >= 50 {
				break
			}
			fmt.Fprintf(os.Stderr, "  - %s\n", item)
		}
		if len(issues) > 50 {
			fmt.Fprintf(os.Stderr, "  ... and %d more\n", len(issues)-50)
		}
		return 1
	}

	fmt.Println("Surge ruleset validation OK")
	return 0
}
