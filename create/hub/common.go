package hub

import (
	"bufio"
	"crypto/md5"
	"encoding/hex"
	"fmt"
	"io"
	"iter"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"sort"
	"strings"

	"github.com/nowaytouse/script_hub/create/network"
)

// COLORS & LOGGING

type Logger struct{}

const (
	BLUE   = "\033[0;34m"
	CYAN   = "\033[0;36m"
	GREEN  = "\033[0;32m"
	YELLOW = "\033[1;33m"
	RED    = "\033[0;31m"
	PURPLE = "\033[1;35m"
	NC     = "\033[0m"
)

func Info(msg string) {
	fmt.Printf("%s[INFO]%s %s\n", BLUE, NC, msg)
}

func Success(msg string) {
	fmt.Printf("%s[✓]%s %s\n", GREEN, NC, msg)
}

func Warn(msg string) {
	fmt.Printf("%s[⚠]%s %s\n", YELLOW, NC, msg)
}

func Error(msg string) {
	fmt.Fprintf(os.Stderr, "%s[✗ ERROR]%s %s\n", RED, NC, msg)
}

func ErrorWithExit(msg string, exitCode int) {
	Error(msg)
	os.Exit(exitCode)
}

func Section(msg string) {
	fmt.Printf("\n%s═══ %s ═══%s\n", PURPLE, msg, NC)
}

// FILE & PATH UTILITIES

func IsCI() bool {
	if strings.ToLower(os.Getenv("GITHUB_ACTIONS")) == "true" {
		return true
	}
	ci := strings.ToLower(os.Getenv("CI"))
	return ci == "1" || ci == "true" || ci == "yes"
}

func GetProjectRoot() string {
	_, b, _, _ := runtime.Caller(0)
	scriptDir := filepath.Dir(b)
	// create/hub is 2 levels down from repo root, but we want the repo root,
	// so from create/hub -> create -> repo_root = 2 levels up
	return filepath.Join(scriptDir, "..", "..")
}

func GetFileHash(filePath string) string {
	f, err := os.Open(filePath)
	if err != nil {
		return ""
	}
	defer f.Close()
	hash := md5.New()
	if _, err := io.Copy(hash, f); err != nil {
		return ""
	}
	return hex.EncodeToString(hash.Sum(nil))
}

func ReadFile(filePath string) []string {
	content, err := os.ReadFile(filePath)
	if err != nil {
		return []string{}
	}
	lines := strings.Split(string(content), "\n")
	for i := range lines {
		lines[i] = lines[i] + "\n"
	}
	if len(lines) > 0 && lines[len(lines)-1] == "\n" {
		lines = lines[:len(lines)-1]
	}
	return lines
}

var dateBracketRegex = regexp.MustCompile(`\[\d{4}-\d{2}-\d{2}(?:\s+\d{2}:\d{2}(?::\d{2})?)?\]`)

func getSemanticContent(text string) string {
	lines := strings.Split(text, "\n")
	var filtered []string
	for _, line := range lines {
		stripped := strings.TrimSpace(line)
		if stripped == "" {
			continue
		}
		if strings.HasPrefix(stripped, "#") || strings.HasPrefix(stripped, "//") {
			continue
		}
		filtered = append(filtered, stripped)
	}
	return strings.Join(filtered, "\n")
}

func WriteFile(filePath string, content string) error {
	if _, err := os.Stat(filePath); err == nil {
		oldBytes, _ := os.ReadFile(filePath)
		if len(oldBytes) > 0 {
			oldStripped := getSemanticContent(string(oldBytes))
			newStripped := getSemanticContent(content)
			if oldStripped == newStripped {
				Info(fmt.Sprintf("Skipping write for %s: No semantic changes detected.", filepath.Base(filePath)))
				return nil
			}
		}
	}
	tmp := filePath + ".tmp"
	if err := os.WriteFile(tmp, []byte(content), 0644); err != nil {
		return fmt.Errorf("failed to write file: %s: %w", filePath, err)
	}
	return os.Rename(tmp, filePath)
}

func ExtractSection(lines []string, sectionName string) []string {
	var result []string
	inSection := false
	sectionPattern := regexp.MustCompile(`(?i)^\[` + regexp.QuoteMeta(sectionName) + `\]`)

	for _, line := range lines {
		stripped := strings.TrimSpace(line)
		if sectionPattern.MatchString(stripped) {
			inSection = true
			continue
		}
		if strings.HasPrefix(stripped, "[") && inSection {
			break
		}
		if inSection && stripped != "" && !strings.HasPrefix(stripped, "#") {
			result = append(result, strings.TrimRight(line, "\r\n"))
		}
	}
	return result
}

func CleanRules(rules []string) []string {
	seen := make(map[string]bool)
	var cleaned []string
	for _, r := range rules {
		rStrip := strings.TrimSpace(r)
		if rStrip != "" && !strings.HasPrefix(rStrip, "#") && !seen[rStrip] {
			seen[rStrip] = true
			cleaned = append(cleaned, rStrip)
		}
	}
	sort.Strings(cleaned)
	return cleaned
}

const LOON_VERSION = "3.9.9"

var BrowserUA = fmt.Sprintf("Loon/%s", LOON_VERSION)
var CurlUA = BrowserUA

func AtomicWrite(filePath string, content string) bool {
	tmp := filePath + ".tmp"
	if err := os.WriteFile(tmp, []byte(content), 0644); err != nil {
		return false
	}
	return os.Rename(tmp, filePath) == nil
}

func SafeRemove(filePath string, missingOk bool) bool {
	err := os.Remove(filePath)
	if err != nil && !os.IsNotExist(err) {
		return false
	}
	return true
}

func SafeRemoveTree(dirPath string, missingOk bool) bool {
	err := os.RemoveAll(dirPath)
	if err != nil && !os.IsNotExist(err) {
		return false
	}
	return true
}

func isHTMLContent(data []byte) bool {
	sampleLen := len(data)
	if sampleLen > 500 {
		sampleLen = 500
	}
	sample := strings.ToLower(string(data[:sampleLen]))
	return strings.Contains(sample, "<!doctype html") || strings.Contains(sample, "<html")
}

func HasDangerousChars(line string) bool {
	return strings.Contains(line, "<") || strings.Contains(line, ">") || strings.Contains(line, "{") || strings.Contains(line, "}") || strings.Contains(line, "function(") || strings.Contains(line, "window.") || strings.Contains(line, "document.")
}

func ReadFileString(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	return string(data)
}

// IterLines returns an iterator over the lines of the file at the given path.
// This is memory-efficient and avoids reading/allocating the entire file in memory.
func IterLines(path string) iter.Seq[string] {
	return func(yield func(string) bool) {
		file, err := os.Open(path)
		if err != nil {
			return
		}
		defer file.Close()

		scanner := bufio.NewScanner(file)
		// Allocate a 64KB buffer that can expand up to 1MB to handle long lines
		buf := make([]byte, 0, 64*1024)
		scanner.Buffer(buf, 1024*1024)

		for scanner.Scan() {
			if !yield(scanner.Text()) {
				return
			}
		}
	}
}

func UniqueStrings(input []string) []string {
	seen := make(map[string]bool)
	var result []string
	for _, v := range input {
		if !seen[v] {
			seen[v] = true
			result = append(result, v)
		}
	}
	return result
}

// Network function wrappers for backward compatibility
// These delegate to the network package

func SafeDownload(url string, retries int, timeout int) string {
	return network.SafeDownload(url, retries, timeout)
}

func SafeDownloadString(urlStr string, retries int, timeout int, ua string) string {
	return network.SafeDownloadString(urlStr, retries, timeout, ua)
}

func SafeDownloadBinary(urlStr string, retries int, timeout int, ua string) []byte {
	return network.SafeDownloadBinary(urlStr, retries, timeout, ua)
}
