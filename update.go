package main

import (
	"bufio"
	"fmt"
	"os"
	"os/exec"
)

func main() {
	fmt.Println("============================================================")
	fmt.Println("🚀 Script Hub - Native Go Launcher (Update Pipeline)")
	fmt.Println("============================================================")

	// Switch to go_scripts to execute the actual pipeline
	err := os.Chdir("go_scripts")
	if err != nil {
		fmt.Printf("❌ Failed to find 'go_scripts' directory. Please ensure you are running this from the project root: %v\n", err)
		pause()
		return
	}

	cmd := exec.Command("go", "run", "./cmd/main_update", "--execute")
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	cmd.Stdin = os.Stdin

	fmt.Println("⌛ Starting core pipeline...")
	err = cmd.Run()
	if err != nil {
		fmt.Printf("\n❌ Pipeline execution failed with exit code: %v\n", err)
	} else {
		fmt.Println("\n✅ Success! All tasks completed and changes pushed to remote repository.")
	}

	pause()
}

func pause() {
	// For unattended mode, don't pause. 
	if os.Getenv("CI") != "" || os.Getenv("GITHUB_ACTIONS") != "" {
		return
	}
	
	fmt.Println("\nPress Enter to exit...")
	bufio.NewReader(os.Stdin).ReadBytes('\n')
}
