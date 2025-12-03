#!/bin/bash

# ============================================================================
# 🏥 Media Health Check Module - Validate Media File Integrity
# ============================================================================
#
# Universal media health check functions, can be sourced by other scripts.
#
# Features:
#   ✅ Format signature validation (PNG, GIF, WebP, JXL, JPEG, MP4)
#   ✅ Structure validation via ffprobe (dimensions, codec, frames)
#   ✅ Decode test via ffmpeg (ensures playability)
#   ✅ Batch directory scanning
#
# Usage:
#   source ./media_health_check.sh
#   check_media_health "/path/to/file.png"
#
# ============================================================================

# Color definitions (if not already defined)
: "${RED:=\033[0;31m}"
: "${GREEN:=\033[0;32m}"
: "${YELLOW:=\033[1;33m}"
: "${CYAN:=\033[0;36m}"
: "${NC:=\033[0m}"

# Health check statistics
HEALTH_PASSED=${HEALTH_PASSED:-0}
HEALTH_FAILED=${HEALTH_FAILED:-0}
HEALTH_WARNINGS=${HEALTH_WARNINGS:-0}

# Logging function for health checks
log_health() { echo -e "${CYAN}[HEALTH]${NC} $1"; }

# ============================================================================
# Core Health Check Function
# ============================================================================

# Check if media file is valid and viewable/playable
# Arguments:
#   $1 - file path
#   $2 - expected type (optional: png, gif, webp, jxl, mp4, etc.)
#   $3 - verbose mode (optional: true/false)
# Returns: 0=healthy, 1=corrupted
check_media_health() {
    local file="$1"
    local expected_type="${2:-auto}"
    local verbose="${3:-false}"
    
    # 1. Basic file existence check
    if [ ! -f "$file" ]; then
        log_health "✗ FAILED: File does not exist: $file"
        return 1
    fi
    
    # 2. File size check
    local size
    size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null)
    
    if [ "$size" -lt 100 ]; then
        log_health "✗ FAILED: File too small ($size bytes): $(basename "$file")"
        return 1
    fi
    
    # 3. Determine file type from extension if auto
    local file_ext="${file##*.}"
    file_ext=$(echo "$file_ext" | tr '[:upper:]' '[:lower:]')
    
    if [ "$expected_type" = "auto" ]; then
        expected_type="$file_ext"
    fi
    
    # 4. Format-specific signature checks
    case "$expected_type" in
        png)
            local sig
            sig=$(xxd -l 8 -p "$file" 2>/dev/null)
            if [ "$sig" != "89504e470d0a1a0a" ]; then
                log_health "✗ FAILED: Invalid PNG signature: $(basename "$file")"
                return 1
            fi
            ;;
        gif)
            local sig
            sig=$(head -c 6 "$file" 2>/dev/null)
            if [[ "$sig" != "GIF87a" && "$sig" != "GIF89a" ]]; then
                log_health "✗ FAILED: Invalid GIF signature: $(basename "$file")"
                return 1
            fi
            ;;
        webp)
            local sig
            sig=$(head -c 4 "$file" 2>/dev/null)
            if [ "$sig" != "RIFF" ]; then
                log_health "✗ FAILED: Invalid WebP signature: $(basename "$file")"
                return 1
            fi
            ;;
        jxl)
            local sig
            sig=$(xxd -l 2 -p "$file" 2>/dev/null)
            # JXL can start with 0xFF0A (naked codestream) or have ISOBMFF container
            if [[ "$sig" != "ff0a" && "$sig" != "0000" ]]; then
                log_health "✗ FAILED: Invalid JXL signature: $(basename "$file")"
                return 1
            fi
            ;;
        jpg|jpeg)
            local sig
            sig=$(xxd -l 2 -p "$file" 2>/dev/null)
            if [ "$sig" != "ffd8" ]; then
                log_health "✗ FAILED: Invalid JPEG signature: $(basename "$file")"
                return 1
            fi
            ;;
        mp4|mov)
            # Check for ftyp box
            local sig
            sig=$(xxd -s 4 -l 4 "$file" 2>/dev/null | awk '{print $2$3}')
            if [ "$sig" != "6674" ]; then  # "ft" in hex
                log_health "✗ FAILED: Invalid MP4/MOV signature: $(basename "$file")"
                return 1
            fi
            ;;
    esac
    
    # 5. Use ffprobe to validate media structure
    if command -v ffprobe &> /dev/null; then
        local probe_result
        probe_result=$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height,codec_name -of csv=p=0 "$file" 2>&1)
        
        if [ $? -ne 0 ] || [ -z "$probe_result" ]; then
            log_health "✗ FAILED: Cannot read media structure: $(basename "$file")"
            [ "$verbose" = true ] && log_health "  ffprobe: $probe_result"
            return 1
        fi
        
        # Parse dimensions
        local codec width height
        IFS=',' read -r codec width height <<< "$probe_result"
        
        if [ -z "$width" ] || [ -z "$height" ] || [ "$width" -lt 1 ] || [ "$height" -lt 1 ]; then
            log_health "✗ FAILED: Invalid dimensions (${width}x${height}): $(basename "$file")"
            return 1
        fi
        
        [ "$verbose" = true ] && log_health "  Codec: $codec, Dimensions: ${width}x${height}"
    fi
    
    # 6. Try to decode first frame (ultimate validation)
    if command -v ffmpeg &> /dev/null; then
        local decode_test
        decode_test=$(ffmpeg -v error -i "$file" -frames:v 1 -f null - 2>&1)
        if [ $? -ne 0 ]; then
            log_health "✗ FAILED: Cannot decode media: $(basename "$file")"
            [ "$verbose" = true ] && log_health "  Error: $decode_test"
            return 1
        fi
    fi
    
    # 7. Additional checks for animated formats
    case "$expected_type" in
        gif|webp|apng)
            if command -v ffprobe &> /dev/null; then
                local frame_count
                frame_count=$(ffprobe -v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 "$file" 2>/dev/null)
                if [ -n "$frame_count" ] && [ "$frame_count" -gt 0 ]; then
                    [ "$verbose" = true ] && log_health "  Frame count: $frame_count"
                    if [ "$frame_count" -lt 2 ] && [ "$expected_type" = "gif" ]; then
                        log_health "⚠ WARNING: GIF has only $frame_count frame(s): $(basename "$file")"
                        ((HEALTH_WARNINGS++)) || true
                    fi
                fi
            fi
            ;;
    esac
    
    log_health "✓ PASSED: $(basename "$file") (${size} bytes)"
    ((HEALTH_PASSED++)) || true
    return 0
}

# ============================================================================
# Batch Health Check
# ============================================================================

# Check health of all media files in a directory
# Arguments:
#   $1 - directory path
#   $2 - file pattern (optional, e.g., "*.png")
#   $3 - verbose mode (optional: true/false)
check_directory_health() {
    local dir="$1"
    local pattern="${2:-*}"
    local verbose="${3:-false}"
    
    if [ ! -d "$dir" ]; then
        log_health "✗ Directory does not exist: $dir"
        return 1
    fi
    
    log_health "Scanning directory: $dir"
    
    local total=0
    local passed=0
    local failed=0
    
    while IFS= read -r -d '' file; do
        ((total++)) || true
        if check_media_health "$file" "auto" "$verbose"; then
            ((passed++)) || true
        else
            ((failed++)) || true
        fi
    done < <(find "$dir" -type f -name "$pattern" -print0 2>/dev/null)
    
    echo ""
    log_health "Directory scan complete: $total files, $passed passed, $failed failed"
    
    [ "$failed" -gt 0 ] && return 1
    return 0
}

# ============================================================================
# Health Report
# ============================================================================

print_health_report() {
    echo ""
    echo "=============================================="
    echo "  Media Health Report"
    echo "=============================================="
    echo -e "  ${GREEN}Passed:${NC}   $HEALTH_PASSED"
    echo -e "  ${RED}Failed:${NC}   $HEALTH_FAILED"
    echo -e "  ${YELLOW}Warnings:${NC} $HEALTH_WARNINGS"
    
    local total=$((HEALTH_PASSED + HEALTH_FAILED))
    if [ "$total" -gt 0 ]; then
        local health_rate=$((HEALTH_PASSED * 100 / total))
        echo "  Health Rate: ${health_rate}%"
        
        if [ "$health_rate" -lt 100 ]; then
            echo ""
            echo -e "  ${YELLOW}⚠ Some files may have issues.${NC}"
        fi
    fi
    echo "=============================================="
}

# Reset health statistics
reset_health_stats() {
    HEALTH_PASSED=0
    HEALTH_FAILED=0
    HEALTH_WARNINGS=0
}

# ============================================================================
# Standalone Mode
# ============================================================================

# If run directly (not sourced), perform health check on arguments
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    if [ $# -eq 0 ]; then
        echo "Usage: $0 <file_or_directory> [file_or_directory...]"
        echo ""
        echo "Check health of media files to ensure they are viewable/playable."
        echo ""
        echo "Examples:"
        echo "  $0 image.png                    # Check single file"
        echo "  $0 /path/to/images/             # Check all files in directory"
        echo "  $0 *.gif *.png                  # Check multiple files"
        exit 1
    fi
    
    for arg in "$@"; do
        if [ -d "$arg" ]; then
            check_directory_health "$arg" "*" true
        elif [ -f "$arg" ]; then
            check_media_health "$arg" "auto" true
        else
            log_health "✗ Not found: $arg"
            ((HEALTH_FAILED++)) || true
        fi
    done
    
    print_health_report
    
    [ "$HEALTH_FAILED" -gt 0 ] && exit 1
    exit 0
fi
