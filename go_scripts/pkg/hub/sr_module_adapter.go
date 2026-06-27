package hub

import (
	"regexp"
	"strings"
)

var (
	srStripScriptParams = []struct {
		pattern *regexp.Regexp
		repl    string
	}{
		{regexp.MustCompile(`(?i),?\s*ability=(?:"[^"]*"|[^\s,]+)`), ""},
		{regexp.MustCompile(`(?i),?\s*engine=auto\b`), ""},
		{regexp.MustCompile(`(?i),?\s*engine=\{\{\{engine\}\}\}`), ""},
	}
	subStoreLineRe  = regexp.MustCompile(`(?i)sub\.store|sub-store-org`)
	scriptHubLineRe = regexp.MustCompile(`(?i)script\.hub|Script Hub`)
)

const DEVTOOLS_STEM = "🧰 Script Hub 配套工具合集"
const SUB_STORE_NOABILITY_URL = "https://raw.githubusercontent.com/sub-store-org/Sub-Store/master/config/Surge-Noability.sgmodule"

func AdaptScriptLineForSr(line string, moduleStem string) string {
	stripped := strings.TrimSpace(line)
	if stripped == "" || strings.HasPrefix(stripped, "#") {
		return line
	}

	out := line
	for _, pair := range srStripScriptParams {
		out = pair.pattern.ReplaceAllString(out, pair.repl)
	}

	if subStoreLineRe.MatchString(out) {
		out = adaptSubstoreScriptLine(out, moduleStem)
	}

	if scriptHubLineRe.MatchString(out) {
		out = regexp.MustCompile(`\s+,`).ReplaceAllString(out, ",")
		out = regexp.MustCompile(`,\s+,`).ReplaceAllString(out, ",")
	}

	return out
}

func adaptSubstoreScriptLine(line string, moduleStem string) string {
	out := line
	out = regexp.MustCompile(`(?i),?\s*max-size=(?:"[^"]*"|[^\s,]+)`).ReplaceAllString(out, "")
	out = strings.ReplaceAll(out, "timeout={{{timeout}}}", "timeout=120")
	out = strings.ReplaceAll(out, `, argument="cors={{{cors}}}"`, "")
	out = strings.ReplaceAll(out, `argument="cors={{{cors}}}"`, "")

	if regexp.MustCompile(`(?i)\{\{\{produce\}\}\}=type=cron`).MatchString(out) {
		out = regexp.MustCompile(`(?i)^\{\{\{produce\}\}\}`).ReplaceAllString(strings.TrimSpace(out), "Sub-Store Produce")
	}

	if regexp.MustCompile(`(?i)\{\{\{sync\}\}\}=type=cron`).MatchString(out) {
		out = regexp.MustCompile(`(?i)^\{\{\{sync\}\}\}`).ReplaceAllString(strings.TrimSpace(out), "Sub-Store Sync")
	}

	return out
}

func AdaptMitmLineForSr(line string) string {
	stripped := strings.TrimSpace(line)
	if !strings.HasPrefix(strings.ToLower(stripped), "hostname") {
		return line
	}
	reHead := regexp.MustCompile(`(?i)^hostname\s*=\s*`)
	part := reHead.ReplaceAllString(stripped, "")

	appendMatch := regexp.MustCompile(`(?i)^(%APPEND%|%INSERT%)\s*`).MatchString(part)
	prefix := ""
	if appendMatch {
		prefix = "%APPEND% "
	}
	part = regexp.MustCompile(`(?i)^(%APPEND%|%INSERT%)\s*`).ReplaceAllString(part, "")

	var hosts []string
	for _, h := range strings.Split(part, ",") {
		hs := strings.TrimSpace(h)
		if hs != "" && hs != "%INSERT%" && hs != "%APPEND%" {
			hosts = append(hosts, hs)
		}
	}
	if len(hosts) == 0 {
		return line
	}
	return "hostname = " + prefix + strings.Join(hosts, ", ")
}

func AdaptSectionLinesForSr(section string, lines []string, moduleStem string) []string {
	var adapted []string
	for _, line := range lines {
		if section == "Script" {
			adapted = append(adapted, AdaptScriptLineForSr(line, moduleStem))
		} else if section == "MITM" {
			adapted = append(adapted, AdaptMitmLineForSr(line))
		} else {
			adapted = append(adapted, line)
		}
	}
	return adapted
}

func ModuleStemFromMeta(meta map[string]string, fallback string) string {
	if val, ok := meta["name"]; ok {
		return strings.TrimSpace(val)
	}
	return fallback
}
