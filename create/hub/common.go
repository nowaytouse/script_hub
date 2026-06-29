package hub

import (
	"bufio"
	"crypto/md5"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"iter"
	"net/http"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"sort"
	"strings"
	"time"
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
	// pkg/hub is 2 levels down from go_scripts, but we want the repo root,
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
		oldContent, err := os.ReadFile(filePath)
		if err == nil {
			oldStripped := getSemanticContent(string(oldContent))
			newStripped := getSemanticContent(content)
			if oldStripped == newStripped {
				Info(fmt.Sprintf("Skipping write for %s: No semantic changes detected.", filepath.Base(filePath)))
				return nil
			}
		}
	}
	return SafeWriteFile(filePath, content, true)
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
	return SafeWriteFile(filePath, content, true) == nil
}

func SafeRemove(filePath string, missingOk bool) bool {
	err := os.Remove(filePath)
	if err != nil && !os.IsNotExist(err) {
		Error(fmt.Sprintf("Failed to remove file %s: %v", filePath, err))
		return false
	}
	return true
}

func SafeRemoveTree(dirPath string, missingOk bool) bool {
	if _, err := os.Stat(dirPath); os.IsNotExist(err) {
		if missingOk {
			return true
		}
		Warn(fmt.Sprintf("Directory does not exist: %s", dirPath))
		return false
	}
	err := os.RemoveAll(dirPath)
	if err != nil {
		Error(fmt.Sprintf("Failed to remove directory %s: %v", dirPath, err))
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

var vendorSnapshotByURL = map[string]string{
	"https://yfamilys.com/module/adultraplus.sgmodule": "adultraplus.sgmodule",
	"https://yfamilys.com/module/bili.module":          "bili.module",
	"https://yfamilys.com/rule/Kemono.list":            "yfamilys_Kemono.list",
	"https://yfamilys.com/rule/Cloudflare.list":        "yfamilys_Cloudflare.list",
}

func getVendorDir() string {
	return filepath.Join(GetProjectRoot(), "rulesets", "Sources", "vendor")
}

func httpCachePath(urlStr string) string {
	hash := sha256.Sum256([]byte(urlStr))
	digest := hex.EncodeToString(hash[:])[:32]
	return filepath.Join(GetProjectRoot(), ".cache", "http", digest)
}

func readHTTPCache(urlStr string) []byte {
	path := httpCachePath(urlStr)
	data, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	return data
}

func writeHTTPCache(urlStr string, data []byte) {
	path := httpCachePath(urlStr)
	os.MkdirAll(filepath.Dir(path), 0755)
	os.WriteFile(path, data, 0644)
}

func readVendorSnapshot(urlStr string) []byte {
	rel, ok := vendorSnapshotByURL[urlStr]
	if !ok {
		return nil
	}
	path := filepath.Join(getVendorDir(), rel)
	data, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	return data
}

func SafeDownloadString(urlStr string, retries int, timeout int, ua string) string {
	data := SafeDownloadBinary(urlStr, retries, timeout, ua)
	if data == nil {
		return ""
	}
	return string(data)
}

func SafeDownloadBinary(urlStr string, retries int, timeout int, ua string) []byte {
	raw := curlFetch(urlStr, timeout, retries, ua)
	if raw == nil {
		return nil
	}
	if isHTMLContent(raw) {
		Warn(fmt.Sprintf("Download rejected: %s returned HTML instead of raw data (blocked or redirected).", urlStr))
		return nil
	}
	return raw
}

func curlFetch(urlStr string, timeout int, retries int, ua string) []byte {
	userAgent := ua
	if userAgent == "" {
		userAgent = CurlUA
	}

	client := &http.Client{
		Timeout: time.Duration(timeout) * time.Second,
	}

	var lastErr string
	for attempt := 0; attempt <= retries; attempt++ {
		req, err := http.NewRequest("GET", urlStr, nil)
		if err == nil {
			req.Header.Set("User-Agent", userAgent)
			req.Header.Set("Accept", "text/plain, application/octet-stream, */*")
			resp, err := client.Do(req)
			if err == nil {
				defer resp.Body.Close()
				if resp.StatusCode == 200 {
					data, err := io.ReadAll(resp.Body)
					if err == nil {
						writeHTTPCache(urlStr, data)
						return data
					} else {
						lastErr = fmt.Sprintf("Read error: %v", err)
					}
				} else {
					httpHint := ""
					if resp.StatusCode >= 400 {
						httpHint = " (HTTP 4xx/5xx — upstream may block datacenter IPs)"
					}
					lastErr = fmt.Sprintf("HTTP exit %d%s", resp.StatusCode, httpHint)
				}
			} else {
				lastErr = fmt.Sprintf("Request Exception: %v", err)
			}
		} else {
			lastErr = fmt.Sprintf("Request Creation Exception: %v", err)
		}

		if attempt < retries {
			time.Sleep(time.Duration(1<<attempt) * time.Second)
		}
	}

	cached := readHTTPCache(urlStr)
	if cached != nil && !isHTMLContent(cached) {
		Info(fmt.Sprintf("Using cached copy (upstream unavailable): %s", urlStr))
		return cached
	}

	vendor := readVendorSnapshot(urlStr)
	if vendor != nil && !isHTMLContent(vendor) {
		rel := vendorSnapshotByURL[urlStr]
		Info(fmt.Sprintf("Using vendor snapshot: %s", rel))
		return vendor
	}

	// If it was a raw.githubusercontent.com URL that failed, try cdn.jsdelivr.net
	if strings.HasPrefix(urlStr, "https://raw.githubusercontent.com/") {
		parts := strings.SplitN(urlStr[34:], "/", 4)
		if len(parts) == 4 {
			mirrorUrl := fmt.Sprintf("https://cdn.jsdelivr.net/gh/%s/%s@%s/%s", parts[0], parts[1], parts[2], parts[3])
			Info(fmt.Sprintf("Download failed/rejected, trying JSDelivr mirror: %s", mirrorUrl))
			return curlFetch(mirrorUrl, timeout, 1, ua)
		}
	}

	// If it was a cdn.jsdelivr.net URL that failed, try fastly.jsdelivr.net
	if strings.HasPrefix(urlStr, "https://cdn.jsdelivr.net/") {
		fastlyUrl := strings.Replace(urlStr, "https://cdn.jsdelivr.net/", "https://fastly.jsdelivr.net/", 1)
		Info(fmt.Sprintf("Download failed, trying fastly mirror: %s", fastlyUrl))
		return curlFetch(fastlyUrl, timeout, 1, ua)
	}

	Warn(fmt.Sprintf("Download failed: %s | Reason: %s", urlStr, lastErr))
	return nil
}

func ReadFileString(path string) string {
	content, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	return string(content)
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

func SafeDownload(url string, retries int, timeout int) string {
	return SafeDownloadString(url, retries, timeout, "")
}
