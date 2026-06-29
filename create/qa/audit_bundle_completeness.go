package qa

import (
	"fmt"
	"path/filepath"
	"regexp"
	"strings"
	"sync"

	"github.com/nyamiiko/script_hub/go_scripts/hub"
	"github.com/nyamiiko/script_hub/go_scripts/pipeline"
)

var scriptLabelRe = regexp.MustCompile(`(?i)^(.+?)\s*=\s*type=`)
var mitmBadTokens = map[string]bool{"%INSERT%": true, "%APPEND%": true}

type bundleSpec struct {
	label                        string
	output                       string
	sources                      []hub.SourceSpec
	requiredScriptLabels         []string
	requiredMitmHosts            []string
	requiredScriptPathSubstrings []string
}

func getScriptLabels(sections []hub.ModuleSection) map[string]bool {
	labels := make(map[string]bool)
	for _, section := range sections {
		name := section.Name
		if name != "Script" {
			continue
		}
		lines := section.Lines
		for _, line := range lines {
			m := scriptLabelRe.FindStringSubmatch(strings.TrimSpace(line))
			if len(m) > 1 {
				labels[strings.TrimSpace(m[1])] = true
			}
		}
	}
	return labels
}

func getMitmHosts(sections []hub.ModuleSection) map[string]bool {
	hosts := make(map[string]bool)
	for _, section := range sections {
		name := section.Name
		if name != "MITM" {
			continue
		}
		lines := section.Lines
		for _, line := range lines {
			line = strings.TrimSpace(line)
			if !strings.HasPrefix(strings.ToLower(line), "hostname") {
				continue
			}
			part := regexp.MustCompile(`(?i)^hostname\s*=\s*`).ReplaceAllString(line, "")
			part = regexp.MustCompile(`(?i)^(%APPEND%|%INSERT%)\s*`).ReplaceAllString(part, "")
			for _, token := range strings.Split(part, ",") {
				token = strings.TrimSpace(token)
				if token != "" && !mitmBadTokens[token] {
					hosts[token] = true
				}
			}
		}
	}
	return hosts
}

func auditBundle(spec bundleSpec) []string {
	var issues []string
	if !hub.ValidateFileExists(spec.output, "") {
		return []string{fmt.Sprintf("%s: bundle missing at %s", spec.label, spec.output)}
	}

	bundleText := hub.ReadFileString(spec.output)
	_, bundleSections := hub.ParseModule(bundleText)
	bundleScripts := getScriptLabels(bundleSections)
	bundleMitm := getMitmHosts(bundleSections)

	if strings.Contains(bundleText, "%INSERT%") {
		issues = append(issues, fmt.Sprintf("%s: contains invalid MITM token %%INSERT%%", spec.label))
	}

	for _, needle := range spec.requiredScriptPathSubstrings {
		if !strings.Contains(bundleText, needle) {
			issues = append(issues, fmt.Sprintf("%s: vendored script-path missing: %s", spec.label, needle))
		}
	}

	upstreamScripts := make(map[string]bool)
	upstreamMitm := make(map[string]bool)

	for _, src := range spec.sources {
		text := hub.SafeDownload(src.URL, 2, 60)
		if text == "" {
			issues = append(issues, fmt.Sprintf("%s: failed to fetch upstream [%s]", spec.label, src.Label))
			continue
		}
		_, sections := hub.ParseModule(text)
		for k := range getScriptLabels(sections) {
			upstreamScripts[k] = true
		}
		for k := range getMitmHosts(sections) {
			upstreamMitm[k] = true
		}
	}

	var missingScripts []string
	for k := range upstreamScripts {
		if !bundleScripts[k] {
			missingScripts = append(missingScripts, k)
		}
	}
	if len(missingScripts) > 0 {
		preview := strings.Join(missingScripts, ", ")
		if len(missingScripts) > 12 {
			preview = strings.Join(missingScripts[:12], ", ") + "…"
		}
		issues = append(issues, fmt.Sprintf("%s: missing script label(s): %s", spec.label, preview))
	}

	for _, req := range spec.requiredScriptLabels {
		if !bundleScripts[req] {
			issues = append(issues, fmt.Sprintf("%s: required script missing: %s", spec.label, req))
		}
	}

	missingMitmPositive := make(map[string]bool)
	for k := range upstreamMitm {
		if !bundleMitm[k] && !strings.HasPrefix(k, "-") {
			missingMitmPositive[k] = true
		}
	}
	for _, req := range spec.requiredMitmHosts {
		if !bundleMitm[req] {
			missingMitmPositive[req] = true
		}
	}

	if len(missingMitmPositive) > 0 {
		var missingArr []string
		for k := range missingMitmPositive {
			missingArr = append(missingArr, k)
		}
		issues = append(issues, fmt.Sprintf("%s: MITM missing host(s): %s", spec.label, strings.Join(missingArr, ", ")))
	}

	return issues
}

func getSpecs() []bundleSpec {
	return []bundleSpec{
		{
			label:             "BiliBili",
			output:            filepath.Join(hub.MODULES_DIR, "surge/amplify_nexus/📺 BiliBili增强合集.sgmodule"),
			sources:           pipeline.BilibiliSources,
			requiredMitmHosts: []string{"app.bilibili.com"},
		},
		{
			label:   "Apple",
			output:  filepath.Join(hub.MODULES_DIR, "surge/amplify_nexus/🍎 Apple服务增强合集.sgmodule"),
			sources: pipeline.AppleSources,
		},
		{
			label:             "Panel utilities",
			output:            filepath.Join(hub.MODULES_DIR, "surge/amplify_nexus/📊 面板工具合集.sgmodule"),
			sources:           pipeline.UtilitiesSources,
			requiredMitmHosts: []string{"net-lsp-x.com"},
		},
		{
			label:   "Script Hub devtools",
			output:  filepath.Join(hub.MODULES_DIR, "surge/amplify_nexus/🧰 Script Hub 配套工具合集.sgmodule"),
			sources: pipeline.DevtoolsSources,
			requiredScriptLabels: []string{
				"Sub-Store Core",
				"Sub-Store Simple",
				"{{{sync}}}",
				"{{{produce}}}",
				"Rewrite: BoxJs",
				"Script Hub: 前端",
			},
			requiredMitmHosts: []string{"sub.store", "script.hub", "boxjs.com"},
			requiredScriptPathSubstrings: []string{
				"modules/source/scripts/github_com_sub-store-org_sub-store-1.min.js",
				"modules/source/scripts/raw_githubusercontent_com_f59ef7_script-hub.js",
			},
		},
		{
			label:   "Weibo (PROMAX source)",
			output:  filepath.Join(hub.ROOT, "rulesets/Sources/LocalModules/Weibo.ADBlock.sgmodule"),
			sources: pipeline.WeiboSources,
		},
	}
}

func auditPromaxModules() []string {
	var issues []string
	rels := []string{
		"modules/surge/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
		"modules/surge/head_expanse/📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule",
	}
	for _, rel := range rels {
		path := filepath.Join(hub.ROOT, rel)
		if !hub.ValidateFileExists(path, "") {
			issues = append(issues, fmt.Sprintf("PROMAX: missing %s", rel))
			continue
		}
		text := hub.ReadFileString(path)
		if strings.Contains(text, "%INSERT%") {
			issues = append(issues, fmt.Sprintf("PROMAX: %%INSERT%% found in %s", filepath.Base(path)))
		}
	}
	return issues
}

func RunAuditBundleCompleteness() int {
	specs := getSpecs()
	hub.Section("Bundle completeness audit")
	
	var wg sync.WaitGroup
	var mu sync.Mutex
	var allIssues []string

	for _, spec := range specs {
		wg.Add(1)
		go func(s bundleSpec) {
			defer wg.Done()
			hub.Info(fmt.Sprintf("Checking %s…", s.label))
			issues := auditBundle(s)
			if len(issues) > 0 {
				mu.Lock()
				allIssues = append(allIssues, issues...)
				mu.Unlock()
			}
		}(spec)
	}

	wg.Add(1)
	go func() {
		defer wg.Done()
		issues := auditPromaxModules()
		if len(issues) > 0 {
			mu.Lock()
			allIssues = append(allIssues, issues...)
			mu.Unlock()
		}
	}()

	wg.Wait()

	if len(allIssues) > 0 {
		hub.Error(fmt.Sprintf("Found %d issue(s):", len(allIssues)))
		for _, item := range allIssues {
			fmt.Printf("  • %s\n", item)
		}
		return 1
	}

	hub.Success(fmt.Sprintf("All %d bundle(s) + PROMAX passed completeness audit.", len(specs)))
	return 0
}
