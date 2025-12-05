#!/bin/bash

# ============================================================================
# 📷 JPEG to JXL Converter - High Quality with Health Check
# ============================================================================
#
# Batch converts JPEG images to high-quality JXL format.
#
# Features:
#   ✅ Whitelist: Only processes .jpg, .jpeg files
#   ✅ High-quality lossy compression (-d 1)
#   ✅ Health check validation after conversion
#   ✅ System timestamp preservation
#   ✅ In-place conversion mode
#
# Dependencies:
#   - cjxl/djxl (brew install jpeg-xl)
#   - ffprobe (brew install ffmpeg) - for health check
#
# Usage:
#   ./jpeg_to_jxl.sh /path/to/images
#   ./jpeg_to_jxl.sh --in-place /path/to/images
#   ./jpeg_to_jxl.sh --skip-health-check /path/to/images
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

# JXL health check function
check_jxl_health() {
    local file="$1"
    [ "$SKIP_HEALTH_CHECK" = true ] && return 0
    
    # Check JXL signature (0xFF0A or ISOBMFF container)
    local sig
    sig=$(xxd -l 2 -p "$file" 2>/dev/null)
    if [[ "$sig" != "ff0a" && "$sig" != "0000" ]]; then
        log_error "Invalid JXL signature: $(basename "$file")"
        return 1
    fi
    
    # Try djxl decode test
    if command -v djxl &> /dev/null; then
        if ! djxl "$file" /dev/null 2>/dev/null; then
            log_error "Cannot decode JXL: $(basename "$file")"
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
            echo "📷 JPEG to JXL Converter"
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
if ! command -v cjxl &> /dev/null; then
    log_error "cjxl not found. Install: brew install jpeg-xl"
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
echo "║   📷 JPEG to JXL Converter                   ║"
echo "╚══════════════════════════════════════════════╝"
echo ""
log_info "📁 Target: $TARGET_DIR"
log_info "📋 Whitelist: .jpg, .jpeg → .jxl"
log_info "🎯 Quality: High (-d 1)"
[ "$IN_PLACE" = true ] && log_warn "🔄 In-place mode: originals will be replaced"
echo ""

# Count total files for progress bar
echo ""
log_info "📊 Counting files for progress tracking..."
local total_count=0

while IFS= read -r -d '' file; do
    ((total_count++)) || true
done < <(find "$TARGET_DIR" -type f \( -iname "*.jpg" -o -iname "*.jpeg" \) -print0 2>/dev/null)

TOTAL_FILES=$total_count
CURRENT_FILE=0
START_TIME=$(date +%s)

log_info "📁 Found: $TOTAL_FILES files"
echo ""

# Main processing
find "$TARGET_DIR" -type f \( -iname "*.jpg" -o -iname "*.jpeg" \) -print0 | while IFS= read -r -d $'\0' jpeg_file; do
    ((CURRENT_FILE++)) || true
    show_progress $CURRENT_FILE $TOTAL_FILES "$(basename "$jpeg_file")"
    echo "──────────────────────────────────────────────"
    log_info "📷 Processing: $(basename "$jpeg_file")"
    
    output_jxl="${jpeg_file%.*}.jxl"

    if [ "$IN_PLACE" = true ]; then
        temp_jxl="${jpeg_file}.jxl.tmp"
        log_info "🔄 Step 1/4: Converting (high quality -d 1)..."
        cjxl "$jpeg_file" "$temp_jxl" -d 1 > /dev/null 2>&1
        
        if [ $? -eq 0 ]; then
            log_info "📋 Step 2/4: Migrating metadata (EXIF, XMP, IPTC)..."
            exiftool -tagsfromfile "$jpeg_file" -all:all -overwrite_original "$temp_jxl" > /dev/null 2>&1 || true
            
            log_info "⏰ Step 3/4: Preserving timestamps..."
            touch -r "$jpeg_file" "$temp_jxl"
            mv "$temp_jxl" "$output_jxl"
            
            log_info "🏥 Step 4/4: Health validation..."
            if check_jxl_health "$output_jxl"; then
                rm "$jpeg_file"
                log_success "Done: $(basename "$jpeg_file") → $(basename "$output_jxl")"
            else
                log_error "Health check failed, keeping original"
                rm -f "$output_jxl"
                ((HEALTH_FAILED++)) || true
            fi
        else
            log_error "Conversion failed: $(basename "$jpeg_file")"
            rm -f "$temp_jxl"
        fi
    else
        if [ -f "$output_jxl" ]; then
            log_warn "⏭️  Skip: $(basename "$output_jxl") already exists"
            continue
        fi

        log_info "🔄 Step 1/3: Converting (high quality -d 1)..."
        cjxl "$jpeg_file" "$output_jxl" -d 1 > /dev/null 2>&1
        
        if [ $? -eq 0 ]; then
            log_info "📋 Step 2/3: Migrating metadata (EXIF, XMP, IPTC)..."
            exiftool -tagsfromfile "$jpeg_file" -all:all -overwrite_original "$output_jxl" > /dev/null 2>&1 || true
            
            touch -r "$jpeg_file" "$output_jxl"
            
            log_info "🏥 Step 3/3: Health validation..."
            if check_jxl_health "$output_jxl"; then
                log_success "Converted: $(basename "$output_jxl")"
            else
                log_warn "Health check failed, but file created"
                ((HEALTH_FAILED++)) || true
            fi
        else
            log_error "Conversion failed: $(basename "$jpeg_file")"
        fi
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
