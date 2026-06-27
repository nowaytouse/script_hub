package pipeline

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

func (c *RulesetCleanup) load() {
	exemptFilenames := map[string]bool{
		"HTTPDNS_Hijack.list": true,
		"reject-drop.list":    true,
		"reject-no-drop.list": true,
		"AdBlock.list":        true,
	}

	for _, dir := range rulesetDirs {
		if !hub.ValidateDirExists(dir, "") {
			continue
		}
		entries, _ := os.ReadDir(dir)
		var filenames []string
		for _, entry := range entries {
			if !entry.IsDir() && strings.HasSuffix(entry.Name(), ".list") {
				filenames = append(filenames, entry.Name())
			}
		}
		sort.Strings(filenames)

		for _, filename := range filenames {
			if exemptFilenames[filename] {
				continue
			}
			filepath := filepath.Join(dir, filename)
			rawUnique := loadList(filepath)
			c.fileContent[filename] = rawUnique
			c.filePaths[filename] = filepath

			for _, pfx := range adblockOnlyPrefixes {
				if strings.HasPrefix(filename, pfx) {
					for k := range rawUnique {
						c.adblockPayloads[k] = true
					}
					break
				}
			}
		}
	}
	c.stats["files"] = len(c.fileContent)
}

func (c *RulesetCleanup) upsert(rules map[string]string, rule string) {
	payload := extractPayload(rule)
	if payload == "" {
		return
	}
	if existing, ok := rules[payload]; ok {
		rules[payload] = preferRule(existing, rule)
	} else {
		rules[payload] = rule
	}
}

func (c *RulesetCleanup) rebalanceGenericRules() {
	if c.fileContent["Direct.list"] == nil {
		c.fileContent["Direct.list"] = make(map[string]string)
	}
	if c.fileContent["GlobalProxy.list"] == nil {
		c.fileContent["GlobalProxy.list"] = make(map[string]string)
	}

	direct := c.fileContent["Direct.list"]
	proxy := c.fileContent["GlobalProxy.list"]

	for payload, rule := range direct {
		if isProxyishDomain(rule) {
			c.upsert(proxy, rule)
			delete(direct, payload)
			c.stats["moved_direct_to_proxy"]++
		}
	}

	for payload, rule := range proxy {
		if isCnTld(rule) && !isProxyishDomain(rule) {
			c.upsert(direct, rule)
			delete(proxy, payload)
			c.stats["moved_proxy_to_direct"]++
		}
	}
}

func (c *RulesetCleanup) deduplicateByPriority() {
	var allLists []string
	for f := range c.fileContent {
		allLists = append(allLists, f)
	}
	sort.Strings(allLists)

	fallbackRulesets := map[string]bool{
		"Direct.list":         true,
		"Direct_ip.list":      true,
		"GlobalProxy.list":    true,
		"GlobalProxy_ip.list": true,
	}

	var fallbackFiles, specificFiles []string
	for _, f := range allLists {
		if fallbackRulesets[f] {
			fallbackFiles = append(fallbackFiles, f)
		} else {
			specificFiles = append(specificFiles, f)
		}
	}

	var expandedSpecific []string
	seenSpecific := make(map[string]bool)
	for _, p := range priorityOrder {
		isFallbackPrefix := false
		for fb := range fallbackRulesets {
			if strings.HasPrefix(fb, p) {
				isFallbackPrefix = true
				break
			}
		}
		if isFallbackPrefix {
			continue
		}
		for _, filename := range specificFiles {
			if strings.HasPrefix(filename, p) && !seenSpecific[filename] {
				expandedSpecific = append(expandedSpecific, filename)
				seenSpecific[filename] = true
			}
		}
	}
	var remainingSpecific []string
	for _, f := range specificFiles {
		if !seenSpecific[f] {
			remainingSpecific = append(remainingSpecific, f)
		}
	}
	finalSpecific := append(expandedSpecific, remainingSpecific...)

	var expandedFallback []string
	seenFallback := make(map[string]bool)
	for _, p := range priorityOrder {
		for _, filename := range fallbackFiles {
			if strings.HasPrefix(filename, p) && !seenFallback[filename] {
				expandedFallback = append(expandedFallback, filename)
				seenFallback[filename] = true
			}
		}
	}
	var remainingFallback []string
	for _, f := range fallbackFiles {
		if !seenFallback[f] {
			remainingFallback = append(remainingFallback, f)
		}
	}
	finalFallback := append(expandedFallback, remainingFallback...)

	priority := append(finalSpecific, finalFallback...)

	effectiveSemanticCoverage := make(map[string]bool)
	for p := range semanticCoverageRulesets {
		for _, filename := range allLists {
			if strings.HasPrefix(filename, p) {
				effectiveSemanticCoverage[filename] = true
			}
		}
	}

	seenPayloads := make(map[string]bool)
	seenCoveringSuffixes := make(map[string]bool)

	for _, filename := range priority {
		rules := c.fileContent[filename]
		if rules == nil {
			continue
		}

		kept := make(map[string]string)

		var sortedKeys []string
		for k := range rules {
			sortedKeys = append(sortedKeys, k)
		}
		sort.Slice(sortedKeys, func(i, j int) bool {
			// This is an approximation of Python's sort since we just want deterministic ordering
			return sortedKeys[i] < sortedKeys[j]
		})

		for _, payload := range sortedKeys {
			rule := rules[payload]

			isAdblockPrefix := false
			for _, pfx := range adblockOnlyPrefixes {
				if strings.HasPrefix(filename, pfx) {
					isAdblockPrefix = true
					break
				}
			}

			if !isAdblockPrefix {
				if c.adblockPayloads[payload] {
					c.stats["semantic_cover_removed"]++
					continue
				}
			}

			ruleType := extractType(rule)
			if seenPayloads[payload] {
				c.stats["cross_file_removed"]++
				continue
			}

			if semanticTypes[ruleType] {
				covered := false
				for _, parent := range iterDomainParents(payload) {
					if seenCoveringSuffixes[parent] {
						covered = true
						break
					}
				}
				if covered {
					c.stats["semantic_cover_removed"]++
					continue
				}
			}

			seenPayloads[payload] = true
			kept[payload] = rule
			if effectiveSemanticCoverage[filename] && ruleType == "DOMAIN-SUFFIX" {
				seenCoveringSuffixes[payload] = true
			}
		}
		c.fileContent[filename] = kept
	}
}

func (c *RulesetCleanup) purgeNsfwFromSocial() {
	social := c.fileContent["SocialMedia.list"]
	if social == nil {
		return
	}
	removed := 0
	for payload := range social {
		for _, kw := range nsfwContaminationKeywords {
			if strings.Contains(payload, kw) {
				delete(social, payload)
				removed++
				break
			}
		}
	}
	c.stats["nsfw_purged_from_social"] = removed
}

func (c *RulesetCleanup) audit() {
	direct := c.fileContent["Direct.list"]
	proxy := c.fileContent["GlobalProxy.list"]

	for _, rule := range direct {
		if isProxyishDomain(rule) {
			c.stats["direct_proxyish_remaining"]++
		}
	}

	for _, rule := range proxy {
		if isCnTld(rule) && !isProxyishDomain(rule) {
			c.stats["globalproxy_cn_remaining"]++
		}
	}
}

func (c *RulesetCleanup) save() {
	for filename, rules := range c.fileContent {
		filepath := c.filePaths[filename]
		if filepath != "" {
			if len(rules) == 0 && filename != "AdBlock.list" {
				if hub.ValidateFileExists(filepath, "") {
					if err := os.Remove(filepath); err == nil {
						fmt.Printf("✅ Deleted empty ruleset file: %s\n", filename)
					} else {
						fmt.Printf("❌ Failed to delete empty file: %s\n", filename)
					}
				}
			} else {
				writeList(filepath, rules)
			}
		}
	}

	for _, dir := range rulesetDirs {
		readmePath := filepath.Join(dir, "README.md")
		if hub.ValidateFileExists(readmePath, "") {
			os.Remove(readmePath)
			fmt.Printf("✅ Deleted stray document: %s\n", readmePath)
		}
	}
}

func (c *RulesetCleanup) Run() map[string]int {
	c.load()
	c.rebalanceGenericRules()
	c.purgeNsfwFromSocial()
	c.deduplicateByPriority()
	c.audit()
	c.save()
	return c.stats
}

func RunCleanup() map[string]int {
	return NewRulesetCleanup().Run()
}
