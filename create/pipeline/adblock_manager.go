package pipeline

/*
#cgo LDFLAGS: -L../processor/target/release -lrust_processor
#include <stdbool.h>
#include <stdlib.h>

extern bool run_adblock_manager_ffi(const char* root_dir, const char* remote_contents_json, bool execute);
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
	"unsafe"

	"github.com/nyamiiko/script_hub/create/hub"
)

type AdBlockManager struct{}

func NewAdBlockManager() *AdBlockManager {
	return &AdBlockManager{}
}

// Merge performs the orchestration: reads lists to find remote URLs, downloads them, and delegates processing to Rust FFI
func (m *AdBlockManager) Merge(execute bool) {
	hub.Section("AdBlock Module Consolidation (Canonical Rebuild)")

	// 1. Gather all remote URLs from AdBlock_sources.list
	remoteUrls := make(map[string]bool)

	sourcesFile := hub.ADBLOCK_SOURCES_FILE
	if hub.ValidateFileExists(sourcesFile, "") {
		for line := range hub.IterLines(sourcesFile) {
			line = strings.TrimSpace(line)
			if line == "" || strings.HasPrefix(line, "#") {
				continue
			}
			parts := strings.Split(line, "|")
			if len(parts) > 0 {
				src := strings.TrimSpace(parts[0])
				if strings.HasPrefix(src, "http://") || strings.HasPrefix(src, "https://") {
					remoteUrls[src] = true
				}
			}
		}
	}

	// 2. Gather all remote URLs from AdBlock_functional_sources.list
	funcSourcesFile := hub.ADBLOCK_FUNCTIONAL_SOURCES_FILE
	if hub.ValidateFileExists(funcSourcesFile, "") {
		for line := range hub.IterLines(funcSourcesFile) {
			line = strings.TrimSpace(line)
			if line == "" || strings.HasPrefix(line, "#") {
				continue
			}
			if strings.HasPrefix(line, "http://") || strings.HasPrefix(line, "https://") {
				remoteUrls[line] = true
			}
		}
	}

	// 3. Download remote contents serially
	hub.Info(fmt.Sprintf("Gathered %d remote URLs to download", len(remoteUrls)))
	remoteContents := make(map[string]string)
	
	idx := 1
	for u := range remoteUrls {
		hub.Info(fmt.Sprintf("Downloading remote source (%d/%d): %s", idx, len(remoteUrls), u))
		content := hub.SafeDownload(u, 2, 60)
		if content != "" {
			remoteContents[u] = content
		} else {
			hub.Warn(fmt.Sprintf("Failed to download remote source: %s", u))
		}
		idx++
		// Serial delay to avoid overloading
		time.Sleep(1 * time.Second)
	}

	// 4. Serialize remote contents to JSON
	jsonBytes, err := json.Marshal(remoteContents)
	if err != nil {
		hub.Error(fmt.Sprintf("Failed to serialize remote contents to JSON: %v", err))
		return
	}

	// 5. Call Rust FFI
	cRoot := C.CString(hub.ROOT)
	defer C.free(unsafe.Pointer(cRoot))
	cJson := C.CString(string(jsonBytes))
	defer C.free(unsafe.Pointer(cJson))

	hub.Info("Delegating AdBlock processing to Rust FFI...")
	success := bool(C.run_adblock_manager_ffi(cRoot, cJson, C.bool(execute)))
	if success {
		hub.Success("AdBlock Manager completed successfully via Rust FFI")
	} else {
		hub.Error("AdBlock Manager FFI execution failed")
	}
}
