package pipeline

/*
#cgo LDFLAGS: -L../processor/target/release -lrust_processor
#include <stdlib.h>

extern int run_url_rewrites_ffi(const char* directory);
extern void copy_github_variants_ffi(const char* root_dir);
*/
import "C"

import (
	"fmt"
	"unsafe"
	"github.com/nyamiiko/script_hub/go_scripts/hub"
)

func CopyGithubVariants() {
	hub.Info("Starting CopyGithubVariants via Rust FFI...")
	cRoot := C.CString(hub.ROOT)
	defer C.free(unsafe.Pointer(cRoot))
	C.copy_github_variants_ffi(cRoot)
	hub.Success("Copied raw variants via Rust FFI")
}

func RunUrlRewrites(directory string) int {
	cDir := C.CString(directory)
	defer C.free(unsafe.Pointer(cDir))
	count := int(C.run_url_rewrites_ffi(cDir))
	return count
}

func RunAllUrlRewrites() int {
	CopyGithubVariants()
	count := 0
	count += RunUrlRewrites(hub.MODULES_DIR)
	count += RunUrlRewrites(hub.RULESETS_DIR)
	count += RunUrlRewrites(hub.ROOT)
	hub.Info(fmt.Sprintf("URL rewrite completed: %d files modified.", count))
	return count
}
