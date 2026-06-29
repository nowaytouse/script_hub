package pipeline

/*
#cgo LDFLAGS: -L${SRCDIR}/../../rust_processor/target/release -lrust_processor -lm -ldl
#include <stdbool.h>
#include <stdlib.h>

bool process_ruleset_ffi(
    const char* name,
    const char* target_dir,
    const char* conflict_domains,
    bool skip_conflict,
    const char* policy,
    const char* node,
    const char* desc,
    const char* local_paths,
    const char* remote_content
);
*/
import "C"

import (
	"crypto/md5"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"unsafe"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

func (rm *RulesetManager) processRuleset(name string, rp *hub.RuleProcessor) {
	targetFile := filepath.Join(surgeDir, fmt.Sprintf("%s.list", name))
	for _, dep := range deprecatedRulesets {
		if name == dep {
			if hub.ValidateFileExists(targetFile, "") {
				if err := os.Remove(targetFile); err == nil {
					rm.incStat("deleted")
					hub.Warn(fmt.Sprintf("Removed deprecated: %s", name))
				} else {
					hub.Error(fmt.Sprintf("Failed to remove deprecated: %s", name))
				}
			}
			return
		}
	}

	hasher := md5.New()
	found := false

	manifests := []string{
		filepath.Join(hub.LINKS_DIR, fmt.Sprintf("%s_sources.list", name)),
		rm.findCaseInsensitiveFile(hub.METACUBEX_DIR, fmt.Sprintf("MetaCubeX_%s.list", name)),
	}

	for _, f := range manifests {
		if f != "" && hub.ValidateFileExists(f, "") {
			content, _ := os.ReadFile(f)
			hasher.Write(content)
			if strings.HasSuffix(f, "_sources.list") {
				for _, line := range strings.Split(string(content), "\n") {
					line = strings.TrimSpace(line)
					if line != "" && !strings.HasPrefix(line, "#") && !strings.HasPrefix(line, "http") {
						localRef := filepath.Join(hub.LINKS_DIR, strings.Split(line, "|")[0])
						if hub.ValidateFileExists(localRef, "") {
							c, _ := os.ReadFile(localRef)
							hasher.Write(c)
						}
					}
				}
			}
			found = true
		}
	}

	for _, prot := range protectedRulesets {
		if name == prot && hub.ValidateFileExists(targetFile, "") {
			content := hub.ReadFileString(targetFile)
			var validLines []string
			for _, l := range strings.Split(content, "\n") {
				l = strings.TrimSpace(l)
				if l != "" && !strings.HasPrefix(l, "#") {
					validLines = append(validLines, l)
				}
			}
			hasher.Write([]byte(strings.Join(validLines, "")))
			found = true
			break
		}
	}

	currentHash := ""
	if found {
		currentHash = hex.EncodeToString(hasher.Sum(nil))
	}

	hasRemote := false
	sFile := filepath.Join(hub.LINKS_DIR, fmt.Sprintf("%s_sources.list", name))
	if hub.ValidateFileExists(sFile, "") {
		for _, line := range strings.Split(hub.ReadFileString(sFile), "\n") {
			if strings.HasPrefix(strings.TrimSpace(line), "http") {
				hasRemote = true
				break
			}
		}
	}

	if !rm.force && hub.ValidateFileExists(targetFile, "") && rm.getHash(name) == currentHash && !hasRemote {
		rm.incStat("skipped")
		return
	}

	var localPaths []string
	remoteContent := strings.Builder{}
	isProtected := false
	for _, p := range protectedRulesets {
		if name == p {
			isProtected = true
			break
		}
	}

	if isProtected {
		if hub.ValidateFileExists(targetFile, "") {
			localPaths = append(localPaths, targetFile)
		}
	} else {
		mFile := rm.findCaseInsensitiveFile(hub.METACUBEX_DIR, fmt.Sprintf("MetaCubeX_%s.list", name))
		if mFile != "" && hub.ValidateFileExists(mFile, "") {
			localPaths = append(localPaths, mFile)
		}
		if hub.ValidateFileExists(sFile, "") {
			for line := range hub.IterLines(sFile) {
				line = strings.TrimSpace(line)
				if line == "" || strings.HasPrefix(line, "#") {
					continue
				}
				if strings.HasPrefix(line, "http") {
					content := rm.download(line)
					if content != "" {
						remoteContent.WriteString(content)
						remoteContent.WriteByte('\n')
					}
				} else {
					localPath := filepath.Join(hub.LINKS_DIR, strings.Split(line, "|")[0])
					if hub.ValidateFileExists(localPath, "") {
						localPaths = append(localPaths, localPath)
					}
				}
			}
		}
	}

	if len(localPaths) == 0 && remoteContent.Len() == 0 && !isProtected {
		return
	}

	baseName := strings.ReplaceAll(name, "_ip", "")
	var pInfo map[string]string
	if info, ok := rm.policyMap[name]; ok {
		pInfo = info
	} else if info, ok := rm.policyMap[baseName]; ok {
		pInfo = info
	} else {
		pInfo = map[string]string{
			"policy": "PROXY",
			"node":   "Any",
			"desc":   "Auto-merged ruleset",
		}
	}

	skipConflict := false
	for _, sc := range skipConflictCheck {
		if sc == name {
			skipConflict = true
			break
		}
	}

	cName := C.CString(name)
	defer C.free(unsafe.Pointer(cName))
	
	cTargetDir := C.CString(surgeDir)
	defer C.free(unsafe.Pointer(cTargetDir))
	
	cConflictDomains := C.CString(strings.Join(conflictDomains, ","))
	defer C.free(unsafe.Pointer(cConflictDomains))
	
	cPolicy := C.CString(pInfo["policy"])
	defer C.free(unsafe.Pointer(cPolicy))
	
	cNode := C.CString(pInfo["node"])
	defer C.free(unsafe.Pointer(cNode))
	
	cDesc := C.CString(pInfo["desc"])
	defer C.free(unsafe.Pointer(cDesc))
	
	cLocalPaths := C.CString(strings.Join(localPaths, "\n"))
	defer C.free(unsafe.Pointer(cLocalPaths))
	
	cRemoteContent := C.CString(remoteContent.String())
	defer C.free(unsafe.Pointer(cRemoteContent))
	
	success := C.process_ruleset_ffi(
		cName,
		cTargetDir,
		cConflictDomains,
		C.bool(skipConflict),
		cPolicy,
		cNode,
		cDesc,
		cLocalPaths,
		cRemoteContent,
	)

	if !success {
		hub.Error(fmt.Sprintf("rust_processor FFI failed for %s", name))
		return
	}

	rm.setHash(name, currentHash)
	rm.incStat("merged")
	hub.Success(fmt.Sprintf("Processed ruleset in Rust (FFI): %s", name))
}
