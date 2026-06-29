package pipeline

import (
	"fmt"
	"net"
	"net/url"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/hub"
)

var (
	FUNCTIONAL_BLOCK_NAME_TOKENS = []string{
		"解锁", "unlock", "unblock", "破解", "crack", "会员", "vip", "premium", "增强", "enhance", "enhanced", ".global", "global.sgmodule", "redirect", "翻译", "translation", "nsfw", "iringo", "weatherkit", "dualsubs", "sub_info", "boxjs", "timecard", "net-lsp", "surge-beta", "apple服务", "wechat_enhance", "reddit", "扫描全能", "camscanner", "nsringo", "geoservices", "boxjs", "maasea/sgmodule", "youtube.enhance", "增强合集", "bilibili.enhanced", "bilibili.global", "bilibili.redirect",
	}

	ADBLOCK_MODULE_NAME_TOKENS = []string{
		"去广告", "adblock", "anti-ad", "xwebads", "allinone", "all-in-one", "adultraplus", "微博去广告", "rednote", "redpaper",
	}

	ADBLOCK_FILENAME_ALLOWLIST = map[string]bool{
		"[Sukka] Enhance Better ADBlock for Surge.sgmodule": true,
		"sukka_url_redirect.sgmodule":                       true,
		"AllInOne_Mock.sgmodule":                            true,
		"All-in-One-2.x.sgmodule":                           true,
		"XWebAds.sgmodule":                                  true,
		"BiliBili.ADBlock.sgmodule":                         true,
		"YouTube.ADBlock.sgmodule":                          true,
		"WeChat.ADBlock.sgmodule":                           true,
	}

	FUNCTIONAL_BLOCK_LOCAL_SOURCES = map[string]bool{
		"Reddit.ADBlock.sgmodule": true,
	}

	LITE_CATEGORIES = map[string]bool{
		"Local": true, "Advertising": true, "Privacy": true, "Security": true, "AntiAD": true, "Other": true,
	}

	CDN_BASE_URL    = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/"
	GITHUB_RAW_URL  = "https://raw.githubusercontent.com/nowaytouse/script_hub/master/"
	RULES_PER_SHARD = 100000

	CATEGORY_META = map[string]map[string]string{
		"Local": {
			"label_zh": "应用去广告",
			"desc":     "各 App 去广告域名规则（脚本/重写已并入 PROMAX，勿单独安装专项模块）",
		},
		"Advertising": {
			"label_zh": "通用广告",
			"desc":     "blackmatrix7 / ACL4SSR / geekdada / 广告联盟等通用域名",
		},
		"Privacy": {
			"label_zh": "隐私追踪",
			"desc":     "Privacy / tracking-protection / 防追踪列表",
		},
		"Security": {
			"label_zh": "安全威胁",
			"desc":     "劫持 / 钓鱼 / BanProgram / HTTPDNS / skk reject",
		},
		"AntiAD": {
			"label_zh": "Anti-AD",
			"desc":     "privacy-protection-tools anti-AD 主列表与 discretion",
		},
		"ThreatIntel_Ultimate": {
			"label_zh": "威胁情报·终极",
			"desc":     "HaGeZi DNS-Blocklists ultimate",
		},
		"ThreatIntel_TIF": {
			"label_zh": "威胁情报·TIF",
			"desc":     "HaGeZi TIF medium 威胁情报",
		},
		"ThreatIntel": {
			"label_zh": "威胁情报·补充",
			"desc":     "HaGeZi / skk 其它威胁与钓鱼域名集",
		},
		"Other": {
			"label_zh": "其他补充",
			"desc":     "社区 sgmodule / 未归类远程源",
		},
	}

	categoryNames = []string{
		"Local", "Advertising", "Privacy", "Security", "AntiAD", "ThreatIntel_Ultimate", "ThreatIntel_TIF", "ThreatIntel", "Other",
	}

	GROUP_HEAD_EXPANSE   = "『 🔝 Head Expanse › 首端扩域 』"
	SECTION_NAMES        = []string{"URL Rewrite", "Map Local", "Script", "Body Rewrite", "Header Rewrite"}
	MODULE_RULE_SECTIONS = map[string]bool{"Rule": true, "Adblock Plus": true}

	SUPPORTED_POLICIES = map[string]bool{
		"REJECT": true, "REJECT-DROP": true, "REJECT-NO-DROP": true, "REJECT-TINYGIF": true, "REJECT-IMG": true, "DIRECT": true, "PROXY": true,
	}

	ALLOWED_RULE_PREFIXES = []string{
		"DOMAIN,", "DOMAIN-SUFFIX,", "DOMAIN-KEYWORD,", "DOMAIN-REGEX,", "DOMAIN-SET,", "IP-CIDR,", "IP-CIDR6,", "URL-REGEX,", "USER-AGENT,", "PROCESS-NAME,", "DEST-PORT,",
	}

	BODY_REWRITE_PREFIXES = []string{"http-response", "http-request", "http-response-jq", "http-request-jq"}

	UNSAFE_IP_RULE_PREFIXES = []string{
		"IP-CIDR,0.0.0.", "IP-CIDR,127.", "IP-CIDR,10.0.0.0/8", "IP-CIDR,192.168.0.0/16", "IP-CIDR,172.16.0.0/12",
	}

	targetModule     = filepath.Join(hub.SURGE_HEAD_EXPANSE_DIR, "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule")
	targetModuleLite = filepath.Join(hub.SURGE_HEAD_EXPANSE_DIR, "📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule")
)

type SourceEntry struct {
	ManifestPath string
	Source       string
	Policy       string
	Kind         string
}

func (e *SourceEntry) IsRemote() bool {
	return strings.HasPrefix(e.Source, "http://") || strings.HasPrefix(e.Source, "https://")
}

func (e *SourceEntry) ResolvedPath() string {
	if e.IsRemote() {
		return e.Source
	}
	decoded, _ := url.QueryUnescape(e.Source)
	if strings.HasPrefix(decoded, "/") {
		return filepath.Join(hub.ROOT, strings.TrimPrefix(decoded, "/"))
	}
	return filepath.Join(filepath.Dir(e.ManifestPath), decoded)
}

func (e *SourceEntry) DisplayName() string {
	if e.IsRemote() {
		parts := strings.Split(e.Source, "/")
		return parts[len(parts)-1]
	}
	rel, err := filepath.Rel(hub.ROOT, e.ResolvedPath())
	if err == nil {
		return rel
	}
	return filepath.Base(e.ResolvedPath())
}

type AdBlockManager struct {
	WhitelistDomain        map[string]bool
	WhitelistSuffix        map[string]bool
	WhitelistKeyword       map[string]bool
	WhitelistIpNetworks    []*net.IPNet
	WhitelistIpCidrRaw     map[string]bool
	Rules                  map[string]map[string]map[string]bool
	SeenRules              map[string]map[string]bool
	SeenSuffixes           map[string]map[string]bool
	Sections               map[string][]string
	SectionSeen            map[string]map[string]bool
	FunctionalStats        map[string]int
	FunctionalManifestPath map[string]bool
	MitmHosts              map[string]bool
	Hashes                 map[string]string
}

func NewAdBlockManager() *AdBlockManager {
	m := &AdBlockManager{
		WhitelistDomain:        make(map[string]bool),
		WhitelistSuffix:        make(map[string]bool),
		WhitelistKeyword:       make(map[string]bool),
		WhitelistIpNetworks:    []*net.IPNet{},
		WhitelistIpCidrRaw:     make(map[string]bool),
		Rules:                  make(map[string]map[string]map[string]bool),
		SeenRules:              make(map[string]map[string]bool),
		SeenSuffixes:           make(map[string]map[string]bool),
		Sections:               make(map[string][]string),
		SectionSeen:            make(map[string]map[string]bool),
		FunctionalStats:        make(map[string]int),
		FunctionalManifestPath: make(map[string]bool),
		MitmHosts:              make(map[string]bool),
		Hashes:                 make(map[string]string),
	}

	for _, policy := range []string{"REJECT", "REJECT-DROP", "REJECT-NO-DROP", "DIRECT"} {
		m.Rules[policy] = make(map[string]map[string]bool)
		for _, cat := range categoryNames {
			m.Rules[policy][cat] = make(map[string]bool)
		}
		m.SeenRules[policy] = make(map[string]bool)
		m.SeenSuffixes[policy] = make(map[string]bool)
	}

	for _, name := range SECTION_NAMES {
		m.Sections[name] = []string{}
		m.SectionSeen[name] = make(map[string]bool)
	}

	return m
}
func (m *AdBlockManager) CleanupLocalLists() {
	targets := []string{hub.ADBLOCK_LIST, hub.SKK_REJECT, hub.SKK_HTTPDNS}

	sourceEntries := m.LoadSourceEntries()
	for _, entry := range sourceEntries {
		if !entry.IsRemote() && strings.HasSuffix(entry.ResolvedPath(), ".list") {
			targets = append(targets, entry.ResolvedPath())
		}
	}

	if hub.ValidateFileExists(hub.HTTPDNS_HIJACK_LIST, "") {
		targets = append(targets, hub.HTTPDNS_HIJACK_LIST)
	}

	for _, path := range hub.UniqueStrings(targets) {
		if !hub.ValidateFileExists(path, "") {
			continue
		}
		var newLines []string
		changed := false
		for line := range hub.IterLines(path) {
			stripped := strings.TrimSpace(line)
			if stripped == "" || strings.HasPrefix(stripped, "#") {
				newLines = append(newLines, line)
				continue
			}
			rulePayload := stripped
			if idx := strings.Index(stripped, ","); idx != -1 {
				rulePayload = stripped[idx+1:]
			}
			if m.IsWhitelisted(rulePayload) {
				changed = true
				continue
			}
			newLines = append(newLines, line)
		}
		if changed {
			hub.Info(fmt.Sprintf("Scrubbed whitelist items from %s", filepath.Base(path)))
			content := strings.Join(newLines, "\n")
			if !strings.HasSuffix(content, "\n") && len(newLines) > 0 {
				content += "\n"
			}
			hub.SafeWriteFile(path, content, false)
		}
	}
}

func (m *AdBlockManager) LoadWhitelist() {
	if !hub.ValidateFileExists(hub.WHITELIST_FILE, "") {
		return
	}
	for line := range hub.IterLines(hub.WHITELIST_FILE) {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if strings.HasPrefix(line, "DOMAIN-SUFFIX,") {
			m.WhitelistSuffix[strings.TrimSpace(strings.SplitN(line, ",", 2)[1])] = true
		} else if strings.HasPrefix(line, "DOMAIN-KEYWORD,") {
			m.WhitelistKeyword[strings.TrimSpace(strings.SplitN(line, ",", 2)[1])] = true
		} else if strings.HasPrefix(line, "DOMAIN,") {
			m.WhitelistDomain[strings.TrimSpace(strings.SplitN(line, ",", 2)[1])] = true
		} else if strings.HasPrefix(line, "IP-CIDR6,") || strings.HasPrefix(line, "IP-CIDR,") {
			parts := strings.Split(line, ",")
			if len(parts) >= 2 {
				cidr := strings.TrimSpace(parts[1])
				m.WhitelistIpCidrRaw[cidr] = true
				_, network, err := net.ParseCIDR(cidr)
				if err == nil {
					m.WhitelistIpNetworks = append(m.WhitelistIpNetworks, network)
				} else {
					hub.Warn(fmt.Sprintf("Invalid IP-CIDR in whitelist: %s - %v", cidr, err))
				}
			}
		}
	}
	total := len(m.WhitelistDomain) + len(m.WhitelistSuffix) + len(m.WhitelistKeyword) + len(m.WhitelistIpCidrRaw)
	hub.Info(fmt.Sprintf("Loaded %d whitelist patterns (%d IP-CIDR)", total, len(m.WhitelistIpCidrRaw)))
}

func (m *AdBlockManager) IsWhitelisted(rulePayload string) bool {
	if m.WhitelistDomain[rulePayload] {
		return true
	}
	for suffix := range m.WhitelistSuffix {
		if rulePayload == suffix || strings.HasSuffix(rulePayload, "."+suffix) {
			return true
		}
	}
	for keyword := range m.WhitelistKeyword {
		if strings.Contains(rulePayload, keyword) {
			return true
		}
	}
	if m.WhitelistIpCidrRaw[rulePayload] {
		return true
	}

	ip, blockedNet, err := net.ParseCIDR(rulePayload)
	if err != nil {
		ip = net.ParseIP(rulePayload)
		if ip != nil {
			mask := net.CIDRMask(128, 128)
			if ip.To4() != nil {
				mask = net.CIDRMask(32, 32)
			}
			blockedNet = &net.IPNet{IP: ip, Mask: mask}
		}
	}

	if blockedNet != nil {
		for _, wlNet := range m.WhitelistIpNetworks {
			if wlNet.Contains(blockedNet.IP) || blockedNet.Contains(wlNet.IP) {
				return true
			}
		}
	}

	return false
}

func (m *AdBlockManager) LoadHashes() {
	if !hub.ValidateFileExists(hub.ADBLOCK_HASH_FILE, "") {
		return
	}
	for line := range hub.IterLines(hub.ADBLOCK_HASH_FILE) {
		if !strings.Contains(line, "|") {
			continue
		}
		parts := strings.SplitN(strings.TrimSpace(line), "|", 2)
		m.Hashes[parts[0]] = parts[1]
	}
}

func (m *AdBlockManager) SaveHashes(hashes map[string]string) {
	var keys []string
	for k := range hashes {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	var content strings.Builder
	for _, k := range keys {
		content.WriteString(fmt.Sprintf("%s|%s\n", k, hashes[k]))
	}
	hub.SafeWriteFile(hub.ADBLOCK_HASH_FILE, content.String(), false)
}

func (m *AdBlockManager) download(u string) string {
	decoded, _ := url.QueryUnescape(u)
	return hub.SafeDownload(decoded, 2, 60)
}

func (m *AdBlockManager) LoadSourceEntries() []SourceEntry {
	var entries []SourceEntry
	seen := make(map[string]bool)

	if !hub.ValidateFileExists(hub.ADBLOCK_SOURCES_FILE, "") {
		hub.Warn(fmt.Sprintf("Sources file not found: %s", hub.ADBLOCK_SOURCES_FILE))
		return entries
	}

	for line := range hub.IterLines(hub.ADBLOCK_SOURCES_FILE) {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}

		parts := strings.Split(line, "|")
		for i := range parts {
			parts[i] = strings.TrimSpace(parts[i])
		}

		source := parts[0]
		policy := "REJECT"
		if len(parts) > 1 && parts[1] != "" {
			policy = strings.ToUpper(parts[1])
		}
		kind := "auto"
		if len(parts) > 2 && parts[2] != "" {
			kind = strings.ToLower(parts[2])
		}

		if !SUPPORTED_POLICIES[policy] {
			hub.Warn(fmt.Sprintf("Skipping invalid policy in AdBlock manifest: %s", line))
			continue
		}
		if kind != "auto" && kind != "domainset" {
			hub.Warn(fmt.Sprintf("Skipping invalid source kind in AdBlock manifest: %s", line))
			continue
		}

		entry := SourceEntry{
			ManifestPath: hub.ADBLOCK_SOURCES_FILE,
			Source:       source,
			Policy:       policy,
			Kind:         kind,
		}

		key := fmt.Sprintf("%s|%s|%s", entry.Source, entry.Policy, entry.Kind)
		if seen[key] {
			continue
		}
		seen[key] = true
		entries = append(entries, entry)
	}

	hub.Info(fmt.Sprintf("Loaded %d canonical AdBlock sources", len(entries)))
	return entries
}

func (m *AdBlockManager) ResolveModuleIngestMode(path string, fromManifest bool) string {
	if !hub.ValidateFileExists(path, "") {
		return "skip"
	}
	name := filepath.Base(path)
	if FUNCTIONAL_BLOCK_LOCAL_SOURCES[name] {
		return "skip"
	}
	if ADBLOCK_FILENAME_ALLOWLIST[name] {
		return "full"
	}
	low := strings.ToLower(name)

	if !fromManifest {
		dir := filepath.Dir(filepath.Clean(path))
		nexusDir := filepath.Clean(hub.SURGE_AMPLIFY_NEXUS_DIR)
		if dir == nexusDir {
			return "skip"
		}
	}

	if !fromManifest {
		for _, tok := range FUNCTIONAL_BLOCK_NAME_TOKENS {
			if strings.Contains(low, tok) {
				return "skip"
			}
		}
	}

	text := hub.ReadFileString(path)
	mode := hub.ModuleIngestMode(path, text)
	if mode != "skip" {
		return mode
	}

	dir := filepath.Dir(filepath.Clean(path))
	localDir := filepath.Clean(hub.MODULE_LOCAL_DIR)
	if dir == localDir {
		if strings.HasSuffix(name, ".sgmodule") || strings.HasSuffix(name, ".module") {
			return "full"
		}
		return "skip"
	}

	for _, tok := range ADBLOCK_MODULE_NAME_TOKENS {
		if strings.Contains(low, tok) {
			return "full"
		}
	}

	return "skip"
}

func (m *AdBlockManager) LoadFunctionalSourcePaths() ([][2]string, []string) {
	var paths [][2]string
	var remoteUrls []string
	seen := make(map[string]bool)
	m.FunctionalManifestPath = make(map[string]bool)

	add := func(p string, fromManifest bool) {
		norm := filepath.Clean(p)
		if seen[norm] || !hub.ValidateFileExists(norm, "") {
			return
		}
		mode := m.ResolveModuleIngestMode(norm, fromManifest)
		if mode == "skip" {
			rel, _ := filepath.Rel(hub.ROOT, norm)
			hub.Warn(fmt.Sprintf("  ⊘ Skipped (no ad / unlock-only): %s", rel))
			return
		}
		if fromManifest {
			m.FunctionalManifestPath[norm] = true
		}
		seen[norm] = true
		paths = append(paths, [2]string{norm, mode})
	}

	if hub.ValidateFileExists(hub.ADBLOCK_FUNCTIONAL_SOURCES_FILE, "") {
		for rawLine := range hub.IterLines(hub.ADBLOCK_FUNCTIONAL_SOURCES_FILE) {
			line := strings.TrimSpace(rawLine)
			if line == "" || strings.HasPrefix(line, "#") {
				continue
			}
			if strings.HasPrefix(line, "http://") || strings.HasPrefix(line, "https://") {
				remoteUrls = append(remoteUrls, line)
				continue
			}
			resolved := filepath.Clean(filepath.Join(filepath.Dir(hub.ADBLOCK_FUNCTIONAL_SOURCES_FILE), line))
			add(resolved, true)
		}
	}

	if hub.ValidateDirExists(filepath.Join(hub.ROOT, "rulesets/Sources"), "") {
		filepath.WalkDir(filepath.Join(hub.ROOT, "rulesets/Sources"), func(path string, d os.DirEntry, err error) error {
			if err != nil || d.IsDir() {
				return nil
			}
			if strings.HasSuffix(d.Name(), ".sgmodule") || strings.HasSuffix(d.Name(), ".module") {
				add(path, false)
			}
			return nil
		})
	}

	if hub.ValidateDirExists(hub.MODULE_LOCAL_DIR, "") {
		protectedModules := map[string]bool{
			"script_hub.surge.sgmodule": true,
		}
		filepath.WalkDir(hub.MODULE_LOCAL_DIR, func(path string, d os.DirEntry, err error) error {
			if err != nil || d.IsDir() {
				return nil
			}
			if protectedModules[d.Name()] {
				return nil
			}
			if strings.HasSuffix(d.Name(), ".sgmodule") || strings.HasSuffix(d.Name(), ".module") {
				add(path, false)
			}
			return nil
		})
	}

	splitN := 0
	for _, p := range paths {
		if p[1] == "split" {
			splitN++
		}
	}
	hub.Info(fmt.Sprintf("Resolved %d functional module(s) (%d mixed→line-split), %d remote URL(s)", len(paths), splitN, len(remoteUrls)))
	return paths, remoteUrls
}
