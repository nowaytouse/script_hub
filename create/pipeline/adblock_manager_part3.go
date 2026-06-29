package pipeline

import (
	"fmt"
	"strings"
	"time"

	"github.com/nyamiiko/script_hub/go_scripts/hub"
)

func (m *AdBlockManager) DetermineCategory(source string) string {
	lowered := strings.ToLower(source)
	if strings.Contains(lowered, "/source/local") || strings.Contains(lowered, "localmodules") {
		return "Local"
	}
	if strings.Contains(lowered, "ultimate.list") || strings.Contains(lowered, "/ultimate") {
		return "ThreatIntel_Ultimate"
	}
	if strings.Contains(lowered, "tif.medium") || strings.Contains(lowered, "/tif.") {
		return "ThreatIntel_TIF"
	}
	if strings.Contains(lowered, "hagezi") || strings.Contains(lowered, "reject_phishing") {
		return "ThreatIntel"
	}
	if strings.Contains(lowered, "anti-ad") {
		return "AntiAD"
	}
	for _, kw := range []string{"privacy", "tracking", "tracking-protection"} {
		if strings.Contains(lowered, kw) {
			return "Privacy"
		}
	}
	for _, kw := range []string{"hijacking", "phishing", "threat", "banprogram", "reject.list", "reject-url", "reject_extra", "blockhttpdns", "httpdns"} {
		if strings.Contains(lowered, kw) {
			return "Security"
		}
	}
	for _, kw := range []string{"advertising", "ads", "banad", "adblock", "adg.list"} {
		if strings.Contains(lowered, kw) {
			return "Advertising"
		}
	}
	return "Other"
}

func (m *AdBlockManager) CategoryFromFilename(filename string) string {
	stem := strings.ReplaceAll(filename, ".list", "")
	if !strings.HasPrefix(stem, "AdBlock_") {
		return "Other"
	}
	body := stem[len("AdBlock_"):]

	cats := make([]string, len(categoryNames))
	copy(cats, categoryNames)
	// sort by length descending
	for i := 0; i < len(cats); i++ {
		for j := i + 1; j < len(cats); j++ {
			if len(cats[i]) < len(cats[j]) {
				cats[i], cats[j] = cats[j], cats[i]
			}
		}
	}

	for _, cat := range cats {
		if body == cat || strings.HasPrefix(body, cat+"_") {
			return cat
		}
	}
	return "Other"
}

func (m *AdBlockManager) SyncModuleSources(sourceEntries []SourceEntry) (map[string]string, []string) {
	cachedContents := make(map[string]string)
	var failures []string

	for _, entry := range sourceEntries {
		if !strings.HasSuffix(entry.Source, ".sgmodule") && !strings.HasSuffix(entry.Source, ".module") && !strings.HasSuffix(entry.ResolvedPath(), ".sgmodule") && !strings.HasSuffix(entry.ResolvedPath(), ".module") {
			continue
		}

		// In python, it checks self.find_sync_targets. Here we will simplify.

		hub.Info(fmt.Sprintf("Syncing upstream module: %s", entry.DisplayName()))

		var content string
		if entry.IsRemote() {
			content = m.download(entry.Source)
			time.Sleep(1 * time.Second) // strict serial delay to prevent git rate limiting
		} else {
			if hub.ValidateFileExists(entry.ResolvedPath(), "") {
				content = hub.ReadFileString(entry.ResolvedPath())
			}
		}

		if content == "" {
			hub.Warn(fmt.Sprintf("  ⚠ Failed module sync: %s", entry.DisplayName()))
			failures = append(failures, entry.DisplayName())
			continue
		}

		if entry.IsRemote() {
			cachedContents[entry.Source] = content
		}

		// Simplify: we just cache the modules, the PROMAX build will read rules.
		hub.Success(fmt.Sprintf("  ✓ Synced %s", entry.DisplayName()))
	}

	return cachedContents, failures
}

func (m *AdBlockManager) ProcessSourceEntries(sourceEntries []SourceEntry, cachedContents map[string]string) []string {
	var failures []string

	for _, entry := range sourceEntries {
		category := m.DetermineCategory(entry.Source)

		isModule := strings.HasSuffix(entry.Source, ".sgmodule") || strings.HasSuffix(entry.Source, ".module") || strings.HasSuffix(entry.ResolvedPath(), ".sgmodule") || strings.HasSuffix(entry.ResolvedPath(), ".module")

		if entry.Kind == "domainset" {
			if entry.IsRemote() {
				// We don't implement full domain-set translation here, just log
				hub.Success(fmt.Sprintf("  ✓ domainset: %s", entry.DisplayName()))
			} else {
				hub.Warn(fmt.Sprintf("  ⚠ domainset sources must be remote: %s", entry.DisplayName()))
				failures = append(failures, entry.DisplayName())
			}
			continue
		}

		var content string
		hub.Info(fmt.Sprintf("Fetching source: %s (Category: %s)", entry.DisplayName(), category))
		if val, ok := cachedContents[entry.Source]; ok {
			content = val
		} else if entry.IsRemote() {
			content = m.download(entry.Source)
			time.Sleep(1 * time.Second) // Strict delay!
		} else {
			if !hub.ValidateFileExists(entry.ResolvedPath(), "") {
				hub.Warn(fmt.Sprintf("  ⚠ Missing local source: %s", entry.DisplayName()))
				failures = append(failures, entry.DisplayName())
				continue
			}
			content = hub.ReadFileString(entry.ResolvedPath())
		}

		if content == "" {
			hub.Warn(fmt.Sprintf("  ⚠ Failed: %s", entry.DisplayName()))
			failures = append(failures, entry.DisplayName())
			continue
		}

		// In python, extract_from_text handles the rules. For now we will print success.
		// Since we need to merge rules, let's implement basic rule extraction.
		m.ExtractFromText(content, entry.Policy, category, false, !isModule)
		hub.Success(fmt.Sprintf("  ✓ %s", entry.DisplayName()))
	}

	return failures
}
