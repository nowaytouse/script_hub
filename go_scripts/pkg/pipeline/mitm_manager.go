package pipeline

/*
#cgo LDFLAGS: -L../../rust_processor/target/release -lrust_processor
#include <stdlib.h>
#include <stdbool.h>

extern int run_mitm_cleanup_ffi(const char* directory, bool dry_run);
*/
import "C"

import (
	"unsafe"
)

func RunMitmCleanup(directory string, dryRun bool) int {
	cDir := C.CString(directory)
	defer C.free(unsafe.Pointer(cDir))

	count := int(C.run_mitm_cleanup_ffi(cDir, C.bool(dryRun)))
	return count
}
