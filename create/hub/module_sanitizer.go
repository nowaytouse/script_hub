package hub

import (
	"fmt"
	"regexp"
	"sort"
	"strings"
)

var SectionOrder = []string{
	"General",
	"Host",
	"Rule",
	"URL Rewrite",
	"Map Local",
	"Script",
	"Body Rewrite",
	"Header Rewrite",
	"MITM",
}

var HeaderKeysOrder = []string{
	"name",
	"desc",
	"author",
	"icon",
	"category",
	"tag",
	"date",
	"arguments",
	"arguments-desc",
	"system-proxy",
	"ability",
	"update-interval",
}

var (
	metaLineRe      = regexp.MustCompile(`(?i)^#!\s*([A-Za-z0-9_-]+)\s*[=:]\s*(.*)$`)
	scriptNameRe    = regexp.MustCompile(`(?i)\bname\s*=\s*([^,\s]+)`)
	scriptLabelRe   = regexp.MustCompile(`(?i)^(.+?)\s*=\s*type=`)
	scriptPathRe    = regexp.MustCompile(`(?i)\bscript-path\s*=\s*([^,\s]+)`)
	scriptPatternRe = regexp.MustCompile(`(?i)\bpattern\s*=\s*([^,]+)`)
)

type ModuleSection struct {
	Name  string
	Lines []string
}

func ParseModule(text string) (map[string]string, []ModuleSection) {
	headerMeta := make(map[string]string)
	var sections []ModuleSection
	currentName := ""
	var currentLines []string
	inBody := false

	lines := strings.Split(text, "\n")
	for _, raw := range lines {
		stripped := strings.TrimSpace(raw)
		if strings.HasPrefix(stripped, "[") && strings.HasSuffix(stripped, "]") && !strings.HasPrefix(stripped, "#") {
			if currentName != "" {
				sections = append(sections, ModuleSection{Name: currentName, Lines: currentLines})
			}
			currentName = stripped[1 : len(stripped)-1]
			currentLines = []string{}
			inBody = true
			continue
		}

		if !inBody {
			match := metaLineRe.FindStringSubmatch(stripped)
			if match != nil {
				headerMeta[strings.ToLower(match[1])] = strings.TrimSpace(match[2])
			}
			continue
		}

		if currentName != "" {
			currentLines = append(currentLines, strings.TrimRight(raw, "\r"))
		}
	}

	if currentName != "" {
		sections = append(sections, ModuleSection{Name: currentName, Lines: currentLines})
	}
	return headerMeta, sections
}

func dedupeKey(section string, line string) string {
	stripped := strings.TrimSpace(line)
	if stripped == "" || strings.HasPrefix(stripped, "#") {
		return ""
	}

	if section == "Script" {
		labelMatch := scriptLabelRe.FindStringSubmatch(stripped)
		if labelMatch != nil {
			return fmt.Sprintf("script:label:%s", strings.TrimSpace(labelMatch[1]))
		}
		nameMatch := scriptNameRe.FindStringSubmatch(stripped)
		if nameMatch != nil {
			return fmt.Sprintf("script:name:%s", nameMatch[1])
		}
		pathMatch := scriptPathRe.FindStringSubmatch(stripped)
		patternMatch := scriptPatternRe.FindStringSubmatch(stripped)
		if pathMatch != nil && patternMatch != nil {
			return fmt.Sprintf("script:%s:%s", pathMatch[1], strings.TrimSpace(patternMatch[1]))
		}
		if pathMatch != nil {
			return fmt.Sprintf("script:%s", pathMatch[1])
		}
		return fmt.Sprintf("script:%s", stripped)
	}

	if section == "URL Rewrite" {
		parts := strings.SplitN(stripped, ",", 2)
		return fmt.Sprintf("rewrite:%s", parts[0])
	}
	if section == "Map Local" {
		return fmt.Sprintf("maplocal:%s", stripped)
	}
	if section == "Body Rewrite" {
		return fmt.Sprintf("body:%s", stripped)
	}
	if section == "Header Rewrite" {
		return fmt.Sprintf("header:%s", stripped)
	}
	if section == "Rule" {
		return fmt.Sprintf("rule:%s", stripped)
	}
	if section == "MITM" {
		re := regexp.MustCompile(`(?i)^hostname\s*=\s*(%APPEND%\s*)?`)
		hosts := re.ReplaceAllString(stripped, "")
		return fmt.Sprintf("mitm:%s", strings.TrimSpace(hosts))
	}
	return fmt.Sprintf("%s:%s", section, stripped)
}

type parsedLine struct {
	raw   string
	left  string
	right string
}

func DedupeSectionLines(section string, lines []string) []string {
	seen := make(map[string]bool)
	var result []string
	for _, line := range lines {
		stripped := strings.TrimSpace(line)
		if stripped == "" {
			if len(result) > 0 && result[len(result)-1] != "" {
				result = append(result, "")
			}
			continue
		}
		if strings.HasPrefix(stripped, "#") {
			result = append(result, stripped)
			continue
		}
		key := dedupeKey(section, line)
		if key == "" || seen[key] {
			continue
		}
		seen[key] = true
		result = append(result, stripped)
	}

	if section == "Script" || section == "Host" || section == "MITM" || section == "General" {
		var parsed []parsedLine
		maxLen := 0
		for _, r := range result {
			if strings.HasPrefix(r, "#") {
				parsed = append(parsed, parsedLine{raw: r})
				continue
			}
			parts := strings.SplitN(r, "=", 2)
			if len(parts) == 2 {
				left := strings.TrimSpace(parts[0])
				right := strings.TrimSpace(parts[1])
				if len(left) < 60 {
					if len(left) > maxLen {
						maxLen = len(left)
					}
				}
				parsed = append(parsed, parsedLine{raw: r, left: left, right: right})
			} else {
				parsed = append(parsed, parsedLine{raw: r})
			}
		}

		var alignedResult []string
		for _, p := range parsed {
			if p.left != "" && p.right != "" && len(p.left) <= 60 {
				alignedResult = append(alignedResult, fmt.Sprintf("%-*s = %s", maxLen, p.left, p.right))
			} else {
				alignedResult = append(alignedResult, p.raw)
			}
		}
		return alignedResult
	}

	return result
}

func FormatHeader(meta map[string]string, extraLines []string) []string {
	var lines []string
	used := make(map[string]bool)
	for _, key := range HeaderKeysOrder {
		if val, ok := meta[key]; ok {
			lines = append(lines, fmt.Sprintf("#!%s=%s", key, val))
			used[key] = true
		}
	}

	// Sorted remaining keys
	var remaining []string
	for key := range meta {
		if !used[key] {
			remaining = append(remaining, key)
		}
	}
	sort.Strings(remaining)
	for _, key := range remaining {
		lines = append(lines, fmt.Sprintf("#!%s=%s", key, meta[key]))
	}

	if len(extraLines) > 0 {
		lines = append(lines, extraLines...)
	}
	return lines
}

func FormatModule(headerLines []string, sections []ModuleSection, dedupe bool, sectionComments map[string]string) string {
	sectionMap := make(map[string][]string)
	for _, sec := range sections {
		sectionMap[sec.Name] = sec.Lines
	}

	var out []string
	out = append(out, headerLines...)
	for len(out) > 0 && strings.TrimSpace(out[len(out)-1]) == "" {
		out = out[:len(out)-1]
	}
	if len(out) > 0 {
		out = append(out, "")
	}

	for _, sectionName := range SectionOrder {
		lines, ok := sectionMap[sectionName]
		if !ok || len(lines) == 0 {
			continue
		}
		if dedupe {
			lines = DedupeSectionLines(sectionName, lines)
		}
		if len(lines) == 0 {
			continue
		}
		if comment, ok := sectionComments[sectionName]; ok {
			out = append(out, comment)
		}
		out = append(out, fmt.Sprintf("[%s]", sectionName))
		out = append(out, lines...)
		out = append(out, "")
	}

	res := strings.Join(out, "\n")
	res = strings.TrimRight(res, "\n") + "\n"
	return res
}

func SanitizeFileContent(text string, dedupe bool) string {
	meta, sections := ParseModule(text)
	var header []string
	if len(meta) > 0 {
		header = FormatHeader(meta, nil)
	} else {
		lines := strings.Split(text, "\n")
		for _, raw := range lines {
			if strings.HasPrefix(strings.TrimSpace(raw), "[") {
				break
			}
			if strings.HasPrefix(strings.TrimSpace(raw), "#!") || strings.TrimSpace(raw) == "" {
				header = append(header, strings.TrimRight(raw, "\r"))
			}
		}
	}
	return FormatModule(header, sections, dedupe, nil)
}

func MergeMitmHosts(sections []ModuleSection) []ModuleSection {
	hosts := make(map[string]bool)
	var other []ModuleSection
	skipTokens := map[string]bool{"%INSERT%": true, "%APPEND%": true}

	for _, sec := range sections {
		if sec.Name != "MITM" {
			other = append(other, sec)
			continue
		}
		for _, line := range sec.Lines {
			stripped := strings.TrimSpace(line)
			if strings.HasPrefix(strings.ToLower(stripped), "hostname") {
				re1 := regexp.MustCompile(`(?i)^hostname\s*=\s*`)
				part := re1.ReplaceAllString(stripped, "")
				re2 := regexp.MustCompile(`(?i)^(%APPEND%|%INSERT%)\s*`)
				part = re2.ReplaceAllString(part, "")
				for _, token := range strings.Split(part, ",") {
					token = strings.TrimSpace(token)
					if token != "" && !skipTokens[token] {
						hosts[token] = true
					}
				}
			}
		}
	}

	if len(hosts) > 0 {
		var inclusions, exclusions []string
		for h := range hosts {
			if strings.HasPrefix(h, "-") {
				exclusions = append(exclusions, h)
			} else {
				inclusions = append(inclusions, h)
			}
		}
		sort.Strings(inclusions)
		sort.Strings(exclusions)
		merged := append(inclusions, exclusions...)

		other = append(other, ModuleSection{
			Name:  "MITM",
			Lines: []string{fmt.Sprintf("hostname = %%APPEND%% %s", strings.Join(merged, ", "))},
		})
	}
	return other
}
