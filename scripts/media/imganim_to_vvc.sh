#!/bin/bash

# ============================================================================
# 🎬 Animated Image to H.266/VVC Video Converter with Health Check
# ============================================================================
#
# Batch converts animated images (GIF, APNG, WebP) to H.266/VVC video.
#
# Features:
#   ✅ Whitelist: Only processes image/gif, image/apng, image/webp (by MIME)
#   ✅ H.266/VVC encoding (libvvenc)
#   ✅ Health check validation after conversion
#   ✅ Metadata preservation via exiftool
#   ✅ System timestamp preservation
#   ✅ In-place conversion mode
#
# Dependencies:
#   - ffmpeg with libvvenc support
#   - exiftool (brew install exiftool)
#
# Usage:
#   ./imganim_to_vvc.sh /path/to/images
#   ./imganim_to_vvc.sh --in-place /path/to/images
#   ./imganim_to_vvc.sh --skip-health-check /path/to/images
#
# ============================================================================

# Configuration
IN_PLACE=false
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

# Video health check function
check_video_health() {
    local file="$1"
    [ "$SKIP_HEALTH_CHECK" = true ] && return 0
    
    if command -v ffprobe &> /dev/null; then
        local probe
        probe=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height,codec_name -of csv=p=0 "$file" 2>&1)
        if [ $? -ne 0 ] || [ -z "$probe" ]; then
            log_error "Cannot read video structure: $(basename "$file")"
            return 1
        fi
        
        IFS=',' read -r codec width height <<< "$probe"
        if [ -z "$width" ] || [ -z "$height" ] || [ "$width" -lt 1 ] || [ "$height" -lt 1 ]; then
            log_error "Invalid video dimensions: $(basename "$file")"
            return 1
        fi
    fi
    
    if command -v ffmpeg &> /dev/null; then
        if ! ffmpeg -v error -i "$file" -frames:v 1 -f null - 2>/dev/null; then
            log_error "Cannot decode video: $(basename "$file")"
            return 1
        fi
    fi
    
    local size
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    log_health "✅ Passed: $(basename "$file") ($size bytes, ${width}x${height})"
    ((HEALTH_PASSED++)) || true
    return 0
}

# Parse arguments
for arg in "$@"; do
    case $arg in
        --in-place)
            IN_PLACE=true
            shift
            ;;
        --skip-health-check)
            SKIP_HEALTH_CHECK=true
            shift
            ;;
        -h|--help)
            echo "🎬 Animated Image to H.266/VVC Converter"
            echo ""
            echo "Usage: $0 [options] <target_directory>"
            echo ""
            echo "Options:"
            echo "  --in-place           Replace original files after conversion"
            echo "  --skip-health-check  Skip health validation (not recommended)"
            echo "  -h, --help           Show this help"
            exit 0
            ;;
        *)
            TARGET_DIR="$arg"
            ;;
    esac
done

# Check dependencies
if ! command -v ffmpeg &> /dev/null; then
    log_error "ffmpeg not found. Install: brew install ffmpeg"
    exit 1
fi

if ! command -v exiftool &> /dev/null; then
    log_error "exiftool not found. Install: brew install exiftool"
    exit 1
fi

if [ -z "$TARGET_DIR" ]; then
    log_error "No target directory specified"
    echo "Usage: $0 [--in-place] [--skip-health-check] <target_directory>"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    log_error "Directory does not exist: $TARGET_DIR"
    exit 1
fi

# Safety check
if [ "$IN_PLACE" = true ]; then
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
echo "║   🎬 Animated Image to H.266/VVC Converter   ║"
echo "╚══════════════════════════════════════════════╝"
echo ""
log_info "📁 Target: $TARGET_DIR"
log_info "📋 Whitelist: GIF, APNG, WebP (by MIME type) → MP4/H.266"
log_info "🎯 Codec: libvvenc (CRF 28)"
[ "$IN_PLACE" = true ] && log_warn "🔄 In-place mode: originals will be replaced"
echo ""

# Count total files for progress bar
echo ""
log_info "📊 Counting files for progress tracking..."
local total_count=0

while IFS= read -r -d '' file; do
    MIME_TYPE=$(exiftool -MIMEType -b "$file" 2>/dev/null)
    case "$MIME_TYPE" in
        "image/gif"|"image/apng"|"image/webp")
            ((total_count++)) || true
            ;;
    esac
done < <(find "$TARGET_DIR" -type f -print0)

TOTAL_FILES=$total_count
CURRENT_FILE=0
START_TIME=$(date +%s)

log_info "📁 Found: $TOTAL_FILES animated images"
echo ""

# Main processing - scan by MIME type
find "$TARGET_DIR" -type f -print0 | while IFS= read -r -d $'\0' file; do
    MIME_TYPE=$(exiftool -MIMEType -b "$file" 2>/dev/null)

    case "$MIME_TYPE" in
        "image/gif"|"image/apng"|"image/webp")
            ((CURRENT_FILE++)) || true
            show_progress $CURRENT_FILE $TOTAL_FILES "$(basename "$file")"
            echo "──────────────────────────────────────────────"
            log_info "🎬 Found animated image: $(basename "$file")"
            log_info "   MIME: $MIME_TYPE"
            
            output_mp4="${file%.*}.mp4"

            if [ "$IN_PLACE" = true ]; then
                temp_mp4="${file}.mp4.tmp"
                log_info "🔄 Step 1/4: Converting to H.266/VVC..."
                ffmpeg -v warning -stats -i "$file" -c:v libvvenc -crf 28 -pix_fmt yuv420p -y "$temp_mp4" 2>&1 | while read line; do
                    if [[ "$line" =~ frame=.*fps=.*speed= ]]; then
                        printf "\r  ▶️  $line"
                    fi
                done
                printf "\n"
                
                if [ $? -eq 0 ]; then
                    log_info "📋 Step 2/4: Migrating metadata..."
                    exiftool -tagsfromfile "$file" -all:all -overwrite_original "$temp_mp4" > /dev/null 2>&1
                    
                    log_info "⏰ Step 3/4: Preserving timestamps..."
                    touch -r "$file" "$temp_mp4"
                    mv "$temp_mp4" "$output_mp4"
                    
                    log_info "🏥 Step 4/4: Health validation..."
                    if check_video_health "$output_mp4"; then
                        rm "$file"
                        log_success "Done: $(basename "$file") → $(basename "$output_mp4")"
                    else
                        log_error "Health check failed, keeping original"
                        rm -f "$output_mp4"
                        ((HEALTH_FAILED++)) || true
                    fi
                else
                    log_error "Conversion failed: $(basename "$file")"
                    rm -f "$temp_mp4"
                fi
            else
                if [ -f "$output_mp4" ]; then
                    log_warn "⏭️  Skip: $(basename "$output_mp4") already exists"
                    continue
                fi

                log_info "🔄 Step 1/3: Converting to H.266/VVC..."
                ffmpeg -v warning -stats -i "$file" -c:v libvvenc -crf 28 -pix_fmt yuv420p -y "$output_mp4" 2>&1 | while read line; do
                    if [[ "$line" =~ frame=.*fps=.*speed= ]]; then
                        printf "\r  ▶️  $line"
                    fi
                done
                printf "\n"
                
                if [ $? -eq 0 ]; then
                    log_info "📋 Step 2/3: Migrating metadata..."
                    exiftool -tagsfromfile "$file" -all:all -overwrite_original "$output_mp4" > /dev/null 2>&1
                    touch -r "$file" "$output_mp4"
                    
                    log_info "🏥 Step 3/3: Health validation..."
                    if check_video_health "$output_mp4"; then
                        log_success "Converted: $(basename "$output_mp4")"
                    else
                        log_warn "Health check failed, but file created"
                        ((HEALTH_FAILED++)) || true
                    fi
                else
                    log_error "Conversion failed: $(basename "$file")"
                fi
            fi
            ;;
    esac
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
