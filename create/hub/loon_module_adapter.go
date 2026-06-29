package hub

import (
	"fmt"
	"strings"
)

// AdaptScriptLineForLoon converts a Surge script line to Loon script line format.
// Surge format: tag = type=http-response,pattern=...,script-path=...
// Loon format: type pattern script-path=...,tag=tag_name
func AdaptScriptLineForLoon(tag string, lineContent string) string {
	params := make(map[string]string)
	
	// We need to parse comma-separated key-value pairs carefully.
	// Since arguments/patterns might contain commas, a naive strings.Split can break.
	// However, for typical module script parameters, we can parse key-value pairs.
	parts := strings.Split(lineContent, ",")
	for _, p := range parts {
		idx := strings.Index(p, "=")
		if idx != -1 {
			k := strings.ToLower(strings.TrimSpace(p[:idx]))
			v := strings.TrimSpace(p[idx+1:])
			params[k] = v
		}
	}

	scriptType := params["type"]
	if scriptType == "" {
		scriptType = "http-response"
	}
	pattern := params["pattern"]
	scriptPath := params["script-path"]
	if pattern == "" || scriptPath == "" {
		return "" // invalid script line
	}

	var loonParts []string
	loonParts = append(loonParts, fmt.Sprintf("script-path=%s", scriptPath))
	loonParts = append(loonParts, fmt.Sprintf("tag=%s", tag))

	if reqBody, ok := params["requires-body"]; ok && (reqBody == "1" || reqBody == "true") {
		loonParts = append(loonParts, "requires-body=true")
	}
	if timeout, ok := params["timeout"]; ok {
		loonParts = append(loonParts, fmt.Sprintf("timeout=%s", timeout))
	}
	if argument, ok := params["argument"]; ok {
		// Strip quotes if present
		if strings.HasPrefix(argument, "\"") && strings.HasSuffix(argument, "\"") {
			argument = argument[1 : len(argument)-1]
		}
		loonParts = append(loonParts, fmt.Sprintf("argument=\"%s\"", argument))
	}

	return fmt.Sprintf("%s %s %s", scriptType, pattern, strings.Join(loonParts, ","))
}

// AdaptRewriteLineForLoon converts Surge URL Rewrite / Map Local lines to Loon compatible rewrite format.
// Surge uses: pattern - reject, pattern - 302 target, pattern data="..."
// Loon uses: pattern reject, pattern 302 target, pattern data="..."
func AdaptRewriteLineForLoon(line string) string {
	trimmed := strings.TrimSpace(line)
	if trimmed == "" || strings.HasPrefix(trimmed, "#") {
		return line
	}

	lower := strings.ToLower(trimmed)
	if strings.Contains(lower, " - reject") {
		line = strings.Replace(line, " - reject", " reject", 1)
	} else if strings.Contains(lower, " - reject-200") {
		line = strings.Replace(line, " - reject-200", " reject-200", 1)
	} else if strings.Contains(lower, " - reject-img") {
		line = strings.Replace(line, " - reject-img", " reject-img", 1)
	} else if strings.Contains(lower, " - reject-tinygif") {
		line = strings.Replace(line, " - reject-tinygif", " reject-tinygif", 1)
	} else if strings.Contains(lower, " - reject-no-drop") {
		line = strings.Replace(line, " - reject-no-drop", " reject-no-drop", 1)
	} else if strings.Contains(lower, " - 302 ") {
		line = strings.Replace(line, " - 302 ", " 302 ", 1)
	} else if strings.Contains(lower, " - 307 ") {
		line = strings.Replace(line, " - 307 ", " 307 ", 1)
	}

	return line
}
