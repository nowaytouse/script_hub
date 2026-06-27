package pipeline

import (
	"fmt"
	"regexp"
	"sort"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

type sortedRuleItem struct {
	payload  string
	rule     string
	sortKey0 int
	sortKey1 int
	sortKey2 int
	sortKey3 int
}

type sortedRuleItemList []sortedRuleItem

func (s sortedRuleItemList) Len() int      { return len(s) }
func (s sortedRuleItemList) Swap(i, j int) { s[i], s[j] = s[j], s[i] }
func (s sortedRuleItemList) Less(i, j int) bool {
	if s[i].sortKey0 != s[j].sortKey0 {
		return s[i].sortKey0 < s[j].sortKey0
	}
	if s[i].sortKey1 != s[j].sortKey1 {
		return s[i].sortKey1 < s[j].sortKey1
	}
	if s[i].sortKey2 != s[j].sortKey2 {
		return s[i].sortKey2 < s[j].sortKey2
	}
	if s[i].sortKey3 != s[j].sortKey3 {
		return s[i].sortKey3 < s[j].sortKey3
	}
	if s[i].payload != s[j].payload {
		return s[i].payload < s[j].payload
	}
	return s[i].rule < s[j].rule
}

func getSortedRules(rules map[string]string) []string {
	var items sortedRuleItemList
	for payload, rule := range rules {
		ruleType := extractType(rule)
		rank := getRank(ruleType)
		if semanticTypes[ruleType] {
			items = append(items, sortedRuleItem{
				payload:  payload,
				rule:     rule,
				sortKey0: 0,
				sortKey1: strings.Count(payload, "."),
				sortKey2: len(payload),
				sortKey3: rank,
			})
		} else {
			items = append(items, sortedRuleItem{
				payload:  payload,
				rule:     rule,
				sortKey0: 1,
				sortKey1: rank,
				sortKey2: 0,
				sortKey3: 0,
			})
		}
	}
	sort.Sort(items)
	var res []string
	for _, item := range items {
		res = append(res, item.rule)
	}
	return res
}

func loadList(filepath string) map[string]string {
	rules := make(map[string]string)
	if !hub.ValidateFileExists(filepath, "") {
		return rules
	}
	content := hub.ReadFileString(filepath)
	lines := strings.Split(content, "\n")
	for _, rawLine := range lines {
		line := cleanRule(rawLine)
		if line == "" || !isValidRule(line) {
			continue
		}
		payload := extractPayload(line)
		if payload == "" {
			continue
		}
		if existing, ok := rules[payload]; ok {
			rules[payload] = preferRule(existing, line)
		} else {
			rules[payload] = line
		}
	}
	return rules
}

func writeList(filepath string, rules map[string]string) {
	sortedRules := getSortedRules(rules)
	var existingHeader []string
	if hub.ValidateFileExists(filepath, "") {
		content := hub.ReadFileString(filepath)
		for _, line := range strings.Split(content, "\n") {
			if strings.HasPrefix(line, "#") || strings.TrimSpace(line) == "" {
				existingHeader = append(existingHeader, line)
			} else {
				break
			}
		}
	}

	var outLines []string
	if len(existingHeader) > 0 {
		re := regexp.MustCompile(`^#\s*(Rules|Total|Total Rules):\s*`)
		for _, line := range existingHeader {
			if re.MatchString(line) {
				key := strings.SplitN(line, ":", 2)[0]
				outLines = append(outLines, fmt.Sprintf("%s: %d\n", key, len(sortedRules)))
			} else {
				outLines = append(outLines, line+"\n")
			}
		}
		if len(existingHeader) > 0 && strings.TrimSpace(existingHeader[len(existingHeader)-1]) != "" {
			outLines = append(outLines, "\n")
		}
	} else {
		outLines = append(outLines, fmt.Sprintf("# Ruleset: %s\n", filepath))
		outLines = append(outLines, "# Cleaned by smart_cleanup.go\n\n")
	}

	for _, rule := range sortedRules {
		outLines = append(outLines, rule+"\n")
	}

	hub.SafeWriteFile(filepath, strings.Join(outLines, ""), true)
}

type RulesetCleanup struct {
	fileContent     map[string]map[string]string
	filePaths       map[string]string
	adblockPayloads map[string]bool
	stats           map[string]int
}

func NewRulesetCleanup() *RulesetCleanup {
	return &RulesetCleanup{
		fileContent:     make(map[string]map[string]string),
		filePaths:       make(map[string]string),
		adblockPayloads: make(map[string]bool),
		stats: map[string]int{
			"files":                     0,
			"within_file_dedup":         0,
			"moved_direct_to_proxy":     0,
			"moved_proxy_to_direct":     0,
			"nsfw_purged_from_social":   0,
			"cross_file_removed":        0,
			"semantic_cover_removed":    0,
			"direct_proxyish_remaining": 0,
			"globalproxy_cn_remaining":  0,
		},
	}
}
