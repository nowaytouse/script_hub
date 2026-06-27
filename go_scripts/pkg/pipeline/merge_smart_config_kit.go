package pipeline

import (
	"fmt"
	"path/filepath"
	"regexp"
	"sort"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

var (
	sckDir     = hub.SCK_SHUNT_RULES_DIR
	customDir  = hub.SMART_CONFIG_KIT_DIR
	sourcesDir = hub.LINKS_DIR

	extraLists = map[string]string{
		"UK.list":     "StreamEU",
		"CiciAi.list": "AI",
	}

	mapping = map[string]string{
		"01-ai-service.list": "AI",
		"02-crypto.list":     "Binance",
		"03-payments.list":   "PayPal",
		"04-email.list":      "SocialMedia",
		"05-im.list":         "SocialMedia",
		"06-social.list":     "SocialMedia",
		"07-work.list":       "GitHub",
		"08-cn-media.list":   "Bilibili",
		"09-sea-media.list":  "StreamHK",
		"10-us-media.list":   "StreamUS",
		"11-hk-media.list":   "StreamHK",
		"12-tw-media.list":   "StreamTW",
		"13-jp-media.list":   "StreamJP",
		"14-eu-media.list":   "StreamEU",
		"15-cn-game.list":    "Direct",
		"16-intl-game.list":  "Gaming",
		"17-search.list":     "Google",
		"18-dev.list":        "GitHub",
		"19-microsoft.list":  "Microsoft",
		"20-apple.list":      "Apple",
		"21-download.list":   "Direct",
		"22-cloud-cdn.list":  "CDN",
		"24-cn-site.list":    "Direct",
		"25-gfw.list":        "GlobalProxy",
		"26-intl-site.list":  "GlobalProxy",
		"27-final.list":      "GlobalProxy",
		"28-ads.list":        "AdBlock",
	}

	surgeRuleRe = regexp.MustCompile(`(?i)^(DOMAIN(?:-SUFFIX|-KEYWORD|-WILDCARD)?|IP-CIDR6?|PROCESS-NAME),(.+)$`)
)

func parseShunt(filepath string) []string {
	var rules []string
	if !hub.ValidateFileExists(filepath, "") {
		return rules
	}

	content := hub.ReadFileString(filepath)
	lines := strings.Split(content, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if strings.HasPrefix(line, "domain:") {
			rules = append(rules, fmt.Sprintf("DOMAIN-SUFFIX,%s", line[7:]))
		} else if strings.HasPrefix(line, "full:") {
			rules = append(rules, fmt.Sprintf("DOMAIN,%s", line[5:]))
		} else if strings.HasPrefix(line, "keyword:") {
			rules = append(rules, fmt.Sprintf("DOMAIN-KEYWORD,%s", line[8:]))
		}
	}
	return rules
}

func parseSurgeList(filepath string) []string {
	var rules []string
	if !hub.ValidateFileExists(filepath, "") {
		return rules
	}

	content := hub.ReadFileString(filepath)
	lines := strings.Split(content, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if surgeRuleRe.MatchString(line) {
			rules = append(rules, line)
		}
	}
	return rules
}

func MergeSmartConfigKit() {
	hub.Section("Merging Smart-Config-Kit supplemental rules")
	hub.EnsureDir(customDir)

	type TargetData struct {
		rules   map[string]bool
		sources []string
	}
	merged := make(map[string]*TargetData)

	ensureTarget := func(t string) *TargetData {
		if merged[t] == nil {
			merged[t] = &TargetData{rules: make(map[string]bool)}
		}
		return merged[t]
	}

	for sckFile, target := range mapping {
		path := filepath.Join(sckDir, sckFile)
		rules := parseShunt(path)
		if len(rules) == 0 {
			continue
		}
		td := ensureTarget(target)
		for _, r := range rules {
			td.rules[r] = true
		}
		td.sources = append(td.sources, sckFile)
		hub.Success(fmt.Sprintf("  %4d rules  %s → %s", len(rules), sckFile, target))
	}

	for listFile, target := range extraLists {
		path := filepath.Join(customDir, listFile)
		rules := parseSurgeList(path)
		if len(rules) == 0 {
			continue
		}
		td := ensureTarget(target)
		for _, r := range rules {
			td.rules[r] = true
		}
		td.sources = append(td.sources, listFile)
		hub.Success(fmt.Sprintf("  %4d rules  %s → %s", len(rules), listFile, target))
	}

	var targets []string
	for t := range merged {
		targets = append(targets, t)
	}
	sort.Strings(targets)

	for _, target := range targets {
		payload := merged[target]
		customFile := fmt.Sprintf("SCK_%s.list", target)
		customPath := filepath.Join(customDir, customFile)

		header := fmt.Sprintf("# Smart-Config-Kit supplement: %s\n", strings.Join(payload.sources, ", "))
		var ruleList []string
		for r := range payload.rules {
			ruleList = append(ruleList, r)
		}
		sort.Strings(ruleList)

		content := header + strings.Join(ruleList, "\n") + "\n"
		hub.SafeWriteFile(customPath, content, true)
		hub.Success(fmt.Sprintf("  Wrote %d rules → %s", len(ruleList), customFile))

		sourcesFile := filepath.Join(sourcesDir, fmt.Sprintf("%s_sources.list", target))
		if !hub.ValidateFileExists(sourcesFile, "") {
			continue
		}

		sourcesContent := hub.ReadFileString(sourcesFile)
		relativeLink := fmt.Sprintf("../custom/SmartConfigKit/%s", customFile)
		if !strings.Contains(sourcesContent, relativeLink) {
			sourcesContent += fmt.Sprintf("\n# Smart-Config-Kit supplement\n%s\n", relativeLink)
			hub.SafeWriteFile(sourcesFile, sourcesContent, true)
			hub.Info(fmt.Sprintf("  Linked %s → %s_sources.list", customFile, target))
		}
	}
}
