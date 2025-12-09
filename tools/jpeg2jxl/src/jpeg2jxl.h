/**
 * jpeg2jxl.h - Header file for JPEG to JXL converter
 */

#ifndef JPEG2JXL_H
#define JPEG2JXL_H

#include <stdbool.h>
#include <stdint.h>
#include <pthread.h>

// Version
#define VERSION "1.0.0"

// Limits
#define MAX_PATH_LEN 4096
#define MAX_FILES 100000
#define MAX_THREADS 32
#define DEFAULT_THREADS 4

// JXL quality settings
#define JXL_DISTANCE_DEFAULT 1.0  // High quality lossy (-d 1)
#define JXL_EFFORT_DEFAULT 7      // Balanced speed/compression

// Configuration
typedef struct {
    char target_dir[MAX_PATH_LEN];
    bool in_place;
    bool skip_health_check;
    bool recursive;
    bool verbose;
    bool dry_run;
    int num_threads;
    double jxl_distance;
    int jxl_effort;
} Config;

// File entry for processing queue
typedef struct {
    char path[MAX_PATH_LEN];
    size_t size;
} FileEntry;

// Processing statistics
typedef struct {
    int total;
    int processed;
    int success;
    int failed;
    int skipped;
    int health_passed;
    int health_failed;
    size_t bytes_input;
    size_t bytes_output;
    time_t start_time;
    pthread_mutex_t mutex;
} Stats;

// Global state
extern Config g_config;
extern Stats g_stats;
extern FileEntry *g_files;
extern int g_file_count;
extern volatile bool g_interrupted;

// Dangerous directories (safety check)
static const char *DANGEROUS_DIRS[] = {
    "/",
    "/etc",
    "/bin",
    "/sbin",
    "/usr",
    "/var",
    "/System",
    "/Library",
    "/Applications",
    "/private",
    NULL
};

// Function prototypes

// Initialization
void init_config(Config *config);
void init_stats(Stats *stats);

// File operations
bool is_jpeg_file(const char *path);
int collect_files(const char *dir, bool recursive);
bool file_exists(const char *path);
size_t get_file_size(const char *path);

// Safety
bool is_dangerous_directory(const char *path);
bool check_dependencies(void);

// Conversion
bool convert_jpeg_to_jxl(const char *input, const char *output);
bool migrate_metadata(const char *source, const char *dest);
bool preserve_timestamps(const char *source, const char *dest);
bool health_check_jxl(const char *path);

// Progress
void show_progress(int current, int total, const char *filename);
void print_summary(void);

// Threading
void *worker_thread(void *arg);

// Utilities
void log_info(const char *fmt, ...);
void log_success(const char *fmt, ...);
void log_warn(const char *fmt, ...);
void log_error(const char *fmt, ...);
char *get_output_path(const char *input);
void signal_handler(int sig);

#endif // JPEG2JXL_H
