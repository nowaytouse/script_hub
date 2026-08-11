package generatedguard

import (
	"fmt"
	"os/exec"
	"strings"
)

func runGit(args ...string) string {
	out, err := exec.Command("git", args...).Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

// Run verifies that generated-tree changes are accompanied by source or
// generator changes. It deliberately depends only on the standard library so
// the CI guard never links the application Rust FFI.
func Run() int {
	base := runGit("rev-parse", "HEAD~1")
	head := runGit("rev-parse", "HEAD")
	author := runGit("log", "-1", "--pretty=%an")

	if author == "github-actions[bot]" {
		fmt.Printf("✅ guard: bot commit (%s) exempt.\n", author)
		return 0
	}

	diffOut := runGit("diff", "--name-status", base, head)
	if diffOut == "" {
		fmt.Println("✅ guard: no changes.")
		return 0
	}

	changedScripts := false
	var generatedContentChanges []string
	validSources := []string{"scripts/", "rulesets/Sources/", "create/"}
	generatedPrefixes := []string{"modules/", "rulesets/", "dns/"}

	for _, row := range strings.Split(diffOut, "\n") {
		parts := strings.Split(row, "\t")
		if len(parts) < 2 {
			continue
		}
		status := parts[0]
		paths := parts[1:]
		for _, path := range paths {
			for _, prefix := range validSources {
				if strings.HasPrefix(path, prefix) {
					changedScripts = true
					break
				}
			}
		}

		if strings.HasPrefix(status, "R") || strings.HasPrefix(status, "C") {
			continue
		}
		target := paths[len(paths)-1]
		isGenerated := false
		for _, prefix := range generatedPrefixes {
			if strings.HasPrefix(target, prefix) {
				isGenerated = true
				break
			}
		}
		if isGenerated && !strings.HasPrefix(target, "rulesets/Sources/") {
			generatedContentChanges = append(
				generatedContentChanges,
				fmt.Sprintf("%s\t%s", status, target),
			)
		}
	}

	if len(generatedContentChanges) > 0 && !changedScripts {
		fmt.Println("❌ guard: generated tree content changed without script-layer changes.")
		fmt.Println("Please modify generation scripts under scripts/ or create/ and regenerate outputs.")
		for _, item := range generatedContentChanges {
			fmt.Printf("  - %s\n", item)
		}
		return 1
	}

	fmt.Println("✅ guard: generated-tree policy passed.")
	return 0
}
