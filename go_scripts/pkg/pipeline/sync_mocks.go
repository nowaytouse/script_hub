package pipeline

import (
	"encoding/base64"
	"fmt"
	"os"
	"path/filepath"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

func SyncMocks() int {
	mocksDir := filepath.Join(hub.ROOT, "modules/source/mocks")
	hub.EnsureDir(mocksDir)

	b64Gif := "R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7"
	gifBytes, _ := base64.StdEncoding.DecodeString(b64Gif)

	mocksData := map[string][]byte{
		"reject-200.txt":     []byte("  "),
		"blank.txt":          []byte("#"),
		"reject-dict.json":   []byte("{}"),
		"blank_dict.json":    []byte("{}"),
		"blank_dict.json.js": []byte("let body = JSON.stringify({});\n$done({response: {status: 200, body: body}});"),
		"reject-img.gif":     gifBytes,
		"blank.gif":          gifBytes,
	}

	count := 0
	for filename, content := range mocksData {
		path := filepath.Join(mocksDir, filename)
		if err := os.WriteFile(path, content, 0644); err == nil {
			hub.Success(fmt.Sprintf("Generated mock locally: %s", filename))
			count++
		} else {
			hub.Error(fmt.Sprintf("Failed to generate mock: %s (%v)", filename, err))
		}
	}
	return count
}
