package tools

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"

	"github.com/nowaytouse/script_hub/create/hub"
)

var (
	surgeOnlyKeys = map[string]bool{
		"use-local-host-item-for-proxy": true, "encrypted-dns-follow-outbound-mode": true,
		"encrypted-dns-skip-cert-verification": true, "force-http-engine-hosts": true,
		"always-raw-tcp-hosts": true, "always-raw-tcp-keywords": true, "tun-included-routes": true,
		"compatibility-mode": true, "http-api-tls": true, "http-api-web-dashboard": true,
		"allow-wifi-access": true, "allow-hotspot-access": true, "wifi-access-http-port": true,
		"wifi-access-socks5-port": true, "proxy-restricted-to-lan": true, "block-quic": true,
		"exclude-simple-hostnames": true, "read-etc-hosts": true, "include-apns": true,
		"include-cellular-services": true, "wifi-assist": true, "allow-dns-svcb": true,
		"ipv6-vif": true, "include-all-networks": true, "include-local-networks": true,
		"auto-suspend": true, "all-hybrid": true, "http-listen": true, "socks5-listen": true,
		"proxy-test-udp": true,
	}

	generalKeyMap = map[string]string{
		"encrypted-dns-server": "dns-server",
		"dns-server":           "fallback-dns-server",
		"tun-excluded-routes":  "tun-excluded-routes",
	}

	ruleReplacements = map[string]string{
		"REJECT-DROP": "REJECT", "REJECT-TINYGIF": "REJECT", "REJECT-NO-DROP": "REJECT",
	}

	preserveRulesetMarkers = []string{
		"/rulesets/AdBlock/AdBlock_",
		"ruleset%2FAdBlock%2FAdBlock_",
		"skk_upstream/",
		"HTTPDNS_Hijack.list",
		"nowaytouse/script_hub@master/rulesets/",
	}

	rewriteModifierRe = regexp.MustCompile(`,\s*(extended-matching|pre-matching)\b|\b(extended-matching|pre-matching)\s*,?`)

	srGeneralDefaults = `bypass-system = true
ipv6 = true
prefer-ipv6 = true
hijack-dns = *:53
dns-direct-fallback-proxy = false`

	hostKeepPatterns = []string{
		`^dns\.`, `^doh\.`, `^cloudflare`, `\.google\.com$`, `^talk\.google`, `^mtalk\.google`,
		`^alt\d+-mtalk`, `^stun`, `^connectivitycheck`, `^detectportal`, `^msftconnecttest`, `^msftncsi`,
		`^\*\.cn$`, `^\*\.tw$`, `^\*\.cht\.com\.tw$`, `^\*\.hinet\.net$`, `^\*\.he\.net$`,
		`^hanime1\.me$`, `^18comic\.vip$`, `^3hentai\.net$`, `^router\.`, `^miwifi\.`, `^tplogin\.`,
		`\.liangxin1\.xyz$`, `^raw\.githubusercontent`, `^github\.com$`, `^\d+\.\d+\.\d+\.\d+$`,
	}

	promaxSurgeModules = []string{
		"🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
		"📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule",
	}

	rulesetCache = make(map[string][]string)
)

func shouldKeepHostLine(line string) bool {
	stripped := strings.TrimSpace(line)
	if stripped == "" || strings.HasPrefix(stripped, "#") {
		return true
	}
	if strings.Contains(stripped, "=") {
		domain := strings.TrimSpace(strings.Split(stripped, "=")[0])
		for _, pattern := range hostKeepPatterns {
			re := regexp.MustCompile("(?i)" + pattern)
			if re.MatchString(domain) {
				return true
			}
		}
	}
	return false
}

func fetchRuleset(urlOrPath string) []string {
	if cached, ok := rulesetCache[urlOrPath]; ok {
		return cached
	}
	if len(rulesetCache) >= 100 {
		rulesetCache = make(map[string][]string)
	}

	var content string

	cdnPatterns := []string{
		"fastly.jsdelivr.net/gh/nowaytouse/script_hub@master/",
		"cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/",
		"gcore.jsdelivr.net/gh/nowaytouse/script_hub@master/",
	}

	for _, cdn := range cdnPatterns {
		if strings.Contains(urlOrPath, cdn) {
			parts := strings.Split(urlOrPath, "@master/")
			if len(parts) == 2 {
				localPath := filepath.Join(hub.ROOT, parts[1])
				if hub.ValidateFileExists(localPath, "") {
					content = hub.ReadFileString(localPath)
					break
				}
			}
		}
	}

	if content == "" && strings.HasPrefix(urlOrPath, "..") {
		localPath := filepath.Join(hub.ROOT, "modules/surge/amplify_nexus", urlOrPath)
		if hub.ValidateFileExists(localPath, "") {
			content = hub.ReadFileString(localPath)
		}
	}

	if content == "" && strings.HasPrefix(urlOrPath, "http") {
		// Download with retries
		content = hub.SafeDownload(urlOrPath, 3, 10)
	}

	if content == "" {
		return nil
	}

	var rules []string
	for _, line := range strings.Split(content, "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, "//") || strings.HasPrefix(line, ";") {
			continue
		}
		parts := strings.Split(line, ",")
		if len(parts) == 0 {
			continue
		}
		rtype := strings.ToUpper(parts[0])
		if rtype == "DOMAIN" || rtype == "DOMAIN-SUFFIX" || rtype == "DOMAIN-KEYWORD" || rtype == "IP-CIDR" || rtype == "IP-CIDR6" || rtype == "GEOIP" {
			if len(parts) >= 2 {
				rules = append(rules, parts[0]+","+parts[1])
			}
		}
	}

	rulesetCache[urlOrPath] = rules
	return rules
}

func convertContent(content string, moduleStem string) string {
	if moduleStem == "" {
		meta, _ := hub.ParseModule(content)
		moduleStem = hub.ModuleStemFromMeta(meta, "")
	}

	lines := strings.Split(content, "\n")
	var out []string
	var section string
	generalDefaultsAdded := false

	for _, line := range lines {
		stripped := strings.TrimSpace(line)

		if strings.HasPrefix(stripped, "[") && strings.HasSuffix(stripped, "]") {
			section = stripped[1 : len(stripped)-1]
			out = append(out, line)
			if section == "General" && !generalDefaultsAdded {
				out = append(out, srGeneralDefaults)
				generalDefaultsAdded = true
			}
			continue
		}

		if strings.HasPrefix(line, "#!") {
			if regexp.MustCompile(`(?i)^#!\s*(update-interval|ability)\s*=`).MatchString(line) {
				continue
			}
			m := regexp.MustCompile(`^#!\s*(\S+?)\s*=\s*(.*)$`).FindStringSubmatch(line)
			if m != nil {
				key, val := strings.TrimSpace(m[1]), m[2]
				if key == "desc" && !strings.Contains(val, "[🚀SR]") {
					val = "[🚀SR] " + val
				}
				out = append(out, fmt.Sprintf("#!%s=%s", key, val))
			} else {
				out = append(out, line)
			}
			continue
		}

		if section == "General" && !strings.HasPrefix(stripped, "#") && stripped != "" {
			hasSurgeKey := false
			for k := range surgeOnlyKeys {
				if strings.Contains(line, k) {
					hasSurgeKey = true
					break
				}
			}
			if hasSurgeKey {
				out = append(out, "# [SR does not support ] "+strings.TrimLeft(line, " \t"))
				continue
			}
			for k, v := range generalKeyMap {
				if strings.HasPrefix(line, k) {
					line = strings.Replace(line, k, v, 1)
				}
			}
			out = append(out, line)
			continue
		}

		if section == "Rule" && !strings.HasPrefix(stripped, "#") && stripped != "" {
			if strings.HasPrefix(stripped, "RULE-SET,") {
				parts := strings.Split(stripped, ",")
				urlOrPath := strings.TrimSpace(parts[1])
				policy := "REJECT"
				if len(parts) > 2 {
					policy = strings.TrimSpace(parts[2])
				}
				if repl, ok := ruleReplacements[policy]; ok {
					policy = repl
				}

				preserve := false
				for _, marker := range preserveRulesetMarkers {
					if strings.Contains(urlOrPath, marker) {
						preserve = true
						break
					}
				}

				if preserve {
					cleaned := rewriteModifierRe.ReplaceAllString(line, "")
					cleaned = regexp.MustCompile(`,update-interval=\d+`).ReplaceAllString(cleaned, "")
					cleaned = regexp.MustCompile(`,no-resolve`).ReplaceAllString(cleaned, "")
					for old, new := range ruleReplacements {
						cleaned = strings.ReplaceAll(cleaned, old, new)
					}
					out = append(out, strings.TrimSpace(cleaned))
					continue
				}

				fmt.Printf("  📦 Expanding RULE-SET: %s\n", urlOrPath)
				expandedRules := fetchRuleset(urlOrPath)
				if len(expandedRules) > 0 {
					out = append(out, fmt.Sprintf("# --- Expanded from %s ---", urlOrPath))
					for _, r := range expandedRules {
						out = append(out, fmt.Sprintf("%s,%s", r, policy))
					}
					out = append(out, "# --- End expansion ---")
				} else {
					out = append(out, fmt.Sprintf("# [Cannot expand] %s", line))
				}
				continue
			}

			if strings.HasPrefix(stripped, "PROTOCOL,") {
				out = append(out, "# [SR does not support PROTOCOL] "+line)
				continue
			}
		}

		if section == "Script" && stripped != "" && !strings.HasPrefix(stripped, "#") {
			line = hub.AdaptScriptLineForSr(line, moduleStem)
		} else if section == "MITM" && stripped != "" && !strings.HasPrefix(stripped, "#") {
			line = hub.AdaptMitmLineForSr(line)
		}

		if section != "MITM" {
			line = regexp.MustCompile(`%(?:INSERT|APPEND)%\s*`).ReplaceAllString(line, "")
		}
		for old, new := range ruleReplacements {
			line = strings.ReplaceAll(line, old, new)
		}
		line = rewriteModifierRe.ReplaceAllString(line, "")
		line = regexp.MustCompile(`,"update-interval=\d+"`).ReplaceAllString(line, "")

		out = append(out, line)
	}

	return hub.SanitizeFileContent(strings.Join(out, "\n"), true)
}

func convertDevtoolsModuleForSr(content string) string {
	meta, sections := hub.ParseModule(content)
	moduleStem := hub.ModuleStemFromMeta(meta, hub.DEVTOOLS_STEM)
	converted := convertContent(content, moduleStem)
	meta, sections = hub.ParseModule(converted)
	desc := meta["desc"]
	if !strings.Contains(desc, "[🚀SR]") {
		meta["desc"] = "[🚀SR] " + desc + `\nSub-Store simplified for Surge-Noability; produce cron only available on Surge`
	}
	header := hub.FormatHeader(meta, nil)
	ordered := make([]hub.ModuleSection, 0)
	for _, s := range sections {
		ordered = append(ordered, hub.ModuleSection{Name: s.Name, Lines: s.Lines})
	}
	return hub.SanitizeFileContent(hub.FormatModule(header, ordered, true, meta), true)
}

func ConvertPromaxModules() bool {
	ok := true
	subdirs := []string{"", "github"}
	for _, subdir := range subdirs {
		srcCat := filepath.Join(hub.ROOT, "modules/surge/head_expanse")
		outCat := filepath.Join(hub.ROOT, "modules/shadowrocket/head_expanse")
		if subdir != "" {
			srcCat = filepath.Join(srcCat, subdir)
			outCat = filepath.Join(outCat, subdir)
		}
		hub.EnsureDir(outCat)

		for _, name := range promaxSurgeModules {
			src := filepath.Join(srcCat, name)
			if !hub.ValidateFileExists(src, "") {
				if subdir == "" {
					fmt.Printf("⚠️ PROMAX source missing: %s\n", name)
					ok = false
				}
				continue
			}
			subdirName := subdir
			if subdirName == "" {
				subdirName = "cdn"
			}
			fmt.Printf("Converting PROMAX → SR [%s]: %s\n", subdirName, name)

			content := hub.ReadFileString(src)
			converted := convertContent(content, "")

			outPath := filepath.Join(outCat, strings.TrimSuffix(name, ".sgmodule")+".module")
			hub.SafeWriteFile(outPath, converted, true)
			fmt.Printf("✅ SR module written: %s\n", outPath)
		}
	}
	return ok
}

func ProcessAllModules() (int, int, int) {
	srModuleDir := filepath.Join(hub.ROOT, "modules/shadowrocket")
	surgeModuleDir := filepath.Join(hub.ROOT, "modules/surge")
	hub.EnsureDir(srModuleDir)

	categories := []string{"amplify_nexus", "head_expanse"}
	total, converted, failed := 0, 0, 0

	for _, cat := range categories {
		hub.EnsureDir(filepath.Join(srModuleDir, cat))
		catPath := filepath.Join(surgeModuleDir, cat)
		if !hub.ValidateDirExists(catPath, "") {
			continue
		}

		entries, _ := os.ReadDir(catPath)
		var files []string
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".sgmodule") {
				files = append(files, e.Name())
			}
		}
		sort.Strings(files)

		for _, file := range files {
			total++
			fmt.Printf("🔄 Converting: %s\n", file)

			moduleFile := filepath.Join(catPath, file)
			content := hub.ReadFileString(moduleFile)

			stem := strings.TrimSuffix(file, ".sgmodule")
			var convertedStr string
			if stem == hub.DEVTOOLS_STEM {
				convertedStr = convertDevtoolsModuleForSr(content)
			} else {
				convertedStr = convertContent(content, stem)
			}

			outPath := filepath.Join(srModuleDir, cat, stem+".module")
			hub.SafeWriteFile(outPath, convertedStr, true)
			converted++
		}
	}
	return total, converted, failed
}

func convertProxyGroupLine(line string) string {
	line = regexp.MustCompile(`,\s*no-alert=\d+`).ReplaceAllString(line, "")
	line = regexp.MustCompile(`,\s*hidden=\d+`).ReplaceAllString(line, "")
	line = regexp.MustCompile(`,\s*persistent=\d+`).ReplaceAllString(line, "")
	line = regexp.MustCompile(`,\s*evaluate-before-use=\d+`).ReplaceAllString(line, "")
	line = regexp.MustCompile(`,\s*include-all-proxies=\d+`).ReplaceAllString(line, "")
	line = regexp.MustCompile(`,\s*icon-url=[^\s,]+`).ReplaceAllString(line, "")

	line = regexp.MustCompile(`=\s*smart,`).ReplaceAllString(line, "= url-test,")

	if strings.Contains(line, "include-other-group=") && !strings.Contains(line, "policy-path=") {
		match := regexp.MustCompile(`include-other-group="([^"]+)"`).FindStringSubmatch(line)
		if match != nil {
			line = regexp.MustCompile(`include-other-group="[^"]+"`).ReplaceAllString(line, "")
		}
	}
	return line
}

func convertMainConfig(surgeConfPath string, outputPath string, compactHost bool) bool {
	fmt.Printf("Converting main config: %s\n", filepath.Base(surgeConfPath))

	if !hub.ValidateFileExists(surgeConfPath, "") {
		fmt.Printf("Config file not found: %s\n", surgeConfPath)
		return false
	}

	content := hub.ReadFileString(surgeConfPath)
	lines := strings.Split(content, "\n")
	var out []string
	var section string
	hostLinesKept := 0
	hostLinesSkipped := 0

	var dnsServerLine string
	var fallbackDnsLine string

	for _, line := range lines {
		stripped := strings.TrimSpace(line)

		if strings.HasPrefix(stripped, "[") && strings.HasSuffix(stripped, "]") {
			if section == "General" && (dnsServerLine != "" || fallbackDnsLine != "") {
				if dnsServerLine != "" {
					out = append(out, dnsServerLine)
				}
				if fallbackDnsLine != "" {
					out = append(out, fallbackDnsLine)
				}
				dnsServerLine = ""
				fallbackDnsLine = ""
			}

			section = stripped[1 : len(stripped)-1]
			out = append(out, line)

			if section == "Host" && compactHost {
				out = append(out, "# Shadowrocket Compact - Keep only critical DNS config")
				out = append(out, "# Full domain-level DoH split via [General] doh-server global config")
			}
			continue
		}

		if section == "Ponte" {
			if !strings.HasPrefix(stripped, "#") && stripped != "" {
				out = append(out, "# [SR does not support Ponte] "+line)
			} else {
				out = append(out, line)
			}
			continue
		}

		if section == "General" && !strings.HasPrefix(stripped, "#") && stripped != "" {
			hasSurgeKey := false
			for k := range surgeOnlyKeys {
				if strings.Contains(line, k) {
					hasSurgeKey = true
					break
				}
			}
			if hasSurgeKey {
				out = append(out, "# [SR does not support ] "+strings.TrimLeft(line, " \t"))
				continue
			}

			converted := false
			foundK := ""
			for k, v := range generalKeyMap {
				if strings.HasPrefix(line, k) {
					line = strings.Replace(line, k, v, 1)
					converted = true
					if k == "encrypted-dns-server" {
						dnsServerLine = line
						foundK = k
					} else if k == "dns-server" {
						fallbackDnsLine = line
						foundK = k
					}
					break
				}
			}

			if !converted || (foundK != "encrypted-dns-server" && foundK != "dns-server") {
				out = append(out, line)
			}
			continue
		}

		if section == "Proxy Group" && !strings.HasPrefix(stripped, "#") && stripped != "" && strings.Contains(stripped, "=") {
			line = convertProxyGroupLine(line)
			out = append(out, line)
			continue
		}

		if section == "Rule" && !strings.HasPrefix(stripped, "#") && stripped != "" {
			line = regexp.MustCompile(`,\s*extended-matching`).ReplaceAllString(line, "")
			line = regexp.MustCompile(`,\s*no-resolve\s*$`).ReplaceAllString(line, "")
			if strings.HasPrefix(stripped, "PROTOCOL,") {
				out = append(out, "# [SR does not support PROTOCOL] "+line)
				continue
			}
			out = append(out, line)
			continue
		}

		if section == "Host" {
			if compactHost {
				if shouldKeepHostLine(line) {
					line = strings.ReplaceAll(line, "server:force-syslib", "server:syslib")
					out = append(out, line)
					if !strings.HasPrefix(stripped, "#") && stripped != "" {
						hostLinesKept++
					}
				} else {
					hostLinesSkipped++
				}
			} else {
				line = strings.ReplaceAll(line, "server:force-syslib", "server:syslib")
				out = append(out, line)
			}
			continue
		}

		if section == "Script" {
			if !strings.HasPrefix(stripped, "#") && stripped != "" {
				out = append(out, "# [SR does not support Script] "+line)
			} else {
				out = append(out, line)
			}
			continue
		}

		out = append(out, line)
	}

	result := strings.Join(out, "\n")
	hub.SafeWriteFile(outputPath, result, true)
	fmt.Printf("Config converted: %s\n", outputPath)
	if compactHost {
		fmt.Printf("Host compact: Kept %d lines, skipped %d lines\n", hostLinesKept, hostLinesSkipped)
	}
	fmt.Printf("总lines数: %d\n", len(out))
	return true
}

// RunConvertSurgeToShadowrocket takes an args map. It supports modules, promaxOnly, config, output, fullHost.
func RunConvertSurgeToShadowrocket(args map[string]string) int {
	_, modules := args["modules"]
	_, promaxOnly := args["promax-only"]
	config := args["config"]
	output := args["output"]
	_, fullHost := args["full-host"]

	if config != "" {
		if !hub.ValidateFileExists(config, "") {
			fmt.Printf("Config file not found: %s\n", config)
			return 1
		}
		if output == "" {
			output = filepath.Join(filepath.Dir(config), strings.TrimSuffix(filepath.Base(config), filepath.Ext(config))+"_Shadowrocket.conf")
		}
		success := convertMainConfig(config, output, !fullHost)
		if success {
			return 0
		}
		return 1
	}

	if promaxOnly {
		if ConvertPromaxModules() {
			return 0
		}
		return 1
	}

	if modules {
		total, converted, failed := ProcessAllModules()
		fmt.Printf("All modules converted: %d/%d\n", converted, total)
		if failed > 0 {
			fmt.Printf("Failed: %d modules\n", failed)
		}
		if converted == 0 {
			return 1
		}
		return 0
	}

	surgeConf := filepath.Join(hub.ROOT, ".claude", "NyaMiiKo.conf.conf")
	if hub.ValidateFileExists(surgeConf, "") {
		output := filepath.Join(hub.ROOT, ".claude", "NyaMiiKo_Shadowrocket.conf")
		fmt.Println("Converting default project config")
		success := convertMainConfig(surgeConf, output, !fullHost)
		if success {
			return 0
		}
		return 1
	}

	fmt.Println("Not found .claude/NyaMiiKo.conf.conf, skipping main config, converting modules only")
	total, converted, failed := ProcessAllModules()
	fmt.Printf("All modules converted: %d/%d\n", converted, total)
	if failed > 0 {
		fmt.Printf("Failed: %d modules\n", failed)
	}
	if converted == 0 {
		return 1
	}
	return 0
}
