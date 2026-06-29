package pipeline

/*
#cgo LDFLAGS: -L../../target/release -lrust_processor
#include <stdbool.h>
#include <stdlib.h>

extern bool run_srs_generator_ffi(const char* root_dir, const char* singbox_path);
*/
import "C"

import (
	"os/exec"
	"strings"
	"unsafe"

	"github.com/nowaytouse/script_hub/create/hub"
)

type SRSGenerator struct {
	singboxPath string
}

func NewSRSGenerator() *SRSGenerator {
	g := &SRSGenerator{}
	g.singboxPath = g.findSingbox()
	hub.EnsureDir(hub.SINGBOX_DIR)
	hub.EnsureDir(hub.CACHE_DIR)
	return g
}

func (g *SRSGenerator) findSingbox() string {
	out, err := exec.Command("which", "sing-box").Output()
	if err == nil && len(strings.TrimSpace(string(out))) > 0 {
		return strings.TrimSpace(string(out))
	}

	if hub.ValidateFileExists(hub.SINGBOX_LOCAL_BIN, "") {
		return hub.SINGBOX_LOCAL_BIN
	}
	if hub.ValidateFileExists(hub.SINGBOX_ROOT_BIN, "") {
		return hub.SINGBOX_ROOT_BIN
	}
	return ""
}

func (g *SRSGenerator) Run() {
	hub.Section("Sing-box SRS Generation (English Interface)")
	if g.singboxPath == "" {
		hub.Error("sing-box binary not found. Skipping SRS generation.")
		return
	}

	cRoot := C.CString(hub.ROOT)
	defer C.free(unsafe.Pointer(cRoot))
	cSingbox := C.CString(g.singboxPath)
	defer C.free(unsafe.Pointer(cSingbox))

	hub.Info("Delegating SRS generation to Rust FFI...")
	success := bool(C.run_srs_generator_ffi(cRoot, cSingbox))
	if success {
		hub.Success("SRS generation completed successfully via Rust FFI")
	} else {
		hub.Error("SRS generation FFI execution failed")
	}
}
