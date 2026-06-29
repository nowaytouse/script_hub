package hub

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

var (
	SRS_SOURCE_DIRS   = []string{RULE_SET_DIR, DNS_MAPPING_DIR, ADBLOCK_DIR}
	SRS_SKIP_PREFIXES = []string{"AdBlock_"}
)

func ruleCount(path string) int {
	n := 0
	lines := ReadFile(path)
	for _, line := range lines {
		s := strings.TrimSpace(line)
		if s != "" && !strings.HasPrefix(s, "#") {
			n++
		}
	}
	return n
}

func collectListFiles() []string {
	var files []string
	for _, directory := range SRS_SOURCE_DIRS {
		if !ValidateDirExists(directory, "") {
			continue
		}
		entries, err := os.ReadDir(directory)
		if err != nil {
			continue
		}
		var names []string
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".list") {
				names = append(names, e.Name())
			}
		}
		for _, name := range names {
			files = append(files, filepath.Join(directory, name))
		}
	}
	return files
}

func AuditSrsCoverage() (int, []string) {
	var missing []string
	total := 0
	listFiles := collectListFiles()

	for _, listPath := range listFiles {
		base := strings.TrimSuffix(filepath.Base(listPath), ".list")
		skip := false
		for _, p := range SRS_SKIP_PREFIXES {
			if strings.HasPrefix(base, p) {
				skip = true
				break
			}
		}
		if skip {
			continue
		}
		if ruleCount(listPath) == 0 {
			continue
		}
		total++

		var srsPath string
		if strings.Contains(listPath, DNS_MAPPING_DIR) || strings.Contains(listPath, "/dns/") {
			srsPath = filepath.Join(SINGBOX_DNS_DIR, fmt.Sprintf("%s_Singbox.srs", base))
		} else if strings.Contains(listPath, ADBLOCK_DIR) {
			srsPath = filepath.Join(ADBLOCK_DIR, fmt.Sprintf("%s_Singbox.srs", base))
		} else {
			srsPath = filepath.Join(SINGBOX_DIR, fmt.Sprintf("%s_Singbox.srs", base))
		}

		if !ValidateFileExists(srsPath, "") {
			missing = append(missing, fmt.Sprintf("%s.list -> %s", base, filepath.Base(srsPath)))
			continue
		}

		f, err := os.Open(srsPath)
		if err != nil {
			missing = append(missing, fmt.Sprintf("%s.list -> %s", base, filepath.Base(srsPath)))
			continue
		}

		magic := make([]byte, 3)
		n, err := f.Read(magic)
		f.Close()

		if err != nil || n < 3 || string(magic) != "SRS" {
			missing = append(missing, fmt.Sprintf("%s.list -> %s", base, filepath.Base(srsPath)))
		}
	}
	return total, missing
}

func PrintPipelineSummary() {
	Section("Pipeline Summary")

	surgeLists := 0
	if ValidateDirExists(RULE_SET_DIR, "") {
		entries, _ := os.ReadDir(RULE_SET_DIR)
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".list") {
				surgeLists++
			}
		}
	}

	adblockLists := 0
	if ValidateDirExists(ADBLOCK_DIR, "") {
		entries, _ := os.ReadDir(ADBLOCK_DIR)
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".list") {
				adblockLists++
			}
		}
	}

	srsFiles := 0
	if ValidateDirExists(SINGBOX_DIR, "") {
		entries, _ := os.ReadDir(SINGBOX_DIR)
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".srs") {
				srsFiles++
			}
		}
	}

	surgeModules := 0
	filepath.Walk(SURGE_MODULE_DIR, func(path string, info os.FileInfo, err error) error {
		if err == nil && !info.IsDir() && strings.HasSuffix(info.Name(), ".sgmodule") {
			surgeModules++
		}
		return nil
	})

	srModules := 0
	if ValidateDirExists(SHADOWROCKET_MODULE_DIR, "") {
		filepath.Walk(SHADOWROCKET_MODULE_DIR, func(path string, info os.FileInfo, err error) error {
			if err == nil && !info.IsDir() && strings.HasSuffix(info.Name(), ".module") {
				srModules++
			}
			return nil
		})
	}

	Info(fmt.Sprintf("Surge rulesets: %d | AdBlock shards: %d", surgeLists, adblockLists))
	Info(fmt.Sprintf("Sing-box SRS: %d", srsFiles))
	Info(fmt.Sprintf("Surge modules: %d | Shadowrocket modules: %d", surgeModules, srModules))

	checked, missing := AuditSrsCoverage()
	if len(missing) > 0 {
		Warn(fmt.Sprintf("SRS coverage: %d/%d list files missing SRS", len(missing), checked))
		maxMissing := len(missing)
		if maxMissing > 15 {
			maxMissing = 15
		}
		for _, item := range missing[:maxMissing] {
			Warn(fmt.Sprintf("  Missing: %s", item))
		}
		if len(missing) > 15 {
			Warn(fmt.Sprintf("  ... and %d more", len(missing)-15))
		}
	} else {
		Success(fmt.Sprintf("SRS coverage OK (%d compiled sources checked)", checked))
	}
}
