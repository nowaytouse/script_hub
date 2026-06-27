package pipeline

import (
	"fmt"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

var firewallModules []string

func init() {
	firewallModules = []string{
		filepath.Join(hub.SURGE_HEAD_EXPANSE_DIR, "🔥 Firewall Port Blocker 🛡️🚫.sgmodule"),
		filepath.Join(hub.SHADOWROCKET_HEAD_EXPANSE_DIR, "🔥 Firewall Port Blocker 🛡️🚫.module"),
	}
}

func SyncPorts(execute bool) {
	hub.Section("Firewall Port Sync (English Script Interface)")

	portsSource := filepath.Join(hub.SOURCES_DIR, "conf/SurgeConf_DirectPorts.list")

	if !hub.ValidateFileExists(portsSource, "") {
		hub.Warn(fmt.Sprintf("Port rules source not found: %s", portsSource))
		return
	}

	sourceLines := strings.Split(hub.ReadFileString(portsSource), "\n")
	var portRules []string
	var invalidRules []string
	portPattern := regexp.MustCompile(`^(IN-PORT|DEST-PORT|SRC-PORT)`)

	for _, line := range sourceLines {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		if portPattern.MatchString(line) {
			parts := strings.Split(line, ",")
			if len(parts) < 3 {
				invalidRules = append(invalidRules, line)
				hub.Warn(fmt.Sprintf("  Invalid rule (missing action): %s", line))
			} else {
				portRules = append(portRules, line)
			}
		}
	}

	if len(invalidRules) > 0 {
		hub.Warn(fmt.Sprintf("Skipping %d invalid rules (missing action)", len(invalidRules)))
		for _, rule := range invalidRules {
			hub.Warn(fmt.Sprintf("  - %s", rule))
		}
	}

	if len(portRules) == 0 {
		hub.Warn("No port rules found in source.")
		return
	}

	hub.Info(fmt.Sprintf("Extracted %d valid rules from source.", len(portRules)))

	for _, modulePath := range firewallModules {
		if !hub.ValidateFileExists(modulePath, "") {
			hub.Warn(fmt.Sprintf("Firewall module not found: %s", modulePath))
			continue
		}

		moduleLines := strings.Split(hub.ReadFileString(modulePath), "\n")
		var newModuleContent []string
		startMarker := "# --- SYNCED PORT RULES START ---"
		endMarker := "# --- SYNCED PORT RULES END ---"
		managedKeys := make(map[string]bool)

		for _, rule := range portRules {
			parts := strings.Split(rule, ",")
			if len(parts) >= 2 {
				key := strings.ToUpper(strings.TrimSpace(parts[0])) + "," + strings.ToUpper(strings.TrimSpace(parts[1]))
				managedKeys[key] = true
			}
		}

		skip := false
		inRuleSection := false

		for _, line := range moduleLines {
			stripped := strings.TrimSpace(line)
			if strings.HasPrefix(stripped, "[") && strings.HasSuffix(stripped, "]") {
				inRuleSection = (strings.ToLower(stripped) == "[rule]")
			}
			if strings.Contains(line, startMarker) {
				newModuleContent = append(newModuleContent, line)
				newModuleContent = append(newModuleContent, "# Automated update from ports source")

				sort.Strings(portRules)
				for _, rule := range portRules {
					newModuleContent = append(newModuleContent, rule)
				}
				skip = true
				continue
			}
			if strings.Contains(line, endMarker) {
				skip = false
				newModuleContent = append(newModuleContent, line)
				continue
			}
			if !skip {
				if inRuleSection && stripped != "" && !strings.HasPrefix(stripped, "#") {
					parts := strings.Split(stripped, ",")
					if len(parts) >= 2 {
						key := strings.ToUpper(strings.TrimSpace(parts[0])) + "," + strings.ToUpper(strings.TrimSpace(parts[1]))
						if managedKeys[key] {
							continue
						}
					}
				}
				newModuleContent = append(newModuleContent, line)
			}
		}

		finalContent := strings.Join(newModuleContent, "\n")
		if !strings.HasSuffix(finalContent, "\n") {
			finalContent += "\n"
		}

		if execute {
			versionRe := regexp.MustCompile(`#!version=.*`)
			newVersion := fmt.Sprintf("#!version=%s", time.Now().Format("2006.01.02"))
			finalContent = versionRe.ReplaceAllString(finalContent, newVersion)
			hub.SafeWriteFile(modulePath, finalContent, true)
			hub.Success(fmt.Sprintf("Firewall module updated successfully: %s", filepath.Base(modulePath)))
		} else {
			hub.Info(fmt.Sprintf("Dry Run: Use execute=true to apply changes to %s.", filepath.Base(modulePath)))
		}
	}
}
