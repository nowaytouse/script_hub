package pipeline

/*
#cgo LDFLAGS: -L../processor/target/release -lrust_processor
#include <stdlib.h>

extern char* run_cleanup_ffi(const char* root_dir);
extern void free_string_ffi(char* s);
*/
import "C"

import (
	"encoding/json"
	"unsafe"

	"github.com/nyamiiko/script_hub/create/hub"
)

func RunCleanup() map[string]int {
	cRoot := C.CString(hub.ROOT)
	defer C.free(unsafe.Pointer(cRoot))

	resPtr := C.run_cleanup_ffi(cRoot)
	if resPtr == nil {
		return nil
	}
	defer C.free_string_ffi(resPtr)

	jsonStr := C.GoString(resPtr)
	var stats map[string]int
	if err := json.Unmarshal([]byte(jsonStr), &stats); err != nil {
		hub.Error("Failed to parse cleanup stats JSON from Rust FFI: " + err.Error())
		return nil
	}

	return stats
}
