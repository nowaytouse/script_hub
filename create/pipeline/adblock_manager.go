package pipeline

/*
#cgo LDFLAGS: -L../../target/release -lrust_processor
#include <stdbool.h>
#include <stdlib.h>

extern bool run_adblock_manager_ffi(const char* root_dir, bool execute);
*/
import "C"

import (
	"unsafe"

	"github.com/nowaytouse/script_hub/create/hub"
)

type AdBlockManager struct{}

func NewAdBlockManager() *AdBlockManager {
	return &AdBlockManager{}
}

// Merge delegates the complete PROMAX pipeline, including network access, to Rust.
// The result is a publishing gate: callers must not commit generated artifacts
// when Rust rejects a source, validation, or publication step.
func (m *AdBlockManager) Merge(execute bool) bool {
	hub.Section("AdBlock Module Consolidation (Canonical Rebuild)")

	cRoot := C.CString(hub.ROOT)
	defer C.free(unsafe.Pointer(cRoot))

	hub.Info("Delegating the complete PROMAX pipeline to Rust...")
	success := bool(C.run_adblock_manager_ffi(cRoot, C.bool(execute)))
	if success {
		hub.Success("AdBlock Manager completed successfully via Rust FFI")
	} else {
		hub.Error("AdBlock Manager FFI execution failed")
	}
	return success
}
