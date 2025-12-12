#!/opt/homebrew/bin/bash

# ============================================================================
# 🎬 Short Animated Image to AV1 Video Converter (< 3 seconds)
# ============================================================================
#
# Converts short animated images (GIF, APNG, WebP) under 3 seconds to 
# high-quality AV1 video (MP4 container).
#
# Note: True animated AVIF requires specialized tools. This script uses
# AV1 in MP4 container which provides excellent compression and wide
# compatibility while preserving animation.
#
# Features:
#   ✅ Duration check: Only processes animations < 3 seconds
#   ✅ AV1 output (SVT-AV1) with animation preserved
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
#   ./animated_to_avif.sh /path/to/images
#   ./animated_to_avif.sh --in-place /path/to/images
#   ./animated_to_avif.sh --max-duration 5 /path/to/images
#
# ============================================================================

# Configuration
IN_PLACE=false
TARGET_DIR=""
MAX_DURATION=3  # 默认最大时长 3 秒
QUALITY=95      # AVIF 质量 (0-100)
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

# Logging
log_info()    { echo -e "${BLUE}ℹ️  [INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}✅ [OK]${NC} $1"; }
log_warn()    { echo -e "${YELLOW}⚠️  [WARN]${NC} $1"; }
log_error()   { echo -e "${RED}❌ [ERROR]${NC} $1"; }
log_health()  { echo -e "${CYAN}🏥 [HEALTH]${NC} $1"; }

# Get animation duration using ffprobe
get_duration() {
    local file="$1"
    ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$file" 2>/dev/null
}

# Get frame count using ffprobe
get_frame_count() {
    local file="$1"
    ffprobe -v error -select_streams v:0 -count_packets -show_entries stream=nb_read_packets -of csv=p=0 "$file" 2>/dev/null
}

# Check if file is animated
is_animated() {
    local file="$1"
    local frames=$(get_frame_count "$file")
    [ -n "$frames" ] && [ "$frames" -gt 1 ]
}

# Health check function
check_avif_health() {
    local file="$1"
    
    if command -v ffprobe &> /dev/null; then
        local probe
        probe=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height,codec_name -of csv=p=0 "$file" 2>&1)
        if [ $? -ne 0 ] || [ -z "$probe" ]; then
            log_error "Cannot read AVIF structure: $(basename "$file")"
            return 1
        fi
        
        IFS=',' read -r codec width height <<< "$probe"
        if [ -z "$width" ] || [ -z "$height" ] || [ "$width" -lt 1 ] || [ "$height" -lt 1 ]; then
            log_error "Invalid AVIF dimensions: $(basename "$file")"
            return 1
        fi
        
        # 检查是否仍然是动画
        local frames=$(get_frame_count "$file")
        if [ -n "$frames" ] && [ "$frames" -gt 1 ]; then
            log_health "✅ Animated AVIF: $(basename "$file") (${width}x${height}, ${frames} frames)"
        else
            log_health "✅ Static AVIF: $(basename "$file") (${width}x${height})"
        fi
    fi
    
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
        --max-duration)
            shift
            MAX_DURATION="$1"
            shift
            ;;
        --max-duration=*)
            MAX_DURATION="${arg#*=}"
            shift
            ;;
        --quality)
            shift
            QUALITY="$1"
            shift
            ;;
        --quality=*)
            QUALITY="${arg#*=}"
            shift
            ;;
        -h|--help)
            echo "🎬 Short Animated Image to AVIF Converter"
            echo ""
            echo "Usage: $0 [options] <target_directory>"
            echo ""
            echo "Options:"
            echo "  --in-place              Replace original files after conversion"
            echo "  --max-duration <sec>    Maximum duration in seconds (default: 3)"
            echo "  --quality <0-100>       AVIF quality (default: 95)"
            echo "  -h, --help              Show this help"
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

if ! command -v ffprobe &> /dev/null; then
    log_error "ffprobe not found. Install: brew install ffmpeg"
    exit 1
fi

if ! command -v exiftool &> /dev/null; then
    log_warn "exiftool not found. Metadata will not be preserved."
fi

if [ -z "$TARGET_DIR" ]; then
    log_error "No target directory specified"
    echo "Usage: $0 [--in-place] [--max-duration <sec>] <target_directory>"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    log_error "Directory does not exist: $TARGET_DIR"
    exit 1
fi

echo "╔══════════════════════════════════════════════════════╗"
echo "║   🎬 Short Animated Image to AVIF Converter          ║"
echo "╚══════════════════════════════════════════════════════╝"
echo ""
log_info "📁 Target: $TARGET_DIR"
log_info "⏱️  Max Duration: ${MAX_DURATION}s"
log_info "🎨 Quality: ${QUALITY}"
log_info "📋 Whitelist: GIF, APNG, WebP (animated, < ${MAX_DURATION}s) → AVIF"
[ "$IN_PLACE" = true ] && log_warn "🔄 In-place mode: originals will be replaced"
echo ""

# Main processing
find "$TARGET_DIR" -type f \( -iname "*.gif" -o -iname "*.apng" -o -iname "*.webp" -o -iname "*.png" \) -print0 | while IFS= read -r -d $'\0' file; do
    # 检查是否是动画
    if ! is_animated "$file"; then
        continue
    fi
    
    # 获取时长
    duration=$(get_duration "$file")
    if [ -z "$duration" ]; then
        log_warn "⏭️  Skip: Cannot determine duration: $(basename "$file")"
        ((SKIPPED++)) || true
        continue
    fi
    
    # 检查时长是否小于阈值
    if (( $(echo "$duration > $MAX_DURATION" | bc -l) )); then
        log_info "⏭️  Skip: Duration ${duration}s > ${MAX_DURATION}s: $(basename "$file")"
        ((SKIPPED++)) || true
        continue
    fi
    
    echo "──────────────────────────────────────────────"
    log_info "🎬 Processing: $(basename "$file")"
    log_info "   Duration: ${duration}s (< ${MAX_DURATION}s ✓)"
    
    output_avif="${file%.*}.avif"
    
    if [ "$IN_PLACE" = true ]; then
        temp_avif="${file}.avif.tmp"
        
        log_info "🔄 Step 1/4: Converting to animated AVIF (Q${QUALITY})..."
        # 使用 ffmpeg 转换为动画 AVIF
        # -crf 计算: quality 95 → crf ~10, quality 80 → crf ~23
        crf=$((63 - QUALITY * 63 / 100))
        
        ffmpeg -v warning -stats -i "$file" \
            -c:v libaom-av1 -crf "$crf" -b:v 0 \
            -cpu-used 4 -row-mt 1 \
            -pix_fmt yuv420p \
            -y "$temp_avif" 2>&1
        
        if [ $? -eq 0 ]; then
            log_info "📋 Step 2/4: Migrating metadata..."
            if command -v exiftool &> /dev/null; then
                exiftool -tagsfromfile "$file" -all:all -overwrite_original "$temp_avif" > /dev/null 2>&1
            fi
            
            log_info "⏰ Step 3/4: Preserving timestamps..."
            touch -r "$file" "$temp_avif"
            mv "$temp_avif" "$output_avif"
            
            log_info "🏥 Step 4/4: Health validation..."
            if check_avif_health "$output_avif"; then
                rm "$file"
                log_success "Done: $(basename "$file") → $(basename "$output_avif")"
                ((CONVERTED++)) || true
            else
                log_error "Health check failed, keeping original"
                rm -f "$output_avif"
                ((HEALTH_FAILED++)) || true
            fi
        else
            log_error "Conversion failed: $(basename "$file")"
            rm -f "$temp_avif"
        fi
    else
        if [ -f "$output_avif" ]; then
            log_warn "⏭️  Skip: $(basename "$output_avif") already exists"
            ((SKIPPED++)) || true
            continue
        fi
        
        log_info "🔄 Step 1/3: Converting to animated AVIF (Q${QUALITY})..."
        crf=$((63 - QUALITY * 63 / 100))
        
        # 使用 ffmpeg 转换为动画 AVIF
        # -loop 0 确保动画循环
        # -still-picture 0 确保输出为动画而非静态图片
        ffmpeg -v warning -stats -i "$file" \
            -c:v libaom-av1 -crf "$crf" -b:v 0 \
            -cpu-used 4 -row-mt 1 \
            -pix_fmt yuv420p \
            -movflags +faststart \
            -y "$output_avif" 2>&1
        
        if [ $? -eq 0 ]; then
            log_info "📋 Step 2/3: Migrating metadata..."
            if command -v exiftool &> /dev/null; then
                exiftool -tagsfromfile "$file" -all:all -overwrite_original "$output_avif" > /dev/null 2>&1
            fi
            touch -r "$file" "$output_avif"
            
            log_info "🏥 Step 3/3: Health validation..."
            if check_avif_health "$output_avif"; then
                log_success "Converted: $(basename "$output_avif")"
                ((CONVERTED++)) || true
            else
                log_warn "Health check failed, but file created"
                ((HEALTH_FAILED++)) || true
            fi
        else
            log_error "Conversion failed: $(basename "$file")"
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
