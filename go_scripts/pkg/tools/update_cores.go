package tools

import (
	"compress/gzip"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

var (
	singboxSystemPath = "/usr/local/bin/sing-box"
	singboxLocalPath  = filepath.Join(hub.ROOT, "scripts/config-manager-auto-update/bin/sing-box")
	mihomoSystemPath  = "/usr/local/bin/mihomo"
	mihomoLocalPath   = filepath.Join(hub.ROOT, "scripts/config-manager-auto-update/bin/mihomo")
)

func logInfo(msg string) { fmt.Printf("[INFO] %s\n", msg) }
func logSuccess(msg string) { fmt.Printf("[OK] %s\n", msg) }
func logWarning(msg string) { fmt.Printf("[WARN] %s\n", msg) }
func logError(msg string) { fmt.Printf("[ERROR] %s\n", msg) }

func getLatestVersion(repo string, includePrerelease bool) string {
	url := fmt.Sprintf("https://github.com/%s/releases", repo)
	if !includePrerelease {
		url += "/latest"
	}
	resp, err := http.Get(url)
	if err != nil {
		return ""
	}
	defer resp.Body.Close()

	bodyBytes, err := io.ReadAll(resp.Body)
	if err != nil {
		return ""
	}
	body := string(bodyBytes)

	pattern := fmt.Sprintf(`href="/%s/releases/tag/v([0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?)`, repo)
	if !includePrerelease {
		pattern = fmt.Sprintf(`href="/%s/releases/tag/v([0-9]+\.[0-9]+\.[0-9]+)`, repo)
	}

	re := regexp.MustCompile(pattern)
	matches := re.FindStringSubmatch(body)
	if len(matches) > 1 {
		return matches[1]
	}
	return ""
}

func getArchOs() (string, string) {
	osType := runtime.GOOS
	archType := runtime.GOARCH

	if archType == "amd64" {
		archType = "amd64"
	} else if archType == "arm64" {
		archType = "arm64"
	} else if archType == "arm" {
		archType = "armv7"
	}

	return osType, archType
}

func updateSingbox() {
	logInfo("Checking Sing-box updates...")
	logInfo("Fetching latest version from GitHub...")

	latestVersion := getLatestVersion("SagerNet/sing-box", true)
	if latestVersion == "" {
		logError("Cannot get latest version")
		return
	}

	systemVersion := "0.0.0"
	if hub.ValidateFileExists(singboxSystemPath, "") {
		out, _ := exec.Command(singboxSystemPath, "version").Output()
		re := regexp.MustCompile(`[0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?`)
		match := re.FindString(string(out))
		if match != "" {
			systemVersion = match
		}
	}

	localVersion := "0.0.0"
	if hub.ValidateFileExists(singboxLocalPath, "") {
		out, _ := exec.Command(singboxLocalPath, "version").Output()
		re := regexp.MustCompile(`[0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?`)
		match := re.FindString(string(out))
		if match != "" {
			localVersion = match
		}
	}

	logInfo(fmt.Sprintf("System version: v%s", systemVersion))
	logInfo(fmt.Sprintf("Local version: v%s", localVersion))
	logInfo(fmt.Sprintf("Latest version: v%s", latestVersion))

	if systemVersion == latestVersion && localVersion == latestVersion {
		logSuccess("Sing-box is already up to date")
		return
	}

	osType, archType := getArchOs()
	downloadURL := fmt.Sprintf("https://github.com/SagerNet/sing-box/releases/download/v%s/sing-box-%s-%s-%s.tar.gz", latestVersion, latestVersion, osType, archType)

	logInfo(fmt.Sprintf("Downloading Sing-box v%s...", latestVersion))
	
	tempDir, err := os.MkdirTemp("", "singbox-update")
	if err != nil {
		logError("Failed to create temp directory")
		return
	}
	defer os.RemoveAll(tempDir)

	archivePath := filepath.Join(tempDir, "sing-box.tar.gz")
	cmd := exec.Command("curl", "-L", "-o", archivePath, downloadURL)
	if err := cmd.Run(); err != nil {
		logError("Download failed")
		return
	}
	logSuccess("Download complete")

	// Extract
	cmd = exec.Command("tar", "-xzf", archivePath, "-C", tempDir)
	if err := cmd.Run(); err != nil {
		logError("Extraction failed")
		return
	}

	var binaryPath string
	filepath.Walk(tempDir, func(path string, info os.FileInfo, err error) error {
		if !info.IsDir() && info.Name() == "sing-box" {
			binaryPath = path
		}
		return nil
	})

	if binaryPath == "" {
		logError("Binary not found in archive")
		return
	}

	// Install to system path
	cmd = exec.Command("sudo", "cp", binaryPath, singboxSystemPath)
	if err := cmd.Run(); err == nil {
		exec.Command("sudo", "chmod", "+x", singboxSystemPath).Run()
		logSuccess(fmt.Sprintf("System sing-box v%s installed", latestVersion))
	} else {
		logWarning("Cannot install to system path (no sudo or permission denied)")
	}

	// Install to local path
	hub.EnsureDir(filepath.Dir(singboxLocalPath))
	hub.SafeWriteFile(singboxLocalPath, hub.ReadFileString(binaryPath), false)
	os.Chmod(singboxLocalPath, 0755)
	logSuccess(fmt.Sprintf("Local sing-box v%s installed", latestVersion))
}

func updateMihomo() {
	logInfo("Checking Mihomo updates...")
	logInfo("Fetching latest version from GitHub...")

	latestVersion := getLatestVersion("MetaCubeX/mihomo", false)
	if latestVersion == "" {
		logError("Cannot get latest version")
		return
	}

	systemVersion := "0.0.0"
	if hub.ValidateFileExists(mihomoSystemPath, "") {
		out, _ := exec.Command(mihomoSystemPath, "-v").Output()
		re := regexp.MustCompile(`[0-9]+\.[0-9]+\.[0-9]+`)
		match := re.FindString(string(out))
		if match != "" {
			systemVersion = match
		}
	}

	localVersion := "0.0.0"
	if hub.ValidateFileExists(mihomoLocalPath, "") {
		out, _ := exec.Command(mihomoLocalPath, "-v").Output()
		re := regexp.MustCompile(`[0-9]+\.[0-9]+\.[0-9]+`)
		match := re.FindString(string(out))
		if match != "" {
			localVersion = match
		}
	}

	logInfo(fmt.Sprintf("System version: v%s", systemVersion))
	logInfo(fmt.Sprintf("Local version: v%s", localVersion))
	logInfo(fmt.Sprintf("Latest version: v%s", latestVersion))

	if systemVersion == latestVersion && localVersion == latestVersion {
		logSuccess("Mihomo is already up to date")
		return
	}

	osType, archType := getArchOs()
	downloadURL := fmt.Sprintf("https://github.com/MetaCubeX/mihomo/releases/download/v%s/mihomo-%s-%s-v%s.gz", latestVersion, osType, archType, latestVersion)

	logInfo(fmt.Sprintf("Downloading Mihomo v%s...", latestVersion))

	tempDir, err := os.MkdirTemp("", "mihomo-update")
	if err != nil {
		logError("Failed to create temp directory")
		return
	}
	defer os.RemoveAll(tempDir)

	archivePath := filepath.Join(tempDir, "mihomo.gz")
	cmd := exec.Command("curl", "-L", "-o", archivePath, downloadURL)
	if err := cmd.Run(); err != nil {
		logError("Download failed")
		return
	}
	logSuccess("Download complete")

	// Extract gz
	extractedPath := filepath.Join(tempDir, "mihomo")
	inFile, err := os.Open(archivePath)
	if err == nil {
		defer inFile.Close()
		gzReader, err := gzip.NewReader(inFile)
		if err == nil {
			defer gzReader.Close()
			outFile, err := os.Create(extractedPath)
			if err == nil {
				defer outFile.Close()
				io.Copy(outFile, gzReader)
			}
		}
	}

	if !hub.ValidateFileExists(extractedPath, "") {
		logError("Binary not found after extraction")
		return
	}

	// Install to system path
	cmd = exec.Command("sudo", "cp", extractedPath, mihomoSystemPath)
	if err := cmd.Run(); err == nil {
		exec.Command("sudo", "chmod", "+x", mihomoSystemPath).Run()
		logSuccess(fmt.Sprintf("System mihomo v%s installed", latestVersion))
	} else {
		logWarning("Cannot install to system path (no sudo or permission denied)")
	}

	// Install to local path
	hub.EnsureDir(filepath.Dir(mihomoLocalPath))
	hub.SafeWriteFile(mihomoLocalPath, hub.ReadFileString(extractedPath), false)
	os.Chmod(mihomoLocalPath, 0755)
	logSuccess(fmt.Sprintf("Local mihomo v%s installed", latestVersion))
}

// RunUpdateCores executes the core update logic native in Go
func RunUpdateCores() int {
	fmt.Println("==============================================================")
	fmt.Println("       Core Update Tool (Native Go Version)")
	fmt.Println("==============================================================")
	
	updateSingbox()
	fmt.Println("")
	updateMihomo()
	
	logSuccess("Core update complete")
	return 0
}
