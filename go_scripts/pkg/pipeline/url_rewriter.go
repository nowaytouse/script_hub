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
	`(^|[^/])https://github\.com/([^/]+)/([^/]+)/releases/download/([^/]+)/([^"'\s]+\.[a-zA-Z0-9]+)`:                          `$1https://gh-proxy.com/https://github.com/$2/$3/releases/download/$4/$5`,
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
	githubSourceDirs := []string{
		filepath.Clean(hub.SURGE_HEAD_EXPANSE_GITHUB_DIR),
		filepath.Clean(hub.SHADOWROCKET_HEAD_EXPANSE_GITHUB_DIR),
	}
	p := filepath.Clean(path)
	for _, d := range githubSourceDirs {
		if p == d || strings.HasPrefix(p, d+string(os.PathSeparator)) {
			return true
		}
	}
	return false
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
	count := 0
	count += RunUrlRewrites(hub.MODULES_DIR)
	count += RunUrlRewrites(hub.RULESETS_DIR)
	count += RunUrlRewrites(hub.ROOT)
	hub.Info(fmt.Sprintf("URL rewrite completed: %d files modified.", count))
	return count
}
