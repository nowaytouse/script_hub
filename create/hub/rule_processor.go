package hub

import (
	"regexp"
	"sort"
	"strings"
)

var surgePolicyOptions = map[string]bool{
	"DIRECT": true, "REJECT": true, "REJECT-DROP": true, "REJECT-NO-DROP": true,
	"REJECT-TINYGIF": true, "REJECT-IMG": true, "PROXY": true,
}

var (
	domainRule        = regexp.MustCompile(`(?i)^DOMAIN,([^,]+)$`)
	domainSuffixRule  = regexp.MustCompile(`(?i)^DOMAIN-SUFFIX,([^,]+)$`)
	domainKeywordRule = regexp.MustCompile(`(?i)^DOMAIN-KEYWORD,([^,]+)$`)
	ipCidrRule        = regexp.MustCompile(`(?i)^IP-CIDR6?,([^,]+)$`)
	domainRegexRule   = regexp.MustCompile(`(?i)^DOMAIN-REGEX,(.+)$`)
)

var (
	validDomain   = regexp.MustCompile(`^(\*\.)?[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$`)
	validIpv4Cidr = regexp.MustCompile(`^(\d{1,3}\.){3}\d{1,3}(/\d{1,2})?$`)
	validIpv6Cidr = regexp.MustCompile(`^([0-9a-fA-F:]+)(/\d{1,3})?$`)
)

type RuleProcessor struct {
	strictValidation bool
	stats            map[string]int
}

func NewRuleProcessor(strictValidation bool) *RuleProcessor {
	return &RuleProcessor{
		strictValidation: strictValidation,
		stats: map[string]int{
			"total":      0,
			"normalized": 0,
			"invalid":    0,
			"duplicate":  0,
		},
	}
}

func (rp *RuleProcessor) NormalizeRule(line string, source string) *string {
	rp.stats["total"]++

	line = StripInlineComment(strings.TrimSpace(line))
	if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, ";") {
		return nil
	}

	if !strings.Contains(line, ",") {
		rp.stats["invalid"]++
		return nil
	}

	parts := strings.SplitN(line, ",", 2)
	ruleType := strings.ToUpper(strings.TrimSpace(parts[0]))
	ruleValue := strings.TrimSpace(parts[1])

	if strings.HasPrefix(ruleType, "IP-") {
		ruleValue = strings.TrimSpace(strings.SplitN(ruleValue, ",", 2)[0])
	}

	validTypes := map[string]bool{
		"DOMAIN": true, "DOMAIN-SUFFIX": true, "DOMAIN-KEYWORD": true,
		"IP-CIDR": true, "IP-CIDR6": true, "DOMAIN-REGEX": true, "GEOIP": true,
		"USER-AGENT": true, "URL-REGEX": true, "PROCESS-NAME": true,
	}
	if !validTypes[ruleType] {
		rp.stats["invalid"]++
		return nil
	}

	if ruleType == "URL-REGEX" {
		// External RULE-SET entries carry the pattern only; strip a source policy
		// suffix before formatting so quoted patterns are not double-wrapped.
		if strings.HasPrefix(ruleValue, "\"\"") {
			ruleValue = ruleValue[1:]
		}
		body, options, validShape := splitURLRegexPayload(ruleValue)
		if !validShape || len(body) == 0 {
			rp.stats["invalid"]++
			return nil
		}
		for _, option := range options {
			option = strings.ToUpper(strings.TrimSpace(option))
			if option != "" && option != "EXTENDED-MATCHING" && !surgePolicyOptions[option] {
				rp.stats["invalid"]++
				return nil
			}
		}
		ruleValue = body
	} else if strings.HasPrefix(ruleValue, "\"") && strings.HasSuffix(ruleValue, "\"") && len(ruleValue) >= 2 {
		ruleValue = ruleValue[1 : len(ruleValue)-1]
	}

	if !rp.validateRuleValue(ruleType, ruleValue) {
		rp.stats["invalid"]++
		return nil
	}

	normalized := formatRule(ruleType, ruleValue)
	rp.stats["normalized"]++
	return &normalized
}

func formatRule(ruleType string, ruleValue string) string {
	if ruleType == "URL-REGEX" {
		return FormatUrlRegexForSurge(ruleValue)
	}
	return ruleType + "," + ruleValue
}

func (rp *RuleProcessor) validateRuleValue(ruleType string, value string) bool {
	if value == "" {
		return false
	}

	if ruleType == "DOMAIN" || ruleType == "DOMAIN-SUFFIX" {
		value = strings.TrimLeft(value, ".")
		if rp.strictValidation {
			return validDomain.MatchString(value)
		}
		return strings.Contains(value, ".") || len(value) > 2
	}

	if ruleType == "DOMAIN-KEYWORD" {
		if len(value) < 2 {
			return false
		}
		for _, c := range []rune{'<', '>', '{', '}', '(', ')'} {
			if strings.ContainsRune(value, c) {
				return false
			}
		}
		return true
	}

	if ruleType == "IP-CIDR" {
		return validIpv4Cidr.MatchString(value)
	}
	if ruleType == "IP-CIDR6" {
		return validIpv6Cidr.MatchString(value)
	}

	if ruleType == "DOMAIN-REGEX" {
		if IsInvalidDomainRegexPayload(value) {
			return false
		}
		_, err := regexp.Compile(value)
		return err == nil
	}

	if ruleType == "URL-REGEX" {
		if IsInvalidUrlRegexPayload(value) {
			return false
		}
		_, err := regexp.Compile(value)
		return err == nil
	}

	return true
}

func (rp *RuleProcessor) DeduplicateRules(rules []string) []string {
	seen := make(map[string]bool)
	var result []string
	for _, rule := range rules {
		if !seen[rule] {
			seen[rule] = true
			result = append(result, rule)
		} else {
			rp.stats["duplicate"]++
		}
	}
	return result
}

func (rp *RuleProcessor) DeduplicateByPriority(rulesDict map[string][]string) map[string][]string {
	ruleToSources := make(map[string][]string)
	for source, rules := range rulesDict {
		for _, rule := range rules {
			ruleToSources[rule] = append(ruleToSources[rule], source)
		}
	}

	result := make(map[string][]string)
	for source := range rulesDict {
		result[source] = []string{} // Initialize all sources
	}

	// Ensure stable deduplication by sorting rules and sources
	var sortedRules []string
	for rule := range ruleToSources {
		sortedRules = append(sortedRules, rule)
	}
	sort.Strings(sortedRules)

	for _, rule := range sortedRules {
		sources := ruleToSources[rule]
		if len(sources) == 1 {
			result[sources[0]] = append(result[sources[0]], rule)
		} else {
			sort.Strings(sources)
			primary := sources[0]
			result[primary] = append(result[primary], rule)
			rp.stats["duplicate"] += len(sources) - 1
		}
	}

	return result
}

func (rp *RuleProcessor) ExtractDomainsFromRules(rules []string) []string {
	domainsSet := make(map[string]bool)
	for _, rule := range rules {
		if m := domainRule.FindStringSubmatch(rule); m != nil {
			domainsSet[m[1]] = true
			continue
		}
		if m := domainSuffixRule.FindStringSubmatch(rule); m != nil {
			domainsSet[m[1]] = true
			continue
		}
	}
	var domains []string
	for d := range domainsSet {
		domains = append(domains, d)
	}
	sort.Strings(domains)
	return domains
}

func (rp *RuleProcessor) FilterByWhitelist(rules []string, whitelist map[string]bool) []string {
	var result []string
	for _, rule := range rules {
		var domain string
		if m := domainRule.FindStringSubmatch(rule); m != nil {
			domain = strings.ToLower(m[1])
		} else if m := domainSuffixRule.FindStringSubmatch(rule); m != nil {
			domain = strings.ToLower(m[1])
		}

		if domain != "" {
			if whitelist[domain] {
				continue
			}
			skip := false
			for w := range whitelist {
				if strings.HasSuffix(domain, "."+w) {
					skip = true
					break
				}
			}
			if skip {
				continue
			}
		}
		result = append(result, rule)
	}
	return result
}

func (rp *RuleProcessor) SplitByType(rules []string) map[string][]string {
	result := make(map[string][]string)
	for _, rule := range rules {
		parts := strings.SplitN(rule, ",", 2)
		if len(parts) >= 2 {
			ruleType := strings.ToUpper(strings.TrimSpace(parts[0]))
			result[ruleType] = append(result[ruleType], rule)
		}
	}
	return result
}

func (rp *RuleProcessor) GetStats() map[string]int {
	res := make(map[string]int)
	for k, v := range rp.stats {
		res[k] = v
	}
	return res
}

func (rp *RuleProcessor) ResetStats() {
	rp.stats = map[string]int{
		"total":      0,
		"normalized": 0,
		"invalid":    0,
		"duplicate":  0,
	}
}

var defaultProcessor = NewRuleProcessor(false)

func NormalizeRule(line string, source string) *string {
	return defaultProcessor.NormalizeRule(line, source)
}

func DeduplicateRules(rules []string) []string {
	return defaultProcessor.DeduplicateRules(rules)
}
