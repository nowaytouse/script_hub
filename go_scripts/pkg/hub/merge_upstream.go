package hub

import (
	"fmt"
	"path/filepath"
	"regexp"
	"strings"
	"time"
)

type SourceSpec struct {
	Label string
	URL   string
}

func fetchModule(urlStr, label, localFallback, customUA string) (string, error) {
	if strings.HasPrefix(urlStr, "http://") || strings.HasPrefix(urlStr, "https://") {
		content := string(CurlFetch(urlStr, customUA))
		if content != "" && len(strings.TrimSpace(content)) >= 20 {
			time.Sleep(3 * time.Second) // strict delay to avoid shadowban/rate limits
			return content, nil
		}
	} else if ValidateFileExists(urlStr, "") {
		content := ReadFileString(urlStr)
		if len(strings.TrimSpace(content)) >= 20 {
			return content, nil
		}
	}

	if localFallback != "" && ValidateFileExists(localFallback, "") {
		Warn(fmt.Sprintf("Using local fallback for %s", label))
		return ReadFileString(localFallback), nil
	}

	return "", fmt.Errorf("failed to load upstream module: %s (%s)", label, urlStr)
}

func combineMetadata(parts []map[string]interface{}) (string, string, string) {
	var argTokens []string
	var argsDescBlocks []string
	var descBlocks []string

	for _, p := range parts {
		meta := p["meta"].(map[string]string)
		label := p["label"].(string)
		modName := meta["name"]
		if modName == "" {
			modName = label
		}

		rawArgs := strings.TrimSpace(meta["arguments"])
		if rawArgs != "" {
			for _, token := range strings.Split(rawArgs, ",") {
				token = strings.TrimSpace(token)
				if token != "" {
					found := false
					for _, t := range argTokens {
						if t == token {
							found = true
							break
						}
					}
					if !found {
						argTokens = append(argTokens, token)
					}
				}
			}
		}

		rawArgsDesc := strings.TrimSpace(meta["arguments-desc"])
		if rawArgsDesc != "" {
			cleanDesc := regexp.MustCompile(`\n+`).ReplaceAllString(rawArgsDesc, `\n`)
			argsDescBlocks = append(argsDescBlocks, fmt.Sprintf("[%s]\\n%s", modName, cleanDesc))
		}

		rawDesc := strings.TrimSpace(meta["desc"])
		if rawDesc != "" {
			cleanDesc := regexp.MustCompile(`\n+`).ReplaceAllString(rawDesc, `\n`)
			descBlocks = append(descBlocks, fmt.Sprintf("✦ %s: %s", modName, cleanDesc))
		}
	}

	argsLine := strings.Join(argTokens, ",")

	beautifyAndTruncate := func(blocks []string, maxLen int, prefix string) string {
		if len(blocks) == 0 {
			return ""
		}
		combined := prefix + strings.Join(blocks, "\\n\\n")
		if len(combined) > maxLen {
			return combined[:maxLen] + "…\\n(see upstream docs for full details)"
		}
		return combined
	}

	argsDescLine := beautifyAndTruncate(argsDescBlocks, 3800, "")
	descLine := beautifyAndTruncate(descBlocks, 3800, "【Bundled Modules】\\n")

	return argsLine, argsDescLine, descLine
}

func MergeUpstreamModules(sources []SourceSpec, outputPath string, headerMeta map[string]string, provenanceComment string, optionalLabels []string, localFallbacks map[string]string, contentReplacements [][2]string) error {
	optional := make(map[string]bool)
	for _, l := range optionalLabels {
		optional[l] = true
	}

	var parsedParts []map[string]interface{}
	mergedSections := make(map[string][]string)
	var usedSources []SourceSpec

	for _, src := range sources {
		Info(fmt.Sprintf("Downloading %s...", src.Label))

		customUA := ""
		if strings.Contains(src.URL, "yfamilys.com") {
			customUA = "Loon/3.9.9 CFNetwork/1496.0.7 Darwin/23.5.0"
		}

		fallback := ""
		if localFallbacks != nil {
			fallback = localFallbacks[src.Label]
		}

		text, err := fetchModule(src.URL, src.Label, fallback, customUA)
		if err != nil {
			if optional[src.Label] {
				Warn(fmt.Sprintf("Skipping optional source %s: %v", src.Label, err))
				continue
			}
			return err
		}

		if contentReplacements != nil {
			for _, repl := range contentReplacements {
				text = strings.ReplaceAll(text, repl[0], repl[1])
			}
		}

		meta, sections := ParseModule(text)
		parsedParts = append(parsedParts, map[string]interface{}{
			"label":    src.Label,
			"meta":     meta,
			"sections": sections,
		})
		usedSources = append(usedSources, src)

		for _, sec := range sections {
			name := sec.Name
			var filteredLines []string
			for _, line := range sec.Lines {
				if strings.TrimSpace(line) != "" {
					filteredLines = append(filteredLines, line)
				}
			}

			if len(mergedSections[name]) > 0 && len(filteredLines) > 0 {
				if mergedSections[name][len(mergedSections[name])-1] != "" {
					mergedSections[name] = append(mergedSections[name], "")
				}
			}
			mergedSections[name] = append(mergedSections[name], filteredLines...)
		}
	}

	if len(parsedParts) == 0 {
		return fmt.Errorf("no upstream modules downloaded for %s", outputPath)
	}

	combinedArgs, combinedArgsDesc, combinedDesc := combineMetadata(parsedParts)

	outMeta := make(map[string]string)
	for k, v := range headerMeta {
		outMeta[k] = v
	}

	outMeta["date"] = time.Now().Format("2006-01-02 15:04:05")
	if combinedArgs != "" {
		outMeta["arguments"] = combinedArgs
	}
	if combinedArgsDesc != "" {
		outMeta["arguments-desc"] = combinedArgsDesc
	}
	if combinedDesc != "" {
		existing := strings.TrimSpace(outMeta["desc"])
		if existing != "" {
			outMeta["desc"] = existing + "\\n\\n" + combinedDesc
		} else {
			outMeta["desc"] = combinedDesc
		}
	}

	var sectionList []ModuleSection
	for name, lines := range mergedSections {
		sectionList = append(sectionList, ModuleSection{Name: name, Lines: lines})
	}

	sectionList = MergeMitmHosts(sectionList)

	var extra []string
	if provenanceComment != "" {
		extra = append(extra, provenanceComment)
	}
	extra = append(extra, "# Upstream modules merged by go_scripts/pkg/pipeline/merge_bundles.go")
	for _, src := range usedSources {
		extra = append(extra, fmt.Sprintf("# - %s: %s", src.Label, src.URL))
	}

	headerLines := FormatHeader(outMeta, extra)
	EnsureDir(filepath.Dir(outputPath))

	content := FormatModule(headerLines, sectionList, true, nil)
	if err := SafeWriteFile(outputPath, content, true); err != nil {
		return err
	}

	if strings.Contains(content, "%INSERT%") {
		return fmt.Errorf("merged module contains invalid MITM token %%INSERT%%: %s", outputPath)
	}

	Success(fmt.Sprintf("Wrote bundle: %s", outputPath))
	return nil
}
