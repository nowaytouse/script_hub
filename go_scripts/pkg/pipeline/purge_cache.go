package pipeline

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"strings"
	"time"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

const (
	repoOwner = "nowaytouse"
	repoName  = "script_hub"
	branch    = "master"
)

func getAllRelevantFiles() []string {
	var allFiles []string

	out1, err1 := exec.Command("git", "-C", hub.ROOT, "diff", "--name-only", "HEAD~1", "HEAD").Output()
	if err1 == nil {
		allFiles = append(allFiles, strings.Split(strings.TrimSpace(string(out1)), "\n")...)
	}

	out2, err2 := exec.Command("git", "-C", hub.ROOT, "diff", "--name-only", "HEAD").Output()
	if err2 == nil {
		allFiles = append(allFiles, strings.Split(strings.TrimSpace(string(out2)), "\n")...)
	}

	allFiles = hub.UniqueStrings(allFiles)

	if len(allFiles) == 0 || (len(allFiles) == 1 && allFiles[0] == "") {
		out3, err3 := exec.Command("git", "-C", hub.ROOT, "ls-files").Output()
		if err3 == nil {
			allFiles = strings.Split(strings.TrimSpace(string(out3)), "\n")
		}
	}

	validExts := map[string]bool{
		".sgmodule": true, ".module": true, ".list": true,
		".json": true, ".js": true, ".srs": true,
		".md": true, ".txt": true, ".conf": true,
	}

	var relevant []string
	for _, f := range allFiles {
		f = strings.TrimSpace(f)
		if f == "" {
			continue
		}
		for ext := range validExts {
			if strings.HasSuffix(f, ext) {
				relevant = append(relevant, f)
				break
			}
		}
	}

	return relevant
}

func purgeUrl(filepath string, maxRetries int) bool {
	encodedFilepath := url.PathEscape(filepath)
	purgeURL := fmt.Sprintf("https://purge.jsdelivr.net/gh/%s/%s@%s/%s", repoOwner, repoName, branch, encodedFilepath)

	for attempt := 0; attempt < maxRetries; attempt++ {
		req, _ := http.NewRequest("GET", purgeURL, nil)
		req.Header.Set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
		client := &http.Client{Timeout: 8 * time.Second}

		resp, err := client.Do(req)
		if err != nil {
			if attempt < maxRetries-1 {
				time.Sleep(time.Duration(1.5*float64(attempt+1)) * time.Second)
				continue
			}
			hub.Error(fmt.Sprintf("Failed to purge %s after %d attempts: %v", filepath, maxRetries, err))
			return false
		}

		body, _ := io.ReadAll(resp.Body)
		resp.Body.Close()

		if resp.StatusCode == 429 {
			retryAfter := resp.Header.Get("Retry-After")
			delay := 2.0 * float64(int(1)<<attempt)
			if retryAfter != "" {
				fmt.Sscanf(retryAfter, "%f", &delay)
			}
			hub.Warn(fmt.Sprintf("Rate limited (429) for %s, waiting %.1fs...", filepath, delay))
			time.Sleep(time.Duration(delay) * time.Second)
			continue
		}

		if resp.StatusCode != http.StatusOK {
			if attempt < maxRetries-1 {
				time.Sleep(time.Duration(1.5*float64(attempt+1)) * time.Second)
				continue
			}
			hub.Error(fmt.Sprintf("HTTPError %d for %s", resp.StatusCode, filepath))
			return false
		}

		var resJson map[string]interface{}
		if err := json.Unmarshal(body, &resJson); err == nil {
			if status, ok := resJson["status"].(string); ok && status == "finished" {
				time.Sleep(50 * time.Millisecond) // Slight delay to prevent too many requests
				return true
			}
		}

		if attempt < maxRetries-1 {
			time.Sleep(time.Duration(1.5*float64(attempt+1)) * time.Second)
		}
	}
	return false
}

func RunPurgeCache() {
	hub.Section("Deep CDN Cache Cleared")
	modifiedFiles := getAllRelevantFiles()

	if len(modifiedFiles) == 0 {
		hub.Info("No relevant files found to purge.")
		return
	}

	hub.Info(fmt.Sprintf("Attempting to instantly purge %d files from JSDelivr CDN...", len(modifiedFiles)))

	successCount := 0
	var failedFiles []string

	for _, f := range modifiedFiles {
		if purgeUrl(f, 4) {
			successCount++
		} else {
			failedFiles = append(failedFiles, f)
		}
	}

	if successCount > 0 {
		hub.Success(fmt.Sprintf("Successfully purged %d/%d files from global CDN edge nodes!", successCount, len(modifiedFiles)))
		hub.Info("You can now safely pull from JSDelivr without hitting cache.")
	}

	if len(failedFiles) > 0 {
		preview := strings.Join(failedFiles, ", ")
		if len(failedFiles) > 5 {
			preview = strings.Join(failedFiles[:5], ", ") + "..."
		}
		hub.Warn(fmt.Sprintf("Atomicity constraint failed. Could not purge %d files: %s", len(failedFiles), preview))
		os.Exit(1)
	}
}
