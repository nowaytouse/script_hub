package pipeline

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"sort"
	"strings"
	"sync"

	"github.com/nyamiiko/script_hub/go_scripts/hub"
)

var extraSourceFiles = []string{
	filepath.Join(hub.ADBLOCK_DIR, "reject-drop.list"),
	filepath.Join(hub.ADBLOCK_DIR, "reject-no-drop.list"),
	filepath.Join(hub.CUSTOM_DIR, "substore.list"),
}

type SRSGenerator struct {
	singboxPath string
}

func NewSRSGenerator() *SRSGenerator {
	g := &SRSGenerator{}
	g.singboxPath = g.findSingbox()
	hub.EnsureDir(hub.SINGBOX_DIR)
	hub.EnsureDir(hub.CACHE_DIR)
	return g
}

func (g *SRSGenerator) findSingbox() string {
	out, err := exec.Command("which", "sing-box").Output()
	if err == nil && len(strings.TrimSpace(string(out))) > 0 {
		return strings.TrimSpace(string(out))
	}

	if hub.ValidateFileExists(hub.SINGBOX_LOCAL_BIN, "") {
		return hub.SINGBOX_LOCAL_BIN
	}
	if hub.ValidateFileExists(hub.SINGBOX_ROOT_BIN, "") {
		return hub.SINGBOX_ROOT_BIN
	}
	return ""
}

func (g *SRSGenerator) wildcardToRegex(value string) string {
	escaped := regexp.QuoteMeta(value)
	escaped = strings.ReplaceAll(escaped, `\*`, `.*`)
	escaped = strings.ReplaceAll(escaped, `\?`, `.`)
	return fmt.Sprintf("^%s$", escaped)
}

func (g *SRSGenerator) compileSrs(listFile string) {
	base := filepath.Base(listFile)
	name := strings.TrimSuffix(base, filepath.Ext(base))

	outDir := hub.SINGBOX_DIR
	if strings.Contains(listFile, "dns/mapping") || strings.Contains(listFile, "/dns/") {
		outDir = hub.SINGBOX_DNS_DIR
		hub.EnsureDir(outDir)
	} else if strings.Contains(listFile, "rulesets/AdBlock") {
		outDir = hub.ADBLOCK_DIR
		hub.EnsureDir(outDir)
	}

	srsFile := filepath.Join(outDir, fmt.Sprintf("%s_Singbox.srs", name))
	jsonTmp := filepath.Join(hub.CACHE_DIR, fmt.Sprintf("%s.json", name))
	jsonTmpWrite := jsonTmp + ".write"

	defer func() {
		os.Remove(jsonTmp)
		os.Remove(jsonTmpWrite)
	}()

	content := hub.ReadFileString(listFile)
	lines := strings.Split(content, "\n")

	mergedRules := map[string][]string{
		"domain":         {},
		"domain_suffix":  {},
		"domain_keyword": {},
		"domain_regex":   {},
		"ip_cidr":        {},
		"process_name":   {},
	}

	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		parts := strings.Split(line, ",")
		if len(parts) < 2 {
			continue
		}
		rtype := strings.TrimSpace(parts[0])
		val := strings.TrimSpace(parts[1])

		switch rtype {
		case "DOMAIN":
			mergedRules["domain"] = append(mergedRules["domain"], val)
		case "DOMAIN-SUFFIX":
			mergedRules["domain_suffix"] = append(mergedRules["domain_suffix"], val)
		case "DOMAIN-KEYWORD":
			mergedRules["domain_keyword"] = append(mergedRules["domain_keyword"], val)
		case "DOMAIN-REGEX":
			mergedRules["domain_regex"] = append(mergedRules["domain_regex"], val)
		case "DOMAIN-WILDCARD":
			mergedRules["domain_regex"] = append(mergedRules["domain_regex"], g.wildcardToRegex(val))
		case "IP-CIDR", "IP-CIDR6":
			mergedRules["ip_cidr"] = append(mergedRules["ip_cidr"], val)
		case "PROCESS-NAME":
			mergedRules["process_name"] = append(mergedRules["process_name"], val)
		}
	}

	finalRules := make(map[string][]string)
	for k, v := range mergedRules {
		if len(v) > 0 {
			finalRules[k] = hub.UniqueStrings(v)
			sort.Strings(finalRules[k])
		}
	}

	srsJson := map[string]interface{}{
		"version": 1,
		"rules":   []interface{}{finalRules},
	}

	data, err := json.Marshal(srsJson)
	if err != nil {
		hub.Error(fmt.Sprintf("Error encoding JSON for %s: %v", name, err))
		return
	}

	if err := hub.SafeWriteFile(jsonTmpWrite, string(data), true); err != nil {
		hub.Error(fmt.Sprintf("Error writing JSON for %s: %v", name, err))
		return
	}
	os.Rename(jsonTmpWrite, jsonTmp)

	cmd := exec.Command(g.singboxPath, "rule-set", "compile", jsonTmp, "-o", srsFile)
	out, err := cmd.CombinedOutput()
	if err != nil {
		hub.Error(fmt.Sprintf("Failed to compile %s: %s", name, string(out)))
	} else {
		hub.Success(fmt.Sprintf("Compiled SRS: %s", name))
	}
}

func (g *SRSGenerator) pruneStaleSrs(listFiles []string) {
	expectedSrs := make(map[string]bool)
	for _, lf := range listFiles {
		base := filepath.Base(lf)
		name := strings.TrimSuffix(base, filepath.Ext(base))
		expectedSrs[fmt.Sprintf("%s_Singbox.srs", name)] = true
	}

	entries, _ := os.ReadDir(hub.SINGBOX_DIR)
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), "GeoIP_") && strings.HasSuffix(entry.Name(), ".srs") {
			expectedSrs[entry.Name()] = true
		}
	}

	for _, entry := range entries {
		if !strings.HasSuffix(entry.Name(), ".srs") {
			continue
		}
		if !expectedSrs[entry.Name()] {
			path := filepath.Join(hub.SINGBOX_DIR, entry.Name())
			if err := os.Remove(path); err == nil {
				hub.Warn(fmt.Sprintf("Pruned stale SRS: %s", entry.Name()))
			} else {
				hub.Error(fmt.Sprintf("Failed to prune stale SRS: %s", entry.Name()))
			}
		}
	}

	if hub.ValidateDirExists(hub.SINGBOX_DNS_DIR, "") {
		dnsEntries, _ := os.ReadDir(hub.SINGBOX_DNS_DIR)
		for _, entry := range dnsEntries {
			if !strings.HasSuffix(entry.Name(), ".srs") {
				continue
			}
			if !expectedSrs[entry.Name()] {
				path := filepath.Join(hub.SINGBOX_DNS_DIR, entry.Name())
				if err := os.Remove(path); err == nil {
					hub.Warn(fmt.Sprintf("Pruned stale DNS SRS: %s", entry.Name()))
				} else {
					hub.Error(fmt.Sprintf("Failed to prune stale DNS SRS: %s", entry.Name()))
				}
			}
		}
	}
}

func (g *SRSGenerator) Run() {
	hub.Section("Sing-box SRS Generation (English Interface)")
	if g.singboxPath == "" {
		hub.Error("sing-box binary not found. Skipping SRS generation.")
		return
	}

	sourceDirs := []string{hub.RULE_SET_DIR, hub.DNS_MAPPING_DIR, hub.ADBLOCK_DIR}
	var listFiles []string

	for _, dir := range sourceDirs {
		if !hub.ValidateDirExists(dir, "") {
			continue
		}
		entries, err := os.ReadDir(dir)
		if err != nil {
			continue
		}
		for _, entry := range entries {
			if !entry.IsDir() && strings.HasSuffix(entry.Name(), ".list") {
				listFiles = append(listFiles, filepath.Join(dir, entry.Name()))
			}
		}
	}

	for _, path := range extraSourceFiles {
		if hub.ValidateFileExists(path, "") {
			listFiles = append(listFiles, path)
		}
	}

	listFiles = hub.UniqueStrings(listFiles)

	// Compile in parallel using a bounded worker pool matching the CPU count to maximize efficiency
	numWorkers := runtime.NumCPU()
	if numWorkers < 1 {
		numWorkers = 1
	}
	sem := make(chan struct{}, numWorkers)
	var wg sync.WaitGroup

	for _, lf := range listFiles {
		wg.Add(1)
		sem <- struct{}{}
		go func(path string) {
			defer wg.Done()
			defer func() { <-sem }()
			g.compileSrs(path)
		}(lf)
	}
	wg.Wait()

	g.pruneStaleSrs(listFiles)
}
