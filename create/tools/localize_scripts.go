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

	"github.com/nyamiiko/script_hub/create/hub"
)

var (
	skipDomains = map[string]bool{
		"cdn.jsdelivr.net": true,
	}
)

func isValidUrl(u string) bool {
	if strings.Contains(u, ".*") || strings.Contains(u, "{{") || strings.HasSuffix(u, ".js*") {
		return false
	}
	parsed, err := url.Parse(u)
	if err != nil || parsed.Host == "" || !strings.Contains(parsed.Host, ".") {
		return false
	}
	if skipDomains[parsed.Host] {
		return false
	}
	return true
}

func downloadScript(u string) string {
	if !isValidUrl(u) {
		fmt.Printf("  ⏭️  Skipping invalid/regex URL: %s\n", u)
		return ""
	}

	parsed, _ := url.Parse(u)
	domain := strings.ReplaceAll(parsed.Host, ".", "_")
	pathParts := strings.Split(strings.Trim(parsed.Path, "/"), "/")
	filename := pathParts[len(pathParts)-1]
	if !strings.HasSuffix(filename, ".js") {
		filename += ".js"
	}

	hash := md5.Sum([]byte(parsed.Path))
	pathHash := hex.EncodeToString(hash[:])[:6]
	localFilename := fmt.Sprintf("%s_%s_%s", domain, pathHash, filename)
	localPath := filepath.Join(hub.MODULE_SOURCE_SCRIPTS_DIR, localFilename)

	if hub.ValidateFileExists(localPath, "") {
		return localFilename
	}

	fmt.Printf("  📥 Downloading script: %s\n", u)
	content := hub.SafeDownload(u, 2, 15)
	if content != "" {
		hub.SafeWriteFile(localPath, content, true)
		return localFilename
	}

	fmt.Printf("  ⚠️  Keep remote (download failed): %s\n", u)
	return ""
}

func localizeModule(filePath string) {
	content := hub.ReadFileString(filePath)
	if content == "" {
		return
	}

	urls := regexp.MustCompile(`script-path=([^,\s]+)`).FindAllStringSubmatch(content, -1)
	if len(urls) == 0 {
		return
	}

	newContent := content
	changed := false

	uniqueUrls := make(map[string]bool)
	for _, m := range urls {
		uniqueUrls[m[1]] = true
	}

	for u := range uniqueUrls {
		if strings.Contains(u, hub.SCRIPT_RAW_PREFIX) {
			continue
		}

		localFilename := downloadScript(u)
		if localFilename != "" {
			localUrl := hub.SCRIPT_RAW_PREFIX + localFilename
			newContent = strings.ReplaceAll(newContent, u, localUrl)
			changed = true
		}
	}

	if changed {
		hub.SafeWriteFile(filePath, newContent, true)
		rel, _ := filepath.Rel(hub.ROOT, filePath)
		fmt.Printf("🔍 Updated: %s\n", rel)
	}
}

func RunLocalizeScripts() int {
	hub.EnsureDir(hub.MODULE_SOURCE_SCRIPTS_DIR)

	moduleDirs := []string{
		filepath.Join(hub.ROOT, "modules/surge"),
		filepath.Join(hub.ROOT, "modules/shadowrocket"),
		filepath.Join(hub.ROOT, "modules/source/local"),
		filepath.Join(hub.ROOT, "rulesets/Sources/LocalModules"),
	}

	for _, rootDir := range moduleDirs {
		if !hub.ValidateDirExists(rootDir, "") {
			continue
		}

		filepath.Walk(rootDir, func(path string, info os.FileInfo, err error) error {
			if err != nil {
				return nil
			}
			if !info.IsDir() && (strings.HasSuffix(info.Name(), ".sgmodule") || strings.HasSuffix(info.Name(), ".module")) {
				localizeModule(path)
			}
			return nil
		})
	}
	return 0
}
