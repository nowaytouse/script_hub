package network

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"
)

const (
	BLUE   = "\033[0;34m"
	GREEN  = "\033[0;32m"
	YELLOW = "\033[1;33m"
	RED    = "\033[0;31m"
	NC     = "\033[0m"
	CurlUA = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
)

func Info(msg string) {
	fmt.Printf("%s[INFO]%s %s\n", BLUE, NC, msg)
}

func Warn(msg string) {
	fmt.Printf("%s[⚠]%s %s\n", YELLOW, NC, msg)
}

func GetProjectRoot() string {
	_, b, _, _ := runtime.Caller(0)
	scriptDir := filepath.Dir(b)
	return filepath.Join(scriptDir, "..", "..")
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

func isHTMLContent(data []byte) bool {
	sampleLen := len(data)
	if sampleLen > 500 {
		sampleLen = 500
	}
	sample := strings.ToLower(string(data[:sampleLen]))
	return strings.Contains(sample, "<!doctype html") || strings.Contains(sample, "<html")
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

func SafeDownload(url string, retries int, timeout int) string {
	return SafeDownloadString(url, retries, timeout, "")
}

func curlFetch(urlStr string, timeout int, retries int, ua string) []byte {
	vendor := readVendorSnapshot(urlStr)
	if vendor != nil && !isHTMLContent(vendor) {
		rel := vendorSnapshotByURL[urlStr]
		Info(fmt.Sprintf("Using vendor snapshot: %s", rel))
		return vendor
	}

	cached := readHTTPCache(urlStr)
	if cached != nil && !isHTMLContent(cached) {
		return cached
	}

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

// DownloadToFile downloads content from URL and saves to destination file
func DownloadToFile(urlStr string, destPath string, ua string) bool {
	data := SafeDownloadBinary(urlStr, 2, 60, ua)
	if data == nil {
		return false
	}
	os.MkdirAll(filepath.Dir(destPath), 0755)
	err := os.WriteFile(destPath, data, 0644)
	return err == nil
}

// DownloadWithUA downloads content from URL with custom user agent
func DownloadWithUA(urlStr string, ua string) []byte {
	return SafeDownloadBinary(urlStr, 2, 60, ua)
}
