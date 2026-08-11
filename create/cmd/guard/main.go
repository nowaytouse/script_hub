package main

import (
	"os"

	"github.com/nowaytouse/script_hub/create/internal/generatedguard"
)

func main() {
	os.Exit(generatedguard.Run())
}
