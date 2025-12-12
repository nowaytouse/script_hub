#!/opt/homebrew/bin/bash

# ============================================================================
# 🎬 Short Animated Image to AV1 Video Converter (< 3 seconds)
# ============================================================================
#
# Converts short animated images (GIF, APNG, WebP) under 3 seconds to 
# high-quality AV1 video (MP4 container) using SVT-AV1.
#
# Features:
#   ✅ Duration check: Only processes animations < 3 seconds
#   ✅ AV1 output (SVT-AV1) - fast and high quality
#   ✅ Quality 95 equivalent (CRF ~3)
#   ✅ Metadata preservation via exiftool
#   ✅ System timestamp preservation
#   ✅ In-place conversion mode
#
# Dependencies:
#   - ffmpeg with libsvtav1 support
#   - exiftool (brew install exiftool)
#
# Usage:
#   ./animated_to_av1.sh /path/to/images
#   ./animated_to_av1.sh --in-place /path/to/images
#   ./animated_to_av1.sh --max-duration 5 /path/to/images
#
# ============================================================================

# Configuration
IN_PLACE=false
TARGET_DIR=""
MAX_DURATION=3
QUALITY=95
HEALTH_PASSED=0
HEALTH_FAILED=0
CONVERTED=0
SKIPPED=0

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info()    { echo -e "${BLUE}ℹ️  [INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}✅ [OK]${NC} $1"; }
log_warn()    { echo -e "${YELLOW}⚠️  [WARN]${NC} $1"; }
log_error()   { echo -e "${RED}❌ [ERROR]${NC} $1"; }
log_health()  { echo -e "${CYAN}🏥 [HEALTH]${NC} $1"; }

get_duration() {
    ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$1" 2>/dev/null
}

get_frame_count() {
    ffprobe -v error -select_streams v:0 -count_packets -show_entries stream=nb_read_packets -of csv=p=0 "$1" 2>/dev/null
}

is_animated() {
    local frames=$(get_frame_count "$1")
    [ -n "$frames" ] && [ "$frames" -gt 1 ]
}

check_video_health() {
    local file="$1"
    local probe=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height,codec_name,nb_frames -of csv=p=0 "$file" 2>&1)
    
    if [ $? -ne 0 ] || [ -z "$probe" ]; then
        log_error "Cannot read video: $(basename "$file")"
        return 1
    fi
    
    IFS=',' read -r codec width height frames <<< "$probe"
    local size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    log_health "✅ AV1 Video: $(basename "$file") (${width}x${height}, ${frames:-?} frames, ${size} bytes)"
    ((HEALTH_PASSED++)) || true
    return 0
}

# Parse arguments
for arg in "$@"; do
    case $arg in
        --in-place) IN_PLACE=true; shift ;;
        --max-duration) shift; MAX_DURATION="$1"; shift ;;
        --max-duration=*) MAX_DURATION="${arg#*=}"; shift ;;
        --quality) shift; QUALITY="$1"; shift ;;
        --quality=*) QUALITY="${arg#*=}"; shift ;;
        -h|--help)
            echo "🎬 Short Animated Image to AV1 Converter (SVT-AV1)"
            echo ""
            echo "Usage: $0 [options] <target_directory>"
            echo ""
            echo "Options:"
            echo "  --in-place              Replace original files"
            echo "  --max-duration <sec>    Max duration (default: 3)"
            echo "  --quality <0-100>       Quality (default: 95)"
            echo "  -h, --help              Show help"
            exit 0
            ;;
        *) TARGET_DIR="$arg" ;;
    esac
done

# Check dependencies
command -v ffmpeg &> /dev/null || { log_error "ffmpeg not found"; exit 1; }
command -v ffprobe &> /dev/null || { log_error "ffprobe not found"; exit 1; }

[ -z "$TARGET_DIR" ] && { log_error "No target directory"; exit 1; }
[ ! -d "$TARGET_DIR" ] && { log_error "Directory not found: $TARGET_DIR"; exit 1; }

echo "╔══════════════════════════════════════════════════════╗"
echo "║   🎬 Short Animated → AV1 Video (SVT-AV1)            ║"
echo "╚══════════════════════════════════════════════════════╝"
echo ""
log_info "📁 Target: $TARGET_DIR"
log_info "⏱️  Max Duration: ${MAX_DURATION}s"
log_info "🎨 Quality: ${QUALITY} (CRF ~$((63 - QUALITY * 60 / 100)))"
log_info "📋 Input: GIF, APNG, WebP (animated, < ${MAX_DURATION}s) → MP4/AV1"
[ "$IN_PLACE" = true ] && log_warn "🔄 In-place mode enabled"
echo ""

# Main processing
find "$TARGET_DIR" -type f \( -iname "*.gif" -o -iname "*.apng" -o -iname "*.webp" -o -iname "*.png" \) | while read -r file; do
    # Skip non-animated
    is_animated "$file" || continue
    
    # Get duration
    duration=$(get_duration "$file")
    [ -z "$duration" ] && { log_warn "⏭️  Skip (no duration): $(basename "$file")"; ((SKIPPED++)); continue; }
    
    # Check duration
    if (( $(echo "$duration > $MAX_DURATION" | bc -l) )); then
        log_info "⏭️  Skip (${duration}s > ${MAX_DURATION}s): $(basename "$file")"
        ((SKIPPED++)) || true
        continue
    fi
    
    echo "──────────────────────────────────────────────"
    log_info "🎬 Processing: $(basename "$file")"
    log_info "   Duration: ${duration}s"
    
    output_mp4="${file%.*}.mp4"
    crf=$((63 - QUALITY * 60 / 100))
    [ $crf -lt 0 ] && crf=0
    
    if [ "$IN_PLACE" = true ]; then
        temp_mp4="${file}.mp4.tmp"
        
        log_info "🔄 Converting to AV1 (CRF $crf)..."
        if ffmpeg -v warning -stats -i "$file" \
            -c:v libsvtav1 -crf "$crf" -preset 6 \
            -pix_fmt yuv420p -movflags +faststart -an \
            -y "$temp_mp4" 2>&1; then
            
            # Metadata
            command -v exiftool &> /dev/null && \
                exiftool -tagsfromfile "$file" -all:all -overwrite_original "$temp_mp4" > /dev/null 2>&1
            touch -r "$file" "$temp_mp4"
            mv "$temp_mp4" "$output_mp4"
            
            if check_video_health "$output_mp4"; then
                rm "$file"
                log_success "Done: $(basename "$file") → $(basename "$output_mp4")"
                ((CONVERTED++)) || true
            else
                rm -f "$output_mp4"
                ((HEALTH_FAILED++)) || true
            fi
        else
            log_error "Conversion failed"
            rm -f "$temp_mp4"
        fi
    else
        [ -f "$output_mp4" ] && { log_warn "⏭️  Skip (exists): $(basename "$output_mp4")"; ((SKIPPED++)); continue; }
        
        log_info "🔄 Converting to AV1 (CRF $crf)..."
        if ffmpeg -v warning -stats -i "$file" \
            -c:v libsvtav1 -crf "$crf" -preset 6 \
            -pix_fmt yuv420p -movflags +faststart -an \
            -y "$output_mp4" 2>&1; then
            
            command -v exiftool &> /dev/null && \
                exiftool -tagsfromfile "$file" -all:all -overwrite_original "$output_mp4" > /dev/null 2>&1
            touch -r "$file" "$output_mp4"
            
            check_video_health "$output_mp4" && ((CONVERTED++)) || ((HEALTH_FAILED++))
            log_success "Converted: $(basename "$output_mp4")"
        else
            log_error "Conversion failed"
        fi
    fi
done

echo ""
echo "╔══════════════════════════════════════════════════════╗"
echo "║   📊 Conversion Complete                             ║"
echo "╚══════════════════════════════════════════════════════╝"
echo ""
echo "📊 Summary:"
echo -e "   ✅ Converted: $CONVERTED"
echo -e "   ⏭️  Skipped:   $SKIPPED"
echo -e "   ❌ Failed:    $HEALTH_FAILED"
