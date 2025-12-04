#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
# WebP Compression Script - Compress WebP files to target size range
# ═══════════════════════════════════════════════════════════════════════════════
# 
# PERFORMANCE OPTIMIZED VERSION
# - Single webpinfo call per file (cached)
# - Minimal external process calls
# - Efficient binary search
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
FAST_MODE=false
MAX_ATTEMPTS=10
PARALLEL_JOBS=1  # Number of parallel jobs

# Logging functions
log_info() { echo -e "${CYAN}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }

# Check dependencies (only once at startup)
check_deps() {
    local missing=()
    command -v magick &>/dev/null || missing+=("imagemagick")
    command -v exiftool &>/dev/null || missing+=("exiftool")
    
    if [[ ${#missing[@]} -gt 0 ]]; then
        log_error "Missing dependencies: ${missing[*]}"
        log_info "Install with: brew install ${missing[*]}"
        exit 1
    fi
}

# Get file size in bytes (fast)
get_size() {
    stat -f%z "$1" 2>/dev/null || stat -c%s "$1" 2>/dev/null || echo 0
}

# Get basic WebP info - fast method
# Returns: "width height frames is_animated"
get_webp_basic_info() {
    local file="$1"
    
    # Use identify with [0] to get only first frame info (fast)
    local width height frames
    width=$(magick identify -format "%w" "${file}[0]" 2>/dev/null) || width="?"
    height=$(magick identify -format "%h" "${file}[0]" 2>/dev/null) || height="?"
    frames=$(magick identify -format "%n" "${file}[0]" 2>/dev/null) || frames="1"
    
    local is_anim="no"
    [[ "$frames" -gt 1 ]] 2>/dev/null && is_anim="yes"
    
    echo "$width $height $frames $is_anim"
}

# Compress single WebP file - OPTIMIZED
compress_webp() {
    local input="$1"
    local min_size=$2
    local max_size=$3
    local filename=$(basename "$input")
    local dirname=$(dirname "$input")
    local basename="${filename%.*}"
    
    # Output file
    local output="$dirname/${basename}_compressed.webp"
    local final_output="$output"
    [[ "$IN_PLACE" == true ]] && final_output="$input"
    
    local temp_file="/tmp/webp_compress_$$.webp"
    
    log_info "📦 Processing: $filename"
    
    # Get original info (SINGLE call)
    local orig_size=$(get_size "$input")
    local orig_size_mb=$((orig_size / 1024 / 1024))
    
    # Quick info using magick identify (much faster)
    local info
    info=$(get_webp_basic_info "$input")
    local width height frames is_anim
    read -r width height frames is_anim <<< "$info"
    
    log_info "   📋 Original: ${orig_size_mb}MB | ${width}x${height} | frames=$frames"
    
    # Binary search for optimal quality
    local quality=75
    local min_q=5
    local max_q=100
    local attempts=0
    local max_attempts=$MAX_ATTEMPTS
    local best_quality=75
    local best_size=0
    
    # Fast mode: start with estimated quality based on target ratio
    if [[ "$FAST_MODE" == true ]]; then
        local target_ratio=$(( (min_size + max_size) / 2 * 100 / orig_size ))
        quality=$((target_ratio + 10))
        [[ $quality -gt 95 ]] && quality=95
        [[ $quality -lt 30 ]] && quality=30
        max_attempts=5
        log_info "   ⚡ Fast mode: starting at Q=$quality"
    fi
    
    while [[ $attempts -lt $max_attempts ]]; do
        ((++attempts))
        
        # Compress with ImageMagick (handles animated WebP)
        magick "$input" -quality $quality "$temp_file" 2>/dev/null || {
            log_warning "   ⚠️ magick failed at Q=$quality"
            quality=$((quality - 10))
            continue
        }
        
        local size=$(get_size "$temp_file")
        local size_mb=$((size / 1024 / 1024))
        
        log_info "   🔄 Attempt $attempts: Q=$quality → ${size_mb}MB"
        
        # Check if in target range
        if [[ $size -ge $min_size ]] && [[ $size -le $max_size ]]; then
            log_success "   ✅ Target reached: ${size_mb}MB"
            best_quality=$quality
            best_size=$size
            break
        elif [[ $size -gt $max_size ]]; then
            max_q=$quality
            quality=$(( (min_q + quality) / 2 ))
        else
            min_q=$quality
            quality=$(( (quality + max_q) / 2 ))
            
            if [[ $quality -ge 99 ]]; then
                log_warning "   ⚠️ Cannot reach minimum even at Q=100"
                best_quality=$quality
                best_size=$size
                break
            fi
        fi
        
        # Track best result
        if [[ $size -gt $best_size ]] && [[ $size -le $max_size ]]; then
            best_size=$size
            best_quality=$quality
        fi
        
        # Prevent infinite loop
        if [[ $((max_q - min_q)) -le 1 ]]; then
            log_warning "   ⚠️ Using best achievable: Q=$best_quality"
            break
        fi
    done
    
    # Final compression with best quality (if not already done)
    if [[ ! -f "$temp_file" ]] || [[ $(get_size "$temp_file") -eq 0 ]]; then
        magick "$input" -quality $best_quality "$temp_file" 2>/dev/null || {
            log_error "   ❌ Compression failed"
            rm -f "$temp_file" 2>/dev/null
            return 1
        }
    fi
    
    # Copy to output
    cp "$temp_file" "$output"
    
    # Migrate metadata (single exiftool call)
    exiftool -overwrite_original -TagsFromFile "$input" "-all:all>all:all" "$output" 2>/dev/null || true
    
    # Preserve timestamps
    touch -r "$input" "$output"
    
    local new_size=$(get_size "$output")
    local new_size_mb=$((new_size / 1024 / 1024))
    
    if [[ "$IN_PLACE" == true ]]; then
        mv "$output" "$final_output"
        log_success "   ✅ Replaced: $filename (${orig_size_mb}MB → ${new_size_mb}MB)"
    else
        log_success "   ✅ Created: ${basename}_compressed.webp (${new_size_mb}MB)"
    fi
    
    rm -f "$temp_file" 2>/dev/null
    return 0
}

# Process directory
process_dir() {
    local dir="$1"
    local min_bytes=$((MIN_SIZE_MB * 1024 * 1024))
    local max_bytes=$((MAX_SIZE_MB * 1024 * 1024))
    
    log_info "🔍 Scanning for WebP files in: $dir"
    
    # Collect files to process
    local files=()
    while IFS= read -r -d '' file; do
        [[ "${file##*.}" != "webp" ]] && continue
        [[ "$IN_PLACE" == false ]] && [[ "$file" == *"_compressed.webp" ]] && continue
        files+=("$file")
    done < <(find "$dir" -type f -iname "*.webp" -print0)
    
    local total=${#files[@]}
    log_info "📁 Found $total WebP files to process"
    
    if [[ $total -eq 0 ]]; then
        log_warning "No WebP files found"
        return 0
    fi
    
    # Process files (parallel if jobs > 1)
    local success=0
    if [[ $PARALLEL_JOBS -gt 1 ]] && command -v parallel &>/dev/null; then
        log_info "🚀 Using $PARALLEL_JOBS parallel jobs"
        export -f compress_webp get_webp_basic_info get_size log_info log_success log_warning log_error
        export IN_PLACE FAST_MODE MAX_ATTEMPTS RED GREEN YELLOW CYAN NC
        printf '%s\0' "${files[@]}" | parallel -0 -j "$PARALLEL_JOBS" \
            "compress_webp {} $min_bytes $max_bytes"
        success=$total  # Approximate
    else
        for file in "${files[@]}"; do
            compress_webp "$file" "$min_bytes" "$max_bytes" && ((++success)) || true
            echo ""
        done
    fi
    
    log_info "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    log_success "📊 Completed: $success/$total files processed"
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
  --fast        Fast mode (fewer attempts, may be less accurate)
  -j, --jobs N  Parallel jobs for batch processing (default: 1)
  --help        Show this help message

Examples:
  $0 ./images 15 20
  $0 --in-place ./images 10 15
  $0 video.webp 15 20
EOF
}

# Main
main() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --in-place) IN_PLACE=true; shift ;;
            --fast) FAST_MODE=true; shift ;;
            -j|--jobs) PARALLEL_JOBS="$2"; shift 2 ;;
            --help|-h) show_usage; exit 0 ;;
            *) break ;;
        esac
    done
    
    local input="$1"
    MIN_SIZE_MB=${2:-15}
    MAX_SIZE_MB=${3:-20}
    
    [[ -z "$input" ]] && { show_usage; exit 1; }
    
    check_deps
    
    log_info "🎯 Target size: ${MIN_SIZE_MB}MB - ${MAX_SIZE_MB}MB"
    [[ "$IN_PLACE" == true ]] && log_warning "⚠️ In-place mode enabled"
    
    if [[ -d "$input" ]]; then
        process_dir "$input"
    elif [[ -f "$input" ]]; then
        [[ "${input##*.}" != "webp" ]] && { log_error "❌ Not a WebP file"; exit 1; }
        compress_webp "$input" $((MIN_SIZE_MB * 1024 * 1024)) $((MAX_SIZE_MB * 1024 * 1024))
    else
        log_error "❌ Path not found: $input"
        exit 1
    fi
}

main "$@"
