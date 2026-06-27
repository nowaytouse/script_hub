package pipeline

import (
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

var (
	rulesetDirs = []string{
		hub.RULE_SET_DIR,
		hub.ADBLOCK_DIR,
	}

	priorityOrder = []string{
		"AdBlock_",
		"AdBlock.list",
		"reject-drop.list",
		"reject-no-drop.list",
		"HTTPDNS_Hijack.list",
		"AI.list",
		"AI_ip.list",
		"SocialMedia.list",
		"SocialMedia_ip.list",
		"NSFW.list",
		"NSFW_ip.list",
		"substore.list",
		"substore_ip.list",
		"AppleNews.list",
		"AppleNews_ip.list",
		"StreamUS.list",
		"StreamUS_ip.list",
		"StreamJP.list",
		"StreamJP_ip.list",
		"StreamKR.list",
		"StreamKR_ip.list",
		"StreamEU.list",
		"StreamEU_ip.list",
		"StreamHK.list",
		"StreamHK_ip.list",
		"StreamTW.list",
		"StreamTW_ip.list",
		"Spotify.list",
		"Spotify_ip.list",
		"Gaming.list",
		"Gaming_ip.list",
		"Google.list",
		"Google_ip.list",
		"Apple.list",
		"Apple_ip.list",
		"Microsoft.list",
		"Microsoft_ip.list",
		"GitHub.list",
		"GitHub_ip.list",
		"PayPal.list",
		"PayPal_ip.list",
		"Binance.list",
		"Binance_ip.list",
		"Cloudflare.list",
		"Cloudflare_ip.list",
		"Bilibili.list",
		"Bilibili_ip.list",
		"CDN.list",
		"CDN_ip.list",
		"Direct.list",
		"Direct_ip.list",
		"GlobalProxy.list",
		"GlobalProxy_ip.list",
	}

	mandatoryProxyKeywords = []string{
		"google", "gstatic", "gmail", "ggpht", "youtube", "ytimg",
		"facebook", "fbcdn", "instagram", "twitter", "twimg", "t.co",
		"telegram", "netflix", "nflxvideo", "nflxext", "disney", "github",
		"akamai", "fastly", "cloudflare",
		"byteoversea", "tiktok", "tiktokv", "tiktokcdn", "tiktokeu",
		"tiktokw", "muscdn", "tiktokrow", "lark.com",
	}

	adblockOnlyPrefixes = []string{
		"AdBlock_", "AdBlock.list",
	}

	nsfwContaminationKeywords = []string{
		"porn", "hentai", "nsfw", "xxx", "adult", "sex", "erotic",
		"nude", "naked", "fetish", "bdsm", "onlyfans",
		"facebookporn", "facebookofsex", "facboox", "faceboox", "faseboox", "feceboox",
	}

	typeRank = map[string]int{
		"DOMAIN":          0,
		"DOMAIN-SUFFIX":   1,
		"DOMAIN-KEYWORD":  2,
		"DOMAIN-WILDCARD": 3,
		"DOMAIN-REGEX":    4,
		"IP-CIDR":         5,
		"IP-CIDR6":        5,
		"GEOIP":           6,
		"PROCESS-NAME":    7,
		"USER-AGENT":      8,
		"URL-REGEX":       9,
	}

	semanticCoverageRulesets = map[string]bool{
		"AdBlock_":         true,
		"AdBlock.list":     true,
		"AI.list":          true,
		"SocialMedia.list": true,
		"NSFW.list":        true,
		"StreamUS.list":    true,
		"StreamJP.list":    true,
		"StreamKR.list":    true,
		"StreamEU.list":    true,
		"StreamHK.list":    true,
		"StreamTW.list":    true,
		"Spotify.list":     true,
		"Gaming.list":      true,
		"Google.list":      true,
		"Apple.list":       true,
		"AppleNews.list":   true,
		"Microsoft.list":   true,
		"GitHub.list":      true,
		"PayPal.list":      true,
		"Binance.list":     true,
		"Cloudflare.list":  true,
		"Bilibili.list":    true,
	}

	semanticTypes = map[string]bool{"DOMAIN": true, "DOMAIN-SUFFIX": true}
)

func isValidRule(line string) bool {
	if strings.HasPrefix(line, "RULE-SET") {
		return false
	}
	if strings.HasPrefix(line, "DOMAIN") && strings.Contains(line, ",no-resolve") {
		return false
	}
	return strings.Contains(line, ",")
}

func cleanRule(line string) string {
	line = hub.StripInlineComment(strings.TrimSpace(line))
	if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, "//") {
		return ""
	}
	if strings.HasPrefix(line, "DOMAIN") && strings.Contains(line, ",no-resolve") {
		line = strings.ReplaceAll(line, ",no-resolve", "")
	}
	return line
}

func extractPayload(rule string) string {
	if !strings.Contains(rule, ",") {
		return ""
	}
	parts := strings.SplitN(rule, ",", 3)
	if len(parts) >= 2 {
		return strings.ToLower(strings.TrimSpace(parts[1]))
	}
	return ""
}

func extractType(rule string) string {
	parts := strings.SplitN(rule, ",", 2)
	return strings.ToUpper(strings.TrimSpace(parts[0]))
}

func getRank(ruleType string) int {
	if rank, ok := typeRank[ruleType]; ok {
		return rank
	}
	return 999
}

func preferRule(current, candidate string) string {
	currentRank := getRank(extractType(current))
	candidateRank := getRank(extractType(candidate))
	if candidateRank < currentRank {
		return candidate
	}
	if candidateRank == currentRank && candidate < current {
		return candidate
	}
	return current
}

func isProxyishDomain(rule string) bool {
	payload := extractPayload(rule)
	if payload == "" {
		return false
	}
	rtype := extractType(rule)
	validTypes := map[string]bool{
		"DOMAIN":          true,
		"DOMAIN-SUFFIX":   true,
		"DOMAIN-KEYWORD":  true,
		"DOMAIN-WILDCARD": true,
		"DOMAIN-REGEX":    true,
	}
	if !validTypes[rtype] {
		return false
	}
	for _, kw := range mandatoryProxyKeywords {
		if strings.Contains(payload, kw) {
			return true
		}
	}
	return false
}

func isCnTld(rule string) bool {
	payload := extractPayload(rule)
	if payload == "" {
		return false
	}
	rtype := extractType(rule)
	if rtype != "DOMAIN" && rtype != "DOMAIN-SUFFIX" {
		return false
	}
	return payload == "cn" || strings.HasSuffix(payload, ".cn")
}

func iterDomainParents(payload string) []string {
	parts := strings.Split(payload, ".")
	if len(parts) < 2 {
		return []string{payload}
	}
	var res []string
	for i := 0; i < len(parts)-1; i++ {
		res = append(res, strings.Join(parts[i:], "."))
	}
	return res
}
