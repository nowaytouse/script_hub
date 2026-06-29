package qa

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/nowaytouse/script_hub/create/hub"
)

func RunRefreshVendorSnapshots() int {
	vendorDir := filepath.Join(hub.ROOT, "rulesets/Sources/vendor")
	hub.EnsureDir(vendorDir)

	pairs := []struct {
		url  string
		name string
	}{
		{"https://yfamilys.com/module/adultraplus.sgmodule", "adultraplus.sgmodule"},
		{"https://yfamilys.com/module/bili.module", "bili.module"},
		{"https://yfamilys.com/rule/Kemono.list", "yfamilys_Kemono.list"},
		{"https://yfamilys.com/rule/Cloudflare.list", "yfamilys_Cloudflare.list"},
	}

	failed := 0
	for _, pair := range pairs {
		dest := filepath.Join(vendorDir, pair.name)
		content := hub.SafeDownload(pair.url, 2, 90)
		if content == "" {
			fmt.Printf("FAIL %s (%s)\n", pair.name, pair.url)
			failed++
		} else {
			hub.SafeWriteFile(dest, content, true)
			info, err := os.Stat(dest)
			size := int64(0)
			if err == nil {
				size = info.Size()
			}
			fmt.Printf("OK   %s (%d bytes)\n", pair.name, size)
		}
		time.Sleep(500 * time.Millisecond) // Add sleep to prevent rate limiting
	}

	if failed > 0 {
		return 1
	}
	return 0
}
