package qa

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/nowaytouse/script_hub/create/hub"
	"github.com/nowaytouse/script_hub/create/network"
)

func extractUrls(filePath string) []string {
	content := hub.ReadFileString(filePath)
	re := regexp.MustCompile(`https?://[^\s'"<>]+`)
	return re.FindAllString(content, -1)
}

func checkUrl(urlStr string) (string, int, string) {
	status, err := network.CheckURL(urlStr)
	if err != nil {
		return urlStr, 0, err.Error()
	}
	return urlStr, status, ""
}

func RunAuditUpstreamLinks() int {
	pipelineDir := filepath.Join(hub.ROOT, "create/pipeline")
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
