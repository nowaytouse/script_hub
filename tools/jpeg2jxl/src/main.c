/**
 * jpeg2jxl - High-Performance JPEG to JXL Batch Converter
 * 
 * A blazing-fast C implementation for batch converting JPEG images to JXL format.
 * Designed for large-scale batch processing with complete metadata preservation.
 * 
 * Features:
 *   - Multi-threaded parallel processing
 *   - Complete metadata preservation (EXIF, XMP, IPTC via exiftool)
 *   - System timestamp preservation
 *   - Health check validation
 *   - Progress bar with ETA
 *   - Safety checks for dangerous directories
 *   - In-place conversion mode
 * 
 * Dependencies:
 *   - cjxl (libjxl) - JXL encoding
 *   - djxl (libjxl) - JXL decoding (for health check)
 *   - exiftool - Metadata migration
 * 
 * Build:
 *   make
 * 
 * Usage:
 *   jpeg2jxl [options] <directory>
 *   jpeg2jxl --in-place /path/to/images
 *   jpeg2jxl -j 8 /path/to/images  # Use 8 threads
 * 
 * Author: Script Hub Project
 * License: MIT
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <stdarg.h>
#include <dirent.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <unistd.h>
#include <pthread.h>
#include <errno.h>
#include <time.h>
#include <fcntl.h>
#include <signal.h>
#include <limits.h>
#include <strings.h>

// Forward declarations
#include "jpeg2jxl.h"

// Global variables
Config g_config;
Stats g_stats;
FileEntry *g_files = NULL;
int g_file_count = 0;
volatile bool g_interrupted = false;

// ANSI color codes
#define COLOR_RED     "\033[0;31m"
#define COLOR_GREEN   "\033[0;32m"
#define COLOR_YELLOW  "\033[1;33m"
#define COLOR_BLUE    "\033[0;34m"
#define COLOR_CYAN    "\033[0;36m"
#define COLOR_RESET   "\033[0m"

// Logging functions
void log_info(const char *fmt, ...) {
    va_list args;
    va_start(args, fmt);
    printf(COLOR_BLUE "ℹ️  [INFO]" COLOR_RESET " ");
    vprintf(fmt, args);
    printf("\n");
    va_end(args);
}

void log_success(const char *fmt, ...) {
    va_list args;
    va_start(args, fmt);
    printf(COLOR_GREEN "✅ [OK]" COLOR_RESET " ");
    vprintf(fmt, args);
    printf("\n");
    va_end(args);
}

void log_warn(const char *fmt, ...) {
    va_list args;
    va_start(args, fmt);
    printf(COLOR_YELLOW "⚠️  [WARN]" COLOR_RESET " ");
    vprintf(fmt, args);
    printf("\n");
    va_end(args);
}

void log_error(const char *fmt, ...) {
    va_list args;
    va_start(args, fmt);
    fprintf(stderr, COLOR_RED "❌ [ERROR]" COLOR_RESET " ");
    vfprintf(stderr, fmt, args);
    fprintf(stderr, "\n");
    va_end(args);
}

// Initialize configuration with defaults
void init_config(Config *config) {
    memset(config, 0, sizeof(Config));
    config->in_place = false;
    config->skip_health_check = false;
    config->recursive = true;
    config->verbose = false;
    config->dry_run = false;
    config->num_threads = DEFAULT_THREADS;
    config->jxl_distance = JXL_DISTANCE_DEFAULT;
    config->jxl_effort = JXL_EFFORT_DEFAULT;
}

// Initialize statistics
void init_stats(Stats *stats) {
    memset(stats, 0, sizeof(Stats));
    stats->start_time = time(NULL);
    pthread_mutex_init(&stats->mutex, NULL);
}

// Check if file is a JPEG
bool is_jpeg_file(const char *path) {
    const char *ext = strrchr(path, '.');
    if (!ext) return false;
    
    // Case-insensitive comparison
    if (strcasecmp(ext, ".jpg") == 0 || strcasecmp(ext, ".jpeg") == 0) {
        return true;
    }
    return false;
}

// Get file size
size_t get_file_size(const char *path) {
    struct stat st;
    if (stat(path, &st) != 0) return 0;
    return (size_t)st.st_size;
}

// Check if file exists
bool file_exists(const char *path) {
    return access(path, F_OK) == 0;
}

// Get output path (replace extension with .jxl)
char *get_output_path(const char *input) {
    static char output[MAX_PATH_LEN];
    strncpy(output, input, MAX_PATH_LEN - 5);
    
    char *ext = strrchr(output, '.');
    if (ext) {
        strcpy(ext, ".jxl");
    } else {
        strcat(output, ".jxl");
    }
    return output;
}

// Check if directory is dangerous
bool is_dangerous_directory(const char *path) {
    char resolved[PATH_MAX];
    if (realpath(path, resolved) == NULL) {
        return true; // If we can't resolve, assume dangerous
    }
    
    for (int i = 0; DANGEROUS_DIRS[i] != NULL; i++) {
        if (strcmp(resolved, DANGEROUS_DIRS[i]) == 0) {
            return true;
        }
    }
    
    // Also check home directory
    const char *home = getenv("HOME");
    if (home && strcmp(resolved, home) == 0) {
        return true;
    }
    
    return false;
}

// Check if required tools are available
bool check_dependencies(void) {
    bool ok = true;
    
    if (system("which cjxl > /dev/null 2>&1") != 0) {
        log_error("cjxl not found. Install: brew install jpeg-xl");
        ok = false;
    }
    
    if (system("which exiftool > /dev/null 2>&1") != 0) {
        log_error("exiftool not found. Install: brew install exiftool");
        ok = false;
    }
    
    if (!g_config.skip_health_check) {
        if (system("which djxl > /dev/null 2>&1") != 0) {
            log_warn("djxl not found, health check will be limited");
        }
    }
    
    return ok;
}

// Collect JPEG files recursively
int collect_files(const char *dir, bool recursive) {
    DIR *d = opendir(dir);
    if (!d) {
        log_error("Cannot open directory: %s", dir);
        return -1;
    }
    
    struct dirent *entry;
    char path[MAX_PATH_LEN];
    
    while ((entry = readdir(d)) != NULL) {
        if (entry->d_name[0] == '.') continue; // Skip hidden files
        
        snprintf(path, sizeof(path), "%s/%s", dir, entry->d_name);
        
        struct stat st;
        if (stat(path, &st) != 0) continue;

        if (S_ISDIR(st.st_mode)) {
            if (recursive) {
                collect_files(path, recursive);
            }
        } else if (S_ISREG(st.st_mode) && is_jpeg_file(path)) {
            if (g_file_count >= MAX_FILES) {
                log_warn("Maximum file limit reached (%d)", MAX_FILES);
                break;
            }
            
            strncpy(g_files[g_file_count].path, path, MAX_PATH_LEN - 1);
            g_files[g_file_count].size = (size_t)st.st_size;
            g_file_count++;
        }
    }
    
    closedir(d);
    return g_file_count;
}

// Convert JPEG to JXL using cjxl
bool convert_jpeg_to_jxl(const char *input, const char *output) {
    char cmd[MAX_PATH_LEN * 3];
    
    // Build cjxl command with quality settings
    // -d: distance (0=lossless, 1=high quality lossy)
    // -e: effort (1-9, higher = slower but better compression)
    // -j: number of threads (limit to avoid system overload)
    snprintf(cmd, sizeof(cmd),
        "cjxl \"%s\" \"%s\" -d %.1f -e %d -j 2 2>/dev/null",
        input, output, g_config.jxl_distance, g_config.jxl_effort);
    
    int ret = system(cmd);
    return (ret == 0);
}

// Migrate metadata using exiftool
bool migrate_metadata(const char *source, const char *dest) {
    char cmd[MAX_PATH_LEN * 3];
    
    // Copy all metadata from source to destination
    // -overwrite_original: don't create backup files
    // -all:all: copy all tags
    snprintf(cmd, sizeof(cmd),
        "exiftool -tagsfromfile \"%s\" -all:all -overwrite_original \"%s\" 2>/dev/null",
        source, dest);
    
    int ret = system(cmd);
    return (ret == 0);
}

// Preserve file timestamps
bool preserve_timestamps(const char *source, const char *dest) {
    struct stat st;
    if (stat(source, &st) != 0) return false;
    
    struct timeval times[2];
    times[0].tv_sec = st.st_atime;
    times[0].tv_usec = 0;
    times[1].tv_sec = st.st_mtime;
    times[1].tv_usec = 0;
    
    return (utimes(dest, times) == 0);
}

// Health check for JXL file
bool health_check_jxl(const char *path) {
    if (g_config.skip_health_check) return true;
    
    // Check file exists and has size
    size_t size = get_file_size(path);
    if (size == 0) return false;
    
    // Check JXL signature
    FILE *f = fopen(path, "rb");
    if (!f) return false;
    
    unsigned char sig[12];
    size_t read = fread(sig, 1, 12, f);
    fclose(f);
    
    if (read < 2) return false;
    
    // JXL codestream signature: 0xFF 0x0A
    // JXL container (ISOBMFF): starts with 0x00 0x00 0x00
    bool valid_sig = (sig[0] == 0xFF && sig[1] == 0x0A) ||
                     (sig[0] == 0x00 && sig[1] == 0x00 && sig[2] == 0x00);
    
    if (!valid_sig) return false;
    
    // Try djxl decode test if available
    char cmd[MAX_PATH_LEN + 64];
    snprintf(cmd, sizeof(cmd), "djxl \"%s\" /dev/null 2>/dev/null", path);
    
    if (system("which djxl > /dev/null 2>&1") == 0) {
        if (system(cmd) != 0) return false;
    }
    
    return true;
}

// Show progress bar
void show_progress(int current, int total, const char *filename) {
    int percent = (current * 100) / total;
    int filled = percent / 2;
    
    // Clear line and print progress
    printf("\r\033[K");
    printf("📊 Progress: [");
    printf(COLOR_GREEN);
    for (int i = 0; i < filled; i++) printf("█");
    printf(COLOR_RESET);
    for (int i = filled; i < 50; i++) printf("░");
    printf("] %d%% ", percent);
    printf("(%d/%d) ", current, total);
    
    // ETA calculation
    if (current > 0) {
        time_t elapsed = time(NULL) - g_stats.start_time;
        int avg_time = (int)(elapsed / current);
        int remaining = (total - current) * avg_time;
        
        if (remaining > 60) {
            printf("| ⏱️  ETA: ~%dm %ds", remaining / 60, remaining % 60);
        } else {
            printf("| ⏱️  ETA: ~%ds", remaining);
        }
    }
    
    // Current file (truncated)
    if (filename) {
        char display[45];
        size_t len = strlen(filename);
        if (len > 40) {
            strncpy(display, filename, 37);
            strcpy(display + 37, "...");
        } else {
            strcpy(display, filename);
        }
        printf("\n   📄 %s", display);
    }
    
    fflush(stdout);
}

// Print summary report
void print_summary(void) {
    time_t elapsed = time(NULL) - g_stats.start_time;
    
    printf("\n\n");
    printf("╔══════════════════════════════════════════════╗\n");
    printf("║   📊 Conversion Complete                     ║\n");
    printf("╚══════════════════════════════════════════════╝\n\n");

    printf("📈 Statistics:\n");
    printf("   Total files:    %d\n", g_stats.total);
    printf("   " COLOR_GREEN "✅ Success:      %d" COLOR_RESET "\n", g_stats.success);
    printf("   " COLOR_RED "❌ Failed:       %d" COLOR_RESET "\n", g_stats.failed);
    printf("   ⏭️  Skipped:      %d\n", g_stats.skipped);
    printf("   ⏱️  Time:         %ldm %lds\n", elapsed / 60, elapsed % 60);
    
    if (g_stats.bytes_input > 0) {
        double input_mb = g_stats.bytes_input / (1024.0 * 1024.0);
        double output_mb = g_stats.bytes_output / (1024.0 * 1024.0);
        double ratio = (1.0 - (double)g_stats.bytes_output / g_stats.bytes_input) * 100;
        printf("   💾 Input:        %.2f MB\n", input_mb);
        printf("   💾 Output:       %.2f MB\n", output_mb);
        printf("   📉 Reduction:    %.1f%%\n", ratio);
    }
    
    if (!g_config.skip_health_check) {
        printf("\n🏥 Health Report:\n");
        printf("   ✅ Passed:  %d\n", g_stats.health_passed);
        printf("   ❌ Failed:  %d\n", g_stats.health_failed);
        int total_health = g_stats.health_passed + g_stats.health_failed;
        if (total_health > 0) {
            int rate = (g_stats.health_passed * 100) / total_health;
            printf("   📊 Rate:    %d%%\n", rate);
        }
    }
}

// Signal handler for graceful shutdown
void signal_handler(int sig) {
    (void)sig;
    g_interrupted = true;
    printf("\n\n⚠️  Interrupted! Finishing current file...\n");
}

// Process a single file
bool process_file(const FileEntry *entry) {
    const char *input = entry->path;
    char *output = get_output_path(input);
    char temp_output[MAX_PATH_LEN];
    
    // Check if output already exists (skip if not in-place and exists)
    if (!g_config.in_place && file_exists(output)) {
        if (g_config.verbose) {
            log_warn("Skip: %s already exists", output);
        }
        pthread_mutex_lock(&g_stats.mutex);
        g_stats.skipped++;
        pthread_mutex_unlock(&g_stats.mutex);
        return true;
    }
    
    // For in-place mode, use temp file
    if (g_config.in_place) {
        snprintf(temp_output, sizeof(temp_output), "%s.jxl.tmp", input);
    } else {
        strcpy(temp_output, output);
    }
    
    if (g_config.verbose) {
        log_info("Converting: %s", input);
    }
    
    // Step 1: Convert
    if (!convert_jpeg_to_jxl(input, temp_output)) {
        log_error("Conversion failed: %s", input);
        unlink(temp_output);
        pthread_mutex_lock(&g_stats.mutex);
        g_stats.failed++;
        pthread_mutex_unlock(&g_stats.mutex);
        return false;
    }
    
    // Step 2: Migrate metadata
    migrate_metadata(input, temp_output);
    
    // Step 3: Preserve timestamps
    preserve_timestamps(input, temp_output);
    
    // Step 4: Health check
    if (!health_check_jxl(temp_output)) {
        log_error("Health check failed: %s", temp_output);
        unlink(temp_output);
        pthread_mutex_lock(&g_stats.mutex);
        g_stats.failed++;
        g_stats.health_failed++;
        pthread_mutex_unlock(&g_stats.mutex);
        return false;
    }

    // For in-place mode: rename temp to final and delete original
    if (g_config.in_place) {
        if (rename(temp_output, output) != 0) {
            log_error("Failed to rename temp file: %s", temp_output);
            unlink(temp_output);
            pthread_mutex_lock(&g_stats.mutex);
            g_stats.failed++;
            pthread_mutex_unlock(&g_stats.mutex);
            return false;
        }
        
        // Delete original JPEG
        if (unlink(input) != 0) {
            log_warn("Failed to delete original: %s", input);
        }
    }
    
    // Update statistics
    size_t output_size = get_file_size(output);
    
    pthread_mutex_lock(&g_stats.mutex);
    g_stats.success++;
    g_stats.health_passed++;
    g_stats.bytes_input += entry->size;
    g_stats.bytes_output += output_size;
    pthread_mutex_unlock(&g_stats.mutex);
    
    if (g_config.verbose) {
        double ratio = (1.0 - (double)output_size / entry->size) * 100;
        log_success("Done: %s (%.1f%% smaller)", output, ratio);
    }
    
    return true;
}

// Worker thread function
typedef struct {
    int start_idx;
    int end_idx;
} ThreadArg;

void *worker_thread(void *arg) {
    ThreadArg *targ = (ThreadArg *)arg;
    
    for (int i = targ->start_idx; i < targ->end_idx && !g_interrupted; i++) {
        process_file(&g_files[i]);
        
        pthread_mutex_lock(&g_stats.mutex);
        g_stats.processed++;
        int processed = g_stats.processed;
        pthread_mutex_unlock(&g_stats.mutex);
        
        // Update progress (only from main display thread)
        if (targ->start_idx == 0) {
            show_progress(processed, g_stats.total, g_files[i].path);
        }
    }
    
    return NULL;
}

// Print usage
void print_usage(const char *prog) {
    printf("📷 jpeg2jxl - High-Performance JPEG to JXL Batch Converter v%s\n\n", VERSION);
    printf("Usage: %s [options] <directory>\n\n", prog);
    printf("Options:\n");
    printf("  --in-place, -i       Replace original files after conversion\n");
    printf("  --skip-health-check  Skip health validation (not recommended)\n");
    printf("  --no-recursive       Don't process subdirectories\n");
    printf("  --verbose, -v        Show detailed output\n");
    printf("  --dry-run            Preview without converting\n");
    printf("  -j <N>               Number of parallel threads (default: %d)\n", DEFAULT_THREADS);
    printf("  -d <distance>        JXL distance (0=lossless, 1=high quality, default: %.1f)\n", JXL_DISTANCE_DEFAULT);
    printf("  -e <effort>          JXL effort 1-9 (default: %d)\n", JXL_EFFORT_DEFAULT);
    printf("  -h, --help           Show this help\n\n");
    printf("Examples:\n");
    printf("  %s /path/to/images                    # Standard mode\n", prog);
    printf("  %s --in-place /path/to/images         # Replace originals\n", prog);
    printf("  %s -j 8 -d 0 /path/to/images          # 8 threads, lossless\n", prog);
}

// Main function
int main(int argc, char *argv[]) {
    // Initialize
    init_config(&g_config);
    init_stats(&g_stats);
    
    // Parse arguments
    int i;
    for (i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--in-place") == 0 || strcmp(argv[i], "-i") == 0) {
            g_config.in_place = true;
        } else if (strcmp(argv[i], "--skip-health-check") == 0) {
            g_config.skip_health_check = true;
        } else if (strcmp(argv[i], "--no-recursive") == 0) {
            g_config.recursive = false;
        } else if (strcmp(argv[i], "--verbose") == 0 || strcmp(argv[i], "-v") == 0) {
            g_config.verbose = true;
        } else if (strcmp(argv[i], "--dry-run") == 0) {
            g_config.dry_run = true;
        } else if (strcmp(argv[i], "-j") == 0 && i + 1 < argc) {
            g_config.num_threads = atoi(argv[++i]);
            if (g_config.num_threads < 1) g_config.num_threads = 1;
            if (g_config.num_threads > MAX_THREADS) g_config.num_threads = MAX_THREADS;
        } else if (strcmp(argv[i], "-d") == 0 && i + 1 < argc) {
            g_config.jxl_distance = atof(argv[++i]);
        } else if (strcmp(argv[i], "-e") == 0 && i + 1 < argc) {
            g_config.jxl_effort = atoi(argv[++i]);
        } else if (strcmp(argv[i], "-h") == 0 || strcmp(argv[i], "--help") == 0) {
            print_usage(argv[0]);
            return 0;
        } else if (argv[i][0] != '-') {
            strncpy(g_config.target_dir, argv[i], MAX_PATH_LEN - 1);
        }
    }

    // Validate arguments
    if (strlen(g_config.target_dir) == 0) {
        log_error("No target directory specified");
        print_usage(argv[0]);
        return 1;
    }
    
    // Check directory exists
    struct stat st;
    if (stat(g_config.target_dir, &st) != 0 || !S_ISDIR(st.st_mode)) {
        log_error("Directory does not exist: %s", g_config.target_dir);
        return 1;
    }
    
    // Safety check for in-place mode
    if (g_config.in_place && is_dangerous_directory(g_config.target_dir)) {
        log_error("🚫 SAFETY: Cannot operate on protected directory: %s", g_config.target_dir);
        return 1;
    }
    
    // Check dependencies
    if (!check_dependencies()) {
        return 1;
    }
    
    // Print header
    printf("╔══════════════════════════════════════════════╗\n");
    printf("║   📷 jpeg2jxl - High-Performance Converter   ║\n");
    printf("╚══════════════════════════════════════════════╝\n\n");
    
    log_info("📁 Target: %s", g_config.target_dir);
    log_info("📋 Whitelist: .jpg, .jpeg → .jxl");
    log_info("🎯 Quality: distance=%.1f, effort=%d", g_config.jxl_distance, g_config.jxl_effort);
    log_info("🔧 Threads: %d", g_config.num_threads);
    
    if (g_config.in_place) {
        log_warn("🔄 In-place mode: originals will be replaced");
    }
    if (g_config.dry_run) {
        log_warn("🔍 Dry-run mode: no files will be modified");
    }
    
    printf("\n");
    
    // Allocate file list
    g_files = malloc(sizeof(FileEntry) * MAX_FILES);
    if (!g_files) {
        log_error("Memory allocation failed");
        return 1;
    }
    
    // Collect files
    log_info("📊 Scanning for JPEG files...");
    collect_files(g_config.target_dir, g_config.recursive);
    
    if (g_file_count == 0) {
        log_info("📂 No JPEG files found");
        free(g_files);
        return 0;
    }
    
    log_info("📁 Found: %d files", g_file_count);
    printf("\n");

    // Dry run - just list files
    if (g_config.dry_run) {
        log_info("Files that would be converted:");
        for (int j = 0; j < g_file_count; j++) {
            printf("   %s\n", g_files[j].path);
        }
        free(g_files);
        return 0;
    }
    
    // Setup signal handler
    signal(SIGINT, signal_handler);
    signal(SIGTERM, signal_handler);
    
    // Initialize stats
    g_stats.total = g_file_count;
    g_stats.start_time = time(NULL);
    
    // Process files with threading
    int num_threads = g_config.num_threads;
    if (num_threads > g_file_count) {
        num_threads = g_file_count;
    }
    
    pthread_t *threads = malloc(sizeof(pthread_t) * num_threads);
    ThreadArg *thread_args = malloc(sizeof(ThreadArg) * num_threads);
    
    int files_per_thread = g_file_count / num_threads;
    int remainder = g_file_count % num_threads;
    
    int current_idx = 0;
    for (int t = 0; t < num_threads; t++) {
        thread_args[t].start_idx = current_idx;
        thread_args[t].end_idx = current_idx + files_per_thread + (t < remainder ? 1 : 0);
        current_idx = thread_args[t].end_idx;
        
        pthread_create(&threads[t], NULL, worker_thread, &thread_args[t]);
    }
    
    // Wait for all threads
    for (int t = 0; t < num_threads; t++) {
        pthread_join(threads[t], NULL);
    }
    
    // Clear progress line
    printf("\r\033[K\033[A\033[K");
    
    // Print summary
    print_summary();
    
    // Cleanup
    free(threads);
    free(thread_args);
    free(g_files);
    pthread_mutex_destroy(&g_stats.mutex);
    
    return (g_stats.failed > 0) ? 1 : 0;
}
