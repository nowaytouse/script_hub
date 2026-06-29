package pipeline

import (
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"
	"unsafe"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

/*
#include <stdlib.h>
char* normalize_rule_ffi(const char* line);
void free_string_ffi(char* s);
*/
import "C"

var SECTION_NAMES_MAP = map[string]bool{
	"URL Rewrite":    true,
	"Map Local":      true,
	"Script":         true,
	"Body Rewrite":   true,
	"Header Rewrite": true,
}

func expandComplexRule(line string) []string {
	var extracted []string
	if strings.HasPrefix(line, "AND,((") || strings.HasPrefix(line, "OR,((") {
		reSuffix := regexp.MustCompile(`DOMAIN-SUFFIX,([^,\)\]]+)`)
		reKeyword := regexp.MustCompile(`DOMAIN-KEYWORD,-?([^,\)\]]+)`)

		matchesSuffix := reSuffix.FindAllStringSubmatch(line, -1)
		for _, m := range matchesSuffix {
			suffix := strings.TrimSpace(m[1])
			if suffix != "" && strings.Contains(suffix, ".") {
				extracted = append(extracted, "DOMAIN-SUFFIX,"+suffix)
			}
		}

		matchesKeyword := reKeyword.FindAllStringSubmatch(line, -1)
		reTld := regexp.MustCompile(`(?i)\.(com|net|org|io|cn|jp|kr|tw|hk|sg|uk|de|fr|ru|br|in|au|co|me|tv|cc|xyz|top|app|dev)$`)
		for _, m := range matchesKeyword {
			suffix := strings.TrimSpace(m[1])
			if reTld.MatchString(suffix) {
				extracted = append(extracted, "DOMAIN-SUFFIX,"+suffix)
			}
		}
	}
	return extracted
}

func (m *AdBlockManager) ExtractFromText(text, defaultPolicy, category string, includeSections, rulesOnly bool) {
	lines := strings.Split(text, "\n")
	var currentSection string
	inRuleSection := false
	seenRealHeader := false

	for _, line := range lines {
		stripped := strings.TrimSpace(line)
		if stripped == "" {
			continue
		}

		if strings.HasPrefix(stripped, "[") && strings.HasSuffix(stripped, "]") && !strings.HasPrefix(stripped, "#") {
			currentSection = stripped[1 : len(stripped)-1]
			inRuleSection = MODULE_RULE_SECTIONS[currentSection]
			seenRealHeader = true
			continue
		}

		if !seenRealHeader {
			if rulesOnly {
				continue
			}
			if strings.HasPrefix(stripped, "AND,((") || strings.HasPrefix(stripped, "OR,((") {
				for _, r := range expandComplexRule(stripped) {
					m.AddRuleLine(r, defaultPolicy, category)
				}
			} else {
				m.AddRuleLine(stripped, defaultPolicy, category)
			}
			continue
		}

		if inRuleSection {
			if strings.HasPrefix(stripped, "AND,((") || strings.HasPrefix(stripped, "OR,((") {
				for _, r := range expandComplexRule(stripped) {
					m.AddRuleLine(r, defaultPolicy, category)
				}
			} else {
				m.AddRuleLine(stripped, defaultPolicy, category)
			}
		} else if includeSections && !strings.HasPrefix(stripped, "#") {
			if SECTION_NAMES_MAP[currentSection] {
				m.Sections[currentSection] = append(m.Sections[currentSection], stripped)
			} else if currentSection == "MITM" && strings.HasPrefix(stripped, "hostname") {
				hostsStr := stripped
				if idx := strings.Index(stripped, "="); idx != -1 {
					hostsStr = stripped[idx+1:]
				}
				hostsStr = strings.ReplaceAll(hostsStr, "%APPEND%", "")
				hostsStr = strings.ReplaceAll(hostsStr, "%INSERT%", "")
				for _, h := range strings.Split(hostsStr, ",") {
					h = strings.TrimSpace(h)
					if h != "" {
						m.MitmHosts[h] = true
					}
				}
			}
		}
	}
}

func (m *AdBlockManager) AddRuleLine(line, defaultPolicy, category string) {
	line = strings.TrimSpace(line)
	if line == "" {
		return
	}
	
	// Ensure line has allowed prefix (quick filter)
	if !strings.HasPrefix(line, "||") && !strings.HasPrefix(line, ".") {
		hasPrefix := false
		for _, p := range ALLOWED_RULE_PREFIXES {
			if strings.HasPrefix(line, p) {
				hasPrefix = true
				break
			}
		}
		if !hasPrefix {
			return
		}
	}
	
	cStr := C.CString(line)
	defer C.free(unsafe.Pointer(cStr))
	
	resPtr := C.normalize_rule_ffi(cStr)
	if resPtr == nil {
		return
	}
	defer C.free_string_ffi(resPtr)
	
	normalized := C.GoString(resPtr)
	if normalized == "" {
		return
	}

	parts := strings.Split(normalized, ",")
	if len(parts) >= 2 {
		rulePayload := strings.TrimSpace(parts[1])
		if m.IsWhitelisted(rulePayload) {
			return
		}

		// Malicious drops
		if strings.HasPrefix(normalized, "DOMAIN-KEYWORD,") {
			kw := strings.ToLower(rulePayload)
			if kw == "google" || kw == "apple" || kw == "microsoft" || kw == "github" || kw == "git" {
				return
			}
		}
		if strings.HasPrefix(normalized, "DOMAIN-SUFFIX,") || strings.HasPrefix(normalized, "DOMAIN,") {
			kw := strings.ToLower(rulePayload)
			if kw == "google.com" || kw == "apple.com" || kw == "github.com" {
				return
			}
		}

		policy := defaultPolicy
		// Rebuild bare rule since normalized strips policies
		bare := fmt.Sprintf("%s,%s", strings.TrimSpace(parts[0]), rulePayload)
		if m.Rules[policy] != nil && m.Rules[policy][category] != nil {
			if !m.SeenRules[policy][bare] {
				m.Rules[policy][category][bare] = true
				m.SeenRules[policy][bare] = true
			}
		}
	}
}

func (m *AdBlockManager) filterRules(rules map[string]bool) []string {
	var filtered []string
	for r := range rules {
		rulePayload := r
		if idx := strings.Index(r, ","); idx != -1 {
			rulePayload = r[idx+1:]
		}
		if m.IsWhitelisted(rulePayload) {
			continue
		}
		filtered = append(filtered, r)
	}
	sort.Strings(filtered)
	return filtered
}

func (m *AdBlockManager) generateRulesets() []string {
	var generatedFiles []string
	hub.EnsureDir(hub.ADBLOCK_DIR)

	hub.Info(fmt.Sprintf("Generating split rulesets in %s (≤%d rules/shard)", hub.ADBLOCK_DIR, RULES_PER_SHARD))

	for _, category := range categoryNames {
		rules := m.filterRules(m.Rules["REJECT"][category])
		if len(rules) == 0 {
			hub.Info(fmt.Sprintf("  ⊘ Skip shard for %s: no unique rules", category))
			continue
		}

		meta := CATEGORY_META[category]
		var chunks [][]string
		for i := 0; i < len(rules); i += RULES_PER_SHARD {
			end := i + RULES_PER_SHARD
			if end > len(rules) {
				end = len(rules)
			}
			chunks = append(chunks, rules[i:end])
		}

		for idx, chunk := range chunks {
			var filename string
			var shardLabel string
			if len(chunks) == 1 {
				filename = fmt.Sprintf("AdBlock_%s.list", category)
				shardLabel = "1/1"
			} else {
				filename = fmt.Sprintf("AdBlock_%s_%02d.list", category, idx+1)
				shardLabel = fmt.Sprintf("%d/%d", idx+1, len(chunks))
			}

			filePath := filepath.Join(hub.ADBLOCK_DIR, filename)
			var content strings.Builder
			content.WriteString(fmt.Sprintf("# Ruleset: AdBlock · %s\n", meta["label_zh"]))
			content.WriteString(fmt.Sprintf("# Purpose: %s\n", meta["desc"]))
			content.WriteString(fmt.Sprintf("# Category-Key: %s\n", category))
			content.WriteString(fmt.Sprintf("# Shard: %s\n", shardLabel))
			content.WriteString(fmt.Sprintf("# Total Rules: %d\n", len(chunk)))

			for _, rule := range chunk {
				content.WriteString(rule)
				content.WriteString("\n")
			}

			hub.SafeWriteFile(filePath, content.String(), true)
			relPath, _ := filepath.Rel(hub.ROOT, filePath)
			generatedFiles = append(generatedFiles, relPath)
			hub.Success(fmt.Sprintf("  ✓ %s (%d rules, %s)", filename, len(chunk), meta["label_zh"]))
		}
	}

	// Legacy AdBlock.list
	var legacyContent strings.Builder
	legacyContent.WriteString("# Ruleset: AdBlock (LEGACY / REDIRECT)\n")
	legacyContent.WriteString("# This file is now split into multiple files in rulesets/AdBlock/\n")
	legacyContent.WriteString("# Please update your module to use the new split rulesets.\n")

	var allRejectRules []string
	for _, cat := range categoryNames {
		allRejectRules = append(allRejectRules, m.filterRules(m.Rules["REJECT"][cat])...)
	}

	limit := 1000
	if len(allRejectRules) < limit {
		limit = len(allRejectRules)
	}
	for _, rule := range allRejectRules[:limit] {
		legacyContent.WriteString(rule)
		legacyContent.WriteString("\n")
	}
	hub.SafeWriteFile(hub.ADBLOCK_LIST, legacyContent.String(), true)

	m.pruneStaleRulesets(generatedFiles)
	return generatedFiles
}

func (m *AdBlockManager) pruneStaleRulesets(generatedFiles []string) {
	keep := make(map[string]bool)
	for _, f := range generatedFiles {
		keep[filepath.Base(f)] = true
	}
	keep["HTTPDNS_Hijack.list"] = true
	keep["reject-drop.list"] = true
	keep["reject-no-drop.list"] = true
	keep["AdBlock.list"] = true

	files, err := os.ReadDir(hub.ADBLOCK_DIR)
	if err != nil {
		return
	}
	var removed []string
	for _, f := range files {
		if f.IsDir() || !strings.HasSuffix(f.Name(), ".list") || keep[f.Name()] {
			continue
		}
		filePath := filepath.Join(hub.ADBLOCK_DIR, f.Name())
		if err := os.Remove(filePath); err == nil {
			removed = append(removed, f.Name())
			srsPath := filepath.Join(hub.SINGBOX_DIR, strings.Replace(f.Name(), ".list", "_Singbox.srs", 1))
			os.Remove(srsPath)
		}
	}
	if len(removed) > 0 {
		hub.Info(fmt.Sprintf("Pruned %d stale AdBlock ruleset(s): %s", len(removed), strings.Join(removed, ", ")))
	}
}

func (m *AdBlockManager) integrateFunctionalSections() map[string]int {
	hub.Section("Integrating Functional Sections (Script / Rewrite / MITM)")

	countsBefore := make(map[string]int)
	for _, name := range SECTION_NAMES {
		countsBefore[name] = len(m.Sections[name])
	}
	mitmBefore := len(m.MitmHosts)

	localPaths, remoteUrls := m.LoadFunctionalSourcePaths()
	for _, u := range remoteUrls {
		content := m.download(u)
		if content == "" {
			hub.Warn(fmt.Sprintf("  ⚠ Functional remote fetch failed: %s", u))
			continue
		}
		parts := strings.Split(u, "/")
		hub.Info(fmt.Sprintf("  + Functional remote: %s", parts[len(parts)-1]))
		m.ExtractFromText(content, "REJECT", "Local", true, false)
	}

	for _, lp := range localPaths {
		path := lp[0]
		ingestMode := lp[1]
		tag := "full"
		if ingestMode == "split" {
			tag = "split"
		}
		rel, _ := filepath.Rel(hub.ROOT, path)
		hub.Info(fmt.Sprintf("  + Functional (%s): %s", tag, rel))

		content := hub.ReadFileString(path)
		m.ExtractFromText(content, "REJECT", "Local", true, false)
	}

	added := make(map[string]int)
	var summaryParts []string
	for _, name := range SECTION_NAMES {
		diff := len(m.Sections[name]) - countsBefore[name]
		if diff > 0 {
			added[name] = diff
			summaryParts = append(summaryParts, fmt.Sprintf("%s=%d", name, diff))
		}
	}
	diffMitm := len(m.MitmHosts) - mitmBefore
	if diffMitm > 0 {
		added["MITM"] = diffMitm
		summaryParts = append(summaryParts, fmt.Sprintf("MITM=%d", diffMitm))
	}

	summary := strings.Join(summaryParts, ", ")
	if summary == "" {
		summary = "no new lines"
	}
	hub.Success(fmt.Sprintf("Functional sections merged: %s", summary))
	return added
}

func (m *AdBlockManager) writeCatalog(generatedRulesets []string) {
	now := time.Now().Format("2006-01-02 15:04:05")

	surgePromaxCdn := "modules/surge/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule"
	surgePromaxGithub := "modules/surge/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule"
	surgeLiteCdn := "modules/surge/head_expanse/📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule"
	surgeLiteGithub := "modules/surge/head_expanse/github/📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule"

	srPromaxCdn := "modules/shadowrocket/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module"
	srPromaxGithub := "modules/shadowrocket/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module"
	srLiteCdn := "modules/shadowrocket/head_expanse/📱 Universal Ad-Blocking Rules (PROMAX Lite).module"
	srLiteGithub := "modules/shadowrocket/head_expanse/github/📱 Universal Ad-Blocking Rules (PROMAX Lite).module"

	type CatalogItem struct {
		Path       string `json:"path"`
		InstallURL string `json:"install_url"`
	}

	type ShardItem struct {
		File      string `json:"file"`
		Category  string `json:"category"`
		RuleCount int    `json:"rule_count"`
	}

	catalog := map[string]interface{}{
		"generated": now,
		"modules": map[string]CatalogItem{
			"surge_promax_cdn":    {Path: surgePromaxCdn, InstallURL: CDN_BASE_URL + url.QueryEscape(surgePromaxCdn)},
			"surge_promax_github": {Path: surgePromaxGithub, InstallURL: GITHUB_RAW_URL + url.QueryEscape(surgePromaxGithub)},
			"surge_lite_cdn":      {Path: surgeLiteCdn, InstallURL: CDN_BASE_URL + url.QueryEscape(surgeLiteCdn)},
			"surge_lite_github":   {Path: surgeLiteGithub, InstallURL: GITHUB_RAW_URL + url.QueryEscape(surgeLiteGithub)},
			"sr_promax_cdn":       {Path: srPromaxCdn, InstallURL: CDN_BASE_URL + url.QueryEscape(srPromaxCdn)},
			"sr_promax_github":    {Path: srPromaxGithub, InstallURL: GITHUB_RAW_URL + url.QueryEscape(srPromaxGithub)},
			"sr_lite_cdn":         {Path: srLiteCdn, InstallURL: CDN_BASE_URL + url.QueryEscape(srLiteCdn)},
			"sr_lite_github":      {Path: srLiteGithub, InstallURL: GITHUB_RAW_URL + url.QueryEscape(srLiteGithub)},
		},
	}

	var shards []ShardItem
	for _, rsPath := range generatedRulesets {
		filename := filepath.Base(rsPath)
		cat := m.CategoryFromFilename(filename)

		ruleCount := 0
		fullPath := filepath.Join(hub.ROOT, rsPath)
		if hub.ValidateFileExists(fullPath, "") {
			for l := range hub.IterLines(fullPath) {
				l = strings.TrimSpace(l)
				if l != "" && !strings.HasPrefix(l, "#") {
					ruleCount++
				}
			}
		}
		shards = append(shards, ShardItem{
			File:      rsPath,
			Category:  cat,
			RuleCount: ruleCount,
		})
	}
	catalog["shards"] = shards

	data, err := json.MarshalIndent(catalog, "", "  ")
	if err == nil {
		hub.SafeWriteFile(hub.ADBLOCK_CATALOG_JSON, string(data)+"\n", true)
	}
}

func (m *AdBlockManager) generateModule(generatedRulesets []string, liteOnly bool, urlSource string) {
	currentDate := time.Now().Format("2006-01-02")
	complexPrefixes := []string{"URL-REGEX,", "USER-AGENT,", "PROCESS-NAME,", "DEST-PORT,"}

	var baseUrl string
	var outDir string
	if urlSource == "cdn" {
		baseUrl = CDN_BASE_URL
		outDir = hub.SURGE_HEAD_EXPANSE_DIR
	} else {
		baseUrl = GITHUB_RAW_URL
		outDir = hub.SURGE_HEAD_EXPANSE_GITHUB_DIR
	}
	hub.EnsureDir(outDir)

	var baseFilename string
	if liteOnly {
		baseFilename = "📱 Universal Ad-Blocking Rules (PROMAX Lite)"
	} else {
		baseFilename = "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style)"
	}
	targetPath := filepath.Join(outDir, fmt.Sprintf("%s.sgmodule", baseFilename))

	shardCount := len(generatedRulesets)

	var ruleLines []string
	ruleLines = append(ruleLines, "# --- SYNCED PORT RULES START ---")
	ruleLines = append(ruleLines, "# Automated update from ports source")

	portsSource := filepath.Join(hub.ROOT, "rulesets/Sources/DirectPorts.list")
	if hub.ValidateFileExists(portsSource, "") {
		for line := range hub.IterLines(portsSource) {
			stripped := strings.TrimSpace(line)
			if stripped != "" && !strings.HasPrefix(stripped, "#") {
				ruleLines = append(ruleLines, stripped)
			}
		}
	}

	ruleLines = append(ruleLines, "# --- SYNCED PORT RULES END ---")
	ruleLines = append(ruleLines, "")
	ruleLines = append(ruleLines, "# High-Priority White-list (Prevent DNS failure)")
	ruleLines = append(ruleLines, "DOMAIN,dns.alidns.com,DIRECT")
	ruleLines = append(ruleLines, "DOMAIN,doh.pub,DIRECT")
	ruleLines = append(ruleLines, "DOMAIN,dns.pub,DIRECT")
	ruleLines = append(ruleLines, "DOMAIN,dot.pub,DIRECT")
	ruleLines = append(ruleLines, "DOMAIN,doh.360.cn,DIRECT")
	ruleLines = append(ruleLines, "DOMAIN,doh.dns.apple.com,DIRECT")
	ruleLines = append(ruleLines, "# Block app-layer HTTPDNS first")
	ruleLines = append(ruleLines, fmt.Sprintf("RULE-SET,%srulesets/AdBlock/HTTPDNS_Hijack.list,REJECT", baseUrl))
	ruleLines = append(ruleLines, "# Split REJECT Rulesets (purpose-grouped; see rulesets/AdBlock/README.md)")

	shardsByCategory := make(map[string][]string)
	for _, rsPath := range generatedRulesets {
		cat := m.CategoryFromFilename(filepath.Base(rsPath))
		shardsByCategory[cat] = append(shardsByCategory[cat], rsPath)
	}

	for _, category := range categoryNames {
		paths := shardsByCategory[category]
		if len(paths) == 0 {
			continue
		}
		meta := CATEGORY_META[category]
		ruleLines = append(ruleLines, fmt.Sprintf("# ── %s · %s ──", meta["label_zh"], meta["desc"]))
		sort.Strings(paths) // Sorting them
		for _, rsPath := range paths {
			ruleLines = append(ruleLines, fmt.Sprintf("RULE-SET,%s%s,REJECT,extended-matching,pre-matching,update-interval=86400,no-resolve", baseUrl, rsPath))
		}
	}

	ruleLines = append(ruleLines, "")
	ruleLines = append(ruleLines, "# SKK Upstream Rulesets")
	ruleLines = append(ruleLines, fmt.Sprintf("RULE-SET,%srulesets/Sources/skk_upstream/reject-no-drop.list,REJECT-NO-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve", baseUrl))
	ruleLines = append(ruleLines, fmt.Sprintf("RULE-SET,%srulesets/Sources/skk_upstream/reject-drop.list,REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve", baseUrl))
	ruleLines = append(ruleLines, fmt.Sprintf("RULE-SET,%srulesets/Sources/skk_upstream/BlockHttpDNS.list,REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve", baseUrl))

	for _, policy := range []string{"REJECT", "REJECT-DROP", "REJECT-NO-DROP"} {
		pool := make(map[string]bool)
		for _, cat := range categoryNames {
			for r := range m.Rules[policy][cat] {
				pool[r] = true
			}
		}
		if len(pool) == 0 {
			continue
		}

		var candidates []string
		for r := range pool {
			if policy == "REJECT" {
				hasComplexPrefix := false
				for _, p := range complexPrefixes {
					if strings.HasPrefix(r, p) {
						hasComplexPrefix = true
						break
					}
				}
				if hasComplexPrefix {
					candidates = append(candidates, r)
				}
			} else {
				candidates = append(candidates, r)
			}
		}

		if len(candidates) == 0 {
			continue
		}

		ruleLines = append(ruleLines, fmt.Sprintf("# Merged %s complex rules (%d)", policy, len(candidates)))
		sort.Strings(candidates)
		for _, raw := range candidates {
			line := attachPolicy(raw, policy)
			ruleLines = append(ruleLines, line)
		}
	}

	var functionalSections []hub.ModuleSection
	var funcCounts []string

	for _, secName := range SECTION_NAMES {
		lines := m.Sections[secName]
		if len(lines) == 0 {
			continue
		}
		deduped := hub.DedupeSectionLines(secName, lines)
		if len(deduped) > 0 {
			functionalSections = append(functionalSections, hub.ModuleSection{Name: secName, Lines: deduped})
			funcCounts = append(funcCounts, fmt.Sprintf("%s=%d", secName, len(deduped)))
		}
	}

	if len(m.MitmHosts) > 0 {
		var hosts []string
		for h := range m.MitmHosts {
			hosts = append(hosts, h)
		}
		sort.Strings(hosts)
		mitmLine := fmt.Sprintf("hostname = %%APPEND%% %s", strings.Join(hosts, ", "))
		functionalSections = append(functionalSections, hub.ModuleSection{Name: "MITM", Lines: []string{mitmLine}})
		funcCounts = append(funcCounts, fmt.Sprintf("MITM=%d", len(m.MitmHosts)))
	}

	var allSections []hub.ModuleSection
	var name string
	var desc string
	var tag string

	if liteOnly {
		allSections = []hub.ModuleSection{{Name: "Rule", Lines: ruleLines}}
		name = fmt.Sprintf("📱 Universal Ad-Blocking Rules (PROMAX Lite) - [%s]", currentDate)
		desc = fmt.Sprintf("轻量基础版(%d分片); 保留完整规则集与防火墙，纯净无脚本无MITM，性能拉满", shardCount)
		tag = "AdBlock, Lite, Basic, Mobile"
	} else {
		allSections = append([]hub.ModuleSection{{Name: "Rule", Lines: ruleLines}}, functionalSections...)
		allSections = hub.MergeMitmHosts(allSections)
		funcSummary := "无脚本层"
		if len(funcCounts) > 0 {
			funcSummary = strings.Join(funcCounts, ", ")
		}
		name = fmt.Sprintf("🚫 Universal Ad-Blocking Rules (PROMAX) - [%s]", currentDate)
		desc = fmt.Sprintf("按用途分片(%d片) + 应用内去广告(%s); 索引 rulesets/AdBlock/catalog.json", shardCount, funcSummary)
		tag = "AdBlock, Dependency, HTTPDNS, Script"
	}

	headerMeta := map[string]string{
		"name":     name,
		"desc":     desc,
		"author":   "ScriptHub-Automated",
		"icon":     "https://cdn.jsdelivr.net/gh/luestr/IconResource@main/Other_icon/120px/KeLee.png",
		"category": GROUP_HEAD_EXPANSE,
		"tag":      tag,
		"date":     time.Now().Format("2006-01-02 15:04:05"),
	}

	headerLines := hub.FormatHeader(headerMeta, nil)
	moduleContent := hub.FormatModule(headerLines, allSections, false, nil)

	if urlSource == "github" {
		cdnOwn := "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/"
		ghOwn := "https://raw.githubusercontent.com/nowaytouse/script_hub/master/"

		linesOut := strings.Split(moduleContent, "\n")
		inUrlRewrite := false
		for i, line := range linesOut {
			stripped := strings.TrimSpace(line)
			if strings.HasPrefix(stripped, "[") {
				inUrlRewrite = strings.HasPrefix(strings.ToLower(stripped), "[url rewrite]")
			}
			if !inUrlRewrite && strings.Contains(line, cdnOwn) {
				linesOut[i] = strings.ReplaceAll(line, cdnOwn, ghOwn)
			}
		}
		moduleContent = strings.Join(linesOut, "\n")
	}

	hub.SafeWriteFile(targetPath, moduleContent, true)
	hub.Success(fmt.Sprintf("Module generated: %s", filepath.Base(targetPath)))
}

func (m *AdBlockManager) generateLoonPlugin(generatedRulesets []string, liteOnly bool, urlSource string) {
	currentDate := time.Now().Format("2006-01-02")
	complexPrefixes := []string{"URL-REGEX,", "USER-AGENT,", "PROCESS-NAME,", "DEST-PORT,"}

	var baseUrl string
	var outDir string
	if urlSource == "cdn" {
		baseUrl = CDN_BASE_URL
		outDir = filepath.Join(hub.ROOT, "modules/loon")
	} else {
		baseUrl = GITHUB_RAW_URL
		outDir = filepath.Join(hub.ROOT, "modules/loon/github")
	}
	hub.EnsureDir(outDir)

	var baseFilename string
	if liteOnly {
		baseFilename = "📱 Universal Ad-Blocking Rules (PROMAX Lite)"
	} else {
		baseFilename = "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style)"
	}
	targetPath := filepath.Join(outDir, fmt.Sprintf("%s.plugin", baseFilename))

	shardCount := len(generatedRulesets)

	var ruleLines []string
	ruleLines = append(ruleLines, "# --- SYNCED PORT RULES START ---")
	ruleLines = append(ruleLines, "# Automated update from ports source")

	portsSource := filepath.Join(hub.ROOT, "rulesets/Sources/DirectPorts.list")
	if hub.ValidateFileExists(portsSource, "") {
		for line := range hub.IterLines(portsSource) {
			stripped := strings.TrimSpace(line)
			if stripped != "" && !strings.HasPrefix(stripped, "#") {
				ruleLines = append(ruleLines, stripped)
			}
		}
	}

	ruleLines = append(ruleLines, "# --- SYNCED PORT RULES END ---")
	ruleLines = append(ruleLines, "")
	ruleLines = append(ruleLines, "# High-Priority White-list (Prevent DNS failure)")
	ruleLines = append(ruleLines, "DOMAIN,dns.alidns.com,DIRECT")
	ruleLines = append(ruleLines, "DOMAIN,doh.pub,DIRECT")
	ruleLines = append(ruleLines, "DOMAIN,dns.pub,DIRECT")
	ruleLines = append(ruleLines, "DOMAIN,dot.pub,DIRECT")
	ruleLines = append(ruleLines, "DOMAIN,doh.360.cn,DIRECT")
	ruleLines = append(ruleLines, "DOMAIN,doh.dns.apple.com,DIRECT")
	ruleLines = append(ruleLines, "# Block app-layer HTTPDNS first")
	ruleLines = append(ruleLines, fmt.Sprintf("RULE-SET,%srulesets/AdBlock/HTTPDNS_Hijack.list,REJECT", baseUrl))
	ruleLines = append(ruleLines, "# Split REJECT Rulesets (purpose-grouped; see rulesets/AdBlock/README.md)")

	shardsByCategory := make(map[string][]string)
	for _, rsPath := range generatedRulesets {
		cat := m.CategoryFromFilename(filepath.Base(rsPath))
		shardsByCategory[cat] = append(shardsByCategory[cat], rsPath)
	}

	for _, category := range categoryNames {
		paths := shardsByCategory[category]
		if len(paths) == 0 {
			continue
		}
		meta := CATEGORY_META[category]
		ruleLines = append(ruleLines, fmt.Sprintf("# ── %s · %s ──", meta["label_zh"], meta["desc"]))
		sort.Strings(paths)
		for _, rsPath := range paths {
			ruleLines = append(ruleLines, fmt.Sprintf("RULE-SET,%s%s,REJECT,no-resolve", baseUrl, rsPath))
		}
	}

	ruleLines = append(ruleLines, "")
	ruleLines = append(ruleLines, "# SKK Upstream Rulesets")
	ruleLines = append(ruleLines, fmt.Sprintf("RULE-SET,%srulesets/Sources/skk_upstream/reject-no-drop.list,REJECT-NO-DROP,no-resolve", baseUrl))
	ruleLines = append(ruleLines, fmt.Sprintf("RULE-SET,%srulesets/Sources/skk_upstream/reject-drop.list,REJECT-DROP,no-resolve", baseUrl))
	ruleLines = append(ruleLines, fmt.Sprintf("RULE-SET,%srulesets/Sources/skk_upstream/BlockHttpDNS.list,REJECT-DROP,no-resolve", baseUrl))

	for _, policy := range []string{"REJECT", "REJECT-DROP", "REJECT-NO-DROP"} {
		pool := make(map[string]bool)
		for _, cat := range categoryNames {
			for r := range m.Rules[policy][cat] {
				pool[r] = true
			}
		}
		if len(pool) == 0 {
			continue
		}

		var candidates []string
		for r := range pool {
			if policy == "REJECT" {
				hasComplexPrefix := false
				for _, p := range complexPrefixes {
					if strings.HasPrefix(r, p) {
						hasComplexPrefix = true
						break
					}
				}
				if hasComplexPrefix {
					candidates = append(candidates, r)
				}
			} else {
				candidates = append(candidates, r)
			}
		}

		if len(candidates) == 0 {
			continue
		}

		ruleLines = append(ruleLines, fmt.Sprintf("# Merged %s complex rules (%d)", policy, len(candidates)))
		sort.Strings(candidates)
		for _, raw := range candidates {
			line := attachPolicy(raw, policy)
			ruleLines = append(ruleLines, line)
		}
	}

	var rewriteLines []string
	var funcCounts []string

	for _, secName := range []string{"URL Rewrite", "Map Local", "Body Rewrite", "Header Rewrite"} {
		lines := m.Sections[secName]
		if len(lines) == 0 {
			continue
		}
		deduped := hub.DedupeSectionLines(secName, lines)
		if len(deduped) > 0 {
			for _, line := range deduped {
				rewriteLines = append(rewriteLines, hub.AdaptRewriteLineForLoon(line))
			}
			funcCounts = append(funcCounts, fmt.Sprintf("%s=%d", secName, len(deduped)))
		}
	}

	var scriptLines []string
	lines := m.Sections["Script"]
	if len(lines) > 0 {
		deduped := hub.DedupeSectionLines("Script", lines)
		if len(deduped) > 0 {
			for _, line := range deduped {
				trimmed := strings.TrimSpace(line)
				if trimmed == "" || strings.HasPrefix(trimmed, "#") {
					scriptLines = append(scriptLines, line)
					continue
				}
				idx := strings.Index(trimmed, "=")
				if idx != -1 {
					tag := strings.TrimSpace(trimmed[:idx])
					content := strings.TrimSpace(trimmed[idx+1:])
					converted := hub.AdaptScriptLineForLoon(tag, content)
					if converted != "" {
						scriptLines = append(scriptLines, converted)
					} else {
						scriptLines = append(scriptLines, line)
					}
				} else {
					scriptLines = append(scriptLines, line)
				}
			}
			funcCounts = append(funcCounts, fmt.Sprintf("Script=%d", len(deduped)))
		}
	}

	var mitmLines []string
	if len(m.MitmHosts) > 0 {
		var hosts []string
		for h := range m.MitmHosts {
			hosts = append(hosts, h)
		}
		sort.Strings(hosts)
		mitmLines = append(mitmLines, fmt.Sprintf("hostname = %s", strings.Join(hosts, ", ")))
		funcCounts = append(funcCounts, fmt.Sprintf("MITM=%d", len(m.MitmHosts)))
	}

	var name string
	var desc string

	if liteOnly {
		name = fmt.Sprintf("📱 Universal Ad-Blocking Rules (PROMAX Lite) - [%s]", currentDate)
		desc = fmt.Sprintf("轻量基础版(%d分片); 保留完整规则集与防火墙，纯净无脚本无MITM，性能拉满", shardCount)
	} else {
		funcSummary := "无脚本层"
		if len(funcCounts) > 0 {
			funcSummary = strings.Join(funcCounts, ", ")
		}
		name = fmt.Sprintf("🚫 Universal Ad-Blocking Rules (PROMAX) - [%s]", currentDate)
		desc = fmt.Sprintf("按用途分片(%d片) + 应用内去广告(%s); 索引 rulesets/AdBlock/catalog.json", shardCount, funcSummary)
	}

	var output []string
	output = append(output, fmt.Sprintf("#!name=%s", name))
	output = append(output, fmt.Sprintf("#!desc=%s", desc))
	output = append(output, "#!author=ScriptHub-Automated")
	output = append(output, "#!icon=https://cdn.jsdelivr.net/gh/luestr/IconResource@main/Other_icon/120px/KeLee.png")
	output = append(output, fmt.Sprintf("#!date=%s", time.Now().Format("2006-01-02 15:04:05")))
	output = append(output, "")

	if len(ruleLines) > 0 {
		output = append(output, "[Rule]")
		output = append(output, ruleLines...)
		output = append(output, "")
	}

	if !liteOnly {
		if len(rewriteLines) > 0 {
			output = append(output, "[Rewrite]")
			output = append(output, rewriteLines...)
			output = append(output, "")
		}

		if len(scriptLines) > 0 {
			output = append(output, "[Script]")
			output = append(output, scriptLines...)
			output = append(output, "")
		}

		if len(mitmLines) > 0 {
			output = append(output, "[MitM]")
			output = append(output, mitmLines...)
			output = append(output, "")
		}
	}

	pluginContent := strings.Join(output, "\n")

	if urlSource == "github" {
		cdnOwn := "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/"
		ghOwn := "https://raw.githubusercontent.com/nowaytouse/script_hub/master/"

		linesOut := strings.Split(pluginContent, "\n")
		inUrlRewrite := false
		for i, line := range linesOut {
			stripped := strings.TrimSpace(line)
			if strings.HasPrefix(stripped, "[") {
				inUrlRewrite = strings.HasPrefix(strings.ToLower(stripped), "[rewrite]")
			}
			if !inUrlRewrite && strings.Contains(line, cdnOwn) {
				linesOut[i] = strings.ReplaceAll(line, cdnOwn, ghOwn)
			}
		}
		pluginContent = strings.Join(linesOut, "\n")
	}

	hub.SafeWriteFile(targetPath, pluginContent, true)
	hub.Success(fmt.Sprintf("Loon Plugin generated: %s", filepath.Base(targetPath)))
}

func (m *AdBlockManager) Merge(execute bool) {
	hub.Section("AdBlock Module Consolidation (Canonical Rebuild)")
	m.LoadWhitelist()
	m.LoadHashes()
	m.CleanupLocalLists()

	sourceEntries := m.LoadSourceEntries()

	// Sort sourceEntries based on categoryNames order
	priorityMap := make(map[string]int)
	for i, cat := range categoryNames {
		priorityMap[cat] = i
	}

	for i := 0; i < len(sourceEntries); i++ {
		for j := i + 1; j < len(sourceEntries); j++ {
			catI := m.DetermineCategory(sourceEntries[i].Source)
			catJ := m.DetermineCategory(sourceEntries[j].Source)
			pI, okI := priorityMap[catI]
			if !okI {
				pI = 99
			}
			pJ, okJ := priorityMap[catJ]
			if !okJ {
				pJ = 99
			}
			if pI > pJ {
				sourceEntries[i], sourceEntries[j] = sourceEntries[j], sourceEntries[i]
			}
		}
	}

	hub.Section("Syncing Upstream Ad Modules")
	cachedModules, syncFailures := m.SyncModuleSources(sourceEntries)

	hub.Section("Fetching Canonical AdBlock Sources")
	failures := m.ProcessSourceEntries(sourceEntries, cachedModules)

	hub.Section("Integrating Local AdBlock Sources")
	if hub.ValidateDirExists(hub.MODULE_LOCAL_DIR, "") {
		entries, _ := os.ReadDir(hub.MODULE_LOCAL_DIR)
		var names []string
		for _, e := range entries {
			names = append(names, e.Name())
		}
		sort.Strings(names)
		for _, name := range names {
			if strings.HasSuffix(name, ".sgmodule") || strings.HasSuffix(name, ".module") {
				path := filepath.Join(hub.MODULE_LOCAL_DIR, name)
				hub.Info(fmt.Sprintf("  + Rules from local source: %s", name))
				content := hub.ReadFileString(path)
				m.ExtractFromText(content, "REJECT", "Local", false, true)
			}
		}
	}

	hub.Section("Integrating App AdBlock Rules (LocalModules)")
	if hub.ValidateDirExists(hub.LOCAL_MODULES_DIR, "") {
		entries, _ := os.ReadDir(hub.LOCAL_MODULES_DIR)
		var names []string
		for _, e := range entries {
			names = append(names, e.Name())
		}
		sort.Strings(names)
		for _, name := range names {
			if !strings.HasSuffix(name, ".sgmodule") && !strings.HasSuffix(name, ".module") {
				continue
			}
			path := filepath.Join(hub.LOCAL_MODULES_DIR, name)
			mode := m.ResolveModuleIngestMode(path, false)
			if mode == "skip" {
				continue
			}
			hub.Info(fmt.Sprintf("  + Rules from LocalModules (%s): %s", mode, name))
			content := hub.ReadFileString(path)
			m.ExtractFromText(content, "REJECT", "Local", false, true)
		}
	}

	if execute {
		generatedRulesets := m.generateRulesets()
		m.integrateFunctionalSections()
		m.writeCatalog(generatedRulesets)

		hub.Info("Generating PROMAX modules (CDN + GitHub variants)...")
		m.generateModule(generatedRulesets, false, "cdn")
		m.generateModule(generatedRulesets, false, "github")

		hub.Info("Generating PROMAX Lite (mobile-friendly) modules (CDN + GitHub variants)...")
		m.generateModule(generatedRulesets, true, "cdn")
		m.generateModule(generatedRulesets, true, "github")

		hub.Info("Generating PROMAX Loon plugins (CDN + GitHub variants)...")
		m.generateLoonPlugin(generatedRulesets, false, "cdn")
		m.generateLoonPlugin(generatedRulesets, false, "github")

		hub.Info("Generating PROMAX Lite Loon plugins (CDN + GitHub variants)...")
		m.generateLoonPlugin(generatedRulesets, true, "cdn")
		m.generateLoonPlugin(generatedRulesets, true, "github")

		// Save hashes for change tracking
		localSourcePaths := []string{}
		for _, e := range sourceEntries {
			if !e.IsRemote() {
				localSourcePaths = append(localSourcePaths, e.ResolvedPath())
			}
		}

		inputHashes := make(map[string]string)
		for _, p := range hub.UniqueStrings(localSourcePaths) {
			if hub.ValidateFileExists(p, "") {
				rel, _ := filepath.Rel(hub.ROOT, p)
				inputHashes[rel] = hub.GetFileHash(p)
			}
		}
		m.SaveHashes(inputHashes)
	} else {
		hub.Info("Dry Run Mode - Generation skipped.")
	}

	if len(syncFailures) > 0 || len(failures) > 0 {
		hub.Warn(fmt.Sprintf("AdBlock merge completed with %d failures.", len(syncFailures)+len(failures)))
	} else {
		hub.Success("AdBlock merge completed successfully without failures.")
	}
}

func attachPolicy(rule string, policy string) string {
	stripValues := map[string]bool{
		"REJECT": true, "REJECT-DROP": true, "REJECT-NO-DROP": true, "DIRECT": true, "PROXY": true,
		"REJECT-TINYGIF": true, "REJECT-IMG": true, "EXTENDED-MATCHING": true, "PRE-MATCHING": true, "NO-RESOLVE": true,
	}
	parts := strings.Split(rule, ",")
	var cleaned []string
	for _, p := range parts {
		trimmed := strings.TrimSpace(p)
		if trimmed != "" && !stripValues[strings.ToUpper(trimmed)] {
			cleaned = append(cleaned, trimmed)
		}
	}
	cleaned = append(cleaned, policy)
	return strings.Join(cleaned, ",")
}
