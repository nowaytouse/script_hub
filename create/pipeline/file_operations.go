package pipeline

/*
#cgo LDFLAGS: -L../../target/release -lrust_processor
#include <stdlib.h>
#include <stdbool.h>

extern bool safe_write_file_ffi(const char* path, const char* content, bool atomic);
extern char* read_file_to_string_ffi(const char* path);
extern bool file_exists_ffi(const char* path);
extern bool ensure_dir_ffi(const char* path);
extern bool remove_file_ffi(const char* path, bool missing_ok);
extern bool remove_dir_all_ffi(const char* path, bool missing_ok);
extern void free_string_ffi(char* s);
*/
import "C"
import (
	"os"
	"unsafe"
)

// SafeWriteFile writes content to file with optional atomic write
func SafeWriteFile(path string, content string, atomic bool) bool {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))
	cContent := C.CString(content)
	defer C.free(unsafe.Pointer(cContent))
	return bool(C.safe_write_file_ffi(cPath, cContent, C.bool(atomic)))
}

// ReadFileToString reads file content as string
func ReadFileToString(path string) string {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))
	cResult := C.read_file_to_string_ffi(cPath)
	if cResult == nil {
		return ""
	}
	defer C.free_string_ffi(cResult)
	return C.GoString(cResult)
}

// FileExists checks if file exists
func FileExists(path string) bool {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))
	return bool(C.file_exists_ffi(cPath))
}

// EnsureDir creates directory if it doesn't exist
func EnsureDir(path string) bool {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))
	return bool(C.ensure_dir_ffi(cPath))
}

// RemoveFile removes file with optional missing_ok
func RemoveFile(path string, missingOk bool) bool {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))
	return bool(C.remove_file_ffi(cPath, C.bool(missingOk)))
}

// RemoveDirAll removes directory recursively with optional missing_ok
func RemoveDirAll(path string, missingOk bool) bool {
	cPath := C.CString(path)
	defer C.free(unsafe.Pointer(cPath))
	return bool(C.remove_dir_all_ffi(cPath, C.bool(missingOk)))
}

// ReadDir reads directory entries (wrapper for os.ReadDir for compatibility)
func ReadDir(path string) ([]os.DirEntry, error) {
	return os.ReadDir(path)
}

// ReadFile reads file content as bytes (wrapper for os.ReadFile for compatibility)
func ReadFile(path string) ([]byte, error) {
	return os.ReadFile(path)
}

// WriteFile writes content to file (wrapper for os.WriteFile for compatibility)
func WriteFile(path string, data []byte, perm os.FileMode) error {
	return os.WriteFile(path, data, perm)
}
