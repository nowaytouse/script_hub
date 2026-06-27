package tools

import (
	"fmt"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

var (
	dohCnAlidns       = "https://dns.alidns.com/dns-query"
	dohCnVolcano      = "https://dns.volcengine.com/dns-query"
	dohCn360          = "https://doh.360.cn/dns-query"
	dohGoogle         = "https://dns.google/dns-query"
	dohCloudflare     = "https://cloudflare-dns.com/dns-query"
	dohControld       = "https://dns.controld.com/p2"
	dohMullvadAdblock = "https://adblock.dns.mullvad.net/dns-query"
	dohQuad9          = "https://dns.quad9.net/dns-query"
	dohTwTwnic        = "https://dns.twnic.tw/dns-query"
	dohHeOrdns        = "https://ordns.he.net/dns-query"
	dohNextdns        = "h3://doh-sg.blahdns.com/dns-query"
	dohNjalla         = "https://doh.njalla.fo/dns-query"

	traditionalCnGeneric = "119.29.29.29"
	traditionalCnAli     = "223.5.5.5"
	traditionalGlobal    = "1.1.1.1"

	dnsMappingDoh = map[string]string{
		"DNS_China_AliDNS":      dohCnAlidns,
		"DNS_China_ByteDance":   dohCnVolcano,
		"DNS_China_360":         dohCn360,
		"DNS_China_114":         traditionalCnGeneric,
		"DNS_China_114_manual":  traditionalCnGeneric,
		"DNS_Global_Google":     dohGoogle,
		"DNS_Global_Cloudflare": dohCloudflare,
		"DNS_Global_Microsoft":  dohControld,
		"DNS_Global_Apple":      "system",
		"DNS_Global_Social":     dohMullvadAdblock,
		"DNS_Global_Quad9":      dohQuad9,
	}

	surgeRulesetDoh = map[string]string{
		"NSFW":        dohNjalla,
		"SocialMedia": dohMullvadAdblock,
		"Bilibili":    traditionalCnAli,
		"Apple":       "system",
		"AppleNews":   "system",
		"Spotify":     traditionalGlobal,
		"Gaming":      traditionalGlobal,
		"StreamEU":    traditionalGlobal,
		"StreamHK":    traditionalGlobal,
		"StreamJP":    traditionalGlobal,
		"StreamKR":    traditionalGlobal,
		"StreamTW":    traditionalGlobal,
		"StreamUS":    traditionalGlobal,
		"GitHub":      dohCloudflare,
		"Google":      dohGoogle,
		"Microsoft":   dohControld,
	}

	telegramDcIps = map[string]bool{
		"91.108.56.100": true, "91.108.56.101": true, "91.108.56.104": true, "91.108.56.107": true,
		"91.108.56.120": true, "91.108.56.125": true, "91.108.56.126": true, "91.108.56.128": true,
		"91.108.56.156":  true,
		"149.154.175.10": true, "149.154.175.50": true, "149.154.175.54": true, "149.154.175.55": true,
		"149.154.175.56": true, "149.154.175.57": true, "149.154.175.100": true, "149.154.175.101": true,
		"149.154.175.102": true, "149.154.175.103": true, "149.154.175.117": true, "149.154.175.40": true,
		"91.108.4.0": true, "91.108.8.0": true, "91.108.12.0": true, "91.108.16.0": true,
		"149.154.167.0": true, "149.154.171.0": true, "149.154.163.0": true, "149.154.167.40": true,
	}

	telegramDomainMarkers = []string{
		"telegram.org", "telegram.me", "telegram.dog", "telegram.space",
		"telegram-cdn.org", "telegramdownload.com", "t.me", "telesco.pe",
	}

	ipv4HostRegex     = regexp.MustCompile(`^\d{1,3}(?:\.\d{1,3}){3}$`)
	validHostKeyRegex = regexp.MustCompile(`^(\*\.[a-zA-Z0-9_]([a-zA-Z0-9._-]*[a-zA-Z0-9_])?|[a-zA-Z0-9_]([a-zA-Z0-9_-]*[a-zA-Z0-9_])?(\.[a-zA-Z0-9_]([a-zA-Z0-9_-]*[a-zA-Z0-9_])?)*|[a-zA-Z0-9_]?\?([a-zA-Z0-9._?-]*)?)$`)
)

func isReservedAutoHost(key string) bool {
	if telegramDcIps[key] {
		return true
	}
	low := strings.TrimPrefix(strings.ToLower(key), "*.")
	for _, marker := range telegramDomainMarkers {
		if low == marker || strings.HasSuffix(low, "."+marker) || strings.Contains(low, marker) {
			return true
		}
	}
	return false
}

func isValidSurgeHostKey(key string) bool {
	if key == "" || len(key) > 253 {
		return false
	}
	if ipv4HostRegex.MatchString(key) {
		return true
	}
	lowered := strings.ToLower(key)
	if strings.ContainsAny(key, "/\\()") {
		return false
	}
	if strings.Contains(key, "..") || strings.HasSuffix(lowered, ".list") || strings.Contains(lowered, "ruleset") {
		return false
	}
	if regexp.MustCompile(`\?\?+`).MatchString(key) {
		return false
	}
	if strings.HasSuffix(key, ".*") || strings.HasSuffix(key, ".") || strings.HasPrefix(key, ".") {
		return false
	}
	if strings.Contains(key, "*") && !strings.HasPrefix(key, "*.") {
		return false
	}
	if strings.Contains(key, "?") {
		if strings.Count(key, "?") > 1 {
			return false
		}
		if !regexp.MustCompile(`^[a-zA-Z0-9_.?-]+$`).MatchString(key) {
			return false
		}
	} else if !validHostKeyRegex.MatchString(key) {
		return false
	}
	labels := strings.Split(key, ".")
	if strings.HasPrefix(key, "*.") {
		labels = strings.Split(key[2:], ".")
	}
	for _, label := range labels {
		if label == "" || len(label) > 63 {
			return false
		}
		if strings.HasPrefix(label, "-") || strings.HasSuffix(label, "-") {
			return false
		}
		if strings.Count(label, "?") > 1 {
			return false
		}
	}
	return true
}

func parseListFile(path string) [][]string {
	var result [][]string
	text := hub.ReadFileString(path)
	for _, raw := range strings.Split(text, "\n") {
		line := strings.TrimSpace(strings.Split(strings.Split(raw, "//")[0], "#")[0])
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		m := regexp.MustCompile(`(?i)^(DOMAIN(?:-SUFFIX|-KEYWORD)?),(.+)$`).FindStringSubmatch(line)
		if m != nil {
			result = append(result, []string{strings.ToUpper(m[1]), strings.TrimSpace(m[2])})
		}
	}
	return result
}

func hostKey(ruleType, domain string) string {
	if domain == "" || len(domain) > 253 {
		return ""
	}
	domain = strings.Trim(strings.TrimSpace(domain), `"'`)
	candidate := ""
	if ruleType == "DOMAIN-SUFFIX" {
		candidate = "*." + strings.TrimPrefix(domain, ".")
	} else if ruleType == "DOMAIN-KEYWORD" {
		return ""
	} else {
		candidate = domain
	}
	if isValidSurgeHostKey(candidate) {
		return candidate
	}
	return ""
}

func collectHosts(sources []map[string]string, seen map[string]bool) []string {
	var out []string
	for _, source := range sources {
		label, path, doh := source["label"], source["path"], source["doh"]
		if !hub.ValidateFileExists(path, "") {
			continue
		}
		var block []string
		count := 0
		for _, pair := range parseListFile(path) {
			ruleType, domain := pair[0], pair[1]
			key := hostKey(ruleType, domain)
			if key == "" || seen[key] || isReservedAutoHost(key) {
				continue
			}
			seen[key] = true
			block = append(block, fmt.Sprintf("%s = server:%s", key, doh))
			count++
		}
		if len(block) > 0 {
			out = append(out, fmt.Sprintf("# --- %s (%d hosts) → %s", label, count, doh))
			out = append(out, block...)
			out = append(out, "")
		}
	}
	return out
}

func bootstrapBlock() []string {
	return []string{
		"# SECTION A: DoH bootstrap (resolve provider hostnames without circular DoH)",
		"dns.google = 8.8.8.8, 8.8.4.4, 2001:4860:4860::8888, 2001:4860:4860::8844",
		"dns64.dns.google = 2001:4860:4860::6464, 2001:4860:4860::64",
		"cloudflare-dns.com = 104.16.249.249, 104.16.248.249, 2606:4700::6810:f8f9, 2606:4700::6810:f9f9",
		"1dot1dot1dot1.cloudflare-dns.com = 1.1.1.1, 1.0.0.1, 2606:4700:4700::1001, 2606:4700:4700::1111",
		"one.one.one.one = 1.1.1.1, 1.0.0.1, 2606:4700:4700::1001, 2606:4700:4700::1111",
		"dns.quad9.net = 9.9.9.9, 149.112.112.112, 2620:fe::fe, 2620:fe::9",
		"dns.alidns.com = 223.5.5.5, 223.6.6.6, 2400:3200:baba::1, 2400:3200::1",
		"doh.pub = 1.12.12.12, 120.53.53.53",
		"dns.pub = 1.12.12.12, 120.53.53.53",
		"doh.360.cn = 23.6.48.18, 112.65.69.15",
		"dns.baidu.com = 180.76.76.76, 110.242.68.66",
		"dns.twnic.tw = 101.101.101.101, 2001:de4::101",
		"ordns.he.net = 74.82.42.42, 2001:470:20::2",
		"dns.adguard.com = 94.140.14.14, 94.140.15.15",
		"doh.libredns.gr = 116.202.176.26",
		"doh.ffmuc.net = 5.1.66.255, 185.150.99.255, 2001:678:e68:f000::, 2001:678:ed0:f000::",
		"dns.mullvad.net = 194.242.2.2, 194.242.2.3",
		"adblock.dns.mullvad.net = 194.242.2.2",
		"freedns.controld.com = 76.76.2.0, 76.76.10.0",
		"dns.controld.com = 76.76.2.0, 76.76.10.0",
		"doh.dns.apple.com = 17.253.1.201, 17.253.1.202",
		"doh.tiar.app = 139.162.110.150",
		"doh.njalla.fo = 146.255.56.98",
		"dns.arapurayil.com = 185.95.218.42",
		"jp.blahdns.com = 185.150.99.255",
		"doh.ahadns.com = 2a09::, 2a09::1",
		"doh.applied-privacy.net = 2a02:1fb8:0:1::62",
		"dns.digitale-gesellschaft.ch = 2a05:dfc7:5::53",
		"dns.sudo.is = 2400:8902::f03c:91ff:fe06:787f",
		"dns.captnemo.in = 2606:1a40::, 2606:1a40:1::",
		"doh-pure.onedns.net = 117.50.11.11, 52.80.3.111",
		"wikimedia-dns.org = 185.71.138.138",
		"doh.dns4all.eu = 194.0.5.3",
		"dot.360.cn = 101.198.198.198, 101.198.199.200, 101.198.192.33, 112.65.69.15",
		"dns.cn = 1.2.4.8, 210.2.4.8, 2001:dc7:1000::1",
		"dns.tuna.tsinghua.edu.cn = 101.6.6.6, 2001:da8::666",
		"dns.volcengine.com = 180.184.1.1, 180.184.2.2, 2402:4e00:1020:1404::10, 2402:4e00:1430:1102::a",
		"dns6.cfiec.net = 240c:6666::6666, 240c:6644::6644",
		"raw.githubusercontent.com = 185.199.108.133, 185.199.109.133, 185.199.110.133, 185.199.111.133",
		"github.com = 140.82.113.4, 140.82.112.3",
		"",
		"# SECTION B: Pinned hosts (Telegram DC / FCM / proxy — must stay above bulk DoH)",
		"104.236.69.55 = server:1.1.1.1",
		"91.108.56.100 = 91.108.56.147,91.108.56.135,91.108.56.130",
		"91.108.56.101 = 91.108.56.147,91.108.56.135,91.108.56.130",
		"91.108.56.104 = 91.108.56.147,91.108.56.135,91.108.56.130",
		"91.108.56.107 = 91.108.56.147,91.108.56.135,91.108.56.130",
		"91.108.56.120 = 91.108.56.147,91.108.56.135,91.108.56.130",
		"91.108.56.125 = 91.108.56.147,91.108.56.135,91.108.56.130",
		"91.108.56.126 = 91.108.56.147,91.108.56.135,91.108.56.130",
		"91.108.56.128 = 91.108.56.147,91.108.56.135,91.108.56.130",
		"91.108.56.156 = 91.108.56.147,91.108.56.135,91.108.56.130",
		"149.154.175.10 = 149.154.175.53",
		"149.154.175.50 = 149.154.175.53",
		"149.154.175.54 = 149.154.175.53",
		"149.154.175.55 = 149.154.175.53",
		"149.154.175.56 = 149.154.175.53",
		"149.154.175.57 = 149.154.175.53",
		"149.154.175.100 = 149.154.175.53",
		"149.154.175.101 = 149.154.175.53",
		"149.154.175.102 = 149.154.175.53",
		"149.154.175.103 = 149.154.175.53",
		"149.154.175.117 = 149.154.175.53",
		"91.108.4.0 = 91.108.4.1",
		"91.108.8.0 = 91.108.8.1",
		"91.108.12.0 = 91.108.12.1",
		"91.108.16.0 = 91.108.16.1",
		"149.154.167.0 = 149.154.167.1",
		"149.154.171.0 = 149.154.171.1",
		"149.154.163.0 = 149.154.163.1",
		"149.154.167.40 = 149.154.167.41",
		"149.154.175.40 = 149.154.175.41",
		"talk.google.com = 108.177.125.188",
		"mtalk.google.com = 108.177.125.188, 2404:6800:4008:c07::bc, 142.250.31.188",
		"alt1-mtalk.google.com = 3.3.3.3, 2607:f8b0:4023:c0b::bc, 64.233.171.188",
		"alt2-mtalk.google.com = 3.3.3.3, 142.250.115.188",
		"alt3-mtalk.google.com = 74.125.200.188, 173.194.77.188",
		"alt4-mtalk.google.com = 74.125.200.188, 173.194.219.188",
		"alt5-mtalk.google.com = 3.3.3.3, 2607:f8b0:4023:1::bc, 142.250.112.188",
		"alt6-mtalk.google.com = 3.3.3.3, 172.217.197.188",
		"alt7-mtalk.google.com = 74.125.200.188, 2607:f8b0:4002:c03::bc, 108.177.12.188",
		"alt8-mtalk.google.com = 3.3.3.3",
		"stun.l.google.com = server:force-syslib",
		"stun?.l.google.com = server:force-syslib",
		"aws-linkhy15.liangxin1.xyz = 18.183.7.71",
		"*.liangxin1.xyz = server:system",
		"",
		"# SECTION C: Mainland China — DNS_mapping + TLD fallbacks",
		"*.cn = server:" + dohCnAlidns,
		"*.com.cn = server:" + dohCnAlidns,
		"*.net.cn = server:" + dohCnAlidns,
		"*.org.cn = server:" + dohCnAlidns,
		"*.gov.cn = server:" + dohCnAlidns,
		"*.edu.cn = server:" + dohCnAlidns,
		"",
		"# SECTION D: Taiwan / HK regional TLD & carriers",
		"*.cht.com.tw = server:" + dohTwTwnic,
		"*.hinet.net = server:" + dohTwTwnic,
		"*.emome.net = server:" + dohTwTwnic,
		"*.tw = server:" + dohTwTwnic,
		"*.taipei = server:" + dohTwTwnic,
		"*.hk = server:" + dohNextdns,
		"*.he.net = server:" + dohHeOrdns,
		"",
		"# SECTION E: rulesets/Sources/DNS_mapping (manual Host expansion)",
	}
}

func manualHostsBlock() []string {
	return []string{
		"# --- Manual DNS Mappings (from user request) ---",
		"freedns.controld.com = 76.76.10.2, 2606:1a40:1::2",
		"doh-sg.blahdns.com = 139.162.110.150, 2400:8902::f03c:91ff:fe06:787f",
		"doh.ffmuc.net = 185.150.99.255, 2001:678:ed0:f000::",
		"dns.mullvad.net = 194.242.2.3, 2a07:e340::3",
		"doh.libredns.gr = 116.202.176.26, 2a01:4f8:c2c:548f::1",
		"doh.njalla.fo = 95.215.19.53, 2001:67c:2354:2::53",
		"doh.applied-privacy.net = 146.255.56.98, 2a02:1b8:10:234::2",
		"dns.digitale-gesellschaft.ch = 185.95.218.42, 2a05:fc84::42",
		"adblock.dns.mullvad.net = 194.242.2.3, 2a07:e340::3",
		"dns.google = 8.8.8.8, 8.8.4.4, 2001:4860:4860::8888, 2001:4860:4860::8844",
		"cloudflare-dns.com = 1.1.1.1, 1.0.0.1, 2606:4700:4700::1111, 2606:4700:4700::1001",
		"dns.quad9.net = 9.9.9.9, 149.112.112.112, 2620:fe::fe, 2620:fe::9",
		"dns.alidns.com = 223.5.5.5, 223.6.6.6",
		"doh.pub = 1.12.12.12, 120.53.53.53",
		"",
	}
}

func tailBlock() []string {
	return []string{
		"",
		"# SECTION G: Inline / connectivity / NSFW exceptions",
		"hanime1.me = server:" + dohMullvadAdblock,
		"3hentai.net = server:" + dohMullvadAdblock,
		"18comic.vip = server:" + dohMullvadAdblock,
		"connectivitycheck.gstatic.com = server:" + dohNextdns,
		"detectportal.firefox.com = server:" + dohNextdns,
		"msftconnecttest.com = server:" + dohNextdns,
		"msftncsi.com = server:" + dohNextdns,
		"www.msftncsi.com = server:" + dohNextdns,
		"connectivitycheck.android.com = server:" + dohNextdns,
		"connectivity-check.ubuntu.com = server:" + dohNextdns,
		"connectivitycheck.platform.hicloud.com = server:" + dohCnAlidns,
		"",
		"# SECTION H: OCSP / certificate verification (system resolver)",
		"ocsp.digicert.cn = server:system",
		"ocsp.digicert.com = server:system",
		"crl3.digicert.com = server:system",
		"crl4.digicert.com = server:system",
		"ocsp.sectigo.com = server:system",
		"ocsp.verisign.com = server:system",
		"ocsp.globalsign.com = server:system",
		"ocsp.comodoca.com = server:system",
		"ocsp.entrust.net = server:system",
		"ocsp.identrust.com = server:system",
		"ocsp.pki.goog = server:system",
		"ocsp.apple.com = server:system",
		"ocsp2.apple.com = server:system",
		"ocsp-lb.apple.com.akadns.net = server:system",
		"",
		"# SECTION I: LAN / router admin / IPv6 literals",
		"ip6-localhost = ::1",
		"ip6-loopback = ::1",
		"ip6-localnet = fe00::0",
		"ip6-mcastprefix = ff00::0",
		"ip6-allnodes = ff02::1",
		"ip6-allrouters = ff02::2",
		"ip6-allhosts = ff02::3",
		"*.local = server:system",
		"*.lan = server:system",
		"*.test = server:system",
		"*.localhost = server:system",
		"*.localdomain = server:system",
		"_hotspot_.m2m = server:force-syslib",
		"hotspot.cslwifi.com = server:force-syslib",
		"*.id.ui.direct = server:force-syslib",
		"amplifi.lan = server:force-syslib",
		"router.synology.com = server:force-syslib",
		"sila.razer.com = server:force-syslib",
		"router.asus.com = server:force-syslib",
		"routerlogin.net = server:force-syslib",
		"orbilogin.com = server:force-syslib",
		"www.LinksysSmartWiFi.com = server:force-syslib",
		"LinksysSmartWiFi.com = server:force-syslib",
		"instant.arubanetworks.com = server:force-syslib",
		"setmeup.arubanetworks.com = server:force-syslib",
		"www.miwifi.com = server:force-syslib",
		"miwifi.com = server:force-syslib",
		"mediarouter.home = server:force-syslib",
		"tplogin.cn = server:force-syslib",
		"tplinklogin.net = server:force-syslib",
		"tplinkwifi.net = server:force-syslib",
		"melogin.cn = server:force-syslib",
		"falogin.cn = server:force-syslib",
		"tendawifi.com = server:force-syslib",
		"leike.cc = server:force-syslib",
		"zte.home = server:force-syslib",
		"p.to = server:force-syslib",
		"phicomm.me = server:force-syslib",
		"hiwifi.com = server:force-syslib",
		"peiluyou.com = server:force-syslib",
	}
}

func reserveKeysFromLines(seen map[string]bool, lines []string) {
	for _, line := range lines {
		if strings.Contains(line, " = ") && !strings.HasPrefix(strings.TrimSpace(line), "#") {
			key := strings.TrimSpace(strings.Split(line, " = ")[0])
			if isValidSurgeHostKey(key) {
				seen[key] = true
			}
		}
	}
	for k := range telegramDcIps {
		seen[k] = true
	}
}

func mergeHostLines(linesStr string) string {
	lines := strings.Split(linesStr, "\n")
	domainToIps := make(map[string][]string)
	domainToLineIdx := make(map[string]int)

	var outLines []string
	for _, line := range lines {
		if strings.Contains(line, " = ") && !strings.HasPrefix(strings.TrimSpace(line), "#") {
			parts := strings.SplitN(line, " = ", 2)
			domain := strings.TrimSpace(parts[0])
			ipsStr := strings.TrimSpace(parts[1])

			if strings.HasPrefix(ipsStr, "server:") {
				if _, ok := domainToIps[domain]; !ok {
					domainToIps[domain] = []string{ipsStr}
					domainToLineIdx[domain] = len(outLines)
				} else {
					outLines = append(outLines, "")
					continue
				}
			} else {
				ips := strings.Split(ipsStr, ",")
				for i := range ips {
					ips[i] = strings.TrimSpace(ips[i])
				}
				if existing, ok := domainToIps[domain]; ok {
					if !strings.HasPrefix(existing[0], "server:") {
						for _, ip := range ips {
							found := false
							for _, e := range domainToIps[domain] {
								if e == ip {
									found = true
									break
								}
							}
							if !found {
								domainToIps[domain] = append(domainToIps[domain], ip)
							}
						}
						firstIdx := domainToLineIdx[domain]
						outLines[firstIdx] = fmt.Sprintf("%s = %s", domain, strings.Join(domainToIps[domain], ", "))
					}
					outLines = append(outLines, "")
					continue
				} else {
					domainToIps[domain] = ips
					domainToLineIdx[domain] = len(outLines)
				}
			}
		}
		outLines = append(outLines, line)
	}

	var result []string
	for _, line := range outLines {
		if line == "" && len(result) > 0 && result[len(result)-1] == "" {
			continue
		}
		result = append(result, line)
	}
	return strings.Join(result, "\n")
}

func BuildHostSection() string {
	seen := make(map[string]bool)
	reserveKeysFromLines(seen, manualHostsBlock())
	reserveKeysFromLines(seen, bootstrapBlock())
	reserveKeysFromLines(seen, tailBlock())

	var lines []string
	lines = append(lines,
		"# Surge [Host] DNS steering — auto-generated by scripts/generate_surge_host_dns.py",
		"# DoH pool mirrors [General] dns-server + encrypted-dns-server in NyaMiiKo.conf",
		"# Regenerate: python3 scripts/generate_surge_host_dns.py --write",
		"",
	)
	lines = append(lines, manualHostsBlock()...)
	lines = append(lines, bootstrapBlock()...)

	var dnsSources []map[string]string
	dnsDir := filepath.Join(hub.ROOT, "rulesets/Sources/dns/mapping")
	for name, doh := range dnsMappingDoh {
		dnsSources = append(dnsSources, map[string]string{
			"label": name,
			"path":  filepath.Join(dnsDir, name+".list"),
			"doh":   doh,
		})
	}
	lines = append(lines, collectHosts(dnsSources, seen)...)
	lines = append(lines, "", "# SECTION F: Rule-aligned Surge rulesets (from Surge DOMAIN entries)", "# GlobalProxy.list omitted (~37k) — use FINAL proxy group + encrypted-dns pool")

	var surgeSources []map[string]string
	surgeRulesetDir := filepath.Join(hub.ROOT, "rulesets/list")
	for name, doh := range surgeRulesetDoh {
		surgeSources = append(surgeSources, map[string]string{
			"label": "Surge/" + name,
			"path":  filepath.Join(surgeRulesetDir, name+".list"),
			"doh":   doh,
		})
	}
	lines = append(lines, collectHosts(surgeSources, seen)...)
	lines = append(lines, tailBlock()...)

	return mergeHostLines(strings.TrimRight(strings.Join(lines, "\n"), "\n") + "\n")
}

func ReplaceHostSection(confPath string, hostBody string) error {
	text := hub.ReadFileString(confPath)
	re := regexp.MustCompile(`(?ms)^\[Host\]\n.*?(?:^\[[^\n]+\]\n|\z)`)
	if !re.MatchString(text) {
		return fmt.Errorf("no [Host] section in %s", confPath)
	}

	newText := re.ReplaceAllString(text, "[Host]\n"+hostBody+"\n")

	backupPath := confPath + ".backup"
	hub.SafeWriteFile(backupPath, text, true)

	tmpPath := confPath + ".tmp"
	hub.SafeWriteFile(tmpPath, newText, true)
	if err := hub.SafeWriteFile(confPath, newText, true); err != nil {
		if hub.ValidateFileExists(backupPath, "") {
			hub.SafeWriteFile(confPath, text, true)
		}
		return fmt.Errorf("failed to update %s", confPath)
	}
	fmt.Printf("✅ Updated %s (backup: %s)\n", confPath, backupPath)
	return nil
}

func RunGenerateSurgeHostDns(args map[string]string) int {
	_, write := args["write"]
	outputPath := args["output"]
	if outputPath == "" {
		outputPath = filepath.Join(hub.ROOT, ".claude", "generated_host_dns.conf")
	}

	body := BuildHostSection()

	hub.SafeWriteFile(outputPath, body, true)
	fmt.Printf("✅ Generated %s\n", outputPath)
	fmt.Printf("Wrote fragment (%d lines): %s\n", strings.Count(body, "\n"), outputPath)

	if write {
		target := filepath.Join(hub.ROOT, "modules/surge/head_expanse", "🌟 AdBlock Helper .sgmodule")
		if err := ReplaceHostSection(target, body); err != nil {
			fmt.Println(err)
			return 1
		}
		fmt.Printf("Updated [Host] in %s\n", target)
	}

	return 0
}
