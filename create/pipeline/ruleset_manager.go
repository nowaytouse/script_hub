package pipeline

import (
	"bytes"
	"compress/gzip"
	"compress/zlib"
	"fmt"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/nowaytouse/script_hub/create/hub"
	"github.com/nowaytouse/script_hub/create/network"
)

var downloadMu sync.Mutex


var (
	surgeDir      = hub.RULE_SET_DIR
	cacheFile     = filepath.Join(hub.CACHE_DIR, "merge_hashes.list")
	policyMapFile = filepath.Join(hub.SOURCES_DIR, "ruleset_policy_map.list")
)

var protectedRulesets = []string{}

var skipConflictCheck = []string{
	"SocialMedia", "GlobalProxy", "GlobalMedia", "SYSTEM", "Direct", "Spotify",
	"TikTok", "Telegram", "Twitter", "Twitch", "Netflix", "Facebook", "Instagram",
	"Reddit", "StreamUS", "StreamJP", "StreamKR", "StreamEU", "StreamHK", "StreamTW",
}

var conflictDomains = []string{
	"x.com", "twitter.com", "facebook.com", "instagram.com", "reddit.com",
	"discord.com", "discordapp.com", "discordapp.net",
	"media.discordapp.net", "cdn.discordapp.com",
	"netflix.com", "hbomax.com", "hbo.com", "youtube.com", "youtu.be",
	"twitch.tv", "spotify.com", "itch.io", "steampowered.com", "epicgames.com",
	"images.pexels.com", "imgur.com", "happymag.tv", "wortfm.org",
}

var deprecatedRulesets = []string{
	"SYSTEM", "BlockHttpDNS", "FirewallPorts", "YouTube", "GoogleCN", "Steam", "Epic",
	"GamingProcess", "QQ", "WeChat", "DownloadProcess", "GlobalMedia", "XiaoHongShu",
	"NetEaseMusic", "Tencent", "AIProcess", "LAN", "Manual", "Manual_JP", "Manual_US",
	"Manual_West", "Manual_Global", "Telegram", "TikTok", "Twitter", "Instagram", "Reddit",
	"Discord", "Fediverse", "Bing", "Tesla", "ChinaDirect", "DirectProcess", "DownloadDirect",
}

var rulesets = []string{
	"AI", "Gaming", "GlobalProxy", "Microsoft", "NSFW",
	"SocialMedia",
	"Netflix", "Disney", "Spotify", "Bahamut", "AppleNews",
	"Google", "Apple", "GitHub", "PayPal", "Binance",
	"Direct", "Bilibili",
	"CDN",
	"StreamJP", "StreamUS", "StreamKR", "StreamHK", "StreamTW", "StreamEU",
}

var specialManagedRulesets = map[string]bool{"AdBlock": true, "substore": true}

type RulesetManager struct {
	force     bool
	hashes    map[string]string
	hashesMu  sync.Mutex
	policyMap map[string]map[string]string
	stats     map[string]int
	statsMu   sync.Mutex
	processor *hub.RuleProcessor
}

func NewRulesetManager(force bool) *RulesetManager {
	rm := &RulesetManager{
		force:     force,
		stats:     map[string]int{"merged": 0, "skipped": 0, "deleted": 0},
		processor: hub.NewRuleProcessor(false),
	}
	rm.hashes = rm.loadHashes()
	rm.policyMap = rm.loadPolicyMap()
	return rm
}

func (rm *RulesetManager) incStat(key string) {
	rm.statsMu.Lock()
	defer rm.statsMu.Unlock()
	rm.stats[key]++
}

func (rm *RulesetManager) getHash(name string) string {
	rm.hashesMu.Lock()
	defer rm.hashesMu.Unlock()
	return rm.hashes[name]
}

func (rm *RulesetManager) setHash(name, val string) {
	rm.hashesMu.Lock()
	defer rm.hashesMu.Unlock()
	rm.hashes[name] = val
}

func (rm *RulesetManager) findCaseInsensitiveFile(directory, filename string) string {
	exact := filepath.Join(directory, filename)
	if hub.ValidateFileExists(exact, "") {
		return exact
	}
	filenameLower := strings.ToLower(filename)
	entries, err := ReadDir(directory)
	if err == nil {
		for _, entry := range entries {
			if strings.ToLower(entry.Name()) == filenameLower {
				return filepath.Join(directory, entry.Name())
			}
		}
	}
	return ""
}

func (rm *RulesetManager) tryDecompress(data []byte) []byte {
	r, err := gzip.NewReader(bytes.NewReader(data))
	if err == nil {
		dec, err := ReadFile(r.Name)
		if err == nil {
			return dec
		}
	}

	r2, err := zlib.NewReader(bytes.NewReader(data))
	if err == nil {
		// Just a simple attempt, skipping detailed zlib window sizing for brevity
		var buf bytes.Buffer
		buf.ReadFrom(r2)
		return buf.Bytes()
	}
	return nil
}

func (rm *RulesetManager) extractRulesFromText(text string) []string {
	var rules []string
	prefixes := []string{"DOMAIN", "IP-CIDR", "USER-AGENT", "URL-REGEX", "GEOIP", "PROCESS-NAME", "DEST-PORT", "SRC-PORT"}
	lines := strings.Split(text, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, "//") {
			continue
		}
		hasPrefix := false
		for _, p := range prefixes {
			if strings.HasPrefix(line, p) {
				hasPrefix = true
				break
			}
		}
		if hasPrefix || !strings.Contains(line, ",") {
			if !hub.HasDangerousChars(line) {
				rules = append(rules, line)
			}
		}
	}
	return rules
}

func (rm *RulesetManager) download(url string) string {
	downloadMu.Lock()
	defer downloadMu.Unlock()

	// Enforce 1-second delay between network requests to prevent high-frequency git/network hits
	time.Sleep(1000 * time.Millisecond)

	isLsr := strings.HasSuffix(strings.ToLower(url), ".lsr")
	if isLsr {
		// Download binary
		raw := network.SafeDownload(url, 2, 60)
		if raw == "" {
			return ""
		}
		text := raw // simplifying binary handling in go for now
		rules := rm.extractRulesFromText(text)
		if len(rules) > 0 {
			return strings.Join(rules, "\n")
		}
		return ""
	} else {
		return network.SafeDownload(url, 2, 60)
	}
}

func (rm *RulesetManager) loadHashes() map[string]string {
	hashes := make(map[string]string)
	if hub.ValidateFileExists(cacheFile, "") {
		content := hub.ReadFileString(cacheFile)
		for _, line := range strings.Split(content, "\n") {
			if strings.Contains(line, ":") {
				parts := strings.SplitN(strings.TrimSpace(line), ":", 2)
				hashes[parts[0]] = parts[1]
			}
		}
	}
	return hashes
}

func (rm *RulesetManager) saveHashes() {
	var lines []string
	for k, v := range rm.hashes {
		lines = append(lines, fmt.Sprintf("%s:%s", k, v))
	}
	sort.Strings(lines)
	hub.SafeWriteFile(cacheFile, strings.Join(lines, "\n")+"\n", true)
}

func (rm *RulesetManager) loadPolicyMap() map[string]map[string]string {
	mapping := make(map[string]map[string]string)
	if hub.ValidateFileExists(policyMapFile, "") {
		content := hub.ReadFileString(policyMapFile)
		for _, line := range strings.Split(content, "\n") {
			line = strings.TrimSpace(line)
			if line == "" || strings.HasPrefix(line, "#") {
				continue
			}
			parts := strings.Split(line, "|")
			if len(parts) >= 2 {
				m := map[string]string{
					"policy": parts[1],
					"node":   "",
					"desc":   "",
				}
				if len(parts) > 2 {
					m["node"] = parts[2]
				}
				if len(parts) > 3 {
					m["desc"] = parts[3]
				}
				mapping[parts[0]] = m
			}
		}
	}
	return mapping
}
