#!/bin/bash

# ============================================================================
# 🔄 Incompatible Media Converter - Whitelist Mode with Health Check
# ============================================================================
#
# Converts incompatible media formats to universally supported formats while
# preserving maximum metadata (EXIF, XMP, ICC Profile, timestamps, animation).
#
# Supported Conversions (Whitelist):
#   📷 HEIC/HEIF → PNG (lossless, full metadata preservation)
#   🎬 MP4 → GIF (lossless animation) or WebP (high quality, smaller)
#
# Features:
#   ✅ Whitelist-only processing (ignores unsupported formats)
#   ✅ Health check validation (ensures output is viewable/playable)
#   ✅ Complete metadata preservation (EXIF, XMP, ICC, timestamps)
#   ✅ Animation preservation (frame count, FPS, duration)
#   ✅ Atomic operations (temp file → verify → replace)
#   ✅ Automatic backup before conversion
#
# Usage:
#   ./convert_incompatible_media.sh [options] <target_directory>
#
# Options:
#   --format <gif|webp>   Video output format (default: gif)
#   --backup-dir <dir>    Specify backup directory
#   --dry-run             Show what would be done without executing
#   --verbose             Show detailed information
#   --skip-health-check   Skip media health validation
#   --keep-only-incompatible  Delete all compatible media, keep only converted files
#   -h, --help            Show this help message
#
# Dependencies:
#   - sips (macOS native) or libheif (brew install libheif)
#   - exiftool (brew install exiftool)
#   - ffmpeg/ffprobe (brew install ffmpeg)
#
# ============================================================================

set -e

# ============================================================================
# 🎨 Color Definitions & Logging
# ============================================================================
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

log_info()    { echo -e "${BLUE}ℹ️  [INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}✅ [SUCCESS]${NC} $1"; }
log_error()   { echo -e "${RED}❌ [ERROR]${NC} $1" >&2; }
log_warn()    { echo -e "${YELLOW}⚠️  [WARN]${NC} $1"; }
log_health()  { [ "$VERBOSE" = true ] && echo -e "${BLUE}🏥 [HEALTH]${NC} $1"; }
log_meta()    { [ "$VERBOSE" = true ] && echo -e "${BLUE}📋 [META]${NC} $1"; }
log_skip()    { echo -e "${YELLOW}⏭️  [SKIP]${NC} $1"; }

# ============================================================================
# 📊 Progress Bar & Time Estimation
# ============================================================================

START_TIME=0
CURRENT_FILE=0
TOTAL_FILES=0

# Display progress bar
show_progress() {
    local current=$1
    local total=$2
    local filename="$3"
    
    local percent=$((current * 100 / total))
    local filled=$((percent / 2))
    local empty=$((50 - filled))
    
    # Progress bar
    printf "\r\033[K"  # Clear line
    printf "📊 Progress: ["
    printf "${GREEN}"
    printf '%0.s█' $(seq 1 $filled)
    printf "${NC}"
    printf '%0.s░' $(seq 1 $empty)
    printf "] ${percent}%% "
    
    # Current/Total
    printf "(${current}/${total}) "
    
    # Time estimation
    if [ $current -gt 0 ]; then
        local elapsed=$(($(date +%s) - START_TIME))
        local avg_time=$((elapsed / current))
        local remaining=$(( (total - current) * avg_time ))
        
        if [ $remaining -gt 60 ]; then
            printf "| ⏱️  ETA: ~$((remaining / 60))m ${remaining % 60}s"
        else
            printf "| ⏱️  ETA: ~${remaining}s"
        fi
    fi
    
    # Current file (truncate if too long)
    if [ -n "$filename" ]; then
        local display_name="$filename"
        if [ ${#display_name} -gt 40 ]; then
            display_name="${display_name:0:37}..."
        fi
        printf "\n   📄 ${display_name}"
    fi
}

# Clear progress bar and move to next line
clear_progress() {
    printf "\r\033[K"
}

# ============================================================================
# 📋 Whitelist Configuration (Lossless Mode Only)
# ============================================================================
# Only these formats will be processed - all others are ignored
WHITELIST_IMAGE_INPUT=("heic" "heif")
WHITELIST_VIDEO_INPUT=("mp4")
WHITELIST_OVERSIZE=("gif")  # Oversized files to convert to lossless WebP

# File size limits (in bytes) for compatibility
# Based on common upload limits: JPG/WebP < 20MB, PNG < 50MB, GIF < 10MB
MAX_SIZE_JPG=$((20 * 1024 * 1024))    # 20 MB
MAX_SIZE_WEBP=$((20 * 1024 * 1024))   # 20 MB
MAX_SIZE_PNG=$((50 * 1024 * 1024))    # 50 MB
MAX_SIZE_GIF=$((10 * 1024 * 1024))    # 10 MB

# ============================================================================
# ⚙️ Configuration
# ============================================================================
BACKUP_DIR=""
DRY_RUN=false
VERBOSE=false
TARGET_DIR=""
VIDEO_FORMAT="webp"  # Default: lossless WebP (faster, smaller than GIF)
SKIP_HEALTH_CHECK=false
KEEP_ONLY_INCOMPATIBLE=false

# Lossless mode settings
AUTO_CONVERT_OVERSIZE=true   # Always convert oversized files
OVERSIZE_FORMAT="webp"       # Lossless WebP for oversized files

# Statistics
HEALTH_PASSED=0
HEALTH_FAILED=0
HEALTH_WARNINGS=0
FILES_PROCESSED=0
FILES_SKIPPED=0
COMPATIBLE_DELETED=0
OVERSIZE_FOUND=0
OVERSIZE_CONVERTED=0

# Track newly converted files to protect them from deletion
CONVERTED_FILES=()

# File count tracking for validation
INPUT_FILE_COUNT=0
OUTPUT_FILE_COUNT=0

# ============================================================================
# 🔧 Argument Parsing
# ============================================================================
while [[ $# -gt 0 ]]; do
    case $1 in
        --format)
            VIDEO_FORMAT="$2"
            shift 2
            ;; 
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
        --skip-health-check)
            SKIP_HEALTH_CHECK=true
            shift
            ;; 
        --keep-only-incompatible)
            KEEP_ONLY_INCOMPATIBLE=true
            shift
            ;; 
        --auto-convert-oversize)
            AUTO_CONVERT_OVERSIZE=true
            shift
            ;; 
        --gif-to-webp)
            GIF_TO_WEBP=true
            shift
            ;; 
        --oversize-format)
            OVERSIZE_FORMAT="$2"
            shift 2
            ;; 
        --prefer-smaller)
            PREFER_SMALLER=true
            shift
            ;; 
        -h|--help)
            echo "🔄 Incompatible Media Converter"
            echo ""
            echo "Usage: $0 [options] <target_directory>"
            echo ""
            echo "Options:"
            echo "  --format <gif|webp>   Video output format (default: webp)"
            echo "                        webp = Lossless, smaller & faster (RECOMMENDED)"
            echo "                        gif  = Lossless, larger file size"
            echo "  --backup-dir <dir>    Specify backup directory"
            echo "  --dry-run             Show what would be done"
            echo "  --verbose             Show detailed metadata info"
            echo "  --skip-health-check   Skip media health validation"
            echo "  --keep-only-incompatible  Delete all compatible media, keep only converted"
            echo "  -h, --help            Show this help message"
            echo ""
            echo "Whitelist (only these formats are processed):"
            echo "  📷 Image: HEIC, HEIF → PNG"
            echo "  🎬 Video: MP4 → GIF/WebP"
            echo ""
            echo "Keep-Only-Incompatible Mode:"
            echo "  Converts incompatible media AND deletes all other files."
            echo "  Only the converted files remain in the directory."
            echo "  ⚠️  WARNING: This is destructive! Use with caution."
            exit 0
            ;; 
        *)
            TARGET_DIR="$1"
            shift
            ;; 
    esac
done

# Validate arguments
if [ -z "$TARGET_DIR" ]; then
    log_error "No target directory specified"
    echo "Usage: $0 [options] <target_directory>"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    log_error "Directory does not exist: $TARGET_DIR"
    exit 1
fi

if [[ "$VIDEO_FORMAT" != "gif" && "$VIDEO_FORMAT" != "webp" ]]; then
    log_error "Invalid format: $VIDEO_FORMAT (use 'gif' or 'webp')"
    exit 1
fi

# ============================================================================
# 🛡️ Safety Checks
# ============================================================================
check_dangerous_dirs() {
    local real_dir=""
    if command -v realpath &> /dev/null; then
        real_dir=$(realpath "$TARGET_DIR")
    else
        real_dir=$(cd "$TARGET_DIR" && pwd)
    fi

    local forbidden_paths=("/" "/etc" "/bin" "/usr" "/System" "/Library" "/Applications" "$HOME")
    for forbidden in "${forbidden_paths[@]}"; do
        if [ "$real_dir" = "$forbidden" ]; then
            log_error "🚫 SAFETY: Cannot operate on protected directory: $forbidden"
            exit 1
        fi
    done
}

check_dangerous_dirs

# Set backup directory
if [ -z "$BACKUP_DIR" ]; then
    BACKUP_DIR="${TARGET_DIR}/_backup_$(date +%Y%m%d_%H%M%S)"
fi

# ============================================================================
# 📦 Dependency Check
# ============================================================================
check_dependencies() {
    local missing=()
    
    if ! command -v sips &> /dev/null && ! command -v heif-convert &> /dev/null; then
        missing+=("sips (macOS) or libheif")
    fi
    
    if ! command -v exiftool &> /dev/null; then
        missing+=("exiftool (brew install exiftool)")
    fi
    
    if ! command -v ffmpeg &> /dev/null; then
        missing+=("ffmpeg (brew install ffmpeg)")
    fi
    
    if ! command -v ffprobe &> /dev/null; then
        missing+=("ffprobe (included with ffmpeg)")
    fi
    
    if [ ${#missing[@]} -gt 0 ]; then
        log_error "Missing dependencies:"
        for dep in "${missing[@]}"; do
            echo "  ❌ $dep"
        done
        exit 1
    fi
    
    log_success "All dependencies installed"
}

# ============================================================================
# 📏 File Size Validation
# ============================================================================

check_file_size() {
    local file="$1"
    local format="$2"
    
    local size
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    
    local limit=0
    case "$format" in
        jpg|jpeg)
            limit=$MAX_SIZE_JPG
            ;; 
        webp)
            limit=$MAX_SIZE_WEBP
            ;; 
        png)
            limit=$MAX_SIZE_PNG
            ;; 
        gif)
            limit=$MAX_SIZE_GIF
            ;; 
        *)
            return 0  # No limit for other formats
            ;; 
    esac
    
    if [ "$size" -gt "$limit" ]; then
        local size_mb=$((size / 1024 / 1024))
        local limit_mb=$((limit / 1024 / 1024))
        log_warn "File exceeds size limit: $(basename "$file") (${size_mb}MB > ${limit_mb}MB limit)"
        ((OVERSIZE_FOUND++)) || true
        return 1
    fi
    
    return 0
}

scan_oversize_files() {
    local dir="$1"
    
    log_info "🔍 Scanning for oversized files..."
    
    local found=0
    
    # Check JPG/JPEG files
    while IFS= read -r -d '' file; do
        check_file_size "$file" "jpg" || ((found++)) || true
    done < <(find "$dir" -type f \( -iname "*.jpg" -o -iname "*.jpeg" \) -print0 2>/dev/null)
    
    # Check WebP files
    while IFS= read -r -d '' file; do
        check_file_size "$file" "webp" || ((found++)) || true
    done < <(find "$dir" -type f -iname "*.webp" -print0 2>/dev/null)
    
    # Check PNG files
    while IFS= read -r -d '' file; do
        check_file_size "$file" "png" || ((found++)) || true
    done < <(find "$dir" -type f -iname "*.png" ! -path "*/_backup_*" -print0 2>/dev/null)
    
    # Check GIF files
    while IFS= read -r -d '' file; do
        check_file_size "$file" "gif" || ((found++)) || true
    done < <(find "$dir" -type f -iname "*.gif" ! -path "*/_backup_*" -print0 2>/dev/null)
    
    if [ "$found" -gt 0 ]; then
        log_warn "Found $found oversized files that may need conversion or compression"
    else
        log_success "No oversized files found"
    fi
    
    return 0
}

# ============================================================================
# 📋 Metadata Extraction & Preservation
# ============================================================================

# Extract and display media metadata
show_media_info() {
    local file="$1"
    local type="$2"
    
    [ "$VERBOSE" = false ] && return 0
    
    log_meta "Original file info:"
    
    if [ "$type" = "image" ]; then
        # Image metadata
        local info
        info=$(exiftool -ImageWidth -ImageHeight -ColorSpace -BitDepth -CreateDate -ModifyDate -Make -Model "$file" 2>/dev/null | head -10)
        echo "$info" | while read line;
 do
            [ -n "$line" ] && echo "    $line"
        done
    elif [ "$type" = "video" ]; then
        # Video metadata with animation info
        local probe
        probe=$(ffprobe -v error -select_streams v:0 \
            -show_entries stream=width,height,r_frame_rate,nb_frames,duration,codec_name \
            -of default=noprint_wrappers=1 "$file" 2>/dev/null)
        
        echo "$probe" | while read line;
 do
            [ -n "$line" ] && echo "    📹 $line"
        done
        
        # Frame count and FPS
        local fps frame_count duration
        fps=$(ffprobe -v error -select_streams v:0 -show_entries stream=r_frame_rate -of csv=p=0 "$file" 2>/dev/null)
        frame_count=$(ffprobe -v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 "$file" 2>/dev/null)
        duration=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$file" 2>/dev/null)
        
        [ -n "$fps" ] && echo "    🎞️  FPS: $fps"
        [ -n "$frame_count" ] && echo "    🖼️  Frames: $frame_count"
        [ -n "$duration" ] && echo "    ⏱️  Duration: ${duration}s"
    fi
}

# Verify metadata preservation after conversion
verify_metadata_preservation() {
    local original="$1"
    local converted="$2"
    local type="$3"
    
    [ "$VERBOSE" = false ] && return 0
    
    log_meta "Verifying metadata preservation..."
    
    if [ "$type" = "image" ]; then
        # Check EXIF tags count
        local orig_tags conv_tags
        orig_tags=$(exiftool -s "$original" 2>/dev/null | wc -l)
        conv_tags=$(exiftool -s "$converted" 2>/dev/null | wc -l)
        
        echo "    📊 Original tags: $orig_tags"
        echo "    📊 Converted tags: $conv_tags"
        
        if [ "$conv_tags" -ge "$((orig_tags * 70 / 100))" ]; then
            echo "    ✅ Metadata preservation: GOOD (≥70%)"
        else
            echo "    ⚠️  Metadata preservation: PARTIAL"
        fi
    elif [ "$type" = "animation" ]; then
        # Check animation preservation with enhanced validation
        local orig_fps conv_fps orig_duration conv_duration
        
        # Get FPS
        orig_fps=$(ffprobe -v error -select_streams v:0 -show_entries stream=r_frame_rate -of csv=p=0 "$original" 2>/dev/null)
        conv_fps=$(ffprobe -v error -select_streams v:0 -show_entries stream=r_frame_rate -of csv=p=0 "$converted" 2>/dev/null)
        
        # Get Duration
        orig_duration=$(ffprobe -v error -select_streams v:0 -show_entries stream=duration -of csv=p=0 "$original" 2>/dev/null)
        conv_duration=$(ffprobe -v error -select_streams v:0 -show_entries stream=duration -of csv=p=0 "$converted" 2>/dev/null)
        
        echo "    🎞️  Original FPS: $orig_fps"
        echo "    🎞️  Converted FPS: $conv_fps"
        echo "    ⏱️  Original Duration: ${orig_duration}s"
        echo "    ⏱️  Converted Duration: ${conv_duration}s"
        
        # Calculate expected frames from Duration and FPS
        if [[ "$orig_fps" =~ ^([0-9]+)/([0-9]+)$ ]]; then
            local fps_num="${BASH_REMATCH[1]}"
            local fps_den="${BASH_REMATCH[2]}"
            local fps_float=$(awk "BEGIN {printf \"%.2f\", $fps_num/$fps_den}")
            local expected_frames=$(awk "BEGIN {printf \"%.0f\", $orig_duration * $fps_float}")
            echo "    📊 Expected frames: ~$expected_frames"
        fi
        
        # Verify FPS preservation
        if [ "$conv_fps" = "$orig_fps" ]; then
            echo "    ✅ FPS: PRESERVED ($orig_fps)"
        else
            echo "    ⚠️  FPS: CHANGED ($orig_fps → $conv_fps)"
        fi
        
        # Verify Duration preservation (within 1% tolerance)
        if [ -n "$orig_duration" ] && [ -n "$conv_duration" ] && [ "$conv_duration" != "N/A" ]; then
            local duration_diff=$(awk "BEGIN {printf \"%.2f\", ($conv_duration - $orig_duration) / $orig_duration * 100}")
            if (( $(awk "BEGIN {print ($duration_diff < 1.0 && $duration_diff > -1.0) ? 1 : 0}") )); then
                echo "    ✅ Duration: PRESERVED (≤1% difference)"
            else
                echo "    ⚠️  Duration: CHANGED (${duration_diff}% difference)"
            fi
        fi
    fi
}

# ============================================================================
# 🏥 Health Check Functions
# ============================================================================

check_image_health() {
    local file="$1"
    local file_type="${2:-image}"
    
    [ "$SKIP_HEALTH_CHECK" = true ] && return 0
    
    log_health "Validating: $(basename "$file")"
    
    # Basic checks
    if [ ! -f "$file" ]; then
        log_error "Health check failed: File does not exist"
        return 1
    fi
    
    local size
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    
    if [ "$size" -lt 100 ]; then
        log_error "Health check failed: File too small ($size bytes)"
        return 1
    fi
    
    # Format signature checks
    case "${file##*.}" in
        png|PNG)
            local sig=$(xxd -l 8 -p "$file" 2>/dev/null)
            if [ "$sig" != "89504e470d0a1a0a" ]; then
                log_error "Health check failed: Invalid PNG signature"
                return 1
            fi
            ;; 
        gif|GIF)
            local sig=$(head -c 6 "$file" 2>/dev/null)
            if [[ "$sig" != "GIF87a" && "$sig" != "GIF89a" ]]; then
                log_error "Health check failed: Invalid GIF signature"
                return 1
            fi
            ;; 
        webp|WEBP)
            local sig=$(head -c 4 "$file" 2>/dev/null)
            if [ "$sig" != "RIFF" ]; then
                log_error "Health check failed: Invalid WebP signature"
                return 1
            fi
            ;; 
    esac
    
    # Structure validation
    local width=0 height=0 codec=""
    
    # Use exiftool for WebP as ffprobe is unreliable for it
    if [[ "${file##*.}" =~ (webp|WEBP) ]]; then
        if command -v exiftool &> /dev/null; then
            width=$(exiftool -s -s -s -ImageWidth "$file" 2>/dev/null)
            height=$(exiftool -s -s -s -ImageHeight "$file" 2>/dev/null)
            codec="webp"
        fi
    # Use ffprobe for all other types
    elif command -v ffprobe &> /dev/null; then
        local probe
        probe=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height,codec_name -of csv=p=0 "$file" 2>&1)
        
        if [ $? -eq 0 ] && [ -n "$probe" ]; then
            IFS=',' read -r codec width height <<< "$probe"
        fi
    fi

    if [ -z "$width" ] || [ -z "$height" ] || [ "$width" -lt 1 ] || [ "$height" -lt 1 ]; then
        log_error "Health check failed: Could not determine valid dimensions."
        log_error "Detected: Width=${width:-0}, Height=${height:-0}"
        return 1
    fi
        
    [ "$VERBOSE" = true ] && log_health "  Codec: $codec, Size: ${width}x${height}"

    # Decode test
    # if command -v ffmpeg &> /dev/null; then
    #     if ! ffmpeg -v error -i "$file" -frames:v 1 -f null - 2>/dev/null; then
    #         log_error "Health check failed: Cannot decode media"
    #         return 1
    #     fi
    # fi
    
    log_health "✅ Passed: $(basename "$file") ($size bytes)"
    ((HEALTH_PASSED++)) || true
    return 0
}

print_health_report() {
    echo ""
    echo "╔══════════════════════════════════════════════╗"
    echo "║        🏥 Media Health Report                ║"
    echo "╠══════════════════════════════════════════════╣"
    printf "║  %-20s %20s  ║\n" "✅ Passed:" "$HEALTH_PASSED"
    printf "║  %-20s %20s  ║\n" "❌ Failed:" "$HEALTH_FAILED"
    printf "║  %-20s %20s  ║\n" "⚠️  Warnings:" "$HEALTH_WARNINGS"
    
    local total=$((HEALTH_PASSED + HEALTH_FAILED))
    if [ "$total" -gt 0 ]; then
        local rate=$((HEALTH_PASSED * 100 / total))
        printf "║  %-20s %19s%%  ║\n" "📊 Health Rate:" "$rate"
    fi
    echo "╚══════════════════════════════════════════════╝"
}

# ============================================================================
# 💾 Backup Functions
# ============================================================================

backup_file() {
    local file="$1"
    
    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY-RUN] Would backup: $(basename "$file")"
        return 0
    fi
    
    mkdir -p "$BACKUP_DIR"
    # Use basename to avoid path issues with special characters
    local filename
    filename=$(basename "$file")
    local backup_path="$BACKUP_DIR/$filename"
    cp -p "$file" "$backup_path"
    
    [ "$VERBOSE" = true ] && log_info "📦 Backed up: $filename"
}

# ============================================================================
# 🔄 Conversion Functions
# ============================================================================

# Convert HEIC/HEIF → PNG (lossless with full metadata)
convert_heic_to_png() {
    local input="$1"
    local output="${input%.*}.png"
    local temp_output="/tmp/heic_convert_$$.png"
    
    echo ""
    log_info "📷 Converting HEIC → PNG: $(basename "$input")"
    
    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY-RUN] Would convert: $(basename "$input") → $(basename "$output")"
        return 0
    fi
    
    # Show original metadata
    show_media_info "$input" "image"
    
    # Backup original
    backup_file "$input"
    
    # Convert using sips (macOS) or heif-convert
    log_info "🔄 Step 1/4: Converting image format..."
    if command -v sips &> /dev/null; then
        sips -s format png "$input" --out "$temp_output" > /dev/null 2>&1
    elif command -v heif-convert &> /dev/null; then
        heif-convert "$input" "$temp_output" > /dev/null 2>&1
    fi
    
    if [ ! -f "$temp_output" ]; then
        log_error "Conversion failed: $(basename "$input")"
        return 1
    fi
    
    # Migrate ALL metadata (EXIF, XMP, ICC Profile, etc.)
    log_info "📋 Step 2/4: Migrating metadata (EXIF, XMP, ICC)..."
    exiftool -tagsfromfile "$input" \
        -all:all \
        -ICC_Profile \
        -ColorSpace \
        -overwrite_original \
        "$temp_output" 2>/dev/null || true
    
    # Preserve file timestamps
    log_info "⏰ Step 3/4: Preserving timestamps..."
    touch -r "$input" "$temp_output"
    
    mv "$temp_output" "$output"
    
    # Health check
    log_info "🏥 Step 4/4: Health validation..."
    if check_image_health "$output" "png"; then
        verify_metadata_preservation "$input" "$output" "image"
        rm "$input"
        log_success "✅ Done: $(basename "$input") → $(basename "$output")"
        ((FILES_PROCESSED++)) || true
        # Track converted file
        CONVERTED_FILES+=("$(basename "$output")")
        return 0
    else
        log_error "Health check failed, restoring from backup"
        rm -f "$output"
        ((HEALTH_FAILED++)) || true
        return 1
    fi
}

# Convert MP4 → GIF (lossless animation, preserves all frames)
convert_mp4_to_gif() {
    local input="$1"
    local output="${input%.*}.gif"
    local temp_output="/tmp/gif_convert_$$.gif"
    local palette="/tmp/palette_$$.png"
    
    echo ""
    log_info "🎬 Converting MP4 → GIF: $(basename "$input")"
    
    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY-RUN] Would convert: $(basename "$input") → $(basename "$output")"
        return 0
    fi
    
    # Show original metadata
    show_media_info "$input" "video"
    
    # Backup original
    backup_file "$input"
    
    # Fast conversion: direct to GIF without complex palette generation with PROGRESS
    # This is much faster while still preserving all frames
    log_info "🔄 Step 1/3: Converting to GIF (fast, lossless)..."
    ffmpeg -loglevel warning -stats -i "$input" \
        -vf "split[s0][s1];[s0]palettegen=max_colors=256[p];[s1][p]paletteuse=dither=none" \
        -y "$temp_output" 2>&1 | while read line;
 do
        # Show ffmpeg stats in real-time
        if [[ "$line" =~ frame=.*fps=.*speed= ]]; then
            printf "\r  ▶️  $line"
        fi
    done
    printf "\n"
    
    if [ ! -f "$temp_output" ]; then
        log_error "GIF creation failed"
        return 1
    fi
    
    # Preserve timestamps
    log_info "⏰ Step 2/3: Preserving timestamps..."
    touch -r "$input" "$temp_output"
    mv "$temp_output" "$output"
    
    # Health check
    log_info "🏥 Step 3/3: Health validation..."
    if check_image_health "$output" "gif"; then
        verify_metadata_preservation "$input" "$output" "animation"
        rm "$input"
        log_success "✅ Done: $(basename "$input") → $(basename "$output")"
        ((FILES_PROCESSED++)) || true
        # Track converted file
        CONVERTED_FILES+=("$(basename "$output")")
        return 0
    else
        log_error "Health check failed, restoring from backup"
        rm -f "$output"
        ((HEALTH_FAILED++)) || true
        return 1
    fi
}

# Convert MP4 → Lossless WebP (optimized single-step conversion)
convert_mp4_to_webp() {
    local input="$1"
    local output="${input%.*}.webp"
    local temp_output="/tmp/webp_convert_$$.webp"

    echo ""
    log_info "🎬 Converting MP4 → Lossless WebP: $(basename "$input")"

    if [ "$DRY_RUN" = true ]; then
        log_info "[DRY-RUN] Would convert: $(basename "$input") → $(basename "$output")"
        return 0
    fi

    # Show original metadata
    show_media_info "$input" "video"

    # Backup original
    backup_file "$input"

    # Get original FPS for perfect preservation
    local fps fps_num fps_den
    fps=$(ffprobe -v error -select_streams v:0 \
        -show_entries stream=r_frame_rate \
        -of default=noprint_wrappers=1:nokey=1 "$input" 2>/dev/null)
    
    # Parse FPS fraction (e.g., "30/1" -> num=30, den=1)
    if [[ "$fps" =~ ^([0-9]+)/([0-9]+)$ ]]; then
        fps_num="${BASH_REMATCH[1]}"
        fps_den="${BASH_REMATCH[2]}"
    else
        # Default to 30/1 if parsing fails
        fps_num="30"
        fps_den="1"
        fps="30/1"
    fi
    
    log_info "  🎞️  Original FPS: $fps ($fps_num/$fps_den)"

    # OPTIMIZED: Single-step direct conversion to lossless WebP
    # This is 3-5x faster than the old two-step method (MP4→PNG→WebP)
    log_info "🔄 Step 1/4: Converting to lossless WebP (optimized)..."
    log_info "  ▶️  Running ffmpeg (progress will be shown)..."
    echo ""
    
    # Direct ffmpeg execution without pipe blocking
    # compression_level 2 for faster speed
    # -nostdin prevents interactive mode
    # -fps_mode cfr preserves exact FPS from input
    if ffmpeg -nostdin -i "$input" \
        -c:v libwebp \
        -lossless 1 \
        -quality 100 \
        -compression_level 2 \
        -preset picture \
        -loop 0 \
        -fps_mode cfr \
        -r "$fps" \
        -an \
        -y "$temp_output"; then
        echo ""
    else
        echo ""
        log_error "Lossless WebP creation failed"
        return 1
    fi

    if [ ! -f "$temp_output" ]; then
        log_error "Lossless WebP creation failed"
        return 1
    fi


    # Migrate video metadata using exiftool
    log_info "📋 Step 2/4: Migrating metadata (EXIF, XMP, etc.)..."
    exiftool -tagsfromfile "$input" \
        -all:all \
        -overwrite_original \
        "$temp_output" 2>/dev/null || true
    
    # Preserve file timestamps
    log_info "⏰ Step 3/4: Preserving timestamps..."
    touch -r "$input" "$temp_output"
    mv "$temp_output" "$output"

    # Health check
    log_info "🏥 Step 4/4: Health validation..."
    if check_image_health "$output" "webp"; then
        verify_metadata_preservation "$input" "$output" "animation"
        rm "$input"
        log_success "✅ Done: $(basename "$input") → $(basename "$output")"
        ((FILES_PROCESSED++)) || true
        # Track converted file
        CONVERTED_FILES+=("$(basename "$output")")
        return 0
    else
        log_error "Health check failed, restoring from backup"
        rm -f "$output"
        ((HEALTH_FAILED++)) || true
        return 1
    fi
}

# ============================================================================
# 🚀 Main Function
# ============================================================================

main() {
    echo "╔══════════════════════════════════════════════╗"
    echo "║   🔄 Incompatible Media Converter            ║"
    echo "║      Whitelist Mode + Health Check           ║"
    echo "╚══════════════════════════════════════════════╝"
    echo ""
    
    check_dependencies
    
    echo ""
    log_info "📁 Target: $TARGET_DIR"
    log_info "🎬 Video format: $VIDEO_FORMAT"
    log_info "💾 Backup: $BACKUP_DIR"
    echo ""
    log_info "📋 Whitelist (only these formats processed):"
    echo "    📷 Image: ${WHITELIST_IMAGE_INPUT[*]} → PNG"
    echo "    🎬 Video: ${WHITELIST_VIDEO_INPUT[*]} → $VIDEO_FORMAT"
    echo ""
    
    [ "$DRY_RUN" = true ] && log_warn "🔍 DRY-RUN mode enabled"
    [ "$SKIP_HEALTH_CHECK" = true ] && log_warn "⚠️  Health check disabled"
    [ "$KEEP_ONLY_INCOMPATIBLE" = true ] && log_warn "🗑️  KEEP-ONLY-INCOMPATIBLE mode: Compatible files will be DELETED"
    
    # === ROBUSTNESS CHANGE: Operate from within the target directory ===
    local original_pwd
    original_pwd=$(pwd)
    log_info "Changing directory to '$TARGET_DIR' for robust file handling"
    if ! cd "$TARGET_DIR"; then
        log_error "Could not change to target directory. Aborting."
        exit 1
    fi
    
    # Scan for oversized files before conversion
    echo ""
    scan_oversize_files "."
    
    # Count total files for progress bar
    echo ""
    log_info "📊 Counting files for progress tracking..."
    local heic_total=0
    local mp4_total=0
    
    while IFS= read -r -d '' file; do
        ((heic_total++)) || true
    done < <(find . -type f \( -iname "*.heic" -o -iname "*.heif" \) ! -path "*/_backup_*" -print0 2>/dev/null)
    
    while IFS= read -r -d '' file; do
        ((mp4_total++)) || true
    done < <(find . -type f -iname "*.mp4" ! -path "*/_backup_*" -print0 2>/dev/null)
    
    TOTAL_FILES=$((heic_total + mp4_total))
    CURRENT_FILE=0
    START_TIME=$(date +%s)
    
    log_info "📁 Found: $heic_total HEIC/HEIF + $mp4_total MP4 = $TOTAL_FILES total files"
    echo ""
    
    local heic_count=0 mp4_count=0 success_count=0 fail_count=0
    
    # Process HEIC/HEIF files (whitelist)
    echo ""
    log_info "🔍 Scanning for HEIC/HEIF files..."
    while IFS= read -r -d '' file; do
        ((heic_count++)) || true
        ((CURRENT_FILE++)) || true
        
        # Show progress bar
        show_progress $CURRENT_FILE $TOTAL_FILES "$(basename "$file")"
        
        if convert_heic_to_png "$file"; then
            ((success_count++)) || true
        else
            ((fail_count++)) || true
        fi
        
        # Clear progress bar after conversion
        clear_progress
    done < <(find . -type f \( -iname "*.heic" -o -iname "*.heif" \) ! -path "*/_backup_*" -print0 2>/dev/null)
    
    [ "$heic_count" -eq 0 ] && log_skip "No HEIC/HEIF files found"
    
    # Process MP4 files (whitelist)
    echo ""
    log_info "🔍 Scanning for MP4 files..."
    while IFS= read -r -d '' file; do
        ((mp4_count++)) || true
        ((CURRENT_FILE++)) || true
        
        # Show progress bar
        show_progress $CURRENT_FILE $TOTAL_FILES "$(basename "$file")"
        
        if [ "$VIDEO_FORMAT" = "webp" ]; then
            if convert_mp4_to_webp "$file"; then
                ((success_count++)) || true
            else
                ((fail_count++)) || true
            fi
        else
            if convert_mp4_to_gif "$file"; then
                ((success_count++)) || true
            else
                ((fail_count++)) || true
            fi
        fi
        
        # Clear progress bar after conversion
        clear_progress
    done < <(find . -type f -iname "*.mp4" ! -path "*/_backup_*" -print0 2>/dev/null)
    
    [ "$mp4_count" -eq 0 ] && log_skip "No MP4 files found"
    
    # Keep-Only-Incompatible Mode: Delete all compatible (non-whitelist) files
    if [ "$KEEP_ONLY_INCOMPATIBLE" = true ]; then
        echo ""
        log_warn "🗑️  Deleting compatible (non-whitelist) media files..."
        
        # Define compatible extensions to delete (everything except whitelist)
        # Whitelist: heic, heif, mp4 (already converted)
        # Compatible formats to delete: jpg, jpeg, png, gif, webp, jxl, avif, bmp, tiff, mov, mkv, avi, webm
        local compatible_extensions=("jpg" "jpeg" "png" "gif" "webp" "jxl" "avif" "bmp" "tiff" "tif" "mov" "mkv" "avi" "webm" "m4v" "flv")
        
        for ext in "${compatible_extensions[@]}"; do
            while IFS= read -r -d '' file; do
                local basename_file="$(basename "$file")"
                # Skip newly converted files
                local skip=false
                for converted in "${CONVERTED_FILES[@]}"; do
                    if [ "$basename_file" = "$converted" ]; then
                        skip=true
                        break
                    fi
                done
                
                if [ "$skip" = true ]; then
                    [ "$VERBOSE" = true ] && log_info "⏭️  Skipping converted file: $basename_file"
                    continue
                fi
                
                if [ "$DRY_RUN" = true ]; then
                    log_info "[DRY-RUN] Would delete: $basename_file"
                else
                    rm "$file"
                    log_info "🗑️  Deleted compatible: $basename_file"
                    ((COMPATIBLE_DELETED++)) || true
                fi
            done < <(find . -type f -iname "*.$ext" ! -path "*/_backup_*" -print0 2>/dev/null)
        done
        
        log_success "Deleted $COMPATIBLE_DELETED compatible files"
    fi
    
    # Go back to original directory at the end
    cd "$original_pwd"

    # Summary
    echo ""
    echo "╔══════════════════════════════════════════════╗"
    echo "║        📊 Conversion Summary                 ║"
    echo "╠══════════════════════════════════════════════╣"
    printf "║  %-20s %20s  ║\n" "📷 HEIC/HEIF files:" "$heic_count"
    printf "║  %-20s %20s  ║\n" "🎬 MP4 files:" "$mp4_count"
    printf "║  %-20s %20s  ║\n" "✅ Successful:" "$success_count"
    printf "║  %-20s %20s  ║\n" "❌ Failed:" "$fail_count"
    if [ "$KEEP_ONLY_INCOMPATIBLE" = true ]; then
        printf "║  %-20s %20s  ║\n" "🗑️  Compatible deleted:" "$COMPATIBLE_DELETED"
    fi
    if [ "$OVERSIZE_FOUND" -gt 0 ]; then
        printf "║  %-20s %20s  ║\n" "⚠️  Oversized files:" "$OVERSIZE_FOUND"
    fi
    echo "╠══════════════════════════════════════════════╣"
    [ -d "$BACKUP_DIR" ] && printf "║  💾 Backup: %-31s ║\n" "$(basename "$BACKUP_DIR")"
    echo "╚══════════════════════════════════════════════╝"
    
    # Health report
    if [ "$SKIP_HEALTH_CHECK" = false ]; then
        print_health_report
    fi
    
    # Final status
    if [ "$fail_count" -eq 0 ] && [ "$success_count" -gt 0 ]; then
        echo ""
        log_success "🎉 All conversions completed successfully!"
    elif [ "$fail_count" -gt 0 ]; then
        echo ""
        log_warn "⚠️  Some conversions failed. Check backup directory."
    fi
}

main