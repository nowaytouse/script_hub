package hub

import (
	"fmt"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
)

var sectionHeaderRe = regexp.MustCompile(`^\[([^\]]+)\]\s*$`)

type SplitStats struct {
	TotalLines   int
	KeptLines    int
	SkippedLines int
}

func SplitModuleSections(text string, sourcePath string) (map[string][]string, SplitStats) {
	adSections := make(map[string][]string)
	currentSection := ""
	total := 0
	kept := 0
	skipped := 0

	lines := strings.Split(text, "\n")
	for _, raw := range lines {
		stripped := strings.TrimSpace(raw)
		m := sectionHeaderRe.FindStringSubmatch(stripped)
		if m != nil && !strings.HasPrefix(stripped, "#") {
			currentSection = m[1]
			continue
		}
		if stripped == "" || strings.HasPrefix(stripped, "#") {
			continue
		}
		if currentSection == "" {
			continue
		}
		secLower := strings.ToLower(currentSection)
		if secLower == "general" || secLower == "ponte" {
			skipped++
			total++
			continue
		}
		total++
		if ShouldKeepPromaxLine(stripped, currentSection, sourcePath) {
			adSections[currentSection] = append(adSections[currentSection], stripped)
			kept++
		} else {
			skipped++
		}
	}

	return adSections, SplitStats{
		TotalLines:   total,
		KeptLines:    kept,
		SkippedLines: skipped,
	}
}

func FormatSplitModule(sourcePath string, adSections map[string][]string, note string) string {
	base := filepath.Base(sourcePath)
	lines := []string{
		fmt.Sprintf("#!name=PROMAX split · %s", base),
		fmt.Sprintf("#!desc=%s", note),
		"#!author=ScriptHub-Automated",
		fmt.Sprintf("#!split-source=%s", sourcePath),
	}

	order := []string{
		"Rule",
		"URL Rewrite",
		"Map Local",
		"Script",
		"Body Rewrite",
		"Header Rewrite",
		"MITM",
	}

	seen := make(map[string]bool)
	for k := range adSections {
		seen[k] = true
	}

	for _, sec := range order {
		if !seen[sec] {
			continue
		}
		body := adSections[sec]
		if len(body) == 0 {
			continue
		}
		lines = append(lines, "")
		lines = append(lines, fmt.Sprintf("[%s]", sec))
		lines = append(lines, body...)
	}

	var remaining []string
	for sec := range seen {
		inOrder := false
		for _, o := range order {
			if o == sec {
				inOrder = true
				break
			}
		}
		if !inOrder {
			remaining = append(remaining, sec)
		}
	}
	sort.Strings(remaining)

	for _, sec := range remaining {
		lines = append(lines, "")
		lines = append(lines, fmt.Sprintf("[%s]", sec))
		lines = append(lines, adSections[sec]...)
	}
	lines = append(lines, "")

	return strings.Join(lines, "\n")
}
