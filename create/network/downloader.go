package network

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"regexp"
	"time"
)

const browserUA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"

// DownloadFile downloads a URL to a local filepath with mirror fallback.
func DownloadFile(url, destPath string) error {
	mirrorURL := "https://ghproxy.net/" + url
	if err := DownloadOnce(url, destPath, 15*time.Second); err == nil {
		return nil
	}
	return DownloadWithRetries(mirrorURL, destPath, 3, 120*time.Second)
}

// DownloadOnce performs a single HTTP GET download to destPath.
func DownloadOnce(url, destPath string, timeout time.Duration) error {
	client := &http.Client{Timeout: timeout}
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return err
	}
	req.Header.Set("User-Agent", browserUA)
	req.Header.Set("Accept", "application/octet-stream, */*")

	resp, err := client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("bad status: %s", resp.Status)
	}

	out, err := os.Create(destPath)
	if err != nil {
		return err
	}
	defer out.Close()
	_, err = io.Copy(out, resp.Body)
	return err
}

// DownloadWithRetries retries DownloadOnce up to maxAttempts times.
func DownloadWithRetries(url, destPath string, maxAttempts int, timeout time.Duration) error {
	var lastErr error
	for attempt := 1; attempt <= maxAttempts; attempt++ {
		if err := DownloadOnce(url, destPath, timeout); err == nil {
			return nil
		} else {
			lastErr = err
			if attempt < maxAttempts {
				Info(fmt.Sprintf("Download attempt %d failed: %v. Retrying in 1s...", attempt, err))
				time.Sleep(1 * time.Second)
			}
		}
	}
	return lastErr
}

// GetLatestGitHubVersion fetches the latest release tag from a GitHub repo.
func GetLatestGitHubVersion(repo string, includePrerelease bool) string {
	url := fmt.Sprintf("https://github.com/%s/releases", repo)
	if !includePrerelease {
		url += "/latest"
	}

	client := &http.Client{Timeout: 10 * time.Second}
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return ""
	}
	req.Header.Set("User-Agent", browserUA)

	resp, err := client.Do(req)
	if err != nil {
		return ""
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return ""
	}

	bodyBytes, err := io.ReadAll(resp.Body)
	if err != nil {
		return ""
	}
	body := string(bodyBytes)

	pattern := fmt.Sprintf(`href="/%s/releases/tag/v([0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?)["']`, repo)
	if !includePrerelease {
		pattern = fmt.Sprintf(`href="/%s/releases/tag/v([0-9]+\.[0-9]+\.[0-9]+)["']`, repo)
	}

	re := regexp.MustCompile(pattern)
	matches := re.FindStringSubmatch(body)
	if len(matches) > 1 {
		return matches[1]
	}
	return ""
}

// CheckURL performs a single HTTP GET and returns the status code.
func CheckURL(urlStr string) (int, error) {
	client := &http.Client{Timeout: 10 * time.Second}
	req, _ := http.NewRequest("GET", urlStr, nil)
	req.Header.Set("User-Agent", "Mozilla/5.0")
	resp, err := client.Do(req)
	if err != nil {
		return 0, err
	}
	defer resp.Body.Close()
	return resp.StatusCode, nil
}
