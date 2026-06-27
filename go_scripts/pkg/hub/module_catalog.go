package hub

import (
	"crypto/md5"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"
)

const (
	PROMAX_FILENAME    = "🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule"
	PROMAX_MERGE_LABEL = "🚫 Universal Ad-Blocking Rules (PROMAX)"
)

var PROMAX_MERGED_NAME_HINTS = []string{"去广告", "AdBlock", "adblock", "ADBlock"}

var (
	CDN_BASE    = REPO_RAW_PREFIX
	GITHUB_BASE = GITHUB_RAW_PREFIX
)

const (
	HEAD_EXPANSE_GITHUB_REL    = "modules/surge/head_expanse/github/"
	HEAD_EXPANSE_SR_GITHUB_REL = "modules/shadowrocket/head_expanse/github/"
	HEAD_EXPANSE_GITHUB_CAT    = "head_expanse"
)

var CATEGORIES = map[string]string{
	"amplify_nexus": "『 🛠️ Amplify Nexus › 增幅枢纽 』",
	"head_expanse":  "『 🔝 Head Expanse › 首端扩域 』",
	"narrow_pierce": "『 🎯 Narrow Pierce › 窄域穿刺 』",
}

var CATEGORY_SHORT = map[string]string{
	"amplify_nexus": "🛠️ Amplify Nexus › 增幅枢纽",
	"head_expanse":  "🔝 Head Expanse › 首端扩域",
	"narrow_pierce": "🎯 Narrow Pierce › 窄域穿刺",
}

var MERGED_ALIASES = map[string]string{
	"bili.sgmodule":                  "📺 BiliBili增强合集",
	"BiliBili.Enhanced.sgmodule":     "📺 BiliBili增强合集",
	"BiliBili.Global.sgmodule":       "📺 BiliBili增强合集",
	"BiliBili.Redirect.sgmodule":     "📺 BiliBili增强合集",
	"sukka_url_redirect.sgmodule":    PROMAX_MERGE_LABEL,
	"Timecard.sgmodule":              "📊 面板工具合集",
	"net-lsp-x.sgmodule":             "📊 面板工具合集",
	"Sub_Info.sgmodule":              "📊 面板工具合集",
	"YouTube.Enhance.sgmodule":       "📺 YouTube增强合集",
	"iRingo.Maps.sgmodule":           "🍎 Apple服务增强合集",
	"iRingo.WeatherKit.sgmodule":     "🍎 Apple服务增强合集",
	"iRingo.News.sgmodule":           "🍎 Apple服务增强合集",
	"iRingo.TV.sgmodule":             "🍎 Apple服务增强合集",
	"DualSubs.Universal.sgmodule":    "🍎 Apple服务增强合集",
	"boxjs.rewrite.surge.sgmodule":   "🧰 Script Hub 配套工具合集",
	"Surge-Beta.sgmodule":            "🧰 Script Hub 配套工具合集",
	"Script Hub 重写 & 规则集转换.sgmodule": "🧰 Script Hub 配套工具合集",
}

var UI_BADGES = map[string]string{
	"🧰 Script Hub 配套工具合集":                    "⭐",
	"🚫 Universal Ad-Blocking Rules (PROMAX)": "🔥",
}

var metaCategoryLineRe = regexp.MustCompile(`(?i)^#!\s*category\s*[=:]\s*.*$`)
var srCategoryLineRe = regexp.MustCompile(`(?i)^#\s*category\s*:.*$`)

func NormalizeCategoriesTree(baseDir string, projectRoot string, pattern string) int {
	changed := 0
	if !ValidateDirExists(baseDir, "Base Directory") {
		return 0
	}

	filepath.Walk(baseDir, func(path string, info os.FileInfo, err error) error {
		if err != nil || info.IsDir() {
			return nil
		}
		if matched, _ := filepath.Match(pattern, info.Name()); matched {
			rel, err := filepath.Rel(baseDir, path)
			if err != nil {
				return nil
			}
			parts := strings.Split(rel, string(os.PathSeparator))
			if len(parts) < 2 {
				return nil
			}
			catKey := parts[0]
			expected, ok := CATEGORIES[catKey]
			if !ok {
				return nil
			}

			content := SafeReadFile(path)
			newText, modified := applyCategoryToFile(content, expected, strings.ToLower(filepath.Ext(path)))
			if modified {
				SafeWriteFile(path, newText, false)
				changed++
				relProj, _ := filepath.Rel(projectRoot, path)
				fmt.Printf("  📂 Category fixed: %s → %s\n", relProj, expected)
			}
		}
		return nil
	})
	return changed
}

func applyCategoryToFile(content string, expected string, suffix string) (string, bool) {
	isSurge := suffix == ".sgmodule"
	categoryLine := fmt.Sprintf("#!category=%s", expected)
	srLine := fmt.Sprintf("# category: %s", expected)

	lines := strings.Split(content, "\n")
	var out []string
	found := false
	modified := false
	nameIdx := -1

	for i, line := range lines {
		stripped := strings.TrimSpace(line)
		if strings.HasPrefix(strings.ToLower(stripped), "#!name") {
			nameIdx = len(out)
		}

		if isSurge && metaCategoryLineRe.MatchString(stripped) {
			found = true
			if stripped != categoryLine {
				out = append(out, categoryLine)
				modified = true
			} else {
				out = append(out, line)
			}
			continue
		}

		if !isSurge && srCategoryLineRe.MatchString(stripped) {
			found = true
			if stripped != srLine {
				out = append(out, srLine)
				modified = true
			} else {
				out = append(out, line)
			}
			continue
		}

		if !isSurge && metaCategoryLineRe.MatchString(stripped) {
			found = true
			if stripped != categoryLine {
				out = append(out, categoryLine)
				modified = true
			} else {
				out = append(out, line)
			}
			continue
		}

		// Don't append the last empty element if content didn't have a trailing newline
		if i == len(lines)-1 && line == "" && !strings.HasSuffix(content, "\n") {
			continue
		}
		out = append(out, line)
	}

	if !found && isSurge {
		insertAt := 0
		if nameIdx >= 0 {
			insertAt = nameIdx + 1
		}
		out = append(out[:insertAt], append([]string{categoryLine}, out[insertAt:]...)...)
		modified = true
	} else if !found && !isSurge {
		out = append([]string{srLine}, out...)
		modified = true
	}

	text := strings.Join(out, "\n")
	if strings.HasSuffix(content, "\n") {
		text += "\n"
	}
	return text, modified
}

type ModuleInfo struct {
	Id               string   `json:"id"`
	Filename         string   `json:"filename"`
	Category         string   `json:"category"`
	Path             string   `json:"path"`
	HasArguments     bool     `json:"has_arguments"`
	MergedInto       string   `json:"merged_into"`
	InstallUrl       string   `json:"install_url"`
	GithubInstallUrl string   `json:"github_install_url,omitempty"`
	Name             string   `json:"name"`
	Desc             string   `json:"desc"`
	Tags             []string `json:"tags"`
	Author           string   `json:"author"`
	Date             string   `json:"date"`
	Icon             string   `json:"icon"`
	Essential        *bool    `json:"essential,omitempty"`
	Note             string   `json:"note,omitempty"`
}

func ScanModules(projectRoot string, surgeDir string) []ModuleInfo {
	deduped := make(map[string]ModuleInfo)

	for catKey := range CATEGORIES {
		catPath := filepath.Join(surgeDir, catKey)
		if !ValidateDirExists(catPath, "") {
			continue
		}
		files, _ := filepath.Glob(filepath.Join(catPath, "*.sgmodule"))
		for _, moduleFile := range files {
			relPath, _ := filepath.Rel(projectRoot, moduleFile)
			relPath = strings.ReplaceAll(relPath, "\\", "/") // Ensure POSIX for URLs
			encodedPath := (&url.URL{Path: relPath}).String()
			filename := filepath.Base(moduleFile)

			githubInstallUrl := ""
			if catKey == HEAD_EXPANSE_GITHUB_CAT {
				githubRel := HEAD_EXPANSE_GITHUB_REL + filename
				githubInstallUrl = GITHUB_BASE + (&url.URL{Path: githubRel}).String()
			}

			mergedInto := MERGED_ALIASES[filename]

			info := ModuleInfo{
				Id:               strings.TrimSuffix(filename, filepath.Ext(filename)),
				Filename:         filename,
				Category:         catKey,
				Path:             relPath,
				HasArguments:     false,
				MergedInto:       mergedInto,
				InstallUrl:       CDN_BASE + encodedPath,
				GithubInstallUrl: githubInstallUrl,
			}

			content := SafeReadFile(moduleFile)
			meta, _ := ParseModule(content)

			info.Name = meta["name"]
			if info.Name == "" {
				info.Name = info.Id
			}
			info.Desc = meta["desc"]
			if tagsStr, ok := meta["tag"]; ok {
				for _, t := range strings.Split(tagsStr, ",") {
					t = strings.TrimSpace(t)
					if t != "" {
						info.Tags = append(info.Tags, t)
					}
				}
			} else {
				info.Tags = []string{}
			}
			_, info.HasArguments = meta["arguments"]
			info.Author = meta["author"]
			info.Date = meta["date"]
			info.Icon = meta["icon"]

			isPromaxHint := false
			for _, h := range PROMAX_MERGED_NAME_HINTS {
				if strings.Contains(filename, h) {
					isPromaxHint = true
					break
				}
			}

			if info.MergedInto == "" && !strings.Contains(filename, "Helper") && isPromaxHint {
				info.MergedInto = PROMAX_MERGE_LABEL
			}

			if info.MergedInto != "" {
				f := false
				info.Essential = &f
				info.Note = fmt.Sprintf("已合并进「%s」，请勿重复安装", info.MergedInto)
			}

			unquotedName, _ := url.QueryUnescape(filename)
			key := fmt.Sprintf("%s|%s", catKey, unquotedName)

			existing, ok := deduped[key]
			prefer := !ok || (strings.Contains(filename, "%") && !strings.Contains(existing.Filename, "%"))
			if prefer {
				deduped[key] = info
			}
		}
	}

	var result []ModuleInfo
	for _, v := range deduped {
		result = append(result, v)
	}
	sort.Slice(result, func(i, j int) bool {
		if result[i].Category != result[j].Category {
			return result[i].Category < result[j].Category
		}
		return strings.ToLower(result[i].Filename) < strings.ToLower(result[j].Filename)
	})
	return result
}

func SanitizeTree(baseDir string, projectRoot string, pattern string) int {
	changed := 0
	if !ValidateDirExists(baseDir, "") {
		return 0
	}
	filepath.Walk(baseDir, func(path string, info os.FileInfo, err error) error {
		if err != nil || info.IsDir() {
			return nil
		}
		if matched, _ := filepath.Match(pattern, info.Name()); matched {
			if info.Name() == PROMAX_FILENAME {
				return nil
			}
			original := SafeReadFile(path)
			cleaned := SanitizeFileContent(original, true)
			if cleaned != original {
				SafeWriteFile(path, cleaned, true)
				changed++
				relProj, _ := filepath.Rel(projectRoot, path)
				fmt.Printf("  🧹 Sanitized: %s\n", relProj)
			}
		}
		return nil
	})
	return changed
}

func WriteModulesJson(modules []ModuleInfo, outputPath string) {
	payload := map[string]interface{}{
		"generated": time.Now().Format(time.RFC3339),
		"total":     len(modules),
		"policy": map[string]string{
			"adblock":  "仅安装 PROMAX（含各 App 去广告脚本；专项去广告模块勿重复安装）",
			"features": "解锁/增强/翻译等保留 Amplify Nexus 独立模块",
		},
		"modules": modules,
	}
	b, _ := json.MarshalIndent(payload, "", "  ")
	SafeWriteFile(outputPath, string(b)+"\n", true)
}

// WriteShadowrocketModulesJson, BuildHelperHtml omitted temporarily for brevity/focus,
// to be translated iteratively or in a separate file if needed.
// These are purely static template generations.

func modulesToHelperGroups(modules []ModuleInfo, projectRoot string, shadowrocket bool) map[string]map[string]interface{} {
	groups := make(map[string]map[string]interface{})
	for k := range CATEGORIES {
		groups[k] = map[string]interface{}{
			"name":  CATEGORY_SHORT[k],
			"items": []map[string]interface{}{},
		}
	}
	for _, m := range modules {
		if m.MergedInto != "" {
			continue
		}
		cat := m.Category
		if _, ok := groups[cat]; !ok {
			continue
		}
		path := m.Path
		desc := m.Desc
		githubUrl := ""

		if shadowrocket {
			path = strings.Replace(path, "modules/surge", "modules/shadowrocket", 1)
			path = strings.Replace(path, ".sgmodule", ".module", 1)
			if !ValidateFileExists(filepath.Join(projectRoot, path), "") {
				continue
			}
			desc = "[🚀SR] " + desc
			if m.GithubInstallUrl != "" {
				srGithubRel := HEAD_EXPANSE_SR_GITHUB_REL + strings.Replace(m.Filename, ".sgmodule", ".module", 1)
				githubUrl = GITHUB_BASE + (&url.URL{Path: srGithubRel}).String()
			} else {
				githubUrl = CDN_BASE + (&url.URL{Path: path}).String()
			}
		} else {
			githubUrl = m.GithubInstallUrl
			if githubUrl == "" {
				githubUrl = CDN_BASE + (&url.URL{Path: path}).String()
			}
		}

		encodedPath := (&url.URL{Path: strings.ReplaceAll(path, "\\", "/")}).String()
		urlStr := CDN_BASE + encodedPath
		name := m.Name
		badge := ""
		for key, icon := range UI_BADGES {
			if strings.Contains(name, key) || strings.Contains(m.Filename, key) {
				badge = icon
				break
			}
		}

		hash := md5.Sum([]byte(urlStr))
		id := hex.EncodeToString(hash[:])

		item := map[string]interface{}{
			"id":         id,
			"name":       name,
			"desc":       desc,
			"author":     m.Author,
			"date":       m.Date,
			"icon":       m.Icon,
			"url":        urlStr,
			"github_url": githubUrl,
			"badge":      badge,
			"filename":   m.Filename,
		}
		items := groups[cat]["items"].([]map[string]interface{})
		groups[cat]["items"] = append(items, item)
	}
	return groups
}

func BuildHelperHtml(modules []ModuleInfo, projectRoot string, outputPath string) {
	surgeGroups := modulesToHelperGroups(modules, projectRoot, false)
	srGroups := modulesToHelperGroups(modules, projectRoot, true)

	surgeTotal := 0
	for _, g := range surgeGroups {
		surgeTotal += len(g["items"].([]map[string]interface{}))
	}
	srTotal := 0
	for _, g := range srGroups {
		srTotal += len(g["items"].([]map[string]interface{}))
	}

	surgeData, _ := json.Marshal(surgeGroups)
	srData, _ := json.Marshal(srGroups)

	html := fmt.Sprintf(`<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <title>Script Hub | 模块快速复制中心</title>
</head>
<body>
    <div id="content"></div>
    <script>
        const surgeData = %s;
        const srData = %s;
        console.log(surgeData, srData);
    </script>
</body>
</html>`, string(surgeData), string(srData))
	SafeWriteFile(outputPath, html, true)
}

func WriteShadowrocketModulesJson(modules []ModuleInfo, outputPath string, projectRoot string) {
	categoriesData := make(map[string]map[string]interface{})
	totalSrModules := 0
	catKeys := []string{"amplify_nexus", "head_expanse"}
	categoryDescs := map[string]string{
		"amplify_nexus": "功能增强类模块",
		"head_expanse":  "核心拦截/重写",
	}

	for _, cat := range catKeys {
		categoriesData[cat] = map[string]interface{}{
			"name":  CATEGORY_SHORT[cat],
			"desc":  categoryDescs[cat],
			"items": []map[string]string{},
		}
	}

	for _, m := range modules {
		if m.MergedInto != "" {
			continue
		}
		cat := m.Category
		if _, ok := categoriesData[cat]; !ok {
			continue
		}
		path := m.Path
		srRelPath := strings.Replace(path, "modules/surge", "modules/shadowrocket", 1)
		srRelPath = strings.Replace(srRelPath, ".sgmodule", ".module", 1)
		if !ValidateFileExists(filepath.Join(projectRoot, srRelPath), "") {
			continue
		}
		desc := m.Desc
		if desc != "" && !strings.HasPrefix(desc, "[🚀SR]") {
			desc = "[🚀SR] " + desc
		}
		encodedSrPath := (&url.URL{Path: strings.ReplaceAll(srRelPath, "\\", "/")}).String()
		urlStr := CDN_BASE + encodedSrPath

		item := map[string]string{
			"name": m.Name,
			"desc": desc,
			"url":  urlStr,
		}

		items := categoriesData[cat]["items"].([]map[string]string)
		categoriesData[cat]["items"] = append(items, item)
		totalSrModules++
	}

	payload := map[string]interface{}{
		"generated":  time.Now().Format(time.RFC3339),
		"categories": categoriesData,
		"total":      totalSrModules,
	}
	b, _ := json.MarshalIndent(payload, "", "  ")
	SafeWriteFile(outputPath, string(b)+"\n", true)
}
