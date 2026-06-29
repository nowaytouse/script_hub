package tools

import (
	"encoding/json"
	"fmt"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/nyamiiko/script_hub/create/hub"
)

func getSectionBody(text, section string) string {
	re := regexp.MustCompile(fmt.Sprintf(`(?ms)^\[%s\]\n(.*?)(?:^\[[^\n]+\]\n|\z)`, regexp.QuoteMeta(section)))
	match := re.FindStringSubmatch(text)
	if len(match) > 1 {
		return match[1]
	}
	return ""
}

func replaceSectionBody(text, section, newBody string) string {
	re := regexp.MustCompile(fmt.Sprintf(`(?ms)(^\[%s\]\n)(.*?)(?:^\[[^\n]+\]\n|\z)`, regexp.QuoteMeta(section)))
	match := re.FindStringSubmatchIndex(text)
	if match == nil {
		return text
	}
	return text[:match[2]] + strings.TrimRight(newBody, "\n") + "\n\n" + text[match[5]:]
}

func getGeneralValue(text, key string) string {
	body := getSectionBody(text, "General")
	for _, line := range strings.Split(body, "\n") {
		m := regexp.MustCompile(`^(\s*)([A-Za-z0-9_-]+)(\s*=\s*)(.*)$`).FindStringSubmatch(line)
		if len(m) > 4 && m[2] == key {
			return m[4]
		}
	}
	return ""
}

func updateShadowrocketGeneral(text, surgeText string) string {
	generalBody := getSectionBody(text, "General")
	ipv6 := getGeneralValue(surgeText, "ipv6")
	if ipv6 == "" {
		ipv6 = "true"
	}
	hijackDns := getGeneralValue(surgeText, "hijack-dns")
	if hijackDns == "" {
		hijackDns = "*:53"
	}

	replacements := map[string]string{
		"ipv6":                      ipv6,
		"prefer-ipv6":               "true",
		"hijack-dns":                hijackDns,
		"dns-server":                "223.5.5.5, 1.1.1.1, system",
		"doh-server":                "https://cloudflare-dns.com/dns-query, https://dns.google/dns-query, https://dns.quad9.net/dns-query",
		"dns-direct-fallback-proxy": "false",
	}

	seen := make(map[string]bool)
	var outLines []string

	for _, line := range strings.Split(generalBody, "\n") {
		m := regexp.MustCompile(`^(\s*)([A-Za-z0-9_-]+)(\s*=\s*)(.*)$`).FindStringSubmatch(line)
		if len(m) > 0 {
			indent, key, sep := m[1], m[2], m[3]
			if val, ok := replacements[key]; ok {
				outLines = append(outLines, fmt.Sprintf("%s%s%s%s", indent, key, sep, val))
				seen[key] = true
			} else {
				outLines = append(outLines, line)
			}
		} else {
			outLines = append(outLines, line)
		}
	}

	var missingLines []string
	keys := []string{"ipv6", "prefer-ipv6", "hijack-dns", "dns-server", "doh-server", "dns-direct-fallback-proxy"}
	for _, k := range keys {
		if !seen[k] {
			missingLines = append(missingLines, fmt.Sprintf("%s = %s", k, replacements[k]))
		}
	}

	if len(missingLines) > 0 {
		if len(outLines) > 0 && outLines[len(outLines)-1] != "" {
			outLines = append(outLines, "")
		}
		outLines = append(outLines, missingLines...)
	}

	return replaceSectionBody(text, "General", strings.Join(outLines, "\n"))
}

func buildSingboxDns() map[string]interface{} {
	providerDomains := []string{
		"dns.google", "dns.cloudflare.com", "dns.quad9.net", "dns.adguard-dns.com",
		"dns.adguard.com", "doh-sg.blahdns.com/dns-query", "doh.dns.apple.com", "dns.twnic.tw",
		"wikimedia-dns.org", "adblock.dns.mullvad.net", "doh.pub", "dns.pub", "doh.360.cn", "dns.alidns.com",
	}
	httpdnsDomains := []string{
		"httpdns.alicdn.com", "httpdns-api.aliyuncs.com", "httpdns-sc.aliyuncs.com",
		"httpsdns.baidu.com", "httpdns.baidu.com", "httpdns.baidubce.com", "httpdns.bilivideo.com",
		"httpdns.calorietech.com", "httpdns.meituan.com", "httpdnsvip.meituan.com", "httpdns.yunxindns.com",
		"httpdns.n.netease.com", "httpdns.music.163.com", "music.httpdns.c.163.com",
		"lofter.httpdns.c.163.com", "httpdns.push.oppomobile.com", "httpdns.volcengineapi.com",
		"dns.weibo.cn", "dns.weixin.qq.com", "dns.weixin.qq.com.cn", "httpdns.c.cdnhwc2.com", "dns.jd.com",
	}

	httpsServer := func(tag, server, detour, serverName string) map[string]interface{} {
		tls := map[string]interface{}{
			"enabled":           true,
			"min_version":       "1.3",
			"max_version":       "1.3",
			"curve_preferences": []string{"x25519", "p256", "p384"},
			"alpn":              []string{"h2", "http/1.1"},
		}
		if serverName != "" {
			tls["server_name"] = serverName
		}
		return map[string]interface{}{
			"tag":    tag,
			"type":   "https",
			"server": server,
			"detour": detour,
			"tls":    tls,
		}
	}

	return map[string]interface{}{
		"strategy":          "prefer_ipv4",
		"disable_cache":     false,
		"disable_expire":    false,
		"independent_cache": true,
		"reverse_mapping":   true,
		"servers": []interface{}{
			map[string]interface{}{
				"tag":         "fake_dns",
				"type":        "fakeip",
				"inet4_range": "28.0.0.0/8",
				"inet6_range": "fc00::/18",
			},
			map[string]interface{}{
				"tag":       "local_dns",
				"type":      "local",
				"prefer_go": true,
			},
			httpsServer("aliyun_dns", "223.5.5.5", "direct-select", "dns.alidns.com"),
			httpsServer("dnspod_dns", "1.12.12.12", "direct-select", "doh.pub"),
			httpsServer("dns_360_dns", "101.198.198.198", "direct-select", "doh.360.cn"),
			httpsServer("cloudflare_dns", "1.1.1.1", "🌍 海外通用 🌍", "cloudflare-dns.com"),
			httpsServer("google_dns", "8.8.8.8", "🌍 海外通用 🌍", "dns.google"),
			httpsServer("quad9_dns", "9.9.9.9", "🌍 海外通用 🌍", "dns.quad9.net"),
			httpsServer("apple_dns", "17.253.14.125", "🌍 海外通用 🌍", "doh.dns.apple.com"),
			httpsServer("twnic_dns", "101.101.101.101", "🌍 海外通用 🌍", "dns.twnic.tw"),
			httpsServer("mullvad_adblock", "194.242.2.2", "🌍 海外通用 🌍", "adblock.dns.mullvad.net"),
			httpsServer("adguard_dns", "94.140.14.14", "🌍 海外通用 🌍", "dns.adguard-dns.com"),
			httpsServer("wikimedia_dns", "185.71.138.138", "🌍 海外通用 🌍", "wikimedia-dns.org"),
		},
		"rules": []interface{}{
			map[string]interface{}{"clash_mode": "🎯 直连模式 🎯", "server": "aliyun_dns"},
			map[string]interface{}{"clash_mode": "🌍 全局模式 🌍", "server": "cloudflare_dns"},
			map[string]interface{}{"clash_mode": "📋 规则模式 📋", "server": "cloudflare_dns"},
			map[string]interface{}{"domain": providerDomains, "server": "local_dns"},
			map[string]interface{}{"domain_suffix": []string{".local", ".localhost", ".lan", ".test", ".localdomain"}, "server": "local_dns"},
			map[string]interface{}{"domain_suffix": []string{"httpdns.pro"}, "action": "reject"},
			map[string]interface{}{"domain": httpdnsDomains, "action": "reject"},
			map[string]interface{}{"rule_set": "surge-chinadirect", "server": "dnspod_dns"},
			map[string]interface{}{"domain_suffix": []string{".cn", ".gov", ".edu", ".mil", ".int", ".arpa"}, "server": "aliyun_dns"},
			map[string]interface{}{"rule_set": "surge-streamjp", "server": "google_dns"},
			map[string]interface{}{"rule_set": "surge-streamkr", "server": "google_dns"},
			map[string]interface{}{"rule_set": "surge-cdn", "server": "cloudflare_dns"},
			map[string]interface{}{"rule_set": "surge-substore", "server": "quad9_dns"},
			map[string]interface{}{"rule_set": "surge-streamhk", "server": "google_dns"},
			map[string]interface{}{"rule_set": "surge-streamtw", "server": "twnic_dns"},
			map[string]interface{}{"rule_set": "surge-cloudflare", "server": "cloudflare_dns"},
			map[string]interface{}{"rule_set": "surge-streamus", "server": "google_dns"},
			map[string]interface{}{"rule_set": "surge-applenews", "server": "apple_dns"},
			map[string]interface{}{"rule_set": "surge-streameu", "server": "quad9_dns"},
			map[string]interface{}{"rule_set": "surge-spotify", "server": "cloudflare_dns"},
			map[string]interface{}{"rule_set": "surge-socialmedia", "server": "cloudflare_dns"},
			map[string]interface{}{"rule_set": "surge-nsfw", "server": "mullvad_adblock"},
			map[string]interface{}{"rule_set": "surge-github", "server": "google_dns"},
			map[string]interface{}{"rule_set": "surge-google", "server": "google_dns"},
			map[string]interface{}{"rule_set": "surge-microsoft", "server": "quad9_dns"},
			map[string]interface{}{"rule_set": "surge-ai", "server": "cloudflare_dns"},
			map[string]interface{}{"rule_set": "surge-binance", "server": "cloudflare_dns"},
			map[string]interface{}{"rule_set": "surge-globalproxy", "server": "quad9_dns"},
			map[string]interface{}{"domain_suffix": []string{"cht.com.tw", "hinet.net", "emome.net", "tw", "taipei"}, "server": "twnic_dns"},
			map[string]interface{}{"domain_suffix": []string{"he.net"}, "server": "quad9_dns"},
			map[string]interface{}{"domain": []string{"connectivitycheck.gstatic.com", "msftconnecttest.com", "msftncsi.com", "www.msftncsi.com", "connectivitycheck.android.com"}, "server": "google_dns"},
			map[string]interface{}{"domain": []string{"detectportal.firefox.com", "connectivity-check.ubuntu.com"}, "server": "cloudflare_dns"},
			map[string]interface{}{"domain": []string{"connectivitycheck.platform.hicloud.com"}, "server": "aliyun_dns"},
			map[string]interface{}{"domain": []string{"hanime1.me", "3hentai.net", "18comic.vip"}, "server": "mullvad_adblock"},
			map[string]interface{}{"domain": []string{"stun.l.google.com"}, "server": "local_dns"},
			map[string]interface{}{"domain_suffix": []string{".l.google.com"}, "server": "local_dns"},
		},
		"final":  "cloudflare_dns",
		"fakeip": map[string]interface{}{},
	}
}

func replaceTopLevelObject(text, key string, newObject map[string]interface{}) string {
	keyPattern := regexp.MustCompile(fmt.Sprintf(`(?m)^  "%s":\s*\{`, regexp.QuoteMeta(key)))
	match := keyPattern.FindStringIndex(text)
	if match == nil {
		fmt.Printf("Missing top-level \"%s\" object\n", key)
		return text
	}

	objectStart := strings.Index(text[match[0]:], "{")
	if objectStart == -1 {
		return text
	}
	objectStart += match[0]

	inString := false
	escaped := false
	depth := 0
	objectEnd := -1

	for idx := objectStart; idx < len(text); idx++ {
		ch := text[idx]
		if inString {
			if escaped {
				escaped = false
			} else if ch == '\\' {
				escaped = true
			} else if ch == '"' {
				inString = false
			}
			continue
		}

		if ch == '"' {
			inString = true
		} else if ch == '{' {
			depth++
		} else if ch == '}' {
			depth--
			if depth == 0 {
				objectEnd = idx
				break
			}
		}
	}

	if objectEnd == -1 {
		return text
	}

	blockStart := match[0]
	blockEnd := objectEnd + 1
	if blockEnd < len(text) && text[blockEnd] == ',' {
		blockEnd++
	}

	j, _ := json.MarshalIndent(newObject, "  ", "  ")
	newBlock := fmt.Sprintf("  \"%s\": ", key) + strings.ReplaceAll(string(j), "\n", "\n  ")
	if blockEnd <= len(text) && text[blockEnd-1] == ',' {
		newBlock += ","
	}

	return text[:blockStart] + newBlock + text[blockEnd:]
}

func syncShadowrocket(surgeText string) {
	srPath := filepath.Join(hub.ROOT, ".claude/shadowroket.conf")
	if hub.ValidateFileExists(srPath, "") {
		srText := hub.ReadFileString(srPath)
		srText = updateShadowrocketGeneral(srText, surgeText)
		hub.SafeWriteFile(srPath, srText, true)
	}
}

func syncSingbox() {
	sbPath := filepath.Join(hub.ROOT, "scripts/substore/Singbox1.13.0+.conf")
	if hub.ValidateFileExists(sbPath, "") {
		sbText := hub.ReadFileString(sbPath)
		sbText = replaceTopLevelObject(sbText, "dns", buildSingboxDns())
		sbText = regexp.MustCompile(`(?m)^    "default_domain_resolver": "[^"]+",?$`).ReplaceAllString(sbText, `    "default_domain_resolver": "cloudflare_dns",`)
		hub.SafeWriteFile(sbPath, sbText, true)
	}
}

func RunSyncClaudeDnsTemplates() int {
	surgePath := filepath.Join(hub.ROOT, ".claude/NyaMiiKo.conf.conf")
	if !hub.ValidateFileExists(surgePath, "") {
		fmt.Println("Missing NyaMiiKo.conf.conf")
		return 1
	}
	surgeText := hub.ReadFileString(surgePath)
	syncShadowrocket(surgeText)
	syncSingbox()
	return 0
}
