package network

import (
	"fmt"
	"os"
	"strings"
	"time"
)

// FetchModule downloads one upstream Surge module via network or reads it from a local path.
// Network requests are handled by the Go network layer (SafeDownloadString).
// Local fallback paths are read directly from disk.
func FetchModule(urlStr, label, localFallback, customUA string) (string, error) {
	if strings.HasPrefix(urlStr, "http://") || strings.HasPrefix(urlStr, "https://") {
		content := SafeDownloadString(urlStr, 2, 60, customUA)
		if content != "" && len(strings.TrimSpace(content)) >= 20 {
			time.Sleep(3 * time.Second) // strict delay to avoid shadowban/rate limits
			return content, nil
		}
	} else if fileExists(urlStr) {
		content := readFileString(urlStr)
		if len(strings.TrimSpace(content)) >= 20 {
			return content, nil
		}
	}

	if localFallback != "" && fileExists(localFallback) {
		Warn(fmt.Sprintf("Using local fallback for %s", label))
		return readFileString(localFallback), nil
	}

	return "", fmt.Errorf("failed to load upstream module: %s (%s)", label, urlStr)
}

func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func readFileString(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	return string(data)
}
