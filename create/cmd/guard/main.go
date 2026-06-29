package main

import (
	"os"

	"github.com/nyamiiko/script_hub/go_scripts/qa"
)

func main() {
	os.Exit(qa.RunGuardGeneratedTree())
}
