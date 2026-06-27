package hub

import (
	"path/filepath"
	"regexp"
	"strings"
)

type LineVerdict string

const (
	VerdictAd      LineVerdict = "ad"
	VerdictEnhance LineVerdict = "enhance"
	VerdictNeutral LineVerdict = "neutral"
)

var AD_SCRIPT_MARKERS = []string{
	"adblock", "/adblock/", ".adblock.", "anti-ad", "blockad", "remove_ads",
	"remove-ads", "_remove_", "/remove", "去广告", "/ads/", "advertising", "reject",
	"spam", "tracking", "xwebads", "adultraplus", "allinone", "mock",
	"bilibili.helper", "helper.v2.js", "camoufox", "rednote", "redpaper",
	"weibo_ads", "weibo_main", "wool_scripts", "kelee.one/resource/javascript",
	"proxyrules/main/script", "ithome_remove", "zhihu_remove", "amap_remove",
	"cainiao_remove", "taobao_remove", "jd_remove", "spotify_remove", "bilicomic_remove",
}

var AD_ENTRY_NAME_MARKERS = []string{
	"去广告", "adblock", "anti-ad", "xwebads", "移除", "精简", "推广", "开屏", "净化", "广告", "killad", "mock",
}

var ENHANCE_SCRIPT_MARKERS = []string{
	"/enhanced/", ".enhanced.", "/enhance/", "biliuniverse/enhanced", "/global/",
	".global.", "biliuniverse/global", "/redirect/", ".redirect.", "unlock", "解锁",
	"unblock", "vip", "premium", "crack", "破解", "camscanner.js", "external_links_unlock",
	"weixin_external", "1080p", "bilibili_json.js", "dualsubs", "iringo", "weatherkit",
	"zheye", "哲也", "translation", "翻译", "nsfw", "region", "intl", "bypass", "绕过",
	"pip", "画中画", "optimize", "功能增强",
}

var ENHANCE_NAME_PREFIXES = []string{
	"bilibili.enhanced", "bilibili.global", "bilibili.redirect", "youtube.enhance",
	"wechat_enhance", "iringo.", "dualsubs",
}

var AD_NAME_PREFIXES = []string{
	"bilibili.adblock", "adblock", "去广告", "xwebads", "anti-ad",
}

var REJECT_POLICIES = map[string]bool{
	"REJECT":         true,
	"REJECT-DROP":    true,
	"REJECT-NO-DROP": true,
	"REJECT-TINYGIF": true,
	"REJECT-IMG":     true,
}

var PURE_AD_MODULE_NAME_TOKENS = []string{
	"去广告", "adblock", "anti-ad", "xwebads", "adultraplus", "allinone", "all-in-one",
	"rednote", "redpaper", "微博去广告", "remove_ads",
}

var scriptPathReStr = regexp.MustCompile(`(?i)script-path\s*=\s*([^,\s]+)`)

func scriptPath(line string) string {
	m := scriptPathReStr.FindStringSubmatch(line)
	if len(m) > 1 {
		return strings.ToLower(m[1])
	}
	return ""
}

func entryName(line string) string {
	if !strings.Contains(line, "=") {
		return ""
	}
	parts := strings.SplitN(line, "=", 2)
	return strings.ToLower(strings.TrimSpace(parts[0]))
}

func containsAny(s string, markers []string) bool {
	for _, m := range markers {
		if strings.Contains(s, m) {
			return true
		}
	}
	return false
}

func SourceIsPureAdModule(sourcePath string) bool {
	if sourcePath == "" {
		return false
	}
	low := strings.ToLower(filepath.Base(sourcePath))
	return containsAny(low, PURE_AD_MODULE_NAME_TOKENS)
}

func scriptIsAd(name string, spath string, low string) bool {
	if containsAny(name, AD_NAME_PREFIXES) || containsAny(name, AD_ENTRY_NAME_MARKERS) {
		if containsAny(name, []string{"解锁", "unlock", "vip", "premium", "破解", "crack"}) {
			return false
		}
		return true
	}
	if containsAny(spath, AD_SCRIPT_MARKERS) {
		return true
	}
	if strings.Contains(name, "helper") && strings.Contains(name, "bili") {
		return true
	}
	return false
}

func scriptIsEnhance(name string, spath string) bool {
	if containsAny(name, ENHANCE_NAME_PREFIXES) || containsAny(spath, ENHANCE_SCRIPT_MARKERS) {
		return true
	}
	if containsAny(name, []string{"解锁", "unlock", "vip", "premium", "破解", "crack"}) {
		return true
	}
	return false
}

func ClassifyPromaxLine(line string, section string, sourcePath string) LineVerdict {
	stripped := strings.TrimSpace(line)
	if stripped == "" || strings.HasPrefix(stripped, "#") {
		return VerdictNeutral
	}

	low := strings.ToLower(stripped)
	sec := strings.ToLower(strings.TrimSpace(section))
	pureAdSource := SourceIsPureAdModule(sourcePath)

	if sec == "mitm" {
		if !strings.Contains(low, "hostname") {
			return VerdictNeutral
		}
		if containsAny(low, []string{"unlock", "解锁", "camscanner", "intsig.net"}) {
			return VerdictEnhance
		}
		return VerdictNeutral
	}

	if sec == "general" || sec == "header rewrite" {
		return VerdictEnhance
	}

	name := entryName(stripped)
	spath := scriptPath(stripped)

	if sec == "script" || strings.Contains(low, "script-path=") {
		if scriptIsEnhance(name, spath) {
			return VerdictEnhance
		}
		if scriptIsAd(name, spath, low) {
			return VerdictAd
		}
		if pureAdSource {
			return VerdictAd
		}
		return VerdictEnhance
	}

	if sec == "url rewrite" || (strings.HasPrefix(stripped, "^") && (strings.Contains(low, " reject") || strings.Contains(low, " _ reject"))) {
		if strings.Contains(low, " reject") || strings.Contains(low, " _ reject") {
			return VerdictAd
		}
		return VerdictEnhance
	}

	if sec == "map local" || sec == "body rewrite" || sec == "maplocal" {
		if strings.Contains(low, "http-response-jq") || strings.Contains(low, "http-request-jq") {
			if containsAny(low, ENHANCE_SCRIPT_MARKERS) {
				return VerdictEnhance
			}
			return VerdictAd
		}
		if strings.HasPrefix(stripped, "http-response ") || strings.HasPrefix(stripped, "http-request ") {
			return VerdictAd
		}
		if strings.HasPrefix(stripped, "^") && (strings.Contains(low, "data-type=") || strings.Contains(low, "data=\"{}\"") || strings.Contains(low, "data='{}'")) {
			return VerdictAd
		}
		if containsAny(low, []string{"广告", "adcard", "splash", "deliver", "flash", "e-commerce"}) {
			return VerdictAd
		}
		if strings.Contains(low, "data=\"{}\"") || strings.Contains(low, "data='{}'") {
			return VerdictAd
		}
		if strings.Contains(low, "del(.data.payment)") || (strings.Contains(low, "payment") && strings.Contains(low, "del(")) {
			return VerdictAd
		}
		if pureAdSource && strings.HasPrefix(stripped, "^") {
			return VerdictAd
		}
		return VerdictNeutral
	}

	parts := strings.Split(stripped, ",")
	firstUpper := strings.ToUpper(strings.TrimSpace(parts[0]))
	if sec == "rule" || firstUpper == "DOMAIN" || firstUpper == "DOMAIN-SUFFIX" || firstUpper == "DOMAIN-KEYWORD" || firstUpper == "DOMAIN-REGEX" || firstUpper == "DOMAIN-WILDCARD" || firstUpper == "URL-REGEX" || firstUpper == "IP-CIDR" || firstUpper == "IP-CIDR6" || firstUpper == "USER-AGENT" || firstUpper == "PROCESS-NAME" || firstUpper == "DEST-PORT" {
		policy := ""
		if len(parts) > 0 {
			policy = strings.ToUpper(strings.TrimSpace(parts[len(parts)-1]))
		}
		if REJECT_POLICIES[policy] {
			if containsAny(low, []string{"unlock", "解锁", "vip", "premium", "crack"}) {
				return VerdictEnhance
			}
			return VerdictAd
		}
		if len(parts) > 0 {
			if firstUpper == "DOMAIN" || firstUpper == "DOMAIN-SUFFIX" || firstUpper == "DOMAIN-KEYWORD" || firstUpper == "DOMAIN-REGEX" || firstUpper == "DOMAIN-WILDCARD" || firstUpper == "URL-REGEX" || firstUpper == "IP-CIDR" || firstUpper == "IP-CIDR6" || firstUpper == "USER-AGENT" || firstUpper == "PROCESS-NAME" || firstUpper == "DEST-PORT" {
				if containsAny(low, []string{"unlock", "解锁", "vip", "premium", "crack"}) {
					return VerdictEnhance
				}
				return VerdictAd
			}
		}
		return VerdictEnhance
	}

	if containsAny(low, ENHANCE_SCRIPT_MARKERS) {
		return VerdictEnhance
	}
	if containsAny(low, AD_SCRIPT_MARKERS) {
		return VerdictAd
	}
	return VerdictNeutral
}

func ShouldKeepPromaxLine(line string, section string, sourcePath string) bool {
	verdict := ClassifyPromaxLine(line, section, sourcePath)
	if verdict == VerdictAd {
		return true
	}
	if verdict == VerdictEnhance {
		return false
	}
	sec := strings.ToLower(strings.TrimSpace(section))
	if sec == "mitm" && strings.Contains(strings.ToLower(line), "hostname") {
		return true
	}
	return false
}

func ModuleIngestMode(path string, text string) string {
	name := strings.ToLower(filepath.Base(path))
	if strings.Contains(name, "解锁") || strings.Contains(name, "unlock") {
		if !containsAny(name, []string{"去广告", "adblock", "anti-ad"}) {
			return "skip"
		}
	}

	adLines := 0
	enhanceLines := 0
	currentSection := ""

	lines := strings.Split(text, "\n")
	for _, raw := range lines {
		stripped := strings.TrimSpace(raw)
		if strings.HasPrefix(stripped, "[") && strings.HasSuffix(stripped, "]") {
			currentSection = stripped[1 : len(stripped)-1]
			continue
		}
		if stripped == "" || strings.HasPrefix(stripped, "#") {
			continue
		}
		v := ClassifyPromaxLine(stripped, currentSection, path)
		if v == VerdictAd {
			adLines++
		} else if v == VerdictEnhance {
			enhanceLines++
		}
	}

	if adLines == 0 {
		return "skip"
	}
	if enhanceLines > 0 {
		return "split"
	}
	return "full"
}
