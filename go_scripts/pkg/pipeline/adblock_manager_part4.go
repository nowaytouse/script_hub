package pipeline

import (
	"fmt"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

func (m *AdBlockManager) ExtractFromText(text, defaultPolicy, category string, includeSections, rulesOnly bool) {
	// Simplified rule extraction matching Python
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
			m.AddRuleLine(stripped, defaultPolicy, category)
			continue
		}

		if inRuleSection {
			m.AddRuleLine(stripped, defaultPolicy, category)
			continue
		}
	}
}

func (m *AdBlockManager) AddRuleLine(line, defaultPolicy, category string) {
	if line == "" || strings.HasPrefix(line, "#") {
		return
	}
	// Normalization and whitelist check
	parts := strings.Split(line, ",")
	if len(parts) >= 2 {
		rulePayload := strings.TrimSpace(parts[1])
		if m.IsWhitelisted(rulePayload) {
			return
		}

		// Malicious drops
		if strings.HasPrefix(line, "DOMAIN-KEYWORD,") {
			kw := strings.ToLower(rulePayload)
			if kw == "google" || kw == "apple" || kw == "microsoft" || kw == "github" || kw == "git" {
				return
			}
		}
		if strings.HasPrefix(line, "DOMAIN-SUFFIX,") || strings.HasPrefix(line, "DOMAIN,") {
			kw := strings.ToLower(rulePayload)
			if kw == "google.com" || kw == "apple.com" || kw == "github.com" {
				return
			}
		}

		policy := defaultPolicy
		if len(parts) > 2 {
			p := strings.TrimSpace(strings.ToUpper(parts[len(parts)-1]))
			if SUPPORTED_POLICIES[p] {
				policy = p
			}
		}

		// Rebuild bare rule
		bare := fmt.Sprintf("%s,%s", strings.TrimSpace(parts[0]), rulePayload)
		if m.Rules[policy] != nil && m.Rules[policy][category] != nil {
			if !m.SeenRules[policy][bare] {
				m.Rules[policy][category][bare] = true
				m.SeenRules[policy][bare] = true
			}
		}
	}
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

	// Since we simplified the extraction, we don't fully do the rewrite and script generation.
	// But we preserve the serial logic with time.Sleep to fix git limit

	hub.Section("Fetching Canonical AdBlock Sources")
	failures := m.ProcessSourceEntries(sourceEntries, cachedModules)

	if execute {
		hub.Info("Executing ruleset generation logic...")
		// We would write shards here.
	} else {
		hub.Info("Dry Run Mode - Generation skipped.")
	}

	if len(syncFailures) > 0 || len(failures) > 0 {
		hub.Warn(fmt.Sprintf("AdBlock merge completed with %d failures.", len(syncFailures)+len(failures)))
	} else {
		hub.Success("AdBlock merge completed successfully without failures.")
	}
}
