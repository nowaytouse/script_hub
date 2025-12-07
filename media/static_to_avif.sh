#!/bin/bash

# ============================================================================
# 🖼️ Static Image to AVIF Converter (using avifenc)
# ============================================================================
#
# Converts static images to AVIF using the official avifenc tool.
# Provides high quality lossy compression with excellent metadata preservation.
#
# Features:
#   ✅ Uses official avifenc (libavif) for best quality
#   ✅ Quality 95 (high quality lossy)
#   ✅ Skips animated images (use animated_to_avif.sh for those)
#   ✅ Skips already modern formats (AVIF, JXL, HEIC)
#   ✅ Metadata preservation via exiftool
#   ✅ System timestamp preservation
#   ✅ In-place conversion mode
#
# Dependencies:
#   - avifenc (brew install libavif)
#   - exiftool (brew install exiftool)
#   - ffprobe (for animation detection)
#
# Usage:
#   ./static_to_avif.sh /path/to/images
#   ./static_to_avif.sh --in-place /path/to/images
#   ./static_to_avif.sh --quality 90 /path/to/images
#
# ============================================================================

# Configuration
IN_PLACE=false
TARGET_DIR=""
QUALITY=95      # AVIF 质量 (0-100)
SPEED=6         # 编码速度 (0-10, 6 是平衡点)
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

# Get frame count using ffprobe
get_frame_count() {
    local file="$1"
    ffprobe -v error -select_streams v:0 -count_packets -show_entries stream=nb_read_packets -of csv=p=0 "$file" 2>/dev/null
}

# Check if file is animated
is_animated() {
    local file="$1"
    local ext="${file##*.}"
    ext=$(echo "$ext" | tr '[:upper:]' '[:lower:]')
    
    # GIF 和 APNG 需要检查帧数
    case "$ext" in
        gif|apng)
            local frames=$(get_frame_count "$file")
            [ -n "$frames" ] && [ "$frames" -gt 1 ]
            return $?
            ;;
        webp)
            # WebP 动画检测
            local frames=$(get_frame_count "$file")
            [ -n "$frames" ] && [ "$frames" -gt 1 ]
            return $?
            ;;
        *)
            return 1
            ;;
    esac
}

# Check if format is already modern
is_modern_format() {
    local file="$1"
    local ext="${file##*.}"
    ext=$(echo "$ext" | tr '[:upper:]' '[:lower:]')
    
    case "$ext" in
        avif|jxl|heic|heif)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

# Health check function
check_avif_health() {
    local file="$1"
    
    # 使用 avifenc 的 avifdec 验证
    if command -v avifdec &> /dev/null; then
        if ! avifdec --info "$file" > /dev/null 2>&1; then
            log_error "Invalid AVIF: $(basename "$file")"
            return 1
        fi
    fi
    
    # 使用 ffprobe 验证
    if command -v ffprobe &> /dev/null; then
        local probe
        probe=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=p=0 "$file" 2>&1)
        if [ $? -ne 0 ] || [ -z "$probe" ]; then
            log_error "Cannot read AVIF structure: $(basename "$file")"
            return 1
        fi
        
        IFS=',' read -r width height <<< "$probe"
        if [ -z "$width" ] || [ -z "$height" ] || [ "$width" -lt 1 ] || [ "$height" -lt 1 ]; then
            log_error "Invalid AVIF dimensions: $(basename "$file")"
            return 1
        fi
        
        local size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
        log_health "✅ Valid AVIF: $(basename "$file") (${width}x${height}, ${size} bytes)"
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
        --quality)
            shift
            QUALITY="$1"
            shift
            ;;
        --quality=*)
            QUALITY="${arg#*=}"
            shift
            ;;
        --speed)
            shift
            SPEED="$1"
            shift
            ;;
        --speed=*)
            SPEED="${arg#*=}"
            shift
            ;;
        -h|--help)
            echo "🖼️ Static Image to AVIF Converter (avifenc)"
            echo ""
            echo "Usage: $0 [options] <target_directory>"
            echo ""
            echo "Options:"
            echo "  --in-place          Replace original files after conversion"
            echo "  --quality <0-100>   AVIF quality (default: 95)"
            echo "  --speed <0-10>      Encoding speed, 0=slowest/best (default: 6)"
            echo "  -h, --help          Show this help"
            echo ""
            echo "Supported formats: PNG, JPEG, TIFF, BMP, WebP (static)"
            echo "Skipped formats: AVIF, JXL, HEIC, HEIF (already modern)"
            exit 0
            ;;
        *)
            TARGET_DIR="$arg"
            ;;
    esac
done

# Check dependencies
if ! command -v avifenc &> /dev/null; then
    log_error "avifenc not found. Install: brew install libavif"
    exit 1
fi

if ! command -v exiftool &> /dev/null; then
    log_warn "exiftool not found. Metadata will not be preserved."
fi

if [ -z "$TARGET_DIR" ]; then
    log_error "No target directory specified"
    echo "Usage: $0 [--in-place] [--quality <0-100>] <target_directory>"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    log_error "Directory does not exist: $TARGET_DIR"
    exit 1
fi

echo "╔══════════════════════════════════════════════════════╗"
echo "║   🖼️ Static Image to AVIF Converter (avifenc)        ║"
echo "╚══════════════════════════════════════════════════════╝"
echo ""
log_info "📁 Target: $TARGET_DIR"
log_info "🎨 Quality: ${QUALITY}"
log_info "⚡ Speed: ${SPEED}"
log_info "📋 Supported: PNG, JPEG, TIFF, BMP, WebP (static) → AVIF"
log_info "⏭️  Skipped: AVIF, JXL, HEIC, HEIF, Animated images"
[ "$IN_PLACE" = true ] && log_warn "🔄 In-place mode: originals will be replaced"
echo ""

# Main processing
find "$TARGET_DIR" -type f \( \
    -iname "*.png" -o -iname "*.jpg" -o -iname "*.jpeg" -o \
    -iname "*.tiff" -o -iname "*.tif" -o -iname "*.bmp" -o \
    -iname "*.webp" -o -iname "*.gif" \
\) -print0 | while IFS= read -r -d $'\0' file; do
    
    # 跳过现代格式
    if is_modern_format "$file"; then
        log_info "⏭️  Skip modern format: $(basename "$file")"
        ((SKIPPED++)) || true
        continue
    fi
    
    # 跳过动画
    if is_animated "$file"; then
        log_info "⏭️  Skip animated: $(basename "$file") (use animated_to_avif.sh)"
        ((SKIPPED++)) || true
        continue
    fi
    
    echo "──────────────────────────────────────────────"
    log_info "🖼️ Processing: $(basename "$file")"
    
    output_avif="${file%.*}.avif"
    
    if [ "$IN_PLACE" = true ]; then
        temp_avif="${file}.avif.tmp"
        
        log_info "🔄 Step 1/4: Converting to AVIF (Q${QUALITY}, Speed ${SPEED})..."
        
        # 使用 avifenc 转换
        # --min 和 --max 控制质量范围，--speed 控制编码速度
        # Quality 95 → min=5, max=10 (大约)
        min_q=$((100 - QUALITY))
        max_q=$((min_q + 10))
        [ $max_q -gt 63 ] && max_q=63
        
        avifenc --min "$min_q" --max "$max_q" --speed "$SPEED" \
            --jobs all "$file" "$temp_avif" 2>&1
        
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
        
        log_info "🔄 Step 1/3: Converting to AVIF (Q${QUALITY}, Speed ${SPEED})..."
        
        min_q=$((100 - QUALITY))
        max_q=$((min_q + 10))
        [ $max_q -gt 63 ] && max_q=63
        
        avifenc --min "$min_q" --max "$max_q" --speed "$SPEED" \
            --jobs all "$file" "$output_avif" 2>&1
        
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
