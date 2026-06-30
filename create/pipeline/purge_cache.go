package pipeline

import (
	"github.com/nowaytouse/script_hub/create/hub"
	"github.com/nowaytouse/script_hub/create/network"
)

// RunPurgeCache purges recently modified CDN-hosted files from jsDelivr.
// Network logic lives in network/purge_cdn.go.
func RunPurgeCache() {
	hub.Section("Deep CDN Cache Cleared")
	network.RunPurgeCache(hub.ROOT)
}
