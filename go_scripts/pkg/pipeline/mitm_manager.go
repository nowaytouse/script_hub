package pipeline

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

var restrictedDomains = []string{
	"github.com", "api.github.com", "*.github.com", "*.api.github.com",
	"raw.githubusercontent.com", "gist.githubusercontent.com",
	"*.objects.githubusercontent.com", "*.githubusercontent.com", "*.github.io",
	"*.apple.com", "*.icloud.com", "*.mzstatic.com", "*.itunes.com",
	"*.facebook.com", "*.instagram.com", "*.twitter.com",
	"*.google.com", "*.google.cn", "*.gmail.com", "*.youtube.com",
	"*.googlevideo.com", "*.gstatic.com", "*.googleapis.com",
	"*.bankofchina.com", "*.icbc.com.cn", "*.ccb.com", "*.cmbchina.com",
	"*.abchina.com", "*.boc.cn", "*.psbc.com", "*.spdb.com.cn", "*.cebbank.com",
	"*.cmbc.com.cn", "*.cib.com.cn", "*.hxb.com.cn", "*.pingan.com",
	"*.bankcomm.com", "*.cgbchina.com.cn", "*.ghbank.com.cn", "*.czbank.com",
	"*.ebank.com",
	"dns.alidns.com", "doh.pub", "dot.pub", "doh.360.cn", "dot.360.cn",
	"dns.baidu.com", "dns.volcengine.com", "alidns.com",
}

func isRestricted(domain string) bool {
	domainLower := strings.ToLower(domain)
	for _, pattern := range restrictedDomains {
		matched, _ := filepath.Match(strings.ToLower(pattern), domainLower)
		if matched {
			return true
		}
	}
	return false
}

func processFile(path string, dryRun bool) bool {
	if !hub.ValidateFileExists(path, "") {
		return false
	}

	content := hub.ReadFileString(path)
	if !strings.Contains(content, "[MITM]") {
		return false
	}

	lines := strings.Split(content, "\n")
	var newLines []string
	modified := false

	for _, line := range lines {
		stripped := strings.TrimSpace(line)
		if strings.HasPrefix(stripped, "hostname") && !strings.HasPrefix(stripped, "hostname-disabled") {
			if !strings.Contains(line, "=") {
				newLines = append(newLines, line)
				continue
			}

			parts := strings.SplitN(line, "=", 2)
			prefix := parts[0]
			valPart := strings.TrimSpace(parts[1])

			tag := ""
			for _, t := range []string{"%APPEND%", "%INSERT%", "%SET%"} {
				if strings.HasPrefix(valPart, t) {
					tag = t + " "
					valPart = strings.TrimSpace(strings.TrimPrefix(valPart, t))
					break
				}
			}

			var domains []string
			for _, d := range strings.Split(valPart, ",") {
				d = strings.TrimSpace(d)
				if d != "" {
					domains = append(domains, d)
				}
			}

			var newDomains []string
			for _, d := range domains {
				checkDomain := d
				if strings.HasPrefix(d, "-") {
					checkDomain = d[1:]
				}
				if !isRestricted(checkDomain) {
					newDomains = append(newDomains, d)
				}
			}

			if len(newDomains) != len(domains) {
				modified = true
			}

			if len(newDomains) > 0 {
				newLines = append(newLines, fmt.Sprintf("%s= %s%s", prefix, tag, strings.Join(newDomains, ", ")))
			} else {
				if tag != "" {
					newLines = append(newLines, fmt.Sprintf("%s= %s", prefix, tag))
				} else {
					newLines = append(newLines, fmt.Sprintf("%s=", prefix))
				}
			}
		} else {
			newLines = append(newLines, line)
		}
	}

	if modified {
		if dryRun {
			fmt.Printf("[DRY RUN] Would modify: %s\n", path)
		} else {
			finalContent := strings.Join(newLines, "\n")
			// ensure trailing newline
			if !strings.HasSuffix(finalContent, "\n") {
				finalContent += "\n"
			}
			hub.SafeWriteFile(path, finalContent, true)
		}
		return true
	}
	return false
}

func RunMitmCleanup(directory string, dryRun bool) int {
	scanDir := directory
	if scanDir == "" {
		scanDir = hub.MODULES_DIR
	}
	modifiedCount := 0

	filepath.Walk(scanDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		if !info.IsDir() && (strings.HasSuffix(info.Name(), ".sgmodule") || strings.HasSuffix(info.Name(), ".module")) {
			if processFile(path, dryRun) {
				if !dryRun {
					hub.Success(fmt.Sprintf("MITM Cleaned: %s", path))
				}
				modifiedCount++
			}
		}
		return nil
	})

	return modifiedCount
}
