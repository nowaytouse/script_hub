package qa

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/nowaytouse/script_hub/create/hub"
)

func checkAnomalies() []string {
	var errors []string
	rulesetsDir := filepath.Join(hub.ROOT, "rulesets")

	err := filepath.Walk(rulesetsDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if !info.IsDir() && strings.HasSuffix(path, ".list") {
			content := hub.ReadFileString(path)
			lines := strings.Split(content, "\n")

			isEmpty := true
			for _, l := range lines {
				if strings.TrimSpace(l) != "" {
					isEmpty = false
					break
				}
			}
			if isEmpty {
				errors = append(errors, fmt.Sprintf("Empty file: %s", path))
			}

			for i, line := range lines {
				line = strings.TrimSpace(line)
				if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, "//") {
					continue
				}

				if strings.HasPrefix(line, "DOMAIN-REGEX") {
					errors = append(errors, fmt.Sprintf("%s:%d Contains forbidden DOMAIN-REGEX: %s", filepath.Base(path), i+1, line))
				}

				if (strings.Contains(line, "#") && !strings.HasPrefix(line, "URL-REGEX") && !strings.HasPrefix(line, "DOMAIN-REGEX")) ||
					(strings.HasPrefix(line, "PROCESS-NAME") && strings.Contains(line, "#")) {
					if strings.Contains(line, "PROCESS-NAME") && strings.Contains(line, "#") {
						errors = append(errors, fmt.Sprintf("%s:%d Unstripped comment in rule: %s", filepath.Base(path), i+1, line))
					}
				}

				if strings.Contains(line, "URL-REGEX,https:") && !strings.Contains(line, "://") {
					errors = append(errors, fmt.Sprintf("%s:%d Invalid URL-REGEX: %s", filepath.Base(path), i+1, line))
				}
			}
		}
		return nil
	})

	if err != nil {
		fmt.Printf("Error walking directory %s: %v\n", rulesetsDir, err)
	}

	return errors
}

func RunCheckAnomalies() int {
	errors := checkAnomalies()
	for _, err := range errors {
		fmt.Println(err)
	}
	fmt.Printf("Total anomalies found: %d\n", len(errors))
	if len(errors) > 0 {
		return 1
	}
	return 0
}
