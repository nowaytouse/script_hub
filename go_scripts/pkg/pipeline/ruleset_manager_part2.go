package pipeline

import (
	"crypto/md5"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

func (rm *RulesetManager) surgeConvertDomainRegex(rule string) string {
	return hub.ConvertDomainRegexForSurge(rule)
}

func (rm *RulesetManager) cleanRule(rule, rulesetName string) string {
	r := strings.TrimSpace(rule)
	if r == "" || strings.HasPrefix(r, "#") {
		return ""
	}

	if r == "payload:" || r == "payload" {
		return ""
	}
	if strings.HasPrefix(r, "-") {
		r = strings.TrimSpace(r[1:])
		r = strings.Trim(r, "\"'")
		if r == "" {
			return ""
		}
	}

	if !strings.Contains(r, ",") {
		if strings.Contains(r, ":") || (strings.Count(r, ".") == 3 && strings.ContainsAny(strings.Split(r, "/")[0], "0123456789")) {
			if strings.Contains(r, "/") {
				if strings.Contains(r, ":") {
					r = "IP-CIDR6," + r
				} else {
					r = "IP-CIDR," + r
				}
			} else {
				if strings.Contains(r, ":") {
					r = "IP-CIDR6," + r + "/128"
				} else {
					r = "IP-CIDR," + r + "/32"
				}
			}
		} else {
			if strings.HasPrefix(r, ".") {
				r = "DOMAIN-SUFFIX," + r[1:]
			} else if strings.HasPrefix(r, "+.") {
				r = "DOMAIN-SUFFIX," + r[2:]
			} else if strings.HasPrefix(r, "+") {
				r = "DOMAIN-SUFFIX," + r[1:]
			} else {
				r = "DOMAIN," + r
			}
		}
	}

	normalizedPtr := rm.processor.NormalizeRule(r, rulesetName)
	if normalizedPtr == nil {
		return ""
	}
	normalized := *normalizedPtr

	if strings.HasPrefix(normalized, "DOMAIN-REGEX,") {
		return rm.surgeConvertDomainRegex(normalized)
	}

	skipConflict := false
	for _, sc := range skipConflictCheck {
		if sc == rulesetName {
			skipConflict = true
			break
		}
	}
	if !skipConflict {
		for _, domain := range conflictDomains {
			if strings.HasSuffix(normalized, ","+domain) {
				return ""
			}
		}
	}

	if rulesetName == "Direct" {
		rLower := strings.ToLower(normalized)
		for _, kw := range mandatoryProxyKeywords {
			if strings.Contains(rLower, kw) {
				return ""
			}
		}
	}

	return normalized
}

func (rm *RulesetManager) generateHeader(name string, ruleCount int) string {
	baseName := strings.ReplaceAll(name, "_ip", "")
	var pInfo map[string]string
	if info, ok := rm.policyMap[name]; ok {
		pInfo = info
	} else if info, ok := rm.policyMap[baseName]; ok {
		pInfo = info
	} else {
		pInfo = map[string]string{
			"policy": "PROXY",
			"node":   "Any",
			"desc":   "Auto-merged ruleset",
		}
	}

	var header []string
	header = append(header, fmt.Sprintf("# Ruleset: %s", name))
	header = append(header, fmt.Sprintf("# Policy: %s", pInfo["policy"]))
	if pInfo["node"] != "" {
		header = append(header, fmt.Sprintf("# Node: %s", pInfo["node"]))
	}
	header = append(header, fmt.Sprintf("# Description: %s", pInfo["desc"]))
	header = append(header, fmt.Sprintf("# Rules: %d", ruleCount))

	return strings.Join(header, "\n") + "\n"
}

func (rm *RulesetManager) processRuleset(name string) {
	targetFile := filepath.Join(surgeDir, fmt.Sprintf("%s.list", name))
	for _, dep := range deprecatedRulesets {
		if name == dep {
			if hub.ValidateFileExists(targetFile, "") {
				if err := os.Remove(targetFile); err == nil {
					rm.stats["deleted"]++
					hub.Warn(fmt.Sprintf("Removed deprecated: %s", name))
				} else {
					hub.Error(fmt.Sprintf("Failed to remove deprecated: %s", name))
				}
			}
			return
		}
	}

	hasher := md5.New()
	found := false

	manifests := []string{
		filepath.Join(hub.LINKS_DIR, fmt.Sprintf("%s_sources.list", name)),
		rm.findCaseInsensitiveFile(hub.METACUBEX_DIR, fmt.Sprintf("MetaCubeX_%s.list", name)),
	}

	for _, f := range manifests {
		if f != "" && hub.ValidateFileExists(f, "") {
			content, _ := os.ReadFile(f)
			hasher.Write(content)
			if strings.HasSuffix(f, "_sources.list") {
				for _, line := range strings.Split(string(content), "\n") {
					line = strings.TrimSpace(line)
					if line != "" && !strings.HasPrefix(line, "#") && !strings.HasPrefix(line, "http") {
						localRef := filepath.Join(hub.LINKS_DIR, strings.Split(line, "|")[0])
						if hub.ValidateFileExists(localRef, "") {
							c, _ := os.ReadFile(localRef)
							hasher.Write(c)
						}
					}
				}
			}
			found = true
		}
	}

	for _, prot := range protectedRulesets {
		if name == prot && hub.ValidateFileExists(targetFile, "") {
			content := hub.ReadFileString(targetFile)
			var validLines []string
			for _, l := range strings.Split(content, "\n") {
				l = strings.TrimSpace(l)
				if l != "" && !strings.HasPrefix(l, "#") {
					validLines = append(validLines, l)
				}
			}
			hasher.Write([]byte(strings.Join(validLines, "")))
			found = true
			break
		}
	}

	currentHash := ""
	if found {
		currentHash = hex.EncodeToString(hasher.Sum(nil))
	}

	hasRemote := false
	sFile := filepath.Join(hub.LINKS_DIR, fmt.Sprintf("%s_sources.list", name))
	if hub.ValidateFileExists(sFile, "") {
		for _, line := range strings.Split(hub.ReadFileString(sFile), "\n") {
			if strings.HasPrefix(strings.TrimSpace(line), "http") {
				hasRemote = true
				break
			}
		}
	}

	if !rm.force && hub.ValidateFileExists(targetFile, "") && rm.hashes[name] == currentHash && !hasRemote {
		rm.stats["skipped"]++
		return
	}

	allRules := make(map[string]bool)
	isProtected := false
	for _, p := range protectedRulesets {
		if name == p {
			isProtected = true
			break
		}
	}

	if isProtected {
		if hub.ValidateFileExists(targetFile, "") {
			for _, line := range strings.Split(hub.ReadFileString(targetFile), "\n") {
				cleaned := rm.cleanRule(line, name)
				if cleaned != "" {
					allRules[cleaned] = true
				}
			}
		}
	} else {
		mFile := rm.findCaseInsensitiveFile(hub.METACUBEX_DIR, fmt.Sprintf("MetaCubeX_%s.list", name))
		if mFile != "" && hub.ValidateFileExists(mFile, "") {
			for _, line := range strings.Split(hub.ReadFileString(mFile), "\n") {
				cleaned := rm.cleanRule(line, name)
				if cleaned != "" {
					allRules[cleaned] = true
				}
			}
		}
		if hub.ValidateFileExists(sFile, "") {
			for _, line := range strings.Split(hub.ReadFileString(sFile), "\n") {
				line = strings.TrimSpace(line)
				if line == "" || strings.HasPrefix(line, "#") {
					continue
				}
				if strings.HasPrefix(line, "http") {
					remoteContent := rm.download(line)
					if remoteContent != "" {
						for _, rLine := range strings.Split(remoteContent, "\n") {
							cleaned := rm.cleanRule(rLine, name)
							if cleaned != "" {
								allRules[cleaned] = true
							}
						}
					}
				} else {
					localPath := filepath.Join(hub.LINKS_DIR, strings.Split(line, "|")[0])
					if hub.ValidateFileExists(localPath, "") {
						for _, rLine := range strings.Split(hub.ReadFileString(localPath), "\n") {
							cleaned := rm.cleanRule(rLine, name)
							if cleaned != "" {
								allRules[cleaned] = true
							}
						}
					}
				}
			}
		}
	}

	if len(allRules) == 0 && !isProtected {
		return
	}

	var nonIpRules, ipRules []string
	for rule := range allRules {
		if name != "Direct" && (strings.HasPrefix(rule, "IP-CIDR,") || strings.HasPrefix(rule, "IP-CIDR6,") || strings.HasPrefix(rule, "IP-ASN,") || strings.HasPrefix(rule, "GEOIP,")) {
			ipRules = append(ipRules, rule)
		} else {
			nonIpRules = append(nonIpRules, rule)
		}
	}
	sort.Strings(nonIpRules)
	sort.Strings(ipRules)

	targetIpFile := filepath.Join(surgeDir, fmt.Sprintf("%s_ip.list", name))
	if len(ipRules) > 0 {
		content := rm.generateHeader(fmt.Sprintf("%s_ip", name), len(ipRules)) + strings.Join(ipRules, "\n") + "\n"
		hub.SafeWriteFile(targetIpFile, content, true)
		hub.Success(fmt.Sprintf("Processed IP Ruleset: %s_ip (%d rules)", name, len(ipRules)))
	} else {
		if hub.ValidateFileExists(targetIpFile, "") {
			os.Remove(targetIpFile)
		}
	}

	if len(nonIpRules) > 0 || isProtected {
		content := rm.generateHeader(name, len(nonIpRules)) + strings.Join(nonIpRules, "\n") + "\n"
		hub.SafeWriteFile(targetFile, content, true)
		hub.Success(fmt.Sprintf("Processed Domain Ruleset: %s (%d rules)", name, len(nonIpRules)))
	} else {
		if hub.ValidateFileExists(targetFile, "") {
			os.Remove(targetFile)
		}
	}

	rm.hashes[name] = currentHash
	rm.stats["merged"]++
}
