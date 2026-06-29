package pipeline

import (
	"fmt"
	"path/filepath"
	"regexp"
	"sort"
	"strings"

	"github.com/nyamiiko/script_hub/create/hub"
)

func fetchYoutubeUpstream() string {
	text := hub.SafeDownload(hub.YOUTUBE_ENHANCE_URL, 2, 60)
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

	meta, sections := hub.ParseModule(text)
	var cleanedSections []hub.ModuleSection
	mitmHosts := make(map[string]bool)
	var adblockRules []string
	var adblockMapLocals []string
	adblockMitmHosts := make(map[string]bool)

	for _, sec := range sections {
		name := sec.Name
		lines := sec.Lines
		if name == "Rule" {
			adblockRules = append(adblockRules, lines...)
			hub.Info(fmt.Sprintf("  - Stripping section: %s", name))
			continue
		}
		if name == "Map Local" {
			adblockMapLocals = append(adblockMapLocals, lines...)
			hub.Info(fmt.Sprintf("  - Stripping section: %s", name))
			continue
		}

		cleanedSections = append(cleanedSections, hub.ModuleSection{Name: name, Lines: lines})
		if name == "MITM" {
			for _, line := range lines {
				trimmed := strings.TrimSpace(line)
				if strings.HasPrefix(strings.ToLower(trimmed), "hostname") {
					re := regexp.MustCompile(`(?i)^hostname\s*=\s*(%APPEND%\s*)?`)
					hostsStr := re.ReplaceAllString(trimmed, "")
					for _, h := range strings.Split(hostsStr, ",") {
						hClean := strings.TrimSpace(h)
						if hClean == "" {
							continue
						}
						if hClean == "*.googlevideo.com" {
							adblockMitmHosts[hClean] = true
						} else {
							mitmHosts[hClean] = true
							if strings.Contains(hClean, "youtubei") {
								adblockMitmHosts[hClean] = true
							}
						}
					}
				}
			}
		}
	}

	var finalSections []hub.ModuleSection
	for _, sec := range cleanedSections {
		if sec.Name == "MITM" {
			if len(mitmHosts) > 0 {
				var sortedMitm []string
				for h := range mitmHosts {
					sortedMitm = append(sortedMitm, h)
				}
				sort.Strings(sortedMitm)
				finalSections = append(finalSections, hub.ModuleSection{
					Name:  "MITM",
					Lines: []string{fmt.Sprintf("hostname = %%APPEND%% %s", strings.Join(sortedMitm, ", "))},
				})
			}
		} else {
			finalSections = append(finalSections, sec)
		}
	}

	header := map[string]string{
		"name":     "📺 YouTube增强合集",
		"desc":     "合并 YouTube 增强 (Maasea 上游) | Enhance: 画中画/后台播放/字幕翻译",
		"author":   "Maasea",
		"icon":     "https://cdn.jsdelivr.net/gh/Orz-3/mini@master/Color/YouTube.png",
		"category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
		"tag":      "YouTube, 增强",
	}

	if args, ok := meta["arguments"]; ok {
		args = strings.ReplaceAll(args, `字幕翻译语言:off`, `字幕翻译语言:"zh-Hans"`)
		args = strings.ReplaceAll(args, `歌词翻译语言:off`, `歌词翻译语言:"zh-Hans"`)
		args = strings.ReplaceAll(args, `屏蔽Shorts按钮:false`, `屏蔽Shorts按钮:true`)
		args = strings.ReplaceAll(args, `屏蔽上传按钮:false`, `屏蔽上传按钮:true`)
		args = strings.ReplaceAll(args, `屏蔽选段按钮:false`, `屏蔽选段按钮:true`)
		args = strings.ReplaceAll(args, `增加画中画:false`, `增加画中画:true`)
		header["arguments"] = args
	}
	if argsDesc, ok := meta["arguments-desc"]; ok {
		header["arguments-desc"] = argsDesc
	}

	extra := []string{
		"# Upstream module processed by create/pipeline/merge_bundles.go",
		"# - Stripped: [Rule], [Map Local] and *.googlevideo.com from MITM",
	}
	headerLines := hub.FormatHeader(header, extra)
	hub.EnsureDir(filepath.Dir(youtubeOutput))

	content := hub.FormatModule(headerLines, finalSections, true, nil)
	if len(strings.TrimSpace(content)) < 100 {
		return fmt.Errorf("generated YouTube bundle too small: %s", youtubeOutput)
	}
	hub.SafeWriteFile(youtubeOutput, content, true)
	hub.Success(fmt.Sprintf("Wrote stripped bundle: %s", youtubeOutput))

	var adblockHeader = []string{
		"#!name=YouTube 去广告",
		"#!desc=YouTube 广告过滤 (自动从 Maasea 上游提取并合并进入 PROMAX)",
		"#!category=🪐 local",
		"",
	}

	var adblockSectionsLines []string
	if len(adblockRules) > 0 {
		adblockSectionsLines = append(adblockSectionsLines, "[Rule]")
		adblockSectionsLines = append(adblockSectionsLines, adblockRules...)
		adblockSectionsLines = append(adblockSectionsLines, "")
	}
	if len(adblockMapLocals) > 0 {
		adblockSectionsLines = append(adblockSectionsLines, "[Map Local]")
		adblockSectionsLines = append(adblockSectionsLines, adblockMapLocals...)
		adblockSectionsLines = append(adblockSectionsLines, "")
	}
	if len(adblockMitmHosts) > 0 {
		adblockSectionsLines = append(adblockSectionsLines, "[MITM]")
		var sortedAdblockMitm []string
		for h := range adblockMitmHosts {
			sortedAdblockMitm = append(sortedAdblockMitm, h)
		}
		sort.Strings(sortedAdblockMitm)

		adblockSectionsLines = append(adblockSectionsLines, fmt.Sprintf("hostname = %%APPEND%% %s, -github.com, -api.github.com, -*.githubusercontent.com", strings.Join(sortedAdblockMitm, ", ")))
		adblockSectionsLines = append(adblockSectionsLines, "")
	}

	if len(adblockSectionsLines) > 0 {
		adblockContent := strings.Join(adblockHeader, "\n") + "\n" + strings.Join(adblockSectionsLines, "\n")
		hub.EnsureDir(filepath.Dir(youtubeAdblockOutput))
		hub.SafeWriteFile(youtubeAdblockOutput, strings.TrimSpace(adblockContent)+"\n", true)
		hub.Success(fmt.Sprintf("Wrote extracted adblock module: %s", youtubeAdblockOutput))
	} else {
		hub.Warn("No YouTube adblock sections extracted; kept existing local module if any.")
	}

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
