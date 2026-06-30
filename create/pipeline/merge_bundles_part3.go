package pipeline

import (
	"fmt"
	"path/filepath"
	"strings"
	"time"

	"github.com/nowaytouse/script_hub/create/hub"
	"github.com/nowaytouse/script_hub/create/network"
)

func fetchYoutubeUpstream() string {
	text := network.SafeDownload(hub.YOUTUBE_ENHANCE_URL, 2, 60)
	if text != "" && len(strings.TrimSpace(text)) > 100 {
		hub.Info(fmt.Sprintf("Using upstream: %s", hub.YOUTUBE_ENHANCE_URL))
		return text
	}
	hub.Warn(fmt.Sprintf("YouTube upstream unavailable: %s", hub.YOUTUBE_ENHANCE_URL))

	for _, path := range youtubeLocalFallbacks {
		if hub.ValidateFileExists(path, "") {
			rel, _ := filepath.Rel(hub.ROOT, path)
			hub.Warn(fmt.Sprintf("Using local fallback: %s", rel))
			return hub.ReadFileString(path)
		}
	}
	return ""
}

func MergeYoutube() error {
	hub.Section("YouTube upstream bundle merge (Stripped of ADBlock)")
	text := fetchYoutubeUpstream()
	if text == "" {
		return fmt.Errorf("YouTube.Enhance unavailable (Maasea upstream 404 and no local fallback). Run upstream_sync.sync_nexus() first or retry later")
	}

	header := map[string]string{
		"name":     "📺 YouTube增强合集",
		"desc":     "合并 YouTube 增强 (Maasea 上游) | Enhance: 画中画/后台播放/字幕翻译",
		"author":   "Maasea",
		"icon":     "https://cdn.jsdelivr.net/gh/Orz-3/mini@master/Color/YouTube.png",
		"category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
		"tag":      "YouTube, 增强",
	}

	err := callMergeBundleRust(mergeBundleRequest{
		Op:         "youtube_merge",
		OutputPath: youtubeOutput,
		HeaderMeta: header,
		Sources: []mergeSourcePayload{{
			Label:   "YouTube.Enhance",
			URL:     hub.YOUTUBE_ENHANCE_URL,
			Content: text,
		}},
		YoutubeAdblockOutput: youtubeAdblockOutput,
	})
	if err != nil {
		return err
	}
	hub.Success(fmt.Sprintf("Wrote stripped bundle: %s", youtubeOutput))
	if hub.ValidateFileExists(youtubeAdblockOutput, "") {
		hub.Success(fmt.Sprintf("Wrote extracted adblock module: %s", youtubeAdblockOutput))
	}
	time.Sleep(3 * time.Second)
	return nil
}

func MergeAllBundles(strict bool) []string {
	steps := []struct {
		label string
		fn    func() error
	}{
		{"bilibili", MergeBilibili},
		{"youtube", MergeYoutube},
		{"weibo", MergeWeibo},
		{"wool", MergeWool},
		{"apple", MergeApple},
		{"utilities", MergeUtilities},
		{"devtools", MergeDevtools},
	}

	var failures []string
	for _, step := range steps {
		if err := step.fn(); err != nil {
			hub.Error(fmt.Sprintf("Bundle merge '%s' failed: %v", step.label, err))
			failures = append(failures, step.label)
			if strict {
				panic(err)
			}
		}
	}

	cleanupMergedStandaloneModules()

	if len(failures) > 0 {
		hub.Warn(fmt.Sprintf("Bundle merges incomplete: %s", strings.Join(failures, ", ")))
	} else {
		hub.Success("All upstream bundle merges completed.")
	}
	return failures
}
