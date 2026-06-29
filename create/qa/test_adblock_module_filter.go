package qa

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/hub"
	"github.com/nyamiiko/script_hub/go_scripts/pipeline"
)

func runAdblockTest(name string, f func() error) error {
	if err := f(); err != nil {
		return fmt.Errorf("%s: %v", name, err)
	}
	return nil
}

func testFilenameGate() error {
	cases := []struct {
		path     string
		expected string
	}{
		{filepath.Join(hub.MODULE_LOCAL_DIR, "知乎去广告.sgmodule"), "split"},
		{filepath.Join(hub.MODULE_LOCAL_DIR, "扫描全能王解锁.sgmodule"), "skip"},
		{filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, "BiliBili.Enhanced.sgmodule"), "skip"},
		{filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, "扫描全能王解锁.sgmodule"), "skip"},
		{filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, "WeChat_Enhance.sgmodule"), "skip"},
	}

	for _, c := range cases {
		if !hub.ValidateFileExists(c.path, "") {
			continue
		}
		text := hub.ReadFileString(c.path)
		if mode := hub.ModuleIngestMode(c.path, text); mode != c.expected {
			return fmt.Errorf("expected %s, got %s for %s", c.expected, mode, c.path)
		}
	}
	return nil
}

func testBilibiliBundleSplitKeepsAdOnly() error {
	path := filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, "📺 BiliBili增强合集.sgmodule")
	if !hub.ValidateFileExists(path, "") {
		return nil
	}
	text := hub.ReadFileString(path)
	adSections, stats := hub.SplitModuleSections(text, path)

	if stats.KeptLines <= 0 {
		return fmt.Errorf("stats.KeptLines <= 0")
	}
	if stats.KeptLines >= stats.TotalLines {
		return fmt.Errorf("stats.KeptLines >= stats.TotalLines")
	}

	var scripts []string
	for name, lines := range adSections {
		if name == "Script" {
			scripts = append(scripts, lines...)
		}
	}

	hasAd := false
	for _, s := range scripts {
		if strings.Contains(s, "ADBlock") || strings.Contains(strings.ToLower(s), "helper") {
			hasAd = true
		}
		if strings.Contains(s, "BiliBili.Enhanced") || strings.Contains(s, "BiliBili.Global") {
			return fmt.Errorf("found BiliBili.Enhanced/Global in Script section")
		}
	}
	if !hasAd {
		return fmt.Errorf("no ADBlock or helper found in Script section")
	}

	hasReject := false
	for name, lines := range adSections {
		if name == "Rule" {
			for _, r := range lines {
				if strings.Contains(r, "REJECT") {
					hasReject = true
				}
			}
		}
	}
	if !hasReject {
		return fmt.Errorf("no REJECT found in Rule section")
	}
	return nil
}

func testPddModuleFullIngestKeepsAllBodyRewrite() error {
	path := filepath.Join(hub.MODULE_LOCAL_DIR, "拼多多去广告.sgmodule")
	if !hub.ValidateFileExists(path, "") {
		return nil
	}
	text := hub.ReadFileString(path)
	if hub.ModuleIngestMode(path, text) != "full" {
		return fmt.Errorf("expected full mode")
	}
	jqCount := strings.Count(text, "http-response-jq")
	adSections, stats := hub.SplitModuleSections(text, path)
	if stats.SkippedLines != 0 {
		return fmt.Errorf("stats.SkippedLines != 0")
	}
	bodyRewriteCount := 0
	for name, lines := range adSections {
		if name == "Body Rewrite" {
			bodyRewriteCount += len(lines)
		}
	}
	if bodyRewriteCount != jqCount {
		return fmt.Errorf("bodyRewriteCount != jqCount")
	}
	return nil
}

func testWeiboAdScriptsKeptInSplit() error {
	path := filepath.Join(hub.MODULE_LOCAL_DIR, "🐦 微博去广告合集.sgmodule")
	if !hub.ValidateFileExists(path, "") {
		return nil
	}
	line := "微博热搜页面广告 = type=http-response, pattern=^https?:\\/\\/m?api\\.weibo\\.c(n|om)\\/2\\/(page|flowpage)\\?, script-path=https://raw.githubusercontent.com/fmz200/wool_scripts/main/Scripts/weibo/weibo_ads.js, requires-body=true"
	if hub.ClassifyPromaxLine(line, "Script", path) != "ad" {
		return fmt.Errorf("expected ad")
	}
	return nil
}

func testZhihuLegacyBodyRewriteKept() error {
	line := `http-response ^https:\/\/api\.zhihu\.com\/search\/recommend_query\/v2\? "recommend_queries":\{.+\} "recommend_queries":{}`
	if hub.ClassifyPromaxLine(line, "Body Rewrite", "") != "ad" {
		return fmt.Errorf("expected ad")
	}
	return nil
}

func testRednoteScriptsFullModule() error {
	path := filepath.Join(hub.MODULE_LOCAL_DIR, "RedNote.sgmodule")
	if !hub.ValidateFileExists(path, "") {
		return nil
	}
	text := hub.ReadFileString(path)
	if hub.ModuleIngestMode(path, text) != "full" {
		return fmt.Errorf("expected full mode")
	}
	_, stats := hub.SplitModuleSections(text, path)
	if stats.SkippedLines != 0 {
		return fmt.Errorf("stats.SkippedLines != 0")
	}
	return nil
}

func testUnlockWeiboScriptStillDropped() error {
	path := filepath.Join(hub.MODULE_LOCAL_DIR, "🐦 微博去广告合集.sgmodule")
	line := "解锁微博会员APP图标 = type=http-response, pattern=^https?:\\/\\/new\\.vip\\.weibo\\.c(n|om)\\/aj\\/appicon\\/list, script-path=https://example/unlock.js"
	if hub.ShouldKeepPromaxLine(line, "Script", path) {
		return fmt.Errorf("expected ShouldKeepPromaxLine to return false")
	}
	return nil
}

func testPddBottomTabsBodyRewriteKept() error {
	line := `http-response-jq ^https:\/\/api\.pinduoduo\.com\/api\/alexa\/homepage\/hub\? '.result.bottom_tabs? |= map(select(.link | IN("index.html", "chat_list.html", "personal.html")))'`
	if hub.ClassifyPromaxLine(line, "Body Rewrite", "") != "ad" {
		return fmt.Errorf("expected ad")
	}
	return nil
}

func testClassifierAdblockScript() error {
	line := "📺 BiliBili.ADBlock.feed.index.request.json = type=http-request,script-path=https://github.com/BiliUniverse/ADBlock/releases/download/v0.6.24/request.bundle.js"
	if hub.ClassifyPromaxLine(line, "Script", "") != "ad" {
		return fmt.Errorf("expected ad for %s", line)
	}
	enh := "📺 BiliBili.Enhanced.x.resource.show.tab.v2 = type=http-response,script-path=https://github.com/BiliUniverse/Enhanced/releases/download/v0.5.13/response.bundle.js"
	if hub.ClassifyPromaxLine(enh, "Script", "") != "enhance" {
		return fmt.Errorf("expected enhance for %s", enh)
	}
	return nil
}

func testFunctionalResolveIncludesBilibiliAdblockLocal() error {
	mgr := pipeline.NewAdBlockManager()
	paths, _ := mgr.LoadFunctionalSourcePaths()
	byPath := make(map[string]string)
	for _, p := range paths {
		byPath[p[0]] = p[1]
	}

	localAdblock := filepath.Clean(filepath.Join(hub.ROOT, "rulesets/Sources", "BiliBili.ADBlock.sgmodule"))
	if hub.ValidateFileExists(localAdblock, "") {
		if byPath[localAdblock] != "full" {
			return fmt.Errorf("expected full mode for local adblock")
		}
	}
	for p := range byPath {
		if strings.Contains(p, "BiliBili.Enhanced.sgmodule") {
			return fmt.Errorf("found BiliBili.Enhanced.sgmodule")
		}
		if strings.Contains(p, "WeChat_Enhance") {
			return fmt.Errorf("found WeChat_Enhance")
		}
	}
	return nil
}

func testLocalModulesOnlyFunctionalScan() error {
	mgr := pipeline.NewAdBlockManager()
	paths, _ := mgr.LoadFunctionalSourcePaths()
	for _, p := range paths {
		if strings.Contains(p[0], "narrow_pierce") {
			return fmt.Errorf("found narrow_pierce")
		}
	}
	hasLocalModules := false
	for _, p := range paths {
		if strings.Contains(p[0], "LocalModules") {
			hasLocalModules = true
		}
	}
	if !hasLocalModules && len(paths) > 0 {
		return fmt.Errorf("LocalModules not scanned")
	}
	return nil
}

func testAmplifyNexusNotAutoScanned() error {
	mgr := pipeline.NewAdBlockManager()
	paths, _ := mgr.LoadFunctionalSourcePaths()
	for _, p := range paths {
		if strings.Contains(p[0], "iRingo") {
			return fmt.Errorf("found iRingo")
		}
		if strings.Contains(p[0], "YouTube.Enhance") {
			return fmt.Errorf("found YouTube.Enhance")
		}
		if strings.Contains(p[0], "Apple服务增强") {
			return fmt.Errorf("found Apple服务增强")
		}
	}
	return nil
}

func testSukkaAdblockAllowlistedFull() error {
	path := filepath.Join(hub.ROOT, "rulesets/Sources", "[Sukka] Enhance Better ADBlock for Surge.sgmodule")
	if !hub.ValidateFileExists(path, "") {
		return nil
	}
	mgr := pipeline.NewAdBlockManager()
	if mgr.ResolveModuleIngestMode(path, false) != "full" {
		return fmt.Errorf("expected full mode")
	}
	return nil
}

func RunTestAdblockModuleFilter() int {
	tests := []struct {
		name string
		fn   func() error
	}{
		{"testPddModuleFullIngestKeepsAllBodyRewrite", testPddModuleFullIngestKeepsAllBodyRewrite},
		{"testWeiboAdScriptsKeptInSplit", testWeiboAdScriptsKeptInSplit},
		{"testZhihuLegacyBodyRewriteKept", testZhihuLegacyBodyRewriteKept},
		{"testRednoteScriptsFullModule", testRednoteScriptsFullModule},
		{"testUnlockWeiboScriptStillDropped", testUnlockWeiboScriptStillDropped},
		{"testPddBottomTabsBodyRewriteKept", testPddBottomTabsBodyRewriteKept},
		{"testClassifierAdblockScript", testClassifierAdblockScript},
		{"testFilenameGate", testFilenameGate},
		{"testBilibiliBundleSplitKeepsAdOnly", testBilibiliBundleSplitKeepsAdOnly},
		{"testFunctionalResolveIncludesBilibiliAdblockLocal", testFunctionalResolveIncludesBilibiliAdblockLocal},
		{"testLocalModulesOnlyFunctionalScan", testLocalModulesOnlyFunctionalScan},
		{"testAmplifyNexusNotAutoScanned", testAmplifyNexusNotAutoScanned},
		{"testSukkaAdblockAllowlistedFull", testSukkaAdblockAllowlistedFull},
	}

	failed := false
	for _, t := range tests {
		if err := runAdblockTest(t.name, t.fn); err != nil {
			fmt.Println("FAIL:", err)
			failed = true
		}
	}

	if failed {
		return 1
	}

	fmt.Println("test_adblock_module_filter: OK")
	return 0
}
