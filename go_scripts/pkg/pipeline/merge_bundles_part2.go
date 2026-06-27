package pipeline

import (
	"fmt"
	"path/filepath"
	"regexp"
	"sort"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

func finalizeDevtoolsBundle(outputPath string) {
	devtoolsMitmExclusions := []string{
		"-github.com",
		"-api.github.com",
		"-*.githubusercontent.com",
	}
	text := hub.ReadFileString(outputPath)
	meta, sections := hub.ParseModule(text)

	merged := make(map[string][]string)
	for _, sec := range sections {
		merged[sec.Name] = append(merged[sec.Name], sec.Lines...)
	}

	scriptLines := merged["Script"]
	scriptLabels := make(map[string]bool)

	reScript := regexp.MustCompile(`(?i)^(.+?)\s*=\s*type=`)
	for _, line := range scriptLines {
		matches := reScript.FindStringSubmatch(strings.TrimSpace(line))
		if len(matches) > 1 {
			scriptLabels[strings.TrimSpace(matches[1])] = true
		}
	}

	requiredSubStore := []string{"Sub-Store Core", "Sub-Store Simple", "{{{sync}}}", "{{{produce}}}"}
	var missing []string
	for _, req := range requiredSubStore {
		if !scriptLabels[req] {
			missing = append(missing, req)
		}
	}
	if len(missing) > 0 {
		hub.Warn(fmt.Sprintf("Sub-Store scripts incomplete in bundle (missing: %s)", strings.Join(missing, ", ")))
	}

	hosts := make(map[string]bool)
	for _, line := range merged["MITM"] {
		trimmed := strings.TrimSpace(line)
		if !strings.HasPrefix(strings.ToLower(trimmed), "hostname") {
			continue
		}

		re1 := regexp.MustCompile(`(?i)^hostname\s*=\s*`)
		part := re1.ReplaceAllString(trimmed, "")
		re2 := regexp.MustCompile(`(?i)^(%APPEND%|%INSERT%)\s*`)
		part = re2.ReplaceAllString(part, "")

		for _, token := range strings.Split(part, ",") {
			t := strings.TrimSpace(token)
			if t != "" && t != "%INSERT%" && t != "%APPEND%" {
				hosts[t] = true
			}
		}
	}

	for _, excl := range devtoolsMitmExclusions {
		hosts[excl] = true
	}

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

	finalHosts := append(inclusions, exclusions...)
	merged["MITM"] = []string{fmt.Sprintf("hostname = %%APPEND%% %s", strings.Join(finalHosts, ", "))}

	var sectionList []hub.ModuleSection
	for _, name := range hub.SectionOrder {
		if lines, ok := merged[name]; ok && len(lines) > 0 {
			sectionList = append(sectionList, hub.ModuleSection{Name: name, Lines: lines})
		}
	}

	for name, lines := range merged {
		found := false
		for _, ordered := range hub.SectionOrder {
			if ordered == name {
				found = true
				break
			}
		}
		if !found && len(lines) > 0 {
			sectionList = append(sectionList, hub.ModuleSection{Name: name, Lines: lines})
		}
	}

	headerLines := hub.FormatHeader(meta, nil)
	hub.SafeWriteFile(outputPath, hub.FormatModule(headerLines, sectionList, false, nil), true)
}

func MergeDevtools() error {
	hub.Section("Script Hub devtools upstream bundle merge")
	syncSubStoreScripts()
	syncScriptHubScripts()
	if hub.ValidateFileExists(scriptHubLocal, "") {
		pinScriptHubScriptPaths(scriptHubLocal)
	}

	err := hub.MergeUpstreamModules(
		devtoolsSources,
		devtoolsOutput,
		devtoolsHeader,
		"",
		nil,
		map[string]string{
			"BoxJs":     filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, "boxjs.rewrite.surge.sgmodule"),
			"Sub-Store": filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, "Surge-Beta.sgmodule"),
			"ScriptHub": scriptHubLocal,
		},
		[][2]string{
			{`cors:"https://sub-store.vercel.app"`, `cors:"https://sub.store"`},
			{`sync_success_notify:true`, `sync_success_notify:false`},
		},
	)
	if err != nil {
		return err
	}

	pinSubStoreScriptPaths(devtoolsOutput)
	pinScriptHubScriptPaths(devtoolsOutput)
	finalizeDevtoolsBundle(devtoolsOutput)
	return nil
}

func loadPreservedRuleSections(path string) map[string][]string {
	if !hub.ValidateFileExists(path, "") {
		return nil
	}
	_, sections := hub.ParseModule(hub.ReadFileString(path))
	preserved := make(map[string][]string)
	for _, sec := range sections {
		if sec.Name == "Rule" && len(sec.Lines) > 0 {
			preserved[sec.Name] = sec.Lines
		}
	}
	return preserved
}

func rewriteWeiboWithPreserved(path string, preserved map[string][]string) {
	if len(preserved) == 0 {
		return
	}
	meta, sections := hub.ParseModule(hub.ReadFileString(path))
	merged := make(map[string][]string)
	for _, sec := range sections {
		merged[sec.Name] = append(merged[sec.Name], sec.Lines...)
	}

	for name, lines := range preserved {
		merged[name] = lines
	}

	var ordered []hub.ModuleSection
	for _, name := range weiboSectionOrder {
		if lines, ok := merged[name]; ok && len(lines) > 0 {
			ordered = append(ordered, hub.ModuleSection{Name: name, Lines: lines})
		}
	}

	for name, lines := range merged {
		found := false
		for _, o := range weiboSectionOrder {
			if o == name {
				found = true
				break
			}
		}
		if !found && len(lines) > 0 {
			ordered = append(ordered, hub.ModuleSection{Name: name, Lines: lines})
		}
	}

	ordered = hub.MergeMitmHosts(ordered)
	headerLines := hub.FormatHeader(meta, []string{"# PROMAX build source — install head_expanse/PROMAX only"})
	hub.SafeWriteFile(path, hub.FormatModule(headerLines, ordered, true, nil), true)
}

func MergeWeibo() error {
	hub.Section("Weibo → LocalModules (PROMAX build source)")
	preserved := loadPreservedRuleSections(weiboLocal)

	err := hub.MergeUpstreamModules(
		weiboSources,
		weiboLocal,
		weiboHeader,
		"# Merged for rulesets/Sources/LocalModules → PROMAX ingest",
		nil,
		nil,
		nil,
	)
	if err != nil {
		return err
	}
	rewriteWeiboWithPreserved(weiboLocal, preserved)

	rel, _ := filepath.Rel(hub.ROOT, weiboLocal)
	hub.Info(fmt.Sprintf("Build source: %s", rel))
	return nil
}

func MergeWool() error {
	hub.Section("Wool Scripts → LocalModules (PROMAX build source)")
	preserved := loadPreservedRuleSections(woolLocal)

	err := hub.MergeUpstreamModules(
		woolSources,
		woolLocal,
		woolHeader,
		"# Merged for rulesets/Sources/LocalModules → PROMAX ingest",
		nil,
		nil,
		nil,
	)
	if err != nil {
		return err
	}
	rewriteWeiboWithPreserved(woolLocal, preserved)

	rel, _ := filepath.Rel(hub.ROOT, woolLocal)
	hub.Info(fmt.Sprintf("Build source: %s", rel))
	return nil
}
