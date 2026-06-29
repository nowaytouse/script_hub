package pipeline

import (
	"archive/tar"
	"compress/gzip"
	"encoding/json"
	"fmt"
	"io"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"sort"
	"strings"
	"time"

	"github.com/nyamiiko/script_hub/create/hub"
)

var skkSources = map[string]string{
	"reject.list":                    "https://ruleset.skk.moe/List/non_ip/reject.conf",
	"reject-no-drop.list":            "https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf",
	"reject-drop.list":               "https://ruleset.skk.moe/List/non_ip/reject-drop.conf",
	"reject-url-regex.list":          "https://ruleset.skk.moe/List/non_ip/reject-url-regex.conf",
	"sogouinput.list":                "https://ruleset.skk.moe/List/non_ip/sogouinput.conf",
	"my_reject.list":                 "https://ruleset.skk.moe/List/non_ip/my_reject.conf",
	"reject-domainset.list":          "https://ruleset.skk.moe/List/domainset/reject.conf",
	"reject-extra-domainset.list":    "https://ruleset.skk.moe/List/domainset/reject_extra.conf",
	"reject-phishing-domainset.list": "https://ruleset.skk.moe/List/domainset/reject_phishing.conf",
	"reject-ip.list":                 "https://ruleset.skk.moe/List/ip/reject.conf",
	"ai.list":                        "https://ruleset.skk.moe/List/non_ip/ai.conf",
	"apple_cn.list":                  "https://ruleset.skk.moe/List/non_ip/apple_cn.conf",
	"apple_intelligence.list":        "https://ruleset.skk.moe/List/non_ip/apple_intelligence.conf",
	"apple_services.list":            "https://ruleset.skk.moe/List/non_ip/apple_services.conf",
	"microsoft.list":                 "https://ruleset.skk.moe/List/non_ip/microsoft.conf",
	"telegram.list":                  "https://ruleset.skk.moe/List/non_ip/telegram.conf",
	"global.list":                    "https://ruleset.skk.moe/List/non_ip/global.conf",
	"direct.list":                    "https://ruleset.skk.moe/List/non_ip/direct.conf",
	"domestic.list":                  "https://ruleset.skk.moe/List/non_ip/domestic.conf",
	"download.list":                  "https://ruleset.skk.moe/List/non_ip/download.conf",
	"neteasemusic.list":              "https://ruleset.skk.moe/List/non_ip/neteasemusic.conf",
	"game-download.list":             "https://ruleset.skk.moe/List/domainset/game-download.conf",
	"speedtest.list":                 "https://ruleset.skk.moe/List/domainset/speedtest.conf",
	"cdn.list":                       "https://ruleset.skk.moe/List/non_ip/cdn.conf",
	"cdn-domainset.list":             "https://ruleset.skk.moe/List/domainset/cdn.conf",
	"BlockHttpDNS.list":              "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/BlockHttpDNS/BlockHttpDNS.list",
	"telegram-ip.list":               "https://ruleset.skk.moe/List/ip/telegram.conf",
	"telegram_asn-ip.list":           "https://ruleset.skk.moe/List/ip/telegram_asn.conf",
	"apple_services-ip.list":         "https://ruleset.skk.moe/List/ip/apple_services.conf",
	"download-ip.list":               "https://ruleset.skk.moe/List/ip/download.conf",
	"neteasemusic-ip.list":           "https://ruleset.skk.moe/List/ip/neteasemusic.conf",
	"cdn-ip.list":                    "https://ruleset.skk.moe/List/ip/cdn.conf",
	"domestic-ip.list":               "https://ruleset.skk.moe/List/ip/domestic.conf",
}

var metacubexRules = map[string]string{
	"telegram":         "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/telegram.srs",
	"discord":          "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/discord.srs",
	"google":           "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/google.srs",
	"apple":            "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/apple.srs",
	"microsoft":        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/microsoft.srs",
	"bilibili":         "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/bilibili.srs",
	"category-ai-cn":   "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/category-ai-!cn.srs",
	"category-games":   "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/category-games.srs",
	"category-ads-all": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/category-ads-all.srs",
	"instagram":        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/instagram.srs",
	"twitter":          "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/twitter.srs",
	"spotify":          "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/spotify.srs",
	"steam":            "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/steam.srs",
	"youtube":          "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/youtube.srs",
	"github":           "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/github.srs",
	"netflix":          "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/netflix.srs",
	"tiktok":           "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/tiktok.srs",
	"paypal":           "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/paypal.srs",
	"amazon":           "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/amazon.srs",
	"reddit":           "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/reddit.srs",
	"whatsapp":         "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/whatsapp.srs",
	"facebook":         "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/facebook.srs",
	"openai":           "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/openai.srs",
	"cloudflare":       "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/cloudflare.srs",
}

var PROTECTED_MODULES = []string{}

var nexusModules = []string{}

var localSourceModules = map[string]string{
	"XWebAds.sgmodule":                                  "https://raw.githubusercontent.com/fmz200/wool_scripts/refs/heads/main/Surge/module/XWebAds.module",
	"Wool.ADBlock.sgmodule":                             "https://raw.githubusercontent.com/fmz200/wool_scripts/refs/heads/main/Surge/module/blockAds.module",
	"BiliBili.ADBlock.sgmodule":                         "https://github.com/BiliUniverse/ADBlock/releases/latest/download/BiliBili.ADBlock.sgmodule",
	"[Sukka] Enhance Better ADBlock for Surge.sgmodule": "https://ruleset.skk.moe/Modules/sukka_enhance_adblock.sgmodule",
	"sukka_url_redirect.sgmodule":                       "https://ruleset.skk.moe/Modules/sukka_url_redirect.sgmodule",
	"adultraplus.sgmodule":                              "https://yfamilys.com/module/adultraplus.sgmodule",
	"script_hub.surge.sgmodule":                         "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/modules/script-hub.surge.sgmodule",
}

const (
	NEXUS_GROUP        = "『 🛠️ Amplify Nexus › 增幅枢纽 』"
	HEAD_EXPANSE_GROUP = "『 🔝 Head Expanse › 首端扩域 』"
)

var keleeSources = map[string]string{
	"SpeedtestChina.yaml":         "https://kelee.one/Resource/Script/Speedtest/Speedtest_China.yaml",
	"SpeedtestInternational.yaml": "https://kelee.one/Resource/Script/Speedtest/Speedtest_International.yaml",
}

type UpstreamSyncer struct {
	singboxPath string
}

func NewUpstreamSyncer() *UpstreamSyncer {
	u := &UpstreamSyncer{}
	u.singboxPath = u.setupSingbox()
	return u
}

func (u *UpstreamSyncer) setupSingbox() string {
	if path, err := exec.LookPath("sing-box"); err == nil {
		return path
	}

	localPath := hub.SINGBOX_ROOT_BIN
	if _, err := os.Stat(localPath); err == nil {
		return localPath
	}

	hub.Info("sing-box not found. Attempting to download latest beta...")

	versionOut, err := exec.Command("sh", "-c", "curl -L -s https://github.com/SagerNet/sing-box/releases | grep -oE 'v[0-9]+\\.[0-9]+\\.[0-9]+-(beta|alpha|rc)\\.[0-9]+' | head -n 1").Output()
	version := strings.TrimSpace(string(versionOut))
	if version == "" {
		version = "v1.14.0-alpha.11"
	}

	cleanVersion := strings.TrimPrefix(version, "v")
	arch := "amd64"
	if runtime.GOARCH == "arm64" {
		arch = "arm64"
	}

	primaryUrl := fmt.Sprintf("https://github.com/SagerNet/sing-box/releases/download/%s/sing-box-%s-darwin-%s.tar.gz", version, cleanVersion, arch)
	mirrorUrl := fmt.Sprintf("https://ghproxy.net/https://github.com/SagerNet/sing-box/releases/download/%s/sing-box-%s-darwin-%s.tar.gz", version, cleanVersion, arch)

	hub.Info(fmt.Sprintf("Downloading sing-box %s for darwin-%s...", version, arch))
	tarPath := hub.SINGBOX_TAR_GZ

	if !u.downloadToFile(primaryUrl, tarPath, "") {
		hub.Info("Primary source failed. Trying mirror...")
		if !u.downloadToFile(mirrorUrl, tarPath, "") {
			hub.Error("Failed to auto-setup sing-box: Download failed from all sources")
			return ""
		}
	}

	err = extractTarGz(tarPath, hub.ROOT, localPath)
	if err != nil {
		hub.Error(fmt.Sprintf("Failed to extract sing-box: %v", err))
		return ""
	}

	os.Remove(tarPath)
	hub.Success(fmt.Sprintf("sing-box %s installed successfully at %s", version, localPath))
	return localPath
}

func extractTarGz(tarGzPath string, rootPath string, destPath string) error {
	f, err := os.Open(tarGzPath)
	if err != nil {
		return err
	}
	defer f.Close()

	gzr, err := gzip.NewReader(f)
	if err != nil {
		return err
	}
	defer gzr.Close()

	tr := tar.NewReader(gzr)
	for {
		header, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return err
		}

		if strings.HasSuffix(header.Name, "/sing-box") || header.Name == "sing-box" {
			out, err := os.OpenFile(destPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0755)
			if err != nil {
				return err
			}
			_, err = io.Copy(out, tr)
			out.Close()
			if err != nil {
				return err
			}
			return nil
		}
	}
	return fmt.Errorf("sing-box binary not found in tarball")
}

func (u *UpstreamSyncer) downloadToFile(urlStr string, destPath string, ua string) bool {
	data := hub.SafeDownloadBinary(urlStr, 2, 60, ua)
	if data == nil {
		return false
	}
	err := hub.SafeWriteFile(destPath, string(data), true)
	return err == nil
}

func (u *UpstreamSyncer) download(urlStr string, ua string) []byte {
	return hub.SafeDownloadBinary(urlStr, 2, 60, ua)
}

func (u *UpstreamSyncer) SyncSkk() {
	hub.Section("Syncing SKK Upstream Rulesets")
	hub.EnsureDir(hub.SKK_UPSTREAM_DIR)
	hub.EnsureDir(hub.ADBLOCK_DIR)

	// SERIAL EXECUTION for stability
	for name, urlStr := range skkSources {
		data := u.download(urlStr, "")
		if data == nil {
			continue
		}
		content := string(data)
		var lines []string
		for _, line := range strings.Split(content, "\n") {
			stripped := strings.TrimSpace(line)
			if stripped != "" && !strings.HasPrefix(stripped, "#") {
				lines = append(lines, stripped)
			}
		}

		if name == "reject-no-drop.list" {
			var filtered []string
			for _, line := range lines {
				if !strings.Contains(strings.ToLower(line), "bilibili") && !strings.HasPrefix(strings.ToUpper(line), "AND,") {
					filtered = append(filtered, line)
				}
			}
			lines = filtered
		}

		final := fmt.Sprintf("# Ruleset: %s\n\n", name) + strings.Join(lines, "\n") + "\n"
		hub.SafeWriteFile(filepath.Join(hub.SKK_UPSTREAM_DIR, name), final, true)
		hub.Success(fmt.Sprintf("SKK: %s (%d rules)", name, len(lines)))

		if name == "reject-drop.list" || name == "reject-no-drop.list" {
			hub.SafeWriteFile(filepath.Join(hub.ADBLOCK_DIR, name), final, true)
			hub.Info(fmt.Sprintf("  → Copied to AdBlock: %s", name))
		} else if name == "BlockHttpDNS.list" {
			hijackFinal := "# Ruleset: HTTPDNS_Hijack.list\n\n" + strings.Join(lines, "\n") + "\n"
			hub.SafeWriteFile(filepath.Join(hub.ADBLOCK_DIR, "HTTPDNS_Hijack.list"), hijackFinal, true)
			hub.Info("  → Copied to AdBlock: HTTPDNS_Hijack.list")
		}

		time.Sleep(1500 * time.Millisecond) // Slow down to avoid rate limits
	}
}

func (u *UpstreamSyncer) processNexusModule(urlStr string) {
	filename, _ := url.QueryUnescape(filepath.Base(urlStr))
	if strings.HasSuffix(filename, ".module") {
		filename = strings.TrimSuffix(filename, ".module") + ".sgmodule"
	} else if !strings.HasSuffix(filename, ".sgmodule") {
		filename += ".sgmodule"
	}

	for _, protected := range PROTECTED_MODULES {
		if filename == protected {
			hub.Info(fmt.Sprintf("Skipping protected module: %s", filename))
			return
		}
	}

	data := u.download(urlStr, "")
	if data == nil {
		return
	}
	content := string(data)
	if strings.Contains(content, "<!DOCTYPE html>") {
		hub.Error(fmt.Sprintf("Invalid file (HTML) for %s", filename))
		return
	}

	lines := strings.Split(content, "\n")
	var newLines []string
	nameLineIdx := -1

	for _, line := range lines {
		stripped := strings.TrimSpace(line)
		if strings.HasPrefix(stripped, "#!category") || strings.HasPrefix(stripped, "#!group") || strings.HasPrefix(stripped, "# category:") {
			continue
		}
		if strings.HasPrefix(stripped, "#!name") {
			nameLineIdx = len(newLines)
		}
		newLines = append(newLines, strings.TrimRight(line, "\r"))
	}

	categoryLine := fmt.Sprintf("#!category=%s", NEXUS_GROUP)
	insertAt := 0
	if nameLineIdx != -1 {
		insertAt = nameLineIdx + 1
	}
	newLines = append(newLines[:insertAt], append([]string{categoryLine}, newLines[insertAt:]...)...)

	finalContent := strings.TrimSpace(strings.Join(newLines, "\n")) + "\n"
	hub.SafeWriteFile(filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, filename), finalContent, true)
	hub.Success(fmt.Sprintf("Nexus: %s", filename))
}

func (u *UpstreamSyncer) SyncNexus() {
	hub.Section("Syncing Amplify Nexus Modules")
	hub.EnsureDir(hub.SURGE_AMPLIFY_NEXUS_DIR)
	// SERIAL EXECUTION
	for _, urlStr := range nexusModules {
		u.processNexusModule(urlStr)
		time.Sleep(1500 * time.Millisecond)
	}
}

func (u *UpstreamSyncer) processLocalSourceModule(fname string, urlStr string, ua string) {
	data := u.download(urlStr, ua)
	if data == nil {
		return
	}
	content := string(data)
	if strings.Contains(content, "<!DOCTYPE html>") {
		hub.Error(fmt.Sprintf("Invalid file (HTML) for local_source: %s", fname))
		return
	}
	hub.SafeWriteFile(filepath.Join(hub.MODULE_LOCAL_DIR, fname), content, true)
	hub.Success(fmt.Sprintf("LocalSource: %s ← %s", fname, urlStr))
}

func (u *UpstreamSyncer) SyncLocalSources() {
	hub.Section("Syncing Local-Source Modules (→ PROMAX)")
	hub.EnsureDir(hub.MODULE_LOCAL_DIR)
	// SERIAL EXECUTION
	for fname, urlStr := range localSourceModules {
		ua := ""
		if strings.Contains(urlStr, "yfamilys.com") {
			ua = "Loon/3.9.9 CFNetwork/1496.0.7 Darwin/23.5.0"
		}
		u.processLocalSourceModule(fname, urlStr, ua)
		time.Sleep(1500 * time.Millisecond)
	}
}

func (u *UpstreamSyncer) processMetacubexRule(name string, urlStr string) {
	content := u.download(urlStr, "")
	if content == nil {
		return
	}
	hub.EnsureDir(hub.CACHE_DIR)
	tmpSrs := filepath.Join(hub.CACHE_DIR, fmt.Sprintf("%s_%d.srs", name, os.Getpid()))
	tmpJson := filepath.Join(hub.CACHE_DIR, fmt.Sprintf("%s_%d.json", name, os.Getpid()))

	defer func() {
		os.Remove(tmpSrs)
		os.Remove(tmpJson)
	}()

	err := os.WriteFile(tmpSrs, content, 0644)
	if err != nil {
		return
	}

	if u.singboxPath == "" {
		hub.Error("sing-box not found, cannot decompile MetaCubeX rules")
		return
	}

	cmd := exec.Command(u.singboxPath, "rule-set", "decompile", tmpSrs, "-o", tmpJson)
	out, err := cmd.CombinedOutput()
	if err != nil {
		hub.Error(fmt.Sprintf("Decompile failed for %s: %s", name, string(out)))
		return
	}

	b, err := os.ReadFile(tmpJson)
	if err != nil {
		return
	}

	var data map[string]interface{}
	if err := json.Unmarshal(b, &data); err != nil {
		return
	}

	var rules []string

	ensureList := func(v interface{}) []string {
		if v == nil {
			return nil
		}
		switch val := v.(type) {
		case string:
			return []string{val}
		case []interface{}:
			var s []string
			for _, item := range val {
				if str, ok := item.(string); ok {
					s = append(s, str)
				}
			}
			return s
		}
		return nil
	}

	isValidRegex := func(pattern string) bool {
		if len(pattern) < 2 {
			return false
		}
		if pattern == "." || pattern == "*" || pattern == "]" || pattern == "[" || pattern == "^" || pattern == "$" || pattern == "(" || pattern == ")" || pattern == "|" {
			return false
		}
		_, err := regexp.Compile(pattern)
		return err == nil
	}

	if ruleList, ok := data["rules"].([]interface{}); ok {
		for _, r := range ruleList {
			if ruleMap, ok := r.(map[string]interface{}); ok {
				for _, d := range ensureList(ruleMap["domain"]) {
					rules = append(rules, fmt.Sprintf("DOMAIN,%s", strings.TrimSpace(d)))
				}
				for _, d := range ensureList(ruleMap["domain_suffix"]) {
					rules = append(rules, fmt.Sprintf("DOMAIN-SUFFIX,%s", strings.TrimSpace(d)))
				}
				for _, d := range ensureList(ruleMap["domain_keyword"]) {
					rules = append(rules, fmt.Sprintf("DOMAIN-KEYWORD,%s", strings.TrimSpace(d)))
				}
				for _, d := range ensureList(ruleMap["domain_regex"]) {
					dStr := strings.TrimSpace(d)
					if isValidRegex(dStr) {
						rules = append(rules, fmt.Sprintf("DOMAIN-REGEX,%s", dStr))
					}
				}
				for _, ip := range ensureList(ruleMap["ip_cidr"]) {
					ipStr := strings.TrimSpace(ip)
					prefix := "IP-CIDR"
					if strings.Contains(ipStr, ":") {
						prefix = "IP-CIDR6"
					}
					rules = append(rules, fmt.Sprintf("%s,%s,no-resolve", prefix, ipStr))
				}
			}
		}
	}

	rules = hub.DeduplicateRules(rules)
	sort.Strings(rules)

	outputPath := filepath.Join(hub.METACUBEX_DIR, fmt.Sprintf("MetaCubeX_%s.list", name))
	finalContent := fmt.Sprintf("# MetaCubeX geosite-%s\n# Rules: %d\n\n", name, len(rules)) + strings.Join(rules, "\n") + "\n"
	hub.SafeWriteFile(outputPath, finalContent, true)
	hub.Success(fmt.Sprintf("MetaCubeX: %s (%d rules)", name, len(rules)))
}

func (u *UpstreamSyncer) SyncMetacubex() {
	hub.Section("Syncing MetaCubeX Rules (Serial/Throttled)")
	hub.EnsureDir(hub.METACUBEX_DIR)
	// SERIAL EXECUTION
	for name, urlStr := range metacubexRules {
		u.processMetacubexRule(name, urlStr)
		time.Sleep(1000 * time.Millisecond)
	}
}

func (u *UpstreamSyncer) SyncKelee() {
	hub.Section("Syncing Kelee Speedtest YAML Files")
	hub.EnsureDir(hub.KELEE_DIR)

	for filename, urlStr := range keleeSources {
		data := u.download(urlStr, "Loon/3.9.9 CFNetwork/1496.0.7 Darwin/23.5.0")
		if data == nil {
			hub.Warn(fmt.Sprintf("Failed to download Kelee file: %s", filename))
			continue
		}

		content := string(data)
		currentTime := time.Now().Format("2006-01-02 15:04:05")
		lines := strings.Split(content, "\n")
		var newLines []string

		for _, line := range lines {
			stripped := strings.TrimSpace(line)
			if strings.HasPrefix(stripped, "# UpdateTime") {
				newLines = append(newLines, fmt.Sprintf("# UpdateTime：%s", currentTime))
				hub.Info(fmt.Sprintf("  Updated timestamp for %s to %s", filename, currentTime))
			} else if strings.HasPrefix(stripped, "# RuleCount") {
				continue
			} else {
				newLines = append(newLines, strings.TrimRight(line, "\r"))
			}
		}

		ruleCount := 0
		for _, line := range newLines {
			stripped := strings.TrimSpace(line)
			if stripped != "" && !strings.HasPrefix(stripped, "#") && stripped != "payload:" {
				ruleCount++
			}
		}

		var finalLines []string
		for _, line := range newLines {
			finalLines = append(finalLines, line)
			if strings.HasPrefix(strings.TrimSpace(line), "# UpdateTime") {
				finalLines = append(finalLines, fmt.Sprintf("# RuleCount：%d", ruleCount))
			}
		}

		finalContent := strings.Join(finalLines, "\n")
		if !strings.HasSuffix(finalContent, "\n") {
			finalContent += "\n"
		}

		targetPath := filepath.Join(hub.KELEE_DIR, filename)
		if err := hub.SafeWriteFile(targetPath, finalContent, true); err == nil {
			hub.Success(fmt.Sprintf("Kelee: %s (%d rules)", filename, ruleCount))
		} else {
			hub.Error(fmt.Sprintf("Failed to write Kelee file: %s", filename))
		}
		time.Sleep(1500 * time.Millisecond)
	}
}

func (u *UpstreamSyncer) SyncBlockedSitesKorea() {
	hub.Section("Syncing Blocked Sites in South Korea")
	urlStr := "https://raw.githubusercontent.com/wpzzz/blocked-sites-in-south-korea/main/list.txt"
	destPath := filepath.Join(hub.SOURCES_DIR, "vendor", "blocked_sites_south_korea.list")

	data := u.download(urlStr, "")
	if data == nil {
		hub.Error("Failed to download Blocked Sites in South Korea list.")
		return
	}

	content := string(data)
	var lines []string
	for _, line := range strings.Split(content, "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}

		if strings.Contains(line, ",") {
			lines = append(lines, line)
		} else {
			if strings.HasPrefix(line, ".") {
				line = line[1:]
			} else if strings.HasPrefix(line, "+.") {
				line = line[2:]
			} else if strings.HasPrefix(line, "+") {
				line = line[1:]
			}
			lines = append(lines, fmt.Sprintf("DOMAIN-SUFFIX,%s", line))
		}
	}

	final := "# Blocked Sites in South Korea\n# Source: https://github.com/wpzzz/blocked-sites-in-south-korea\n\n" + strings.Join(lines, "\n") + "\n"

	hub.EnsureDir(filepath.Dir(destPath))
	if err := hub.SafeWriteFile(destPath, final, true); err == nil {
		hub.Success(fmt.Sprintf("Blocked Sites South Korea: %d rules saved to %s", len(lines), destPath))
	} else {
		hub.Error("Failed to write Blocked Sites in South Korea list.")
	}
}
