package pipeline

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

var mockReplacements = map[string]string{
	`https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/reject-200\.txt`:      "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-200.txt",
	`https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/blank\.txt`:           "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank.txt",
	`https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/reject-dict\.json`:    "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-dict.json",
	`https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/reject-img\.gif`:      "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/reject-img.gif",
	`https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/blank_dict\.json\.js`: "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank_dict.json.js",
	`https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/blank\.gif`:           "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank.gif",
	`https://raw\.githubusercontent\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/(master|main)/[A-Za-z0-9_.-/]+/blank_dict\.json`:     "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/modules/source/mocks/blank_dict.json",
	`https://raw\.githubusercontent\.com/([^/]+)/([^/]+)/([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)`:                                    `https://cdn.jsdelivr.net/gh/$1/$2@$3/$4`,
	`(^|[^/])https://github\.com/([^/]+)/([^/]+)/raw/([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)`:                                        `$1https://cdn.jsdelivr.net/gh/$2/$3@$4/$5`,
	`(^|[^/])https://github\.com/([^/]+)/([^/]+)/releases/download/([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)`:                          `$1https://ghproxy.net/https://github.com/$2/$3/releases/download/$4/$5`,
}

type urlReplacement struct {
	regex *regexp.Regexp
	repl  string
}

var compiledReplacements []urlReplacement

func init() {
	for pattern, repl := range mockReplacements {
		compiledReplacements = append(compiledReplacements, urlReplacement{
			regex: regexp.MustCompile("(?i)" + pattern),
			repl:  repl,
		})
	}
}

func isGithubSource(path string) bool {
	parts := strings.Split(filepath.Clean(path), string(os.PathSeparator))
	for _, part := range parts {
		if part == "github" {
			return true
		}
	}
	return false
}

func CopyGithubVariants() {
	dirsToCopy := []string{
		filepath.Join(hub.MODULES_DIR, "surge", "amplify_nexus"),
		filepath.Join(hub.MODULES_DIR, "surge", "head_expanse"),
		filepath.Join(hub.MODULES_DIR, "shadowrocket", "amplify_nexus"),
		filepath.Join(hub.MODULES_DIR, "shadowrocket", "head_expanse"),
		filepath.Join(hub.RULESETS_DIR, "RULE-SET"),
		filepath.Join(hub.RULESETS_DIR, "AdBlock"),
	}

	skipCopy := map[string]bool{
		"🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule": true,
		"📱 Universal Ad-Blocking Rules (PROMAX Lite).sgmodule":                         true,
		"🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module":   true,
		"📱 Universal Ad-Blocking Rules (PROMAX Lite).module":                           true,
	}

	for _, dir := range dirsToCopy {
		if !hub.ValidateDirExists(dir, "") {
			continue
		}
		githubDir := filepath.Join(dir, "github")
		hub.EnsureDir(githubDir)

		files, err := os.ReadDir(dir)
		if err != nil {
			continue
		}

		for _, f := range files {
			if f.IsDir() || f.Name() == "github" || skipCopy[f.Name()] {
				continue
			}
			ext := filepath.Ext(f.Name())
			if ext == ".sgmodule" || ext == ".module" || ext == ".list" {
				srcPath := filepath.Join(dir, f.Name())
				destPath := filepath.Join(githubDir, f.Name())
				content := hub.ReadFileString(srcPath)
				hub.SafeWriteFile(destPath, content, true)
			}
		}
	}
}

func RunUrlRewrites(directory string) int {
	if !hub.ValidateDirExists(directory, "") {
		return 0
	}
	directory = filepath.Clean(directory)
	modifiedCount := 0

	err := filepath.Walk(directory, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.IsDir() {
			if strings.Contains(path, ".git") {
				return filepath.SkipDir
			}
			if isGithubSource(path) {
				return filepath.SkipDir
			}
			return nil
		}
		ext := filepath.Ext(path)
		validExts := map[string]bool{
			".sgmodule": true, ".module": true, ".list": true,
			".conf": true, ".json": true, ".html": true, ".md": true,
		}
		if !validExts[ext] {
			return nil
		}

		content := hub.ReadFileString(path)
		origContent := content

		for _, rep := range compiledReplacements {
			content = rep.regex.ReplaceAllString(content, rep.repl)
		}

		if content != origContent {
			hub.SafeWriteFile(path, content, true)
			modifiedCount++
		}
		return nil
	})

	if err != nil {
		hub.Error(fmt.Sprintf("Error walking directory %s: %v", directory, err))
	}
	return modifiedCount
}

func RunAllUrlRewrites() int {
	CopyGithubVariants()
	count := 0
	count += RunUrlRewrites(hub.MODULES_DIR)
	count += RunUrlRewrites(hub.RULESETS_DIR)
	count += RunUrlRewrites(hub.ROOT)
	hub.Info(fmt.Sprintf("URL rewrite completed: %d files modified.", count))
	return count
}
