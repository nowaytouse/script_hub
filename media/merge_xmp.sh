#!/opt/homebrew/bin/bash

# ============================================================================
# 📋 XMP Metadata Merger - Merge Sidecar Files into Media
# ============================================================================
#
# Merges .xmp sidecar metadata files back into their corresponding media files.
#
# Features:
#   ✅ Recursively finds all .xmp files in target directory
#   ✅ Auto-detects corresponding media files (e.g., a.jpg.xmp → a.jpg)
#   ✅ UUID-based matching: reads xmp:DocumentID to find original file
#   ✅ Uses ExifTool to write complete metadata into media files
#   ✅ Preserves original file modification timestamps
#   ✅ Auto-creates backup files (e.g., "filename.jpg_original")
#   ✅ Optional: Delete .xmp files after successful merge
#
# Dependencies:
#   - exiftool (brew install exiftool)
#
# Usage:
#   ./merge_xmp.sh /path/to/media
#   ./merge_xmp.sh --delete-xmp /path/to/media
#
# ============================================================================

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Logging
log_info()    { echo -e "${BLUE}ℹ️  [INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}✅ [OK]${NC} $1"; }
log_warn()    { echo -e "${YELLOW}⚠️  [WARN]${NC} $1"; }
log_error()   { echo -e "${RED}❌ [ERROR]${NC} $1"; }
log_debug()   { echo -e "${CYAN}🔍 [DEBUG]${NC} $1"; }

# ============================================================================
# 📊 Progress Bar & Time Estimation
# ============================================================================

START_TIME=0
CURRENT_FILE=0
TOTAL_FILES=0

show_progress() {
    local current=$1
    local total=$2
    local filename="$3"
    
    [[ $total -eq 0 ]] && return
    
    local percent=$((current * 100 / total))
    local filled=$((percent / 2))
    local empty=$((50 - filled))
    
    printf "\r\033[K"
    printf "📊 Progress: ["
    printf "${GREEN}"
    printf '%0.s█' $(seq 1 $filled 2>/dev/null)
    printf "${NC}"
    printf '%0.s░' $(seq 1 $empty 2>/dev/null)
    printf "] ${percent}%% (${current}/${total})"
    
    if [[ $current -gt 0 ]]; then
        local elapsed=$(($(date +%s) - START_TIME))
        local avg_time=$((elapsed / current))
        local remaining=$(( (total - current) * avg_time ))
        
        if [[ $remaining -gt 60 ]]; then
            printf " | ⏱️  ~$((remaining / 60))m"
        else
            printf " | ⏱️  ~${remaining}s"
        fi
    fi
}

clear_progress() {
    printf "\r\033[K"
}

# ============================================================================
# 🔍 UUID-based Media File Finder
# ============================================================================

# Extract original filename from XMP metadata
# Supports: xmp:DocumentID, xmpMM:OriginalDocumentID, dc:source
extract_original_filename_from_xmp() {
    local xmp_file="$1"
    local original_name=""
    
    # Method 1: Try to extract from xmpMM:DerivedFrom or dc:source
    original_name=$(exiftool -s3 -DerivedFrom "$xmp_file" 2>/dev/null | head -1)
    if [[ -n "$original_name" && "$original_name" != *"uuid:"* ]]; then
        echo "$original_name"
        return 0
    fi
    
    # Method 2: Try dc:source (often contains original filename)
    original_name=$(exiftool -s3 -Source "$xmp_file" 2>/dev/null | head -1)
    if [[ -n "$original_name" && -f "$(dirname "$xmp_file")/$original_name" ]]; then
        echo "$original_name"
        return 0
    fi
    
    # Method 3: Try xmpMM:OriginalDocumentID - extract filename if it's a path
    original_name=$(exiftool -s3 -OriginalDocumentID "$xmp_file" 2>/dev/null | head -1)
    if [[ -n "$original_name" && "$original_name" == *"/"* ]]; then
        original_name=$(basename "$original_name")
        echo "$original_name"
        return 0
    fi
    
    return 1
}

# Find media file by searching for matching DocumentID in same directory
find_media_by_document_id() {
    local xmp_file="$1"
    local xmp_dir
    xmp_dir=$(dirname "$xmp_file")
    
    # Get DocumentID from XMP file
    local xmp_doc_id
    xmp_doc_id=$(exiftool -s3 -DocumentID "$xmp_file" 2>/dev/null | head -1)
    
    if [[ -z "$xmp_doc_id" ]]; then
        return 1
    fi
    
    # Search for media files with matching DocumentID
    local media_extensions=("jpg" "jpeg" "png" "tiff" "tif" "cr2" "cr3" "nef" "arw" "dng" "raf" "orf" "rw2" "pef" "srw" "heic" "heif" "webp" "avif" "jxl" "mp4" "mov" "avi" "mkv")
    
    for ext in "${media_extensions[@]}"; do
        while IFS= read -r -d '' media_file; do
            local media_doc_id
            media_doc_id=$(exiftool -s3 -DocumentID "$media_file" 2>/dev/null | head -1)
            
            if [[ "$media_doc_id" == "$xmp_doc_id" ]]; then
                echo "$media_file"
                return 0
            fi
        done < <(find "$xmp_dir" -maxdepth 1 -type f -iname "*.$ext" -print0 2>/dev/null)
    done
    
    return 1
}

# ============================================================================
# 🎯 Main Media File Finder (with fallback strategies)
# ============================================================================

find_media_file() {
    local xmp_file="$1"
    local xmp_dir
    local xmp_basename
    xmp_dir=$(dirname "$xmp_file")
    xmp_basename=$(basename "$xmp_file")
    
    # Strategy 1: Direct match (photo.jpg.xmp → photo.jpg)
    local base_name="${xmp_file%.xmp}"
    if [[ -f "$base_name" ]]; then
        echo "$base_name"
        return 0
    fi
    
    # Strategy 2: Same name different extension (photo.xmp → photo.jpg)
    local name_no_ext="${xmp_basename%.xmp}"
    local found_file
    found_file=$(find "$xmp_dir" -maxdepth 1 -type f -name "${name_no_ext}.*" ! -iname "*.xmp" 2>/dev/null | head -1)
    if [[ -n "$found_file" && -f "$found_file" ]]; then
        echo "$found_file"
        return 0
    fi
    
    # Strategy 3: Extract original filename from XMP metadata
    local original_name
    original_name=$(extract_original_filename_from_xmp "$xmp_file")
    if [[ -n "$original_name" ]]; then
        local potential_file="$xmp_dir/$original_name"
        if [[ -f "$potential_file" ]]; then
            echo "$potential_file"
            return 0
        fi
    fi
    
    # Strategy 4: UUID filename - search by DocumentID match
    # Check if filename looks like a UUID
    if [[ "$name_no_ext" =~ ^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$ ]]; then
        local matched_file
        matched_file=$(find_media_by_document_id "$xmp_file")
        if [[ -n "$matched_file" ]]; then
            echo "$matched_file"
            return 0
        fi
    fi
    
    return 1
}

# --- Configuration ---
DELETE_XMP=false
TARGET_DIR=""
DEBUG_MODE=false

# Parse arguments
for arg in "$@"; do
    case $arg in
        --delete-xmp)
            DELETE_XMP=true
            ;;
        --debug)
            DEBUG_MODE=true
            ;;
        -h|--help)
            echo "📋 XMP Metadata Merger"
            echo ""
            echo "Usage: $0 [options] <target_directory>"
            echo ""
            echo "Options:"
            echo "  --delete-xmp    Delete .xmp files after successful merge"
            echo "  --debug         Show debug information"
            echo "  -h, --help      Show this help"
            exit 0
            ;;
        *)
            if [[ -z "$TARGET_DIR" ]]; then
                TARGET_DIR="$arg"
            fi
            ;;
    esac
done

# --- Dependency Check ---
if ! command -v exiftool &> /dev/null; then
    log_error "ExifTool not found. Install: brew install exiftool"
    exit 1
fi

if [[ -z "$TARGET_DIR" ]]; then
    log_error "No target directory specified"
    echo "Usage: $0 [--delete-xmp] <target_directory>"
    exit 1
fi

if [[ ! -d "$TARGET_DIR" ]]; then
    log_error "Directory does not exist: $TARGET_DIR"
    exit 1
fi

# --- Safety Check ---
if [[ "$DELETE_XMP" == true ]]; then
    REAL_TARGET_DIR=""
    if command -v realpath &> /dev/null; then
        REAL_TARGET_DIR=$(realpath "$TARGET_DIR")
    else
        REAL_TARGET_DIR=$(cd "$TARGET_DIR" && pwd)
    fi

    FORBIDDEN_PATHS=("/" "/etc" "/bin" "/usr" "/System" "$HOME")

    for forbidden in "${FORBIDDEN_PATHS[@]}"; do
        if [[ "$REAL_TARGET_DIR" == "$forbidden" ]] || [[ "$REAL_TARGET_DIR" == "$forbidden/"* ]]; then
            log_error "🚫 SAFETY: Cannot operate on protected directory: $forbidden"
            exit 1
        fi
    done
fi

echo "╔══════════════════════════════════════════════╗"
echo "║   📋 XMP Metadata Merger v2.0                ║"
echo "╚══════════════════════════════════════════════╝"
echo ""
log_info "📁 Target: $TARGET_DIR"
log_info "📝 ExifTool will create '_original' backups"
[[ "$DELETE_XMP" == true ]] && log_warn "🗑️  Delete mode: .xmp files will be removed after merge"
[[ "$DEBUG_MODE" == true ]] && log_info "🔍 Debug mode enabled"
echo ""

SUCCESS_COUNT=0
FAIL_COUNT=0
SKIPPED_COUNT=0

# Count total files
log_info "📊 Scanning for XMP files..."
mapfile -d '' XMP_FILES < <(find "$TARGET_DIR" -type f -iname "*.xmp" -print0 2>/dev/null)
TOTAL_FILES=${#XMP_FILES[@]}
START_TIME=$(date +%s)

log_info "📁 Found: $TOTAL_FILES XMP files"
echo ""

if [[ $TOTAL_FILES -eq 0 ]]; then
    log_warn "No XMP files found in $TARGET_DIR"
    exit 0
fi

# --- Main Processing Loop ---
for xmp_file in "${XMP_FILES[@]}"; do
    ((CURRENT_FILE++))
    show_progress $CURRENT_FILE $TOTAL_FILES "$(basename "$xmp_file")"
    echo ""
    echo "──────────────────────────────────────────────"
    log_info "📄 XMP: $(basename "$xmp_file")"
    
    # Find corresponding media file
    media_file=$(find_media_file "$xmp_file")
    
    if [[ -z "$media_file" || ! -f "$media_file" ]]; then
        log_warn "⏭️  跳过: $(basename "$xmp_file") (无对应媒体文件)"
        if [[ "$DEBUG_MODE" == true ]]; then
            log_debug "XMP DocumentID: $(exiftool -s3 -DocumentID "$xmp_file" 2>/dev/null)"
        fi
        ((SKIPPED_COUNT++))
        continue
    fi
    
    log_info "🎯 Target: $(basename "$media_file")"
    
    # Execute merge
    log_info "🔄 Merging metadata..."
    if exiftool -P -overwrite_original -tagsfromfile "$xmp_file" -all:all "$media_file" > /dev/null 2>&1; then
        # Preserve timestamps
        touch -r "$xmp_file" "$media_file" 2>/dev/null
        
        log_success "Merged: $(basename "$xmp_file") → $(basename "$media_file")"
        ((SUCCESS_COUNT++))
        
        if [[ "$DELETE_XMP" == true ]]; then
            rm "$xmp_file"
            log_info "🗑️  Deleted: $(basename "$xmp_file")"
        fi
    else
        log_error "Merge failed: $(basename "$xmp_file")"
        ((FAIL_COUNT++))
    fi
done

clear_progress
echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║   📊 Merge Complete                          ║"
echo "╠══════════════════════════════════════════════╣"
printf "║  %-20s %20s  ║\n" "✅ Successful:" "$SUCCESS_COUNT"
printf "║  %-20s %20s  ║\n" "❌ Failed:" "$FAIL_COUNT"
printf "║  %-20s %20s  ║\n" "⏭️  Skipped:" "$SKIPPED_COUNT"
echo "╚══════════════════════════════════════════════╝"

# Exit with error if any failures
[[ $FAIL_COUNT -gt 0 ]] && exit 1
exit 0
