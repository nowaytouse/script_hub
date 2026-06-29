package tools

import (
	"archive/tar"
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

	"github.com/nyamiiko/script_hub/go_scripts/hub"
)

var (
	singboxSystemPath = "/usr/local/bin/sing-box"
	singboxLocalPath  = filepath.Join(hub.ROOT, "scripts/config-manager-auto-update/bin/sing-box")
	mihomoSystemPath  = "/usr/local/bin/mihomo"
	mihomoLocalPath   = filepath.Join(hub.ROOT, "scripts/config-manager-auto-update/bin/mihomo")
)

// downloadFile cleanly downloads a file using http package with retry and mirror fallback
func downloadFile(url, filepath string) error {
	mirrorURL := "https://ghproxy.net/" + url

	hub.Info(fmt.Sprintf("Downloading from primary source: %s", url))
	// Try primary URL exactly once with a shorter timeout (15 seconds) to avoid long hangs
	err := downloadOnce(url, filepath, 15*time.Second)
	if err == nil {
		return nil
	}

	hub.Warn(fmt.Sprintf("Primary source failed: %v. Trying mirror source: %s", err, mirrorURL))
	// Try mirror URL with retries and a longer 120-second timeout for slow connections
	return downloadWithRetries(mirrorURL, filepath, 3, 120*time.Second)
}

func downloadOnce(url, filepath string, timeout time.Duration) error {
	client := &http.Client{Timeout: timeout}
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return err
	}
	// Set browser User-Agent to prevent blockages
	req.Header.Set("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
	req.Header.Set("Accept", "application/octet-stream, */*")

	resp, err := client.Do(req)
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

func downloadWithRetries(url, filepath string, maxAttempts int, timeout time.Duration) error {
	var lastErr error
	for attempt := 1; attempt <= maxAttempts; attempt++ {
		err := downloadOnce(url, filepath, timeout)
		if err == nil {
			return nil
		}
		lastErr = err
		if attempt < maxAttempts {
			hub.Info(fmt.Sprintf("Download attempt %d failed: %v. Retrying in 1s...", attempt, err))
			time.Sleep(1 * time.Second)
		}
	}
	return lastErr
}

func getLatestVersion(repo string, includePrerelease bool) string {
	url := fmt.Sprintf("https://github.com/%s/releases", repo)
	if !includePrerelease {
		url += "/latest"
	}

	client := &http.Client{Timeout: 10 * time.Second} // Shorter timeout for HTML scraping
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return ""
	}
	req.Header.Set("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")

	resp, err := client.Do(req)
	if err != nil {
		return ""
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return ""
	}

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
	hub.Info("Verification:")

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

	tarReader := tar.NewReader(gzReader)
	var binaryPath string

	for {
		header, err := tarReader.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return "", fmt.Errorf("tar reader failed: %w", err)
		}

		if !header.FileInfo().IsDir() && (strings.HasSuffix(header.Name, "/sing-box") || header.Name == "sing-box") {
			baseName := filepath.Base(header.Name)
			targetPath := filepath.Join(tempDir, baseName)

			// Zip Slip Protection
			cleanTempDir := filepath.Clean(tempDir)
			cleanTargetPath := filepath.Clean(targetPath)
			if !strings.HasPrefix(cleanTargetPath, cleanTempDir) {
				return "", fmt.Errorf("Zip Slip vulnerability detected: %s", header.Name)
			}

			outFile, err := os.OpenFile(targetPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, header.FileInfo().Mode()|0700)
			if err != nil {
				return "", fmt.Errorf("create file failed: %w", err)
			}

			if _, err := io.Copy(outFile, tarReader); err != nil {
				outFile.Close()
				return "", fmt.Errorf("write extracted file failed: %w", err)
			}
			outFile.Close()
			binaryPath = targetPath
			break
		}
	}

	if binaryPath == "" {
		return "", fmt.Errorf("sing-box binary not found in archive")
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

	outFile, err := os.OpenFile(extractedPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0755)
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

func copyFile(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()

	out, err := os.OpenFile(dst, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, 0755)
	if err != nil {
		return err
	}
	defer out.Close()

	_, err = io.Copy(out, in)
	return err
}

func updateCore(c CoreConfig, includePrerelease bool) error {
	hub.Info(fmt.Sprintf("Checking %s updates...", c.Name))
	hub.Info("Fetching latest version from GitHub...")

	latestVersion := getLatestVersion(c.Repo, includePrerelease)
	if latestVersion == "" {
		if strings.Contains(c.Repo, "sing-box") {
			latestVersion = "1.14.0-alpha.11"
			hub.Warn("Failed to fetch latest sing-box version, using fallback: v1.14.0-alpha.11")
		} else if strings.Contains(c.Repo, "mihomo") {
			latestVersion = "1.18.8"
			hub.Warn("Failed to fetch latest mihomo version, using fallback: v1.18.8")
		} else {
			return fmt.Errorf("cannot get latest version for %s", c.Name)
		}
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

	hub.Info(fmt.Sprintf("System version: v%s", systemVersion))
	hub.Info(fmt.Sprintf("Local version: v%s", localVersion))
	hub.Info(fmt.Sprintf("Latest version: v%s", latestVersion))

	if systemVersion == latestVersion && localVersion == latestVersion {
		hub.Success(fmt.Sprintf("%s is already up to date", c.Name))
		return nil
	}

	osType, archType := getArchOs()
	downloadURL := c.GetDownloadURL(latestVersion, osType, archType)

	hub.Info(fmt.Sprintf("Downloading %s v%s...", c.Name, latestVersion))

	tempDir, err := os.MkdirTemp("", fmt.Sprintf("%s-update", strings.ToLower(c.Name)))
	if err != nil {
		return fmt.Errorf("failed to create temp directory: %w", err)
	}
	defer os.RemoveAll(tempDir)

	archivePath := filepath.Join(tempDir, "archive")
	if err := downloadFile(downloadURL, archivePath); err != nil {
		return fmt.Errorf("download failed: %w", err)
	}
	hub.Success("Download complete")

	binaryPath, err := c.ExtractFunc(archivePath, tempDir)
	if err != nil {
		return fmt.Errorf("extraction failed: %w", err)
	}

	// Install to system path
	cmd := exec.Command("sudo", "-n", "cp", binaryPath, c.SystemPath)
	if err := cmd.Run(); err == nil {
		exec.Command("sudo", "-n", "chmod", "+x", c.SystemPath).Run()
		hub.Success(fmt.Sprintf("System %s v%s installed", c.Name, latestVersion))
	} else {
		hub.Warn("Cannot install to system path (no sudo or permission denied)")
	}

	// Install to local path
	hub.EnsureDir(filepath.Dir(c.LocalPath))
	if err := copyFile(binaryPath, c.LocalPath); err != nil {
		return fmt.Errorf("failed to install local path: %w", err)
	}
	os.Chmod(c.LocalPath, 0755)
	hub.Success(fmt.Sprintf("Local %s v%s installed", c.Name, latestVersion))

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

	fmt.Println("       Core Update Tool (Native Go Version)")

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
			hub.Error(err.Error())
		}
		fmt.Println("")
	}

	if !opt.SingboxOnly {
		// Mihomo always uses stable (false) as per original shell script logic
		if err := updateCore(mihomoConfig, false); err != nil {
			hub.Error(err.Error())
		}
		fmt.Println("")
	}

	hub.Success("Core update complete")
	return 0
}
