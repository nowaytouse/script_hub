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
	"strings"
	"time"

	"github.com/nyamiiko/script_hub/go_scripts/pkg/hub"
)

var (
	singboxSystemPath = "/usr/local/bin/sing-box"
	singboxLocalPath  = filepath.Join(hub.ROOT, "scripts/config-manager-auto-update/bin/sing-box")
	mihomoSystemPath  = "/usr/local/bin/mihomo"
	mihomoLocalPath   = filepath.Join(hub.ROOT, "scripts/config-manager-auto-update/bin/mihomo")
)

func logInfo(msg string)    { fmt.Printf("[INFO] %s\n", msg) }
func logSuccess(msg string) { fmt.Printf("[OK] %s\n", msg) }
func logWarning(msg string) { fmt.Printf("[WARN] %s\n", msg) }
func logError(msg string)   { fmt.Printf("[ERROR] %s\n", msg) }

// downloadFile cleanly downloads a file using Go's native http package
func downloadFile(url, filepath string) error {
	client := &http.Client{Timeout: 60 * time.Second} // Stable > Speed
	resp, err := client.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("bad status: %s", resp.Status)
	}

	out, err := os.Create(filepath)
	if err != nil {
		return err
	}
	defer out.Close()

	_, err = io.Copy(out, resp.Body)
	return err
}

func getLatestVersion(repo string, includePrerelease bool) string {
	url := fmt.Sprintf("https://github.com/%s/releases", repo)
	if !includePrerelease {
		url += "/latest"
	}

	client := &http.Client{Timeout: 10 * time.Second}
	resp, err := client.Get(url)
	if err != nil {
		return ""
	}
	defer resp.Body.Close()

	bodyBytes, err := io.ReadAll(resp.Body)
	if err != nil {
		return ""
	}
	body := string(bodyBytes)

	pattern := fmt.Sprintf(`href="/%s/releases/tag/v([0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?)["']`, repo)
	if !includePrerelease {
		pattern = fmt.Sprintf(`href="/%s/releases/tag/v([0-9]+\.[0-9]+\.[0-9]+)["']`, repo)
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

func verifyInstallation(systemPath, localPath string, versionArgs []string) {
	fmt.Println("")
	logInfo("Verification:")

	if hub.ValidateFileExists(systemPath, "") {
		out, _ := exec.Command(systemPath, versionArgs...).Output()
		lines := strings.Split(string(out), "\n")
		if len(lines) > 0 && lines[0] != "" {
			fmt.Println(lines[0])
		}
	}

	if hub.ValidateFileExists(localPath, "") {
		out, _ := exec.Command(localPath, versionArgs...).Output()
		lines := strings.Split(string(out), "\n")
		if len(lines) > 0 && lines[0] != "" {
			fmt.Println(lines[0])
		}
	}
}

type CoreConfig struct {
	Name           string
	Repo           string
	SystemPath     string
	LocalPath      string
	VersionArgs    []string
	VersionRegex   *regexp.Regexp
	GetDownloadURL func(version, osType, arch string) string
	ExtractFunc    func(archivePath, tempDir string) (string, error)
}

func extractSingbox(archivePath, tempDir string) (string, error) {
	cmd := exec.Command("tar", "-xzf", archivePath, "-C", tempDir)
	if err := cmd.Run(); err != nil {
		return "", fmt.Errorf("tar extraction failed: %w", err)
	}

	var binaryPath string
	filepath.Walk(tempDir, func(path string, info os.FileInfo, err error) error {
		if !info.IsDir() && info.Name() == "sing-box" {
			binaryPath = path
		}
		return nil
	})

	if binaryPath == "" {
		return "", fmt.Errorf("binary not found in archive")
	}
	return binaryPath, nil
}

func extractMihomo(archivePath, tempDir string) (string, error) {
	extractedPath := filepath.Join(tempDir, "mihomo")
	inFile, err := os.Open(archivePath)
	if err != nil {
		return "", fmt.Errorf("open archive failed: %w", err)
	}
	defer inFile.Close()

	gzReader, err := gzip.NewReader(inFile)
	if err != nil {
		return "", fmt.Errorf("gzip reader failed: %w", err)
	}
	defer gzReader.Close()

	outFile, err := os.Create(extractedPath)
	if err != nil {
		return "", fmt.Errorf("create extracted file failed: %w", err)
	}
	defer outFile.Close()

	if _, err := io.Copy(outFile, gzReader); err != nil {
		return "", fmt.Errorf("write extracted file failed: %w", err)
	}

	if !hub.ValidateFileExists(extractedPath, "") {
		return "", fmt.Errorf("binary not found after extraction")
	}
	return extractedPath, nil
}

func updateCore(c CoreConfig, includePrerelease bool) error {
	logInfo(fmt.Sprintf("Checking %s updates...", c.Name))
	logInfo("Fetching latest version from GitHub...")

	latestVersion := getLatestVersion(c.Repo, includePrerelease)
	if latestVersion == "" {
		return fmt.Errorf("cannot get latest version for %s", c.Name)
	}

	systemVersion := "0.0.0"
	if hub.ValidateFileExists(c.SystemPath, "") {
		out, _ := exec.Command(c.SystemPath, c.VersionArgs...).Output()
		match := c.VersionRegex.FindString(string(out))
		if match != "" {
			systemVersion = match
		}
	}

	localVersion := "0.0.0"
	if hub.ValidateFileExists(c.LocalPath, "") {
		out, _ := exec.Command(c.LocalPath, c.VersionArgs...).Output()
		match := c.VersionRegex.FindString(string(out))
		if match != "" {
			localVersion = match
		}
	}

	logInfo(fmt.Sprintf("System version: v%s", systemVersion))
	logInfo(fmt.Sprintf("Local version: v%s", localVersion))
	logInfo(fmt.Sprintf("Latest version: v%s", latestVersion))

	if systemVersion == latestVersion && localVersion == latestVersion {
		logSuccess(fmt.Sprintf("%s is already up to date", c.Name))
		return nil
	}

	osType, archType := getArchOs()
	downloadURL := c.GetDownloadURL(latestVersion, osType, archType)

	logInfo(fmt.Sprintf("Downloading %s v%s...", c.Name, latestVersion))

	tempDir, err := os.MkdirTemp("", fmt.Sprintf("%s-update", strings.ToLower(c.Name)))
	if err != nil {
		return fmt.Errorf("failed to create temp directory: %w", err)
	}
	defer os.RemoveAll(tempDir)

	archivePath := filepath.Join(tempDir, "archive")
	if err := downloadFile(downloadURL, archivePath); err != nil {
		return fmt.Errorf("download failed: %w", err)
	}
	logSuccess("Download complete")

	binaryPath, err := c.ExtractFunc(archivePath, tempDir)
	if err != nil {
		return fmt.Errorf("extraction failed: %w", err)
	}

	// Install to system path
	cmd := exec.Command("sudo", "cp", binaryPath, c.SystemPath)
	if err := cmd.Run(); err == nil {
		exec.Command("sudo", "chmod", "+x", c.SystemPath).Run()
		logSuccess(fmt.Sprintf("System %s v%s installed", c.Name, latestVersion))
	} else {
		logWarning("Cannot install to system path (no sudo or permission denied)")
	}

	// Install to local path
	hub.EnsureDir(filepath.Dir(c.LocalPath))
	hub.SafeWriteFile(c.LocalPath, hub.ReadFileString(binaryPath), false)
	os.Chmod(c.LocalPath, 0755)
	logSuccess(fmt.Sprintf("Local %s v%s installed", c.Name, latestVersion))

	verifyInstallation(c.SystemPath, c.LocalPath, c.VersionArgs)
	return nil
}

type UpdateOptions struct {
	SingboxOnly       bool
	MihomoOnly        bool
	IncludePrerelease bool
}

// RunUpdateCores executes the core update logic native in Go
func RunUpdateCores(opts ...UpdateOptions) int {
	var opt UpdateOptions
	if len(opts) > 0 {
		opt = opts[0]
	} else {
		opt.IncludePrerelease = true
	}

	fmt.Println("==============================================================")
	fmt.Println("       Core Update Tool (Native Go Version)")
	fmt.Println("==============================================================")

	singboxConfig := CoreConfig{
		Name:         "Sing-box",
		Repo:         "SagerNet/sing-box",
		SystemPath:   singboxSystemPath,
		LocalPath:    singboxLocalPath,
		VersionArgs:  []string{"version"},
		VersionRegex: regexp.MustCompile(`[0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?`),
		GetDownloadURL: func(version, osType, arch string) string {
			return fmt.Sprintf("https://github.com/SagerNet/sing-box/releases/download/v%s/sing-box-%s-%s-%s.tar.gz", version, version, osType, arch)
		},
		ExtractFunc: extractSingbox,
	}

	mihomoConfig := CoreConfig{
		Name:         "Mihomo",
		Repo:         "MetaCubeX/mihomo",
		SystemPath:   mihomoSystemPath,
		LocalPath:    mihomoLocalPath,
		VersionArgs:  []string{"-v"},
		VersionRegex: regexp.MustCompile(`[0-9]+\.[0-9]+\.[0-9]+`),
		GetDownloadURL: func(version, osType, arch string) string {
			return fmt.Sprintf("https://github.com/MetaCubeX/mihomo/releases/download/v%s/mihomo-%s-%s-v%s.gz", version, osType, arch, version)
		},
		ExtractFunc: extractMihomo,
	}

	if !opt.MihomoOnly {
		if err := updateCore(singboxConfig, opt.IncludePrerelease); err != nil {
			logError(err.Error())
		}
		fmt.Println("")
	}

	if !opt.SingboxOnly {
		// Mihomo always uses stable (false) as per original shell script logic
		if err := updateCore(mihomoConfig, false); err != nil {
			logError(err.Error())
		}
		fmt.Println("")
	}

	logSuccess("Core update complete")
	return 0
}
