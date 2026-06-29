package qa

import (
	"fmt"
	"io/fs"
	"path/filepath"
	"regexp"
	"strings"
	"unicode/utf8"

	"github.com/nyamiiko/script_hub/go_scripts/hub"
)

var metaRe = regexp.MustCompile(`^#!\s*([A-Za-z0-9_-]+)\s*=.*$`)

func validateModule(path string) ([]string, []string) {
	var issues []string
	var warnings []string

	text := hub.ReadFileString(path)
	if text == "" {
		return []string{fmt.Sprintf("%s: empty module file", path)}, nil
	}

	lines := strings.Split(text, "\n")
	inHeader := true
	seenSection := false
	seenKeys := make(map[string]bool)

	for lineno, raw := range lines {
		raw = strings.TrimPrefix(raw, "\ufeff")
		stripped := strings.TrimSpace(raw)

		if inHeader && strings.HasPrefix(stripped, "[") && strings.HasSuffix(stripped, "]") {
			inHeader = false
			seenSection = true
			continue
		}

		if inHeader {
			if stripped == "" {
				continue
			}
			if strings.HasPrefix(stripped, "#!") {
				if utf8.RuneCountInString(stripped) > 4096 {
					issues = append(issues, fmt.Sprintf("%s:%d: metadata line too long (> 4096)", path, lineno+1))
				}
				m := metaRe.FindStringSubmatch(stripped)
				if m == nil {
					issues = append(issues, fmt.Sprintf("%s:%d: malformed metadata line", path, lineno+1))
					continue
				}
				seenKeys[strings.ToLower(m[1])] = true
				continue
			}
			// Treat plain text in header as implicit description/comments
			continue
		}
	}

	if !seenSection {
		issues = append(issues, fmt.Sprintf("%s: missing any [Section] block", path))
	}

	recommendedKeys := []string{"name", "desc", "author", "category"}
	var missing []string
	for _, k := range recommendedKeys {
		if !seenKeys[k] {
			missing = append(missing, k)
		}
	}
	if len(missing) > 0 {
		warnings = append(warnings, fmt.Sprintf("%s: missing recommended metadata keys: %s", path, strings.Join(missing, ", ")))
	}

	return issues, warnings
}

func RunValidateModuleHeaders() int {
	moduleRoots := []string{
		filepath.Join(hub.ROOT, "modules/surge"),
		filepath.Join(hub.ROOT, "modules/shadowrocket"),
	}

	var allIssues []string
	var allWarnings []string

	for _, base := range moduleRoots {
		if !hub.ValidateDirExists(base, "") {
			allIssues = append(allIssues, fmt.Sprintf("%s: directory not found", base))
			continue
		}
		pattern := "*.module"
		if strings.Contains(filepath.ToSlash(base), "modules/surge") {
			pattern = "*.sgmodule"
		}

		filepath.WalkDir(base, func(path string, d fs.DirEntry, err error) error {
			if err != nil {
				return nil
			}
			matched, _ := filepath.Match(pattern, d.Name())
			if !d.IsDir() && matched {
				issues, warnings := validateModule(path)
				allIssues = append(allIssues, issues...)
				allWarnings = append(allWarnings, warnings...)
			}
			return nil
		})
	}

	if len(allIssues) > 0 {
		fmt.Println("❌ Module header validation failed:")
		for _, issue := range allIssues {
			fmt.Printf("  - %s\n", issue)
		}
		return 1
	}

	if len(allWarnings) > 0 {
		fmt.Println("⚠️ Module header validation warnings:")
		for _, warning := range allWarnings {
			fmt.Printf("  - %s\n", warning)
		}
	}

	fmt.Println("✅ Module header validation passed.")
	return 0
}
