/**
 * static2jxl.h - Header file for Static Image to JXL converter
 * 
 * Supports: JPEG, PNG, BMP, TIFF (uncompressed/LZW), TGA, PPM/PBM/PGM
 * Conversion logic:
 *   - JPEG → JXL (high quality lossy, -d 1)
 *   - PNG/BMP/TGA/PPM (true lossless + >2MB) → JXL lossless (-d 0)
 *   - TIFF (uncompressed/LZW + >2MB) → JXL lossless (-d 0)
 *   - RAW formats (DNG/CR2/NEF) → SKIP (preserve RAW flexibility)
 */

#ifndef STATIC2JXL_H
#define STATIC2JXL_H

#include <stdbool.h>
#include <stdint.h>
#include <pthread.h>

// Version
#define VERSION "2.0.0"

// Limits
#define MAX_PATH_LEN 4096
#define MAX_FILES 100000
#define MAX_THREADS 32
#define DEFAULT_THREADS 4

// Size threshold for lossless formats (2MB)
#define MIN_LOSSLESS_SIZE (2 * 1024 * 1024)

// JXL quality settings
#define JXL_DISTANCE_LOSSY 1.0     // High quality lossy for JPEG
#define JXL_DISTANCE_LOSSLESS 0.0  // Mathematically lossless
#define JXL_EFFORT_DEFAULT 7       // Balanced speed/compression

// File type enumeration
typedef enum {
    FILE_TYPE_UNKNOWN = 0,
    FILE_TYPE_JPEG,      // JPEG - lossy source, use -d 1
    FILE_TYPE_PNG,       // PNG - lossless source
    FILE_TYPE_BMP,       // BMP - uncompressed
    FILE_TYPE_TIFF,      // TIFF - check compression type
    FILE_TYPE_TGA,       // TGA - game/design format
    FILE_TYPE_PPM,       // PPM/PBM/PGM - simple bitmap
    FILE_TYPE_RAW,       // RAW formats - skip
    FILE_TYPE_JXL        // Already JXL - skip
} FileType;

// TIFF compression types
typedef enum {
    TIFF_COMPRESSION_UNKNOWN = 0,
    TIFF_COMPRESSION_NONE = 1,      // Uncompressed - good for JXL
    TIFF_COMPRESSION_LZW = 5,       // LZW - good for JXL
    TIFF_COMPRESSION_JPEG = 7,      // JPEG - skip (already lossy)
    TIFF_COMPRESSION_DEFLATE = 8,   // Deflate - good for JXL
    TIFF_COMPRESSION_OTHER = 99     // Other - skip
} TiffCompression;

// Configuration
typedef struct {
    char target_dir[MAX_PATH_LEN];
    bool in_place;
    bool skip_health_check;
    bool recursive;
    bool verbose;
    bool dry_run;
    bool force_lossless;           // Force lossless even for JPEG
    int num_threads;
    double jxl_distance;           // Override distance
    int jxl_effort;
} Config;

// File entry for processing queue
typedef struct {
    char path[MAX_PATH_LEN];
    size_t size;
    FileType type;
    bool use_lossless;             // Whether to use lossless mode
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
    // Per-type statistics
    int jpeg_count;
    int png_count;
    int bmp_count;
    int tiff_count;
    int tga_count;
    int ppm_count;
    int skipped_raw;
    int skipped_small;
    int skipped_tiff_jpeg;
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

// File type names for display
static const char *FILE_TYPE_NAMES[] = {
    "Unknown",
    "JPEG",
    "PNG",
    "BMP",
    "TIFF",
    "TGA",
    "PPM/PBM/PGM",
    "RAW",
    "JXL"
};

// Function prototypes

// Initialization
void init_config(Config *config);
void init_stats(Stats *stats);

// File type detection (by magic bytes)
FileType detect_file_type(const char *path);
const char *get_file_type_name(FileType type);
bool is_supported_file(const char *path);
bool is_lossless_source(FileType type);
bool should_use_lossless(const FileEntry *entry);

// TIFF specific
TiffCompression detect_tiff_compression(const char *path);
bool is_tiff_suitable_for_jxl(const char *path);

// File operations
int collect_files(const char *dir, bool recursive);
bool file_exists(const char *path);
size_t get_file_size(const char *path);
char *get_output_path(const char *input);

// Safety
bool is_dangerous_directory(const char *path);
bool check_dependencies(void);

// Conversion
bool convert_to_jxl(const char *input, const char *output, bool lossless);
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
void signal_handler(int sig);

#endif // STATIC2JXL_H
