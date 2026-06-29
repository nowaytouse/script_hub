package hub

/*
#cgo LDFLAGS: -L../../rust_processor/target/release -lrust_processor
#include <stdbool.h>
#include <stdlib.h>
extern bool safe_write_file_ffi(const char* path, const char* content, bool atomic);
*/
import "C"
import "unsafe"

import (
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// PATH VALIDATION

func ValidateFileExists(path string, description string) bool {
	if path == "" {
		Error(fmt.Sprintf("%s path is empty", description))
		return false
	}
	info, err := os.Stat(path)
	if err != nil {
		if os.IsNotExist(err) {
			if description != "" { Error(fmt.Sprintf("%s not found: %s", description, path)) }
		} else {
			if description != "" { Error(fmt.Sprintf("%s stat error: %s", description, err)) }
		}
		return false
	}
	if info.IsDir() {
		if description != "" {
			Error(fmt.Sprintf("%s is not a file: %s", description, path))
		}
		return false
	}
	// In Go, basic existence check is usually enough unless permissions are strict.
	return true
}

func ValidateDirExists(path string, description string) bool {
	if path == "" {
		Error(fmt.Sprintf("%s path is empty", description))
		return false
	}
	info, err := os.Stat(path)
	if err != nil {
		if os.IsNotExist(err) {
			if description != "" { Error(fmt.Sprintf("%s not found: %s", description, path)) }
		} else {
			if description != "" { Error(fmt.Sprintf("%s stat error: %s", description, err)) }
		}
		return false
	}
	if !info.IsDir() {
		if description != "" {
			Error(fmt.Sprintf("%s is not a directory: %s", description, path))
		}
		return false
	}
	return true
}

func EnsureParentDir(filePath string) bool {
	parent := filepath.Dir(filePath)
	if parent != "" && parent != "." {
		if _, err := os.Stat(parent); os.IsNotExist(err) {
			err = os.MkdirAll(parent, 0755)
			if err != nil {
				Error(fmt.Sprintf("Failed to create parent directory for %s: %v", filePath, err))
				return false
			}
			Info(fmt.Sprintf("Created directory: %s", parent))
		}
	}
	return true
}

func ValidateOutputWritable(path string, description string) bool {
	if path == "" {
		Error(fmt.Sprintf("%s path is empty", description))
		return false
	}
	if !EnsureParentDir(path) {
		return false
	}
	// In Go, usually if EnsureParentDir succeeds, it's writable.
	return true
}

// RETRY LOGIC

// RetryOnFailure executes the given function with exponential backoff retries.
func RetryOnFailure(maxAttempts int, initialDelay time.Duration, backoff float64, fn func() error) error {
	delay := initialDelay
	var lastErr error
	for attempt := 1; attempt <= maxAttempts; attempt++ {
		err := fn()
		if err == nil {
			return nil
		}
		lastErr = err
		if attempt == maxAttempts {
			Error(fmt.Sprintf("Operation failed after %d attempts: %v", maxAttempts, err))
			break
		}
		Warn(fmt.Sprintf("Attempt %d/%d failed: %v. Retrying in %v...", attempt, maxAttempts, err, delay))
		time.Sleep(delay)
		delay = time.Duration(float64(delay) * backoff)
	}
	return lastErr
}

// SAFE OPERATIONS

func SafeReadFile(path string) string {
	if !ValidateFileExists(path, "File") {
		return ""
	}
	data, err := os.ReadFile(path)
	if err != nil {
		Error(fmt.Sprintf("Failed to read %s: %v", path, err))
		return ""
	}
	return string(data)
}

func SafeWriteFile(path string, content string, atomic bool) error {
	if !ValidateOutputWritable(path, "Output file") {
		return fmt.Errorf("not writable: %s", path)
	}

	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))
	cContent := C.CString(content)
	defer C.free(unsafe.Pointer(cContent))
	
	success := C.safe_write_file_ffi(cPath, cContent, C.bool(atomic))
	if !success {
		err := fmt.Errorf("failed to write file via Rust FFI: %s", path)
		Error(err.Error())
		return err
	}
	return nil
}

func SafeRemoveFile(path string, missingOk bool) bool {
	info, err := os.Stat(path)
	if err != nil {
		if os.IsNotExist(err) {
			if missingOk {
				return true
			}
			Warn(fmt.Sprintf("File does not exist: %s", path))
			return false
		}
		Error(fmt.Sprintf("Stat error: %v", err))
		return false
	}
	if info.IsDir() {
		Error(fmt.Sprintf("Not a file: %s", path))
		return false
	}
	err = os.Remove(path)
	if err != nil {
		Error(fmt.Sprintf("Failed to remove %s: %v", path, err))
		return false
	}
	return true
}

// INPUT VALIDATION

func ValidateNotEmpty(value string, name string) bool {
	if strings.TrimSpace(value) == "" {
		Error(fmt.Sprintf("%s is empty", name))
		return false
	}
	return true
}

func ValidateURL(urlStr string) bool {
	if urlStr == "" {
		return false
	}
	lower := strings.ToLower(urlStr)
	if !strings.HasPrefix(lower, "http://") && !strings.HasPrefix(lower, "https://") {
		Error(fmt.Sprintf("Invalid URL scheme: %s", urlStr))
		return false
	}
	if strings.Contains(urlStr, " ") || strings.Contains(urlStr, "\n") || strings.Contains(urlStr, "\t") {
		Error(fmt.Sprintf("URL contains whitespace: %s", urlStr))
		return false
	}
	return true
}

// HELPER FUNCTIONS

func CheckNetworkConnectivity(testURL string, timeoutSeconds int) bool {
	if testURL == "" {
		testURL = "https://www.google.com"
	}
	client := &http.Client{
		Timeout: time.Duration(timeoutSeconds) * time.Second,
	}
	_, err := client.Get(testURL)
	if err != nil {
		Warn("Network connectivity check failed")
		return false
	}
	return true
}
