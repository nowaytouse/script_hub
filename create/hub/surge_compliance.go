package hub

import (
	"fmt"
	"regexp"
	"strings"
)

var SURGE_RULESET_ALLOWED = map[string]bool{
	"DOMAIN": true, "DOMAIN-SUFFIX": true, "DOMAIN-KEYWORD": true, "DOMAIN-SET": true,
	"IP-CIDR": true, "IP-CIDR6": true, "IP-ASN": true, "GEOIP": true,
	"USER-AGENT": true, "URL-REGEX": true, "PROCESS-NAME": true,
	"DEST-PORT": true, "SRC-PORT": true, "IN-PORT": true,
}

var SURGE_RULESET_FORBIDDEN = map[string]bool{
	"DOMAIN-REGEX": true, "DOMAIN-WILDCARD": true,
}

var PAYLOAD_SAFE_RULE_TYPES = map[string]bool{
	"DOMAIN-REGEX": true, "URL-REGEX": true, "USER-AGENT": true, "PROCESS-NAME": true,
}

var INVALID_DOMAIN_REGEX_VALUES = map[string]bool{
	"": true, "$": true, ",": true, "-": true, ".": true, "2": true, "6": true, "]": true, "[": true,
}

var INVALID_URL_REGEX_VALUES = map[string]bool{
	"": true, "https:": true, "http:": true, "https": true, "http": true,
	"^https?:": true, "^https?://": true, `^https?:\/\/`: true,
}

var STRIP_POLICY_TOKENS = map[string]bool{
	"REJECT": true, "REJECT-DROP": true, "REJECT-NO-DROP": true, "DIRECT": true, "PROXY": true,
	"REJECT-TINYGIF": true, "REJECT-IMG": true,
	"EXTENDED-MATCHING": true, "PRE-MATCHING": true, "NO-RESOLVE": true, "FORCE-CELLULAR": true,
	"{{{PROXY}}}": true,
}

func ruleHead(line string) string {
	if !strings.Contains(line, ",") {
		return strings.ToUpper(strings.TrimSpace(line))
	}
	parts := strings.SplitN(line, ",", 2)
	return strings.ToUpper(strings.TrimSpace(parts[0]))
}

func StripInlineComment(line string) string {
	line = strings.TrimSpace(line)
	if line == "" || strings.HasPrefix(line, "//") {
		return ""
	}
	head := ruleHead(line)
	if PAYLOAD_SAFE_RULE_TYPES[head] {
		return line
	}
	if strings.Contains(line, "//") {
		line = strings.TrimSpace(strings.SplitN(line, "//", 2)[0])
	}
	if strings.Contains(line, "#") {
		line = strings.TrimSpace(strings.SplitN(line, "#", 2)[0])
	}
	return line
}

func splitRule(line string) (string, string, bool) {
	line = StripInlineComment(line)
	if line == "" || !strings.Contains(line, ",") {
		return "", "", false
	}
	parts := strings.SplitN(line, ",", 2)
	ruleType := strings.ToUpper(strings.TrimSpace(parts[0]))
	payload := strings.TrimSpace(parts[1])
	if strings.HasPrefix(ruleType, "IP-") {
		payload = strings.TrimSpace(strings.SplitN(payload, ",", 2)[0])
	}
	if strings.HasPrefix(payload, "\"") && strings.HasSuffix(payload, "\"") && len(payload) >= 2 {
		payload = payload[1 : len(payload)-1]
	}
	return ruleType, payload, true
}

func StripTrailingPolicy(rule string) string {
	rule = strings.TrimSpace(rule)
	if !strings.Contains(rule, ",") {
		return rule
	}
	head := ruleHead(rule)
	if PAYLOAD_SAFE_RULE_TYPES[head] {
		parts := strings.SplitN(rule, ",", 2)
		ruleType := strings.TrimSpace(parts[0])
		payload := strings.TrimSpace(parts[1])
		for strings.Contains(payload, ",") {
			tailParts := strings.Split(payload, ",")
			tail := strings.ToUpper(strings.TrimSpace(tailParts[len(tailParts)-1]))
			if STRIP_POLICY_TOKENS[tail] || strings.HasPrefix(tail, "UPDATE-INTERVAL=") {
				payload = strings.TrimSpace(strings.Join(tailParts[:len(tailParts)-1], ","))
			} else {
				break
			}
		}
		return ruleType + "," + payload
	}

	parts := strings.Split(rule, ",")
	var trimmed []string
	for _, p := range parts {
		trimmed = append(trimmed, strings.TrimSpace(p))
	}
	for len(trimmed) > 2 {
		tail := strings.ToUpper(trimmed[len(trimmed)-1])
		if STRIP_POLICY_TOKENS[tail] || strings.HasPrefix(tail, "UPDATE-INTERVAL=") {
			trimmed = trimmed[:len(trimmed)-1]
		} else {
			break
		}
	}
	return strings.Join(trimmed, ",")
}

func IsInvalidDomainRegexPayload(payload string) bool {
	if len(strings.TrimSpace(payload)) < 2 {
		return true
	}
	return INVALID_DOMAIN_REGEX_VALUES[payload]
}

func IsInvalidUrlRegexPayload(payload string) bool {
	val := strings.TrimSpace(payload)
	if len(val) < 3 {
		return true
	}
	return INVALID_URL_REGEX_VALUES[strings.ToLower(val)]
}

func ConvertDomainRegexForSurge(rule string) string {
	if !strings.HasPrefix(strings.ToUpper(rule), "DOMAIN-REGEX,") {
		return ""
	}
	parts := strings.SplitN(rule, ",", 2)
	payload := strings.Trim(strings.TrimSpace(parts[1]), "\"")
	if IsInvalidDomainRegexPayload(payload) {
		return ""
	}

	literal := strings.ReplaceAll(payload, "\\.", ".")

	m1 := regexp.MustCompile(`^\(\^\|\.\)(.+?)-\.\+\.`)
	match1 := m1.FindStringSubmatch(literal)
	if match1 != nil {
		return "DOMAIN-KEYWORD," + match1[1]
	}

	m2 := regexp.MustCompile(`^(?:\(\^\|\.\)|\^)([a-zA-Z0-9][-a-zA-Z0-9.]*)$`)
	match2 := m2.FindStringSubmatch(literal)
	if match2 != nil {
		return "DOMAIN-KEYWORD," + match2[1]
	}

	m3 := regexp.MustCompile(`^\.\+(.+)$`)
	match3 := m3.FindStringSubmatch(literal)
	if match3 != nil && len(match3[1]) >= 3 {
		return "DOMAIN-KEYWORD," + match3[1]
	}

	return ""
}

func FormatUrlRegexForSurge(payload string) string {
	if strings.Contains(payload, ",") || strings.Contains(payload, " ") {
		return fmt.Sprintf("URL-REGEX,\"%s\"", payload)
	}
	return "URL-REGEX," + payload
}

func ValidateSurgeRulesetLine(line string, allowDomainRegex bool) *string {
	s := strings.TrimSpace(line)
	if s == "" || strings.HasPrefix(s, "#") {
		return nil
	}

	ruleType, payload, ok := splitRule(s)
	if !ok {
		errStr := "missing rule type or payload"
		return &errStr
	}

	if SURGE_RULESET_FORBIDDEN[ruleType] && !allowDomainRegex {
		errStr := fmt.Sprintf("%s is not supported in Surge external RULE-SET", ruleType)
		return &errStr
	}

	if !SURGE_RULESET_ALLOWED[ruleType] && !SURGE_RULESET_FORBIDDEN[ruleType] {
		errStr := fmt.Sprintf("unknown rule type %s", ruleType)
		return &errStr
	}

	if ruleType == "URL-REGEX" {
		raw := strings.TrimSpace(strings.SplitN(s, ",", 2)[1])
		quoted := strings.HasPrefix(raw, "\"") && strings.HasSuffix(raw, "\"")
		body := raw
		if quoted && len(raw) >= 2 {
			body = raw[1 : len(raw)-1]
		}
		if strings.Contains(body, ",") && !quoted {
			errStr := "URL-REGEX payload with comma must be double-quoted"
			return &errStr
		}
		if IsInvalidUrlRegexPayload(body) {
			errStr := fmt.Sprintf("truncated/invalid URL-REGEX %q", body)
			return &errStr
		}
		_, err := regexp.Compile(body)
		if err != nil {
			errStr := fmt.Sprintf("URL-REGEX compile error: %v", err)
			return &errStr
		}
	}

	if ruleType == "DOMAIN-REGEX" && allowDomainRegex {
		if IsInvalidDomainRegexPayload(payload) {
			errStr := fmt.Sprintf("invalid DOMAIN-REGEX payload %q", payload)
			return &errStr
		}
	}

	return nil
}
