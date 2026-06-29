package main
import (
	"fmt"
	"github.com/nyamiiko/script_hub/go_scripts/pkg/pipeline"
)
func main() {
	count := pipeline.RunUrlRewrites("/Users/nyamiiko/Downloads/GitHub/script_hub/modules/source/local")
	fmt.Printf("Modified files: %d\n", count)
}
