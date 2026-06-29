package tools

import (
	"crypto/md5"
	"encoding/hex"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/hub"
)

var categoryMap = map[string]string{
	"amplify_nexus": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
	"head_expanse":  "『 🔝 Head Expanse › 首端扩域 』",
}

func getModuleName(content string) string {
	m := regexp.MustCompile(`(?m)^#!name\s*[=:]\s*(.+)$`).FindStringSubmatch(content)
	if m != nil {
		return strings.TrimSpace(m[1])
	}
	return ""
}

func getContentHash(content string) string {
	var filtered []string
	for _, line := range strings.Split(content, "\n") {
		if !strings.HasPrefix(line, "#!category") && !strings.HasPrefix(line, "#!url") {
			filtered = append(filtered, line)
		}
	}
	hash := md5.Sum([]byte(strings.Join(filtered, "\n")))
	return hex.EncodeToString(hash[:])[:8]
}

func classifyModule(name, content string) string {
	nameLower := strings.ToLower(name)
	kwsNexus := []string{"wifi", "calling", "helper", "enhanced", "dns", "iringo", "dualsubs", "tiktok", "助手"}
	for _, k := range kwsNexus {
		if strings.Contains(nameLower, k) {
			return "amplify_nexus"
		}
	}
	kwsExpanse := []string{"ad-block", "adblock", "firewall", "script hub", "广告平台", "广告联盟", "universal"}
	for _, k := range kwsExpanse {
		if strings.Contains(nameLower, k) {
			return "head_expanse"
		}
	}
	return "narrow_pierce"
}

func RunImportFromIcloudSr() int {
	fmt.Println(strings.Repeat("=", 60))
	fmt.Println("🚀 Shadowrocket to Surge Module Importer")
	fmt.Println(strings.Repeat("=", 60))

	srDirStr := os.Getenv("SHADOWROCKET_MODULES_DIR")
	if srDirStr == "" {
		homeDir, _ := os.UserHomeDir()
		srDirStr = filepath.Join(homeDir, "Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules")
	}

	if !hub.ValidateDirExists(srDirStr, "") {
		fmt.Printf("❌ Shadowrocket directory not found: %s\n", srDirStr)
		return 1
	}

	surgeDir := filepath.Join(hub.ROOT, "modules/surge")

	type existingInfo struct {
		path string
		hash string
		name string
	}
	existing := make(map[string]existingInfo)

	for _, cat := range []string{"amplify_nexus", "head_expanse"} {
		catPath := filepath.Join(surgeDir, cat)
		if !hub.ValidateDirExists(catPath, "") {
			continue
		}
		entries, _ := os.ReadDir(catPath)
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".sgmodule") {
				fPath := filepath.Join(catPath, e.Name())
				content := hub.ReadFileString(fPath)
				if content == "" {
					continue
				}
				name := getModuleName(content)
				if name == "" {
					name = strings.TrimSuffix(e.Name(), ".sgmodule")
				}
				existing[strings.ToLower(name)] = existingInfo{
					path: fPath,
					hash: getContentHash(content),
					name: name,
				}
			}
		}
	}

	fmt.Printf("Existing modules found: %d\n\n", len(existing))

	added, duplicate, skipped := 0, 0, 0

	entries, err := os.ReadDir(srDirStr)
	if err != nil {
		fmt.Println("❌ Failed to read SR dir")
		return 1
	}

	for _, e := range entries {
		if e.IsDir() || strings.HasPrefix(e.Name(), "__") || !strings.Contains(e.Name(), "module") {
			continue
		}
		srFile := filepath.Join(srDirStr, e.Name())
		info, err := e.Info()
		if err != nil {
			continue
		}
		if info.Size() > 100000 {
			fmt.Printf("⏭️  Skipping large file: %s (%dKB)\n", e.Name(), info.Size()/1024)
			skipped++
			continue
		}

		content := hub.ReadFileString(srFile)
		if content == "" {
			fmt.Printf("❌ Failed to read: %s\n", e.Name())
			continue
		}

		moduleName := getModuleName(content)
		if moduleName == "" {
			decoded, _ := url.QueryUnescape(strings.TrimSuffix(e.Name(), filepath.Ext(e.Name())))
			moduleName = decoded
		}
		contentHash := getContentHash(content)
		nameKey := strings.ToLower(moduleName)

		if ex, found := existing[nameKey]; found {
			if ex.hash == contentHash {
				fmt.Printf("🔄 Duplicate: %s\n", moduleName)
				duplicate++
			} else {
				fmt.Printf("📝 Exists with different content: %s\n", moduleName)
				fmt.Printf("   → Keeping existing: %s\n", filepath.Base(ex.path))
				skipped++
			}
			continue
		}

		category := classifyModule(moduleName, content)
		safeName := regexp.MustCompile(`[<>:"/\\|?*]`).ReplaceAllString(moduleName, "")
		if !strings.HasSuffix(safeName, ".sgmodule") {
			safeName += ".sgmodule"
		}
		dstPath := filepath.Join(surgeDir, category, safeName)

		lines := strings.Split(content, "\n")
		var newLines []string
		catAdded := false

		for _, line := range lines {
			if strings.HasPrefix(line, "#!url") {
				continue
			}
			if strings.HasPrefix(line, "#!category") {
				if !catAdded {
					newLines = append(newLines, fmt.Sprintf("#!category=%s", categoryMap[category]))
					catAdded = true
				}
				continue
			}
			if strings.HasPrefix(line, "#!name") && !catAdded {
				newLines = append(newLines, fmt.Sprintf("#!category=%s", categoryMap[category]))
				catAdded = true
			}
			newLines = append(newLines, line)
		}

		if !catAdded {
			newLines = append([]string{fmt.Sprintf("#!category=%s", categoryMap[category])}, newLines...)
		}

		hub.SafeWriteFile(dstPath, strings.Join(newLines, "\n"), true)
		fmt.Printf("✅ Added: %s → %s/\n", moduleName, category)
		added++
	}

	fmt.Println("\n" + strings.Repeat("=", 60))
	fmt.Printf("Stats: Added %d, Duplicate %d, Skipped %d\n", added, duplicate, skipped)
	fmt.Println(strings.Repeat("=", 60))

	return 0
}
