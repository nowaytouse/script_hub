#!/bin/bash

# ============================================================================
# 🎬 Video to High-Quality GIF Converter with Health Check
# ============================================================================
#
# Batch converts video files to high-quality GIFs using two-pass method.
#
# Features:
#   ✅ Whitelist: Only processes .mp4, .mov, .mkv, .avi, .webm
#   ✅ Two-pass conversion (palette generation + dithering)
#   ✅ Advanced Bayer dithering for smooth color transitions
#   ✅ Health check validation after conversion
#   ✅ Customizable FPS and resolution
#   ✅ System timestamp preservation
#
# Dependencies:
#   - ffmpeg (brew install ffmpeg)
#
# Usage:
#   ./video_to_hq_gif.sh /path/to/videos
#   ./video_to_hq_gif.sh --delete-source /path/to/videos
#   ./video_to_hq_gif.sh -r 24 -s 720 /path/to/videos
#
# ============================================================================

# Configuration
DEFAULT_FPS=15
DEFAULT_SCALE=540

FPS=$DEFAULT_FPS
SCALE=$DEFAULT_SCALE
DELETE_SOURCE=false
TARGET_DIR=""
SKIP_HEALTH_CHECK=false
HEALTH_PASSED=0
HEALTH_FAILED=0

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BLUE='\033[0;34m'
NC='\033[0m'

# Logging
log_info()    { echo -e "${BLUE}ℹ️  [INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}✅ [OK]${NC} $1"; }
log_warn()    { echo -e "${YELLOW}⚠️  [WARN]${NC} $1"; }
log_error()   { echo -e "${RED}❌ [ERROR]${NC} $1"; }
log_health()  { echo -e "${CYAN}🏥 [HEALTH]${NC} $1"; }

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

# GIF health check function
check_gif_health() {
    local file="$1"
    [ "$SKIP_HEALTH_CHECK" = true ] && return 0
    
    local sig
    sig=$(head -c 6 "$file" 2>/dev/null)
    if [[ "$sig" != "GIF87a" && "$sig" != "GIF89a" ]]; then
        log_error "Invalid GIF signature: $(basename "$file")"
        return 1
    fi
    
    if command -v ffprobe &> /dev/null; then
        local probe
        probe=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0 "$file" 2>&1)
        if [ $? -ne 0 ] || [ -z "$probe" ]; then
            log_error "Cannot read GIF structure: $(basename "$file")"
            return 1
        fi
        
        # Get frame count
        local frames
        frames=$(ffprobe -v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 "$file" 2>/dev/null)
        if [ -n "$frames" ] && [ "$frames" -lt 2 ]; then
            log_warn "GIF has only $frames frame(s)"
        fi
    fi
    
    if command -v ffmpeg &> /dev/null; then
        if ! ffmpeg -v error -i "$file" -frames:v 1 -f null - 2>/dev/null; then
            log_error "Cannot decode GIF: $(basename "$file")"
            return 1
        fi
    fi
    
    local size
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    log_health "✅ Passed: $(basename "$file") ($size bytes)"
    ((HEALTH_PASSED++)) || true
    return 0
}

# Parse arguments
while (( "$#" )); do
    case "$1" in
        -r|--fps)
            if [ -n "$2" ] && ! [[ "$2" =~ ^- ]]; then
                FPS="$2"
                shift 2
            else
                log_error "-r/--fps requires a value"
                exit 1
            fi
            ;;
        -s|--scale)
            if [ -n "$2" ] && ! [[ "$2" =~ ^- ]]; then
                SCALE="$2"
                shift 2
            else
                log_error "-s/--scale requires a value"
                exit 1
            fi
            ;;
        --delete-source)
            DELETE_SOURCE=true
            shift
            ;;
        --skip-health-check)
            SKIP_HEALTH_CHECK=true
            shift
            ;;
        -h|--help)
            echo "🎬 Video to High-Quality GIF Converter"
            echo ""
            echo "Usage: $0 [options] <target_directory>"
            echo ""
            echo "Options:"
            echo "  -r, --fps <fps>      Output FPS (default: 15)"
            echo "  -s, --scale <width>  Output width (default: 540)"
            echo "  --delete-source      Delete original after conversion"
            echo "  --skip-health-check  Skip health validation"
            echo "  -h, --help           Show this help"
            exit 0
            ;;
        *)
            TARGET_DIR="$1"
            shift
            ;;
    esac
done

# Check dependencies
if ! command -v ffmpeg &> /dev/null; then
    log_error "ffmpeg not found. Install: brew install ffmpeg"
    exit 1
fi

if [ -z "$TARGET_DIR" ]; then
    log_error "No target directory specified"
    echo "Usage: $0 [-r fps] [-s width] [--delete-source] <target_directory>"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    log_error "Directory does not exist: $TARGET_DIR"
    exit 1
fi

# Safety check
if [ "$DELETE_SOURCE" = true ]; then
    REAL_TARGET_DIR=""
    if command -v realpath &> /dev/null; then
        REAL_TARGET_DIR=$(realpath "$TARGET_DIR")
    else
        REAL_TARGET_DIR=$(cd "$TARGET_DIR"; pwd)
    fi

    FORBIDDEN_PATHS=("/" "/etc" "/bin" "/usr" "/System" "/Library" "/Applications")
    for forbidden in "${FORBIDDEN_PATHS[@]}"; do
        if [ "$REAL_TARGET_DIR" = "$forbidden" ]; then
            log_error "🚫 SAFETY: Cannot operate on protected directory: $forbidden"
            exit 1
        fi
    done
fi

echo "╔══════════════════════════════════════════════╗"
echo "║   🎬 Video to High-Quality GIF Converter     ║"
echo "╚══════════════════════════════════════════════╝"
echo ""
log_info "📁 Target: $TARGET_DIR"
log_info "📋 Whitelist: .mp4, .mov, .mkv, .avi, .webm → .gif"
log_info "🎯 Settings: ${FPS} FPS, ${SCALE}px width"
log_info "🎨 Method: Two-pass with Bayer dithering"
[ "$DELETE_SOURCE" = true ] && log_warn "🗑️  Delete source mode: originals will be removed"
echo ""

# Count total files for progress bar
echo ""
log_info "📊 Counting files for progress tracking..."
local total_count=0

while IFS= read -r -d '' file; do
    ((total_count++)) || true
done < <(find "$TARGET_DIR" -type f \( -iname "*.mp4" -o -iname "*.mov" -o -iname "*.mkv" -o -iname "*.avi" -o -iname "*.webm" \) -print0 2>/dev/null)

TOTAL_FILES=$total_count
CURRENT_FILE=0
START_TIME=$(date +%s)

log_info "📁 Found: $TOTAL_FILES files"
echo ""

# Main processing
find "$TARGET_DIR" -type f \( -iname "*.mp4" -o -iname "*.mov" -o -iname "*.mkv" -o -iname "*.avi" -o -iname "*.webm" \) -print0 | while IFS= read -r -d $'\0' video_file; do
    ((CURRENT_FILE++)) || true
    show_progress $CURRENT_FILE $TOTAL_FILES "$(basename "$video_file")"
    echo "──────────────────────────────────────────────"
    log_info "🎬 Processing: $(basename "$video_file")"
    
    output_gif="${video_file%.*}.gif"

    if [ -f "$output_gif" ]; then
        log_warn "⏭️  Skip: $(basename "$output_gif") already exists"
        continue
    fi

    palette="/tmp/palette_$$.png"
    filters="fps=$FPS,scale=$SCALE:-1:flags=lanczos:force_original_aspect_ratio=disable"
    palette_filters="$filters,palettegen=stats_mode=diff"

    log_info "🎨 Step 1/3: Generating optimized palette..."
    ffmpeg -loglevel error -i "$video_file" -vf "$palette_filters" -y "$palette"
    
    if [ $? -ne 0 ]; then
        log_error "Palette generation failed"
        rm -f "$palette"
        continue
    fi

    log_info "🔄 Step 2/3: Creating GIF with Bayer dithering..."
    ffmpeg -loglevel warning -stats -i "$video_file" -i "$palette" \
        -lavfi "$filters [x]; [x][1:v]paletteuse=dither=bayer:bayer_scale=5" \
        -map_metadata 0 -y "$output_gif" 2>&1 | while read line; do
        if [[ "$line" =~ frame=.*fps=.*speed= ]]; then
            printf "\r  ▶️  $line"
        fi
    done
    printf "\n"

    rm -f "$palette"

    if [ $? -eq 0 ]; then
        touch -r "$video_file" "$output_gif"
        
        log_info "🏥 Step 3/3: Health validation..."
        if check_gif_health "$output_gif"; then
            log_success "Created: $(basename "$output_gif")"

            if [ "$DELETE_SOURCE" = true ]; then
                rm "$video_file"
                log_info "🗑️  Deleted source: $(basename "$video_file")"
            fi
        else
            log_warn "Health check failed, but file created"
            ((HEALTH_FAILED++)) || true
        fi
    else
        log_error "GIF creation failed: $(basename "$video_file")"
    fi
    fi
    
    clear_progress
done

echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║   📊 Conversion Complete                     ║"
echo "╚══════════════════════════════════════════════╝"

# Health report
if [ "$SKIP_HEALTH_CHECK" = false ]; then
    echo ""
    echo "🏥 Health Report:"
    echo -e "   ✅ Passed:  $HEALTH_PASSED"
    echo -e "   ❌ Failed:  $HEALTH_FAILED"
    total=$((HEALTH_PASSED + HEALTH_FAILED))
    if [ "$total" -gt 0 ]; then
        rate=$((HEALTH_PASSED * 100 / total))
        echo "   📊 Rate:    ${rate}%"
    fi
fi
