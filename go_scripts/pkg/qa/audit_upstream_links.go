package qa

import (
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

func extractUrls(filePath string) []string {
	content := hub.ReadFileString(filePath)
	re := regexp.MustCompile(`https?://[^\s'"<>]+`)
	return re.FindAllString(content, -1)
}

func checkUrl(urlStr string) (string, int, string) {
	client := &http.Client{
		Timeout: 10 * time.Second,
		// Using default transport is fine here, skipping SSL validation isn't strictly necessary
		// unless there's a specific issue, but we can do a simple GET.
	}
	req, _ := http.NewRequest("GET", urlStr, nil)
	req.Header.Set("User-Agent", "Mozilla/5.0")
	resp, err := client.Do(req)
	if err != nil {
		return urlStr, 0, err.Error()
	}
	defer resp.Body.Close()
	return urlStr, resp.StatusCode, ""
}

func RunAuditUpstreamLinks() int {
	pipelineDir := filepath.Join(hub.ROOT, "go_scripts/pkg/pipeline")
	fmt.Printf("Scanning for URLs in %s...\n", pipelineDir)

	allUrls := make(map[string]bool)
	err := filepath.Walk(pipelineDir, func(path string, info os.FileInfo, err error) error {
		if err == nil && !info.IsDir() && strings.HasSuffix(path, ".go") {
			urls := extractUrls(path)
			for _, u := range urls {
				if strings.Contains(u, "github.com") || strings.Contains(u, "raw.githubusercontent") || strings.Contains(u, "moe") || strings.Contains(u, "jsdelivr") || strings.Contains(u, "kelee") || strings.Contains(u, "yfamilys") {
					allUrls[u] = true
				}
			}
		}
		return nil
	})

	if err != nil {
		fmt.Printf("Error walking directory: %v\n", err)
		return 1
	}

	var urls []string
	for u := range allUrls {
		urls = append(urls, u)
	}
	sort.Strings(urls)
	fmt.Printf("Found %d unique upstream URLs to check.\n", len(urls))

	var failed []string
	for _, u := range urls {
		// Strictly serial execution to avoid rate limits / concurrency issues on network calls per user constraint
		urlStr, status, errStr := checkUrl(u)
		if status != 200 && status != 301 && status != 302 && status != 304 {
			fmt.Printf("[FAIL] %d | %s | %s\n", status, urlStr, errStr)
			failed = append(failed, urlStr)
		}
		time.Sleep(100 * time.Millisecond) // sleep to prevent rate limiting
	}

	if len(failed) > 0 {
		fmt.Printf("\n%d URLs failed.\n", len(failed))
		return 1
	}

	fmt.Println("\nAll URLs are accessible (200 OK)!")
	return 0
}
