package pipeline

/*
#cgo LDFLAGS: -L../../target/release -lrust_processor
#include <stdbool.h>
#include <stdlib.h>

extern bool process_merge_bundle_ffi(const char* json);
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"
	"unsafe"

	"github.com/nowaytouse/script_hub/create/hub"
)

type mergeSourcePayload struct {
	Label   string `json:"label"`
	URL     string `json:"url"`
	Content string `json:"content"`
}

type mergeBundleRequest struct {
	Op                   string              `json:"op"`
	OutputPath           string              `json:"output_path"`
	HeaderMeta           map[string]string   `json:"header_meta"`
	ProvenanceComment    string              `json:"provenance_comment,omitempty"`
	Sources              []mergeSourcePayload  `json:"sources"`
	ContentReplacements  [][2]string         `json:"content_replacements,omitempty"`
	ExtraHeaderLines     []string            `json:"extra_header_lines,omitempty"`
	YoutubeAdblockOutput string              `json:"youtube_adblock_output,omitempty"`
	ScriptPathSuffixMap  map[string]string   `json:"script_path_suffix_map,omitempty"`
	ScriptRawPrefix      string              `json:"script_raw_prefix,omitempty"`
	ScriptsDir           string              `json:"scripts_dir,omitempty"`
	ScriptHubLocal       string              `json:"script_hub_local,omitempty"`
}

func callMergeBundleRust(req mergeBundleRequest) error {
	jsonBytes, err := json.Marshal(req)
	if err != nil {
		return fmt.Errorf("merge bundle JSON encode: %w", err)
	}
	cJson := C.CString(string(jsonBytes))
	defer C.free(unsafe.Pointer(cJson))
	if !bool(C.process_merge_bundle_ffi(cJson)) {
		return fmt.Errorf("Rust merge_bundles failed for op %s → %s", req.Op, req.OutputPath)
	}
	return nil
}

// fetchBundleSources serially downloads upstream modules (Go network layer only).
func fetchBundleSources(
	sources []hub.SourceSpec,
	optionalLabels []string,
	localFallbacks map[string]string,
) ([]mergeSourcePayload, error) {
	optional := make(map[string]bool)
	for _, l := range optionalLabels {
		optional[l] = true
	}

	var payloads []mergeSourcePayload
	for _, src := range sources {
		hub.Info(fmt.Sprintf("Downloading %s...", src.Label))
		customUA := ""
		if strings.Contains(src.URL, "yfamilys.com") {
			customUA = "Loon/3.9.9 CFNetwork/1496.0.7 Darwin/23.5.0"
		}
		fallback := ""
		if localFallbacks != nil {
			fallback = localFallbacks[src.Label]
		}
		text, err := hub.FetchModule(src.URL, src.Label, fallback, customUA)
		if err != nil {
			if optional[src.Label] {
				hub.Warn(fmt.Sprintf("Skipping optional source %s: %v", src.Label, err))
				continue
			}
			return nil, err
		}
		payloads = append(payloads, mergeSourcePayload{
			Label:   src.Label,
			URL:     src.URL,
			Content: text,
		})
		time.Sleep(3 * time.Second)
	}
	if len(payloads) == 0 {
		return nil, fmt.Errorf("no upstream modules downloaded")
	}
	return payloads, nil
}

func combinedScriptPathMap() map[string]string {
	out := make(map[string]string)
	for k, v := range subStoreScriptPathMap {
		out[k] = v
	}
	for k, v := range scriptHubScriptPathMap {
		out[k] = v
	}
	return out
}
