package main
import (
	"fmt"
	"github.com/nowaytouse/script_hub/create/pipeline"
)
func main() {
	count := pipeline.RunUrlRewrites("/Users/nyamiiko/Downloads/GitHub/script_hub/modules/source/local/adultraplus.sgmodule")
	fmt.Printf("Modified files: %d\n", count)
}
