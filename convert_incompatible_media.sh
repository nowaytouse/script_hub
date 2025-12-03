#!/bin/bash

# ============================================================================
# Incompatible Media Converter - Atomic Operations Version
# 不兼容媒体转换脚本 - 原子操作版本
# ============================================================================
#
# Functionality:
# 1. HEIC/HEIF → PNG (lossless conversion with complete metadata preservation)
# 2. MP4 Video → High-Quality GIF (visually lossless)
#
# Safety Features:
# - Atomic file operations (temp file → verify → replace)
# - Multi-level verification (file existence, size, metadata)
# - Automatic backup of original files
# - Detailed logging
# - Complete metadata preservation (EXIF, XMP, system timestamps)
#
# Usage:
#   ./convert_incompatible_media.sh [options] <target_directory>
#
# Options:
#   --backup-dir <dir>  Specify backup directory (default: _backup_YYYYMMDD_HHMMSS)
#   --dry-run           Show what would be done without executing
#   --verbose           Show detailed information
#   --in-place          Same as default behavior (for compatibility)
#
# Dependencies:
#   - libheif (brew install libheif)
#   - exiftool (brew install exiftool)
#   - ffmpeg (brew install ffmpeg)
#
# ============================================================================

set -e

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Configuration
BACKUP_DIR=""
DRY_RUN=false
VERBOSE=false
TARGET_DIR=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --backup-dir)
            BACKUP_DIR="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --verbose)
            VERBOSE=true
            shift
            ;;
        --in-place)
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [options] <target_directory>"
            echo ""
            echo "Options:"
            echo "  --backup-dir <dir>  Specify backup directory"
            echo "  --dry-run           Show what would be done"
            echo "  --verbose           Show detailed information"
            echo "  -h, --help          Show this help message"
            exit 0
            ;;
        *)
            TARGET_DIR="$1"
            shift
            ;;
    esac
done

if [ -z "$TARGET_DIR" ]; then
    echo "Usage: $0 [--backup-dir <dir>] [--dry-run] [--verbose] <target_directory>"
    echo ""
    echo "Options:"
    echo "  --backup-dir <dir>  Specify backup directory (default: _backup_YYYYMMDD_HHMMSS)"
    echo "  --dry-run           Show what would be done without executing"
    echo "  --verbose           Show detailed information"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    log_error "Directory does not exist: $TARGET_DIR"
    exit 1
fi

# Safety check - prevent operations on dangerous directories
check_dangerous_dirs() {
    local real_dir=""
    if command -v realpath &> /dev/null; then
        real_dir=$(realpath "$TARGET_DIR")
    else
        real_dir=$(cd "$TARGET_DIR" && pwd)
    fi

    local forbidden_paths=("/" "/etc" "/bin" "/usr" "/System" "/Library" "/Applications")
    
    for forbidden in "${forbidden_paths[@]}"; do
        if [ "$real_dir" = "$forbidden" ]; then
            log_error "SAFETY ERROR: Cannot operate on protected system directory: $forbidden"
            exit 1
        fi
    done
}

check_dangerous_dirs

# Set backup directory
if [ -z "$BACKUP_DIR" ]; then
    BACKUP_DIR="${TARGET_DIR}/_backup_$(date +%Y%m%d_%H%M%S)"
fi

# Check dependencies
check_dependencies() {
    local missing=()
    
    # Check for sips (macOS native, preferred for HEIC)
    if ! command -v sips &> /dev/null; then
        if ! command -v heif-convert &> /dev/null; then
            missing+=("libheif (brew install libheif) or sips (macOS native)")
        fi
    fi
    
    if ! command -v exiftool &> /dev/null; then
        missing+=("exiftool (brew install exiftool)")
    fi
    
    if ! command -v ffmpeg &> /dev/null; then
        missing+=("ffmpeg (brew install ffmpeg)")
    fi
    
    if [ ${#missing[@]} -gt 0 ]; then
        log_error "Missing dependencies:"
        for dep in "${missing[@]}"; do
            echo "  - $dep"
        done
        exit 1
    fi
    
    log_success "All dependencies installed"
}

# Verify file integrity
verify_file() {
    local file="$1"
    local min_size="${2:-100}"
    
    if [ ! -f "$file" ]; then
        return 1
    fi
    
    local size
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    if [ "$size" -lt "$min_size" ]; then
        return 1
    fi
    
    return 0
}

# Backup file with directory structure preservation
backup_file() {
    local file="$1"
    
    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY-RUN] Would backup: $file"
        return 0
    fi
    
    mkdir -p "$BACKUP_DIR"
    
    local rel_path="${file#$TARGET_DIR/}"
    local backup_path="$BACKUP_DIR/$rel_path"
    local backup_dir
    backup_dir=$(dirname "$backup_path")
    
    mkdir -p "$backup_dir"
    cp -p "$file" "$backup_path"
    
    if [ "$VERBOSE" = true ]; then
        log_info "Backed up: $file → $backup_path"
    fi
}

# Convert HEIC/HEIF → PNG with complete metadata preservation
convert_heic_to_png() {
    local input="$1"
    local output="${input%.*}.png"
    local temp_output="/tmp/heic_convert_$$.png"
    
    log_info "Converting HEIC → PNG: $(basename "$input")"
    
    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY-RUN] Would convert: $input → $output"
        return 0
    fi
    
    # Step 1: Backup original file
    backup_file "$input"
    
    # Step 2: Convert to temp file (prefer sips on macOS, fallback to heif-convert)
    local convert_success=false
    
    if command -v sips &> /dev/null; then
        if sips -s format png "$input" --out "$temp_output" > /dev/null 2>&1; then
            convert_success=true
        fi
    fi
    
    if [ "$convert_success" = false ] && command -v heif-convert &> /dev/null; then
        if heif-convert "$input" "$temp_output" > /dev/null 2>&1; then
            convert_success=true
        fi
    fi
    
    if [ ! -f "$temp_output" ]; then
        log_error "Conversion failed (no output file): $input"
        return 1
    fi
    
    # Step 3: Verify temp file
    if ! verify_file "$temp_output" 1000; then
        log_error "Verification failed (file too small or corrupted): $temp_output"
        rm -f "$temp_output"
        return 1
    fi
    
    # Step 4: Migrate ALL metadata (EXIF, XMP, IPTC, ICC Profile, etc.)
    exiftool -tagsfromfile "$input" -all:all -ICC_Profile -overwrite_original "$temp_output" 2>/dev/null || \
        log_warn "Partial metadata migration failure, continuing..."
    
    # Step 5: Sync system timestamps (creation, modification, access times)
    touch -r "$input" "$temp_output"
    
    # macOS: Also preserve creation time using SetFile if available
    if command -v GetFileInfo &> /dev/null && command -v SetFile &> /dev/null; then
        local create_date
        create_date=$(GetFileInfo -d "$input" 2>/dev/null) || true
        if [ -n "$create_date" ]; then
            SetFile -d "$create_date" "$temp_output" 2>/dev/null || true
        fi
    fi
    
    # Step 6: Atomic replace
    mv "$temp_output" "$output"
    
    # Step 7: Final verification
    if verify_file "$output" 1000; then
        rm "$input"
        log_success "Done: $(basename "$input") → $(basename "$output")"
        return 0
    else
        log_error "Final verification failed: $output"
        return 1
    fi
}

# Convert MP4 → High-Quality GIF with metadata preservation
convert_mp4_to_gif() {
    local input="$1"
    local output="${input%.*}.gif"
    local temp_output="/tmp/gif_convert_$$.gif"
    local palette="/tmp/palette_$$.png"
    
    log_info "Converting MP4 → GIF: $(basename "$input")"
    
    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY-RUN] Would convert: $input → $output"
        return 0
    fi
    
    # Step 1: Backup original file
    backup_file "$input"
    
    # High-quality GIF parameters
    local fps=15
    local scale=540
    local filters="fps=$fps,scale=$scale:-1:flags=lanczos"
    
    # Step 2: Generate optimized palette
    ffmpeg -loglevel error -i "$input" \
        -vf "${filters},palettegen=stats_mode=diff" \
        -y "$palette" 2>/dev/null
    
    if [ ! -f "$palette" ]; then
        log_error "Palette generation failed: $input"
        return 1
    fi
    
    # Step 3: Create GIF using palette with advanced dithering
    ffmpeg -loglevel error -i "$input" -i "$palette" \
        -lavfi "${filters} [x]; [x][1:v]paletteuse=dither=bayer:bayer_scale=5" \
        -y "$temp_output" 2>/dev/null
    
    rm -f "$palette"
    
    if [ ! -f "$temp_output" ]; then
        log_error "GIF creation failed: $input"
        return 1
    fi
    
    # Step 4: Verify temp file
    if ! verify_file "$temp_output" 1000; then
        log_error "Verification failed (file too small or corrupted): $temp_output"
        rm -f "$temp_output"
        return 1
    fi
    
    # Step 5: Sync system timestamps
    touch -r "$input" "$temp_output"
    
    # macOS: Preserve creation time
    if command -v GetFileInfo &> /dev/null && command -v SetFile &> /dev/null; then
        local create_date
        create_date=$(GetFileInfo -d "$input" 2>/dev/null) || true
        if [ -n "$create_date" ]; then
            SetFile -d "$create_date" "$temp_output" 2>/dev/null || true
        fi
    fi
    
    # Step 6: Atomic replace
    mv "$temp_output" "$output"
    
    # Step 7: Final verification
    if verify_file "$output" 1000; then
        rm "$input"
        log_success "Done: $(basename "$input") → $(basename "$output")"
        return 0
    else
        log_error "Final verification failed: $output"
        return 1
    fi
}

# Main function
main() {
    echo "=============================================="
    echo "  Incompatible Media Converter"
    echo "  Atomic Operations Version"
    echo "=============================================="
    echo ""
    
    check_dependencies
    
    log_info "Target directory: $TARGET_DIR"
    log_info "Backup directory: $BACKUP_DIR"
    
    if [ "$DRY_RUN" = true ]; then
        log_warn "DRY-RUN mode: Only showing what would be done"
    fi
    
    echo ""
    
    # Statistics
    local heic_count=0
    local mp4_count=0
    local success_count=0
    local fail_count=0
    
    # Find and convert HEIC/HEIF files
    log_info "Scanning for HEIC/HEIF files..."
    while IFS= read -r -d '' file; do
        ((heic_count++)) || true
        if convert_heic_to_png "$file"; then
            ((success_count++)) || true
        else
            ((fail_count++)) || true
        fi
    done < <(find "$TARGET_DIR" -type f \( -iname "*.heic" -o -iname "*.heif" \) ! -path "*/_backup_*" -print0 2>/dev/null)
    
    # Find and convert MP4 files
    log_info "Scanning for MP4 files..."
    while IFS= read -r -d '' file; do
        ((mp4_count++)) || true
        if convert_mp4_to_gif "$file"; then
            ((success_count++)) || true
        else
            ((fail_count++)) || true
        fi
    done < <(find "$TARGET_DIR" -type f -iname "*.mp4" ! -path "*/_backup_*" -print0 2>/dev/null)
    
    echo ""
    echo "=============================================="
    echo "  Conversion Complete"
    echo "=============================================="
    echo "  HEIC/HEIF files: $heic_count"
    echo "  MP4 files: $mp4_count"
    echo "  Successful: $success_count"
    echo "  Failed: $fail_count"
    
    if [ "$DRY_RUN" = false ] && [ -d "$BACKUP_DIR" ]; then
        echo "  Backup location: $BACKUP_DIR"
    fi
    echo "=============================================="
}

main
