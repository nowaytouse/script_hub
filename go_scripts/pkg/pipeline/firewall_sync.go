package pipeline

/*
#cgo LDFLAGS: -L../../rust_processor/target/release -lrust_processor
#include <stdlib.h>
#include <stdbool.h>

extern void sync_ports_ffi(
    const char* ports_source,
    const char** firewall_modules,
    int firewall_modules_len,
    bool execute,
    const char* version
);
*/
import "C"

import (
	"path/filepath"
	"time"
	"unsafe"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

var firewallModules []string

func init() {
	firewallModules = []string{
		filepath.Join(hub.SURGE_HEAD_EXPANSE_DIR, "🔥 Firewall Port Blocker 🛡️🚫.sgmodule"),
		filepath.Join(hub.SHADOWROCKET_HEAD_EXPANSE_DIR, "🔥 Firewall Port Blocker 🛡️🚫.module"),
	}
}

func SyncPorts(execute bool) {
	portsSource := filepath.Join(hub.SOURCES_DIR, "conf/SurgeConf_DirectPorts.list")

	cPortsSource := C.CString(portsSource)
	defer C.free(unsafe.Pointer(cPortsSource))

	// Convert Go string slice to C array of strings
	cModules := make([]*C.char, len(firewallModules))
	for i, m := range firewallModules {
		cModules[i] = C.CString(m)
		defer C.free(unsafe.Pointer(cModules[i]))
	}

	var cModulesPtr **C.char
	if len(cModules) > 0 {
		cModulesPtr = (**C.char)(unsafe.Pointer(&cModules[0]))
	}

	versionStr := time.Now().Format("2006.01.02")
	cVersion := C.CString(versionStr)
	defer C.free(unsafe.Pointer(cVersion))

	C.sync_ports_ffi(
		cPortsSource,
		cModulesPtr,
		C.int(len(firewallModules)),
		C.bool(execute),
		cVersion,
	)
}
