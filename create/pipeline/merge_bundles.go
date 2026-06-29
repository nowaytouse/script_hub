package pipeline

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/nyamiiko/script_hub/create/hub"
)

var (
	bilibiliOutput  = filepath.Join(hub.ROOT, "modules/surge/amplify_nexus/📺 BiliBili增强合集.sgmodule")
	BilibiliSources = []hub.SourceSpec{
		{Label: "Enhanced", URL: "https://github.com/BiliUniverse/Enhanced/releases/latest/download/BiliBili.Enhanced.sgmodule"},
		{Label: "Global", URL: "https://github.com/BiliUniverse/Global/releases/latest/download/BiliBili.Global.sgmodule"},
		{Label: "Redirect", URL: "https://github.com/BiliUniverse/Redirect/releases/latest/download/BiliBili.Redirect.sgmodule"},
		{Label: "Helper", URL: hub.BILIBILI_HELPER_URL},
		{Label: "Bili1080P", URL: "https://yfamilys.com/module/bili.module"},
	}
	bilibiliHeader = map[string]string{
		"name":     "📺 BiliBili增强合集",
		"desc":     "合并 BiliUniverse + Maasea + Bili1080P（Enhanced/Global/Redirect/Helper）",
		"author":   "BiliUniverse, Maasea",
		"icon":     "https://www.bilibili.com/favicon.ico",
		"category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
		"tag":      "BiliBili, 增强",
	}

	appleOutput  = filepath.Join(hub.ROOT, "modules/surge/amplify_nexus/🍎 Apple服务增强合集.sgmodule")
	AppleSources = []hub.SourceSpec{
		{Label: "iRingo.Maps", URL: "https://github.com/NSRingo/GeoServices/releases/latest/download/iRingo.Maps.sgmodule"},
		{Label: "iRingo.WeatherKit", URL: "https://github.com/NSRingo/WeatherKit/releases/latest/download/iRingo.WeatherKit.sgmodule"},
		{Label: "iRingo.News", URL: "https://github.com/NSRingo/News/releases/latest/download/iRingo.News.sgmodule"},
		{Label: "iRingo.TV", URL: "https://github.com/NSRingo/TV/releases/latest/download/iRingo.TV.sgmodule"},
		{Label: "DualSubs.Universal", URL: "https://github.com/DualSubs/Universal/releases/latest/download/DualSubs.Universal.sgmodule"},
	}
	appleHeader = map[string]string{
		"name":     "🍎 Apple服务增强合集",
		"desc":     "整合 Apple 生态增强：Maps · WeatherKit · News · TV · DualSubs 双语字幕\nTV 需开启 SSL Pinning 绕过以配合 DualSubs；News 可自定义地区",
		"author":   "VirgilClyne[https://github.com/VirgilClyne]",
		"homepage": "https://NSRingo.github.io",
		"icon":     "https://developer.apple.com/assets/elements/icons/sf-symbols/sf-symbols-128x128.png",
		"category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
	}

	weiboLocal   = filepath.Join(hub.ROOT, "rulesets/Sources/LocalModules/Weibo.ADBlock.sgmodule")
	WeiboSources = []hub.SourceSpec{
		{Label: "Weibo", URL: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/weibo.module"},
		{Label: "Intl", URL: "https://raw.githubusercontent.com/iab0x00/ProxyRules/main/Rewrite/WeiboIntl.sgmodule"},
	}
	weiboHeader = map[string]string{
		"name":   "🐦 微博去广告",
		"desc":   "微博+国际版去广告 · PROMAX 构建源（域名规则+脚本已并入 PROMAX，勿单独安装）\n\n上游: fmz200/wool_scripts, iab0x00",
		"author": "fmz200, iab0x00, ScriptHub",
		"tag":    "去广告, 微博, PROMAX-build",
	}

	woolLocal   = filepath.Join(hub.ROOT, "rulesets/Sources/LocalModules/Wool.ADBlock.sgmodule")
	woolSources = []hub.SourceSpec{
		{Label: "Xiaohongshu", URL: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partX/Xiaohongshu.sgmodule"},
		{Label: "Baidu", URL: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partB/Baidu.sgmodule"},
		{Label: "BaiduTieba", URL: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partB/BaiduTieba.sgmodule"},
		{Label: "NetEaseCloudMusic", URL: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partW/NetEaseCloudMusic.sgmodule"},
		{Label: "AutoNavi", URL: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partG/AutoNavi.sgmodule"},
		{Label: "Douban", URL: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partD/Douban.sgmodule"},
		{Label: "WeChatOfficial", URL: "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Surge/module/split/partW/WeChatOfficialAccount.sgmodule"},
	}
	woolHeader = map[string]string{
		"name":   "🐑 综合去广告",
		"desc":   "国内常用APP去广告集合(小红书/贴吧/高德/爱优腾等) · PROMAX 构建源\n\n上游: fmz200/wool_scripts",
		"author": "fmz200, ScriptHub",
		"tag":    "去广告, 综合, PROMAX-build",
	}
	weiboSectionOrder = []string{"Rule", "URL Rewrite", "Body Rewrite", "Map Local", "Script", "MITM"}

	youtubeOutput         = filepath.Join(hub.ROOT, "modules/surge/amplify_nexus/📺 YouTube增强合集.sgmodule")
	youtubeAdblockOutput  = filepath.Join(hub.MODULE_LOCAL_DIR, "YouTube.ADBlock.sgmodule")
	youtubeLocalFallbacks = []string{
		filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, "YouTube.Enhance.sgmodule"),
		youtubeOutput,
	}

	utilitiesOutput  = filepath.Join(hub.ROOT, "modules/surge/amplify_nexus/📊 面板工具合集.sgmodule")
	UtilitiesSources = []hub.SourceSpec{
		{Label: "Timecard", URL: "https://raw.githubusercontent.com/Rabbit-Spec/Surge/Master/Module/Panel/Timecard/Moore/Timecard.sgmodule"},
		{Label: "net-lsp-x", URL: "https://raw.githubusercontent.com/xream/scripts/main/surge/modules/network-info/net-lsp-x.sgmodule"},
		{Label: "Sub_Info", URL: "https://raw.githubusercontent.com/Coldvvater/Mononoke/refs/heads/master/Surge/Module/Tool/Sub_Info.sgmodule"},
	}

	utilitiesHeader = map[string]string{
		"name":     "📊 面板工具合集",
		"desc":     "节假日信息 · 网络信息 𝕏 · 机场订阅流量/到期面板",
		"author":   "Rabbit-Spec, xream, Coldvvater",
		"icon":     "https://cdn.jsdelivr.net/gh/Orz-3/mini@master/Color/Setting.png",
		"category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
		"tag":      "面板, 工具",
	}

	devtoolsOutput     = filepath.Join(hub.ROOT, "modules/surge/amplify_nexus/🧰 Script Hub 配套工具合集.sgmodule")
	scriptHubModuleUrl = "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/modules/script-hub.surge.sgmodule"
	scriptHubLocal     = filepath.Join(hub.MODULE_LOCAL_DIR, "script_hub.surge.sgmodule")

	DevtoolsSources = []hub.SourceSpec{
		{Label: "BoxJs", URL: "https://raw.githubusercontent.com/chavyleung/scripts/master/box/rewrite/boxjs.rewrite.surge.sgmodule"},
		{Label: "Sub-Store", URL: "https://raw.githubusercontent.com/sub-store-org/Sub-Store/master/config/Surge-Beta.sgmodule"},
		{Label: "ScriptHub", URL: scriptHubModuleUrl},
	}

	subStoreScriptSources = map[string]string{
		"github_com_sub-store-org_sub-store-1.min.js":         "https://github.com/sub-store-org/Sub-Store/releases/latest/download/sub-store-1.min.js",
		"github_com_sub-store-org_sub-store-0.min.js":         "https://github.com/sub-store-org/Sub-Store/releases/latest/download/sub-store-0.min.js",
		"github_com_sub-store-org_cron-sync-artifacts.min.js": "https://github.com/sub-store-org/Sub-Store/releases/latest/download/cron-sync-artifacts.min.js",
	}

	subStoreScriptPathMap = map[string]string{
		"sub-store-1.min.js":         "github_com_sub-store-org_sub-store-1.min.js",
		"sub-store-0.min.js":         "github_com_sub-store-org_sub-store-0.min.js",
		"cron-sync-artifacts.min.js": "github_com_sub-store-org_cron-sync-artifacts.min.js",
	}

	scriptHubScriptSources = map[string]string{
		"raw_githubusercontent_com_f59ef7_script-hub.js":       "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/script-hub.js",
		"raw_githubusercontent_com_4fd0f5_Rewrite-Parser.js":   "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/Rewrite-Parser.js",
		"raw_githubusercontent_com_0aa101_rule-parser.js":      "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/rule-parser.js",
		"raw_githubusercontent_com_95a370_script-converter.js": "https://raw.githubusercontent.com/Script-Hub-Org/Script-Hub/main/script-converter.js",
	}

	mergedStandaloneSurge = []string{
		"boxjs.rewrite.surge.sgmodule",
		"Surge-Beta.sgmodule",
		"Script Hub 重写 & 规则集转换.sgmodule",
		"BiliBili.Enhanced.sgmodule",
		"BiliBili.Global.sgmodule",
		"BiliBili.Redirect.sgmodule",
		"bili.sgmodule",
		"YouTube.Enhance.sgmodule",
		"iRingo.Maps.sgmodule",
		"iRingo.WeatherKit.sgmodule",
		"iRingo.News.sgmodule",
		"iRingo.TV.sgmodule",
		"DualSubs.Universal.sgmodule",
		"Timecard.sgmodule",
		"net-lsp-x.sgmodule",
		"Sub_Info.sgmodule",
	}

	devtoolsHeader = map[string]string{
		"name":     "🧰 Script Hub 配套工具合集",
		"desc":     "Script Hub 转换 (script.hub) · BoxJs · Sub-Store(β)\nScript Hub / Sub-Store 脚本已 vendored 至 modules/source/scripts；BoxJs 仍走 chavyleung 上游",
		"author":   "@小白脸 @xream @keywos @ckyb, ChavyLeung, sub-store-org",
		"icon":     "https://cdn.jsdelivr.net/gh/Orz-3/mini@master/Color/Terminal.png",
		"category": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
		"tag":      "ScriptHub, BoxJs, Sub-Store",
		"homepage": "https://script.hub",
	}

	scriptHubScriptPathMap = map[string]string{
		"script-hub.js":       "raw_githubusercontent_com_f59ef7_script-hub.js",
		"Rewrite-Parser.js":   "raw_githubusercontent_com_4fd0f5_Rewrite-Parser.js",
		"rule-parser.js":      "raw_githubusercontent_com_0aa101_rule-parser.js",
		"script-converter.js": "raw_githubusercontent_com_95a370_script-converter.js",
	}
)

func pinSubStoreScriptPaths(modulePath string) {
	if !hub.ValidateFileExists(modulePath, "") {
		return
	}
	text := hub.ReadFileString(modulePath)
	changed := 0

	for upstreamSuffix, localName := range subStoreScriptPathMap {
		localPath := filepath.Join(hub.MODULE_SOURCE_SCRIPTS_DIR, localName)
		if !hub.ValidateFileExists(localPath, "") {
			continue
		}
		canonical := hub.SCRIPT_RAW_PREFIX + localName

		reStr := fmt.Sprintf(`script-path=https?://[^\s,]*%s`, regexp.QuoteMeta(upstreamSuffix))
		re := regexp.MustCompile(reStr)

		matches := re.FindAllStringIndex(text, -1)
		if len(matches) > 0 {
			text = re.ReplaceAllString(text, fmt.Sprintf(`script-path=%s`, canonical))
			changed += len(matches)
		}
	}

	if changed > 0 {
		hub.SafeWriteFile(modulePath, text, true)
		hub.Info(fmt.Sprintf("  Pinned %d Sub-Store script-path URL(s) in %s", changed, filepath.Base(modulePath)))
	}
}

func syncSubStoreScripts() {
	hub.EnsureDir(hub.MODULE_SOURCE_SCRIPTS_DIR)
	updated := 0
	for localName, urlStr := range subStoreScriptSources {
		content := hub.SafeDownload(urlStr, 2, 90)
		if content == "" {
			hub.Warn(fmt.Sprintf("Sub-Store script sync skipped: %s", localName))
			continue
		}
		target := filepath.Join(hub.MODULE_SOURCE_SCRIPTS_DIR, localName)
		hub.SafeWriteFile(target, content, true)
		updated++
	}
	if updated > 0 {
		hub.Success(fmt.Sprintf("Synced %d/%d Sub-Store scripts to modules/source/scripts/", updated, len(subStoreScriptSources)))
	}
}

func pinScriptHubScriptPaths(modulePath string) {
	if !hub.ValidateFileExists(modulePath, "") {
		return
	}
	text := hub.ReadFileString(modulePath)
	changed := 0

	for upstreamSuffix, localName := range scriptHubScriptPathMap {
		localPath := filepath.Join(hub.MODULE_SOURCE_SCRIPTS_DIR, localName)
		if !hub.ValidateFileExists(localPath, "") {
			continue
		}
		canonical := hub.SCRIPT_RAW_PREFIX + localName

		reStr1 := fmt.Sprintf(`script-path=https?://[^\s,]*%s`, regexp.QuoteMeta(upstreamSuffix))
		re1 := regexp.MustCompile(reStr1)
		matches1 := re1.FindAllStringIndex(text, -1)
		if len(matches1) > 0 {
			text = re1.ReplaceAllString(text, fmt.Sprintf(`script-path=%s`, canonical))
			changed += len(matches1)
		}

		reStr2 := fmt.Sprintf(`script-path=https?://[^\s,]*%s`, regexp.QuoteMeta(localName))
		re2 := regexp.MustCompile(reStr2)
		matches2 := re2.FindAllStringIndex(text, -1)
		if len(matches2) > 0 {
			text = re2.ReplaceAllString(text, fmt.Sprintf(`script-path=%s`, canonical))
			changed += len(matches2)
		}
	}

	if changed > 0 {
		hub.SafeWriteFile(modulePath, text, true)
		hub.Info(fmt.Sprintf("  Pinned %d Script Hub script-path URL(s) in %s", changed, filepath.Base(modulePath)))
	}
}

func syncScriptHubScripts() {
	hub.EnsureDir(hub.MODULE_SOURCE_SCRIPTS_DIR)
	updated := 0
	for localName, urlStr := range scriptHubScriptSources {
		content := hub.SafeDownload(urlStr, 2, 60)
		if content == "" {
			hub.Warn(fmt.Sprintf("Script Hub script sync skipped: %s", localName))
			continue
		}
		target := filepath.Join(hub.MODULE_SOURCE_SCRIPTS_DIR, localName)
		hub.SafeWriteFile(target, content, true)
		updated++
	}
	if updated > 0 {
		hub.Success(fmt.Sprintf("Synced %d/%d Script Hub scripts from upstream", updated, len(scriptHubScriptSources)))
	}
}

func cleanupMergedStandaloneModules() {
	srDir := hub.SHADOWROCKET_AMPLIFY_NEXUS_DIR
	removed := 0
	for _, name := range mergedStandaloneSurge {
		surgePath := filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, name)
		if err := os.Remove(surgePath); err == nil {
			removed++
		}
		srName := strings.ReplaceAll(name, ".sgmodule", ".module")
		srPath := filepath.Join(srDir, srName)
		if err := os.Remove(srPath); err == nil {
			removed++
		}
	}
	if removed > 0 {
		hub.Info(fmt.Sprintf("Removed %d merged standalone module file(s)", removed))
	}
}

func MergeBilibili() error {
	hub.Section("BiliBili upstream bundle merge")
	return hub.MergeUpstreamModules(
		BilibiliSources,
		bilibiliOutput,
		bilibiliHeader,
		"",
		[]string{"Helper"},
		nil,
		[][2]string{
			{`Proxies.HKG:"🇭🇰香港"`, `Proxies.HKG:"📺 哔哩哔哩 📱"`},
			{`Proxies.MAC:"🇲🇴澳门"`, `Proxies.MAC:"📺 哔哩哔哩 📱"`},
			{`Proxies.TWN:"🇹🇼台湾"`, `Proxies.TWN:"📺 哔哩哔哩 📱"`},
			{`Bottom:"home,dynamic,ogv,会员购Bottom,我的Bottom"`, `Bottom:"home,dynamic,ogv,我的Bottom"`},
			{`Home.Tab:"直播tab,推荐tab,hottopic,bangumi,anime,film,koreavtw"`, `Home.Tab:"直播tab,推荐tab,bangumi,anime"`},
			{`Host.OverseaVideo:"upos-sz-mirrorali.bilivideo.com"`, `Host.OverseaVideo:"upos-sz-mirrorakam.akamaized.net"`},
			{`Host.BStar:"upos-sz-mirrorali.bilivideo.com"`, `Host.BStar:"upos-sz-mirrorakam.akamaized.net"`},
		},
	)
}

func MergeApple() error {
	hub.Section("Apple services upstream bundle merge")
	return hub.MergeUpstreamModules(
		AppleSources,
		appleOutput,
		appleHeader,
		"",
		nil,
		nil,
		[][2]string{
			{`Proxy:🇺🇸美国`, `Proxy:"🍎 Apple 🍏"`},
			{`,🇺🇸美国`, `,{{{Proxy}}}`},
			{`AirQuality.Calculate.Algorithm:"EU_EAQI"`, `AirQuality.Calculate.Algorithm:"US_AQI"`},
		},
	)
}

func MergeUtilities() error {
	hub.Section("Panel utilities upstream bundle merge")
	return hub.MergeUpstreamModules(
		UtilitiesSources,
		utilitiesOutput,
		utilitiesHeader,
		"",
		nil,
		map[string]string{
			"Timecard":  filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, "Timecard.sgmodule"),
			"net-lsp-x": filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, "net-lsp-x.sgmodule"),
			"Sub_Info":  filepath.Join(hub.SURGE_AMPLIFY_NEXUS_DIR, "Sub_Info.sgmodule"),
		},
		nil,
	)
}
