#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
# WebP Compression Script - Compress WebP files to target size range
# ═══════════════════════════════════════════════════════════════════════════════
# 
# Features:
#   - ONLY processes WebP files (whitelist mode)
#   - Preserves ALL metadata (EXIF, XMP, ICC Profile)
#   - Preserves animation info (FPS, frame count, duration)
#   - Preserves file system timestamps
#   - Binary search for optimal quality
#   - Safe operation (creates new file, not in-place by default)
#
# Usage:
#   ./compress_webp.sh <input_dir> [min_MB] [max_MB]
#   ./compress_webp.sh --in-place <input_dir> [min_MB] [max_MB]
#
# ═══════════════════════════════════════════════════════════════════════════════

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# Default parameters
MIN_SIZE_MB=15
MAX_SIZE_MB=20
IN_PLACE=false
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Dangerous directories (safety check)
DANGEROUS_DIRS=("/" "/System" "/usr" "/bin" "/sbin" "/var" "/private" "$HOME")

# Logging functions
log_info() { echo -e "${CYAN}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }

# Check dependencies
check_deps() {
    local missing=()
    command -v magick &>/dev/null || missing+=("imagemagick")
    command -v exiftool &>/dev/null || missing+=("exiftool")
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing[*]}"
        log_info "Install with: brew install ${missing[*]}"
        exit 1
    fi
    
    # Optional but recommended
    if ! command -v webpinfo &>/dev/null; then
        log_warning "webpinfo not found (optional but recommended for animated WebP)"
        log_info "Install with: brew install webp"
    fi
}

# Safety check for dangerous directories
check_dangerous_dir() {
    local dir="$1"
    local abs_dir=$(cd "$dir" 2>/dev/null && pwd)
    
    for dangerous in "${DANGEROUS_DIRS[@]}"; do
        if [[ "$abs_dir" == "$dangerous" ]]; then
            log_error "🚨 DANGEROUS DIRECTORY DETECTED: $abs_dir"
            log_error "Operation aborted for safety. Please use a subdirectory."
            exit 1
        fi
    done
}

# Get file size in bytes
get_size() {
    stat -f%z "$1" 2>/dev/null || stat -c%s "$1" 2>/dev/null || echo 0
}

# Get WebP info (FPS, frame count, duration) - uses webpinfo for animated WebP
# NOTE: Avoid storing full webpinfo output (can be huge for animated files)
get_webp_info() {
    local file="$1"
    local width="" height="" fps="" frames="" is_anim="no"
    
    # Try webpinfo first (more reliable for animated WebP)
    if command -v webpinfo &>/dev/null; then
        # Check if animated (quick check, don't store full output)
        if webpinfo "$file" 2>/dev/null | grep -q "Animation: 1"; then
            is_anim="yes"
            # Get canvas size (first line only)
            local canvas
            canvas=$(webpinfo "$file" 2>/dev/null | grep "Canvas size" | head -1) || true
            width=$(echo "$canvas" | grep -oE '[0-9]+' | head -1) || true
            height=$(echo "$canvas" | grep -oE '[0-9]+' | tail -1) || true
            # Count ANMF chunks (frames) - stream through grep -c
            frames=$(webpinfo "$file" 2>/dev/null | grep -c "Chunk ANMF") || frames="0"
            # Get first frame duration to estimate FPS
            local frame_dur
            frame_dur=$(webpinfo "$file" 2>/dev/null | grep "Duration:" | head -1 | grep -oE '[0-9]+') || true
            if [[ -n "$frame_dur" ]] && [[ "$frame_dur" -gt 0 ]]; then
                fps=$(echo "scale=2; 1000 / $frame_dur" | bc) || true
            fi
        else
            # Static WebP
            width=$(webpinfo "$file" 2>/dev/null | grep "Width:" | head -1 | grep -oE '[0-9]+') || true
            height=$(webpinfo "$file" 2>/dev/null | grep "Height:" | head -1 | grep -oE '[0-9]+') || true
            frames="1"
        fi
    fi
    
    # Fallback to ffprobe if webpinfo failed
    if [[ -z "$width" ]] || [[ "$width" == "0" ]]; then
        local info
        info=$(ffprobe -v quiet -print_format json -show_streams -show_format "$file" 2>/dev/null) || true
        fps=$(echo "$info" | grep -o '"r_frame_rate": "[^"]*"' | head -1 | cut -d'"' -f4) || true
        frames=$(echo "$info" | grep -o '"nb_frames": "[^"]*"' | head -1 | cut -d'"' -f4) || true
        width=$(echo "$info" | grep -o '"width": [0-9]*' | head -1 | grep -o '[0-9]*') || true
        height=$(echo "$info" | grep -o '"height": [0-9]*' | head -1 | grep -o '[0-9]*') || true
    fi
    
    echo "width=${width:-?} height=${height:-?} frames=${frames:-?} fps=${fps:-?} animated=$is_anim"
}

# Check if WebP is animated - uses webpinfo for reliability
# Returns 0 if animated, 1 if not (safe with set -e)
is_animated() {
    local file="$1"
    
    # Try webpinfo first (most reliable)
    if command -v webpinfo &>/dev/null; then
        if webpinfo "$file" 2>/dev/null | grep -q "Animation: 1"; then
            return 0
        fi
    fi
    
    # Fallback to ffprobe
    local frames
    frames=$(ffprobe -v quiet -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 "$file" 2>/dev/null | tr -d '[:space:]') || true
    if [[ "$frames" =~ ^[0-9]+$ ]] && [[ "$frames" -gt 1 ]]; then
        return 0
    fi
    
    return 1
}

# Compress single WebP file
compress_webp() {
    local input="$1"
    local min_size=$2
    local max_size=$3
    local filename=$(basename "$input")
    local dirname=$(dirname "$input")
    local basename="${filename%.*}"
    
    # Output file
    if [[ "$IN_PLACE" == true ]]; then
        local output="$dirname/${basename}_compressed.webp"
        local final_output="$input"
    else
        local output="$dirname/${basename}_compressed.webp"
        local final_output="$output"
    fi
    
    local temp_file=$(mktemp /tmp/webp_compress_XXXXXX)
    temp_file="${temp_file}.webp"
    
    log_info "📦 Processing: $filename"
    
    # Get original info
    local orig_info=$(get_webp_info "$input")
    local orig_size=$(get_size "$input")
    local orig_size_mb=$(echo "scale=2; $orig_size / 1024 / 1024" | bc)
    
    log_info "   📋 Original: ${orig_size_mb}MB | $orig_info"
    
    # Check if animated (use || true to prevent set -e from exiting)
    local is_anim=false
    is_animated "$input" && is_anim=true || true
    
    # Binary search for optimal quality using ImageMagick
    local quality=75
    local min_q=5
    local max_q=100
    local attempts=0
    local max_attempts=15
    local best_output=""
    local best_size=0
    
    while [[ $attempts -lt $max_attempts ]]; do
        ((++attempts))  # Use prefix increment to avoid set -e issue when attempts=0
        
        # Use ImageMagick for both animated and static WebP
        # ImageMagick handles animated WebP correctly
        log_info "   🔧 Running magick (Q=$quality)..."
        magick "$input" -quality $quality "$temp_file" 2>/dev/null || {
            log_warning "   ⚠️ magick command failed"
        }
        
        if [[ ! -f "$temp_file" ]] || [[ $(get_size "$temp_file") -eq 0 ]]; then
            log_warning "   ⚠️ Attempt $attempts failed, trying different quality..."
            quality=$((quality - 10))
            continue
        fi
        
        local size=$(get_size "$temp_file")
        local size_mb=$(echo "scale=2; $size / 1024 / 1024" | bc)
        
        log_info "   🔄 Attempt $attempts: Q=$quality → ${size_mb}MB"
        
        # Check if in target range
        if [[ $size -ge $min_size ]] && [[ $size -le $max_size ]]; then
            log_success "   ✅ Target reached: ${size_mb}MB"
            best_output="$temp_file"
            best_size=$size
            break
        elif [[ $size -gt $max_size ]]; then
            # Too big, decrease quality
            max_q=$quality
            quality=$(( (min_q + quality) / 2 ))
        else
            # Too small, increase quality
            min_q=$quality
            quality=$(( (quality + max_q) / 2 ))
            
            # If already at max quality and still too small, warn and stop
            if [[ $quality -ge 99 ]]; then
                local min_mb=$(echo "scale=2; $min_size / 1024 / 1024" | bc)
                log_warning "   ⚠️ Cannot reach minimum ${min_mb}MB even at Q=100"
                log_warning "   ⚠️ Best achievable: ${size_mb}MB (Q=$quality)"
                log_info "   💡 Tip: Original file may be too small or already optimized"
                best_output="$temp_file"
                best_size=$size
                break
            fi
        fi
        
        # Save best result so far (closest to target range)
        if [[ $size -gt $best_size ]]; then
            best_size=$size
            cp "$temp_file" "${temp_file}.best"
            best_output="${temp_file}.best"
        fi
        
        # Prevent infinite loop
        if [[ $((max_q - min_q)) -le 1 ]]; then
            if [[ -n "$best_output" ]] && [[ -f "$best_output" ]]; then
                local best_mb=$(echo "scale=2; $best_size / 1024 / 1024" | bc)
                log_warning "   ⚠️ Cannot reach target range, using best: ${best_mb}MB"
            else
                log_warning "   ⚠️ Using current result: ${size_mb}MB"
                best_output="$temp_file"
            fi
            break
        fi
    done
    
    # Verify we have output
    if [[ -z "$best_output" ]] || [[ ! -f "$best_output" ]]; then
        log_error "   ❌ Compression failed"
        rm -f "$temp_file" "${temp_file}.best" 2>/dev/null
        return 1
    fi
    
    # Step 1: Copy to output location
    cp "$best_output" "$output"
    
    # Step 2: Migrate metadata using exiftool
    log_info "   📋 Migrating metadata (EXIF, XMP, ICC)..."
    exiftool -overwrite_original -TagsFromFile "$input" \
        "-all:all>all:all" \
        "-ICC_Profile" \
        "$output" 2>/dev/null || true
    
    # Step 3: Preserve timestamps
    log_info "   ⏰ Preserving timestamps..."
    touch -r "$input" "$output"
    
    # Step 4: Verify output
    local new_info=$(get_webp_info "$output")
    local new_size=$(get_size "$output")
    local new_size_mb=$(echo "scale=2; $new_size / 1024 / 1024" | bc)
    
    log_info "   📋 Result: ${new_size_mb}MB | $new_info"
    
    # Step 5: In-place replacement (if enabled)
    if [[ "$IN_PLACE" == true ]]; then
        log_info "   🔄 Replacing original file..."
        mv "$output" "$final_output"
        log_success "   ✅ Replaced: $filename (${orig_size_mb}MB → ${new_size_mb}MB)"
    else
        log_success "   ✅ Created: ${basename}_compressed.webp (${new_size_mb}MB)"
    fi
    
    # Cleanup
    rm -f "$temp_file" "${temp_file}.best" 2>/dev/null
    
    return 0
}

# Process directory
process_dir() {
    local dir="$1"
    local count=0
    local success=0
    local min_bytes=$((MIN_SIZE_MB * 1024 * 1024))
    local max_bytes=$((MAX_SIZE_MB * 1024 * 1024))
    
    log_info "🔍 Scanning for WebP files in: $dir"
    
    # Find only WebP files (whitelist mode)
    while IFS= read -r -d '' file; do
        # Double check extension
        [[ "${file##*.}" != "webp" ]] && continue
        
        ((count++))
        compress_webp "$file" "$min_bytes" "$max_bytes" && ((success++)) || true
        echo ""
    done < <(find "$dir" -type f -iname "*.webp" -print0)
    
    log_info "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    log_success "📊 Completed: $success/$count files processed"
}

# Show usage
show_usage() {
    cat << EOF
WebP Compression Script - Compress WebP files to target size range

Usage:
  $0 <input_dir> [min_MB] [max_MB]
  $0 --in-place <input_dir> [min_MB] [max_MB]
  $0 <single_file.webp> [min_MB] [max_MB]

Options:
  --in-place    Replace original files (destructive)
  --help        Show this help message

Arguments:
  input_dir     Directory containing WebP files
  min_MB        Minimum target size in MB (default: 15)
  max_MB        Maximum target size in MB (default: 20)

Examples:
  $0 ./images 15 20
  $0 --in-place ./images 10 15
  $0 video.webp 15 20

Note: This script ONLY processes .webp files (whitelist mode)
EOF
}

# Main function
main() {
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --in-place)
                IN_PLACE=true
                shift
                ;;
            --help|-h)
                show_usage
                exit 0
                ;;
            *)
                break
                ;;
        esac
    done
    
    local input="$1"
    MIN_SIZE_MB=${2:-15}
    MAX_SIZE_MB=${3:-20}
    
    if [[ -z "$input" ]]; then
        show_usage
        exit 1
    fi
    
    # Check dependencies
    check_deps
    
    log_info "🎯 Target size: ${MIN_SIZE_MB}MB - ${MAX_SIZE_MB}MB"
    [[ "$IN_PLACE" == true ]] && log_warning "⚠️ In-place mode enabled (will replace originals)"
    
    if [[ -d "$input" ]]; then
        # Safety check for directories
        [[ "$IN_PLACE" == true ]] && check_dangerous_dir "$input"
        process_dir "$input"
    elif [[ -f "$input" ]]; then
        # Single file
        if [[ "${input##*.}" != "webp" ]]; then
            log_error "❌ Not a WebP file: $input"
            log_info "This script only processes .webp files"
            exit 1
        fi
        local min_bytes=$((MIN_SIZE_MB * 1024 * 1024))
        local max_bytes=$((MAX_SIZE_MB * 1024 * 1024))
        compress_webp "$input" "$min_bytes" "$max_bytes"
    else
        log_error "❌ Path not found: $input"
        exit 1
    fi
}

main "$@"
