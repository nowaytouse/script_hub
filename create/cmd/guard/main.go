package main

import (
	"os"

	"github.com/nyamiiko/script_hub/create/qa"
)

func main() {
	os.Exit(qa.RunGuardGeneratedTree())
}
