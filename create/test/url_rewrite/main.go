package main
import (
	"fmt"
	"github.com/nyamiiko/script_hub/create/pipeline"
)
func main() {
	count := pipeline.RunUrlRewrites("/Users/nyamiiko/Downloads/GitHub/script_hub/modules/source/local")
	fmt.Printf("Modified files: %d\n", count)
}
