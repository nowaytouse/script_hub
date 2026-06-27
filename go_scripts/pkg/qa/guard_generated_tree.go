package qa

import (
	"fmt"
	"os/exec"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

func runGitCmd(args ...string) string {
	cmd := exec.Command("git", args...)
	cmd.Dir = hub.ROOT
	out, err := cmd.Output()
	if err != nil {
		// return empty string on error, same as python throwing but we just swallow for simplicity
		// or return empty and let caller handle it.
		return ""
	}
	return strings.TrimSpace(string(out))
}

func RunGuardGeneratedTree() int {
	base := runGitCmd("rev-parse", "HEAD~1")
	head := runGitCmd("rev-parse", "HEAD")
	author := runGitCmd("log", "-1", "--pretty=%an")

	if author == "github-actions[bot]" {
		fmt.Printf("✅ guard: bot commit (%s) exempt.\n", author)
		return 0
	}

	diffOut := runGitCmd("diff", "--name-status", base, head)
	if diffOut == "" {
		fmt.Println("✅ guard: no changes.")
		return 0
	}

	rows := strings.Split(diffOut, "\n")
	changedScripts := false
	var generatedContentChanges []string

	validSources := []string{"scripts/", "rulesets/Sources/", "go_scripts/"}
	generatedPrefixes := []string{"modules/", "rulesets/", "dns/"}

	for _, row := range rows {
		parts := strings.Split(row, "\t")
		if len(parts) == 0 {
			continue
		}
		status := parts[0]
		paths := parts[1:]
		for _, p := range paths {
			for _, prefix := range validSources {
				if strings.HasPrefix(p, prefix) {
					changedScripts = true
					break
				}
			}
		}

		if strings.HasPrefix(status, "R") || strings.HasPrefix(status, "C") {
			continue
		}

		target := ""
		if len(paths) > 0 {
			target = paths[len(paths)-1]
		}

		isGenerated := false
		for _, prefix := range generatedPrefixes {
			if strings.HasPrefix(target, prefix) {
				isGenerated = true
				break
			}
		}

		if isGenerated && !strings.HasPrefix(target, "rulesets/Sources/") {
			generatedContentChanges = append(generatedContentChanges, fmt.Sprintf("%s\t%s", status, target))
		}
	}

	if len(generatedContentChanges) > 0 && !changedScripts {
		fmt.Println("❌ guard: generated tree content changed without script-layer changes.")
		fmt.Println("Please modify generation scripts under scripts/ or go_scripts/ and regenerate outputs.")
		for _, item := range generatedContentChanges {
			fmt.Printf("  - %s\n", item)
		}
		return 1
	}

	fmt.Println("✅ guard: generated-tree policy passed.")
	return 0
}
