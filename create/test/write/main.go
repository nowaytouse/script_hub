package main
import (
	"fmt"
	"github.com/nyamiiko/script_hub/create/hub"
)
func main() {
	err := hub.SafeWriteFile("/Users/nyamiiko/Downloads/GitHub/script_hub/test_output.txt", "hello world", true)
	fmt.Printf("Error: %v\n", err)
}
