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
NC='\033[0m'

# Logging
log_info()    { echo -e "${BLUE}ℹ️  [INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}✅ [OK]${NC} $1"; }
log_warn()    { echo -e "${YELLOW}⚠️  [WARN]${NC} $1"; }
log_error()   { echo -e "${RED}❌ [ERROR]${NC} $1"; }

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

# --- Configuration ---
DELETE_XMP=false
TARGET_DIR=""

# Parse arguments
for arg in "$@"; do
  case $arg in
    --delete-xmp)
      DELETE_XMP=true
      shift
      ;;
    -h|--help)
      echo "📋 XMP Metadata Merger"
      echo ""
      echo "Usage: $0 [options] <target_directory>"
      echo ""
      echo "Options:"
      echo "  --delete-xmp    Delete .xmp files after successful merge"
      echo "  -h, --help      Show this help"
      exit 0
      ;;
    *)
      TARGET_DIR="$arg"
      ;;
  esac
done

# --- Dependency Check ---
if ! command -v exiftool &> /dev/null; then
    log_error "ExifTool not found. Install: brew install exiftool"
    exit 1
fi

if [ -z "$TARGET_DIR" ]; then
    log_error "No target directory specified"
    echo "Usage: $0 [--delete-xmp] <target_directory>"
    exit 1
fi

if [ ! -d "$TARGET_DIR" ]; then
    log_error "Directory does not exist: $TARGET_DIR"
    exit 1
fi

# --- Safety Check ---
if [ "$DELETE_XMP" = true ]; then
    REAL_TARGET_DIR=""
    if command -v realpath &> /dev/null; then
        REAL_TARGET_DIR=$(realpath "$TARGET_DIR")
    else
        REAL_TARGET_DIR=$(cd "$TARGET_DIR"; pwd)
    fi

    FORBIDDEN_PATHS=("/" "/etc" "/bin" "/usr" "/System" "$HOME")

    for forbidden in "${FORBIDDEN_PATHS[@]}"; do
        if [ "$REAL_TARGET_DIR" = "$forbidden" ] || [[ "$REAL_TARGET_DIR" == "$forbidden/"* ]]; then
            log_error "🚫 SAFETY: Cannot operate on protected directory: $forbidden"
            exit 1
        fi
    done
fi

echo "╔══════════════════════════════════════════════╗"
echo "║   📋 XMP Metadata Merger                     ║"
echo "╚══════════════════════════════════════════════╝"
echo ""
log_info "📁 Target: $TARGET_DIR"
log_info "📝 ExifTool will create '_original' backups"
[ "$DELETE_XMP" = true ] && log_warn "🗑️  Delete mode: .xmp files will be removed after merge"
echo ""

SUCCESS_COUNT=0
FAIL_COUNT=0
SKIPPED_COUNT=0

SKIPPED_COUNT=0

# Count total files for progress bar
echo ""
log_info "📊 Counting files for progress tracking..."
local total_count=0

while IFS= read -r -d '' file; do
    ((total_count++)) || true
done < <(find "$TARGET_DIR" -type f -iname "*.xmp" -print0 2>/dev/null)

TOTAL_FILES=$total_count
CURRENT_FILE=0
START_TIME=$(date +%s)

log_info "📁 Found: $TOTAL_FILES XMP files"
echo ""

# --- Main Logic ---
find "$TARGET_DIR" -type f -iname "*.xmp" -print0 | while IFS= read -r -d $'\0' xmp_file; do
    ((CURRENT_FILE++)) || true
    show_progress $CURRENT_FILE $TOTAL_FILES "$(basename "$xmp_file")"
    echo "──────────────────────────────────────────────"
    log_info "📄 Found XMP: $(basename "$xmp_file")"

    # Remove .xmp suffix to get base filename
    base_name="${xmp_file%.*}"

    # Check if base filename exists (e.g., photo.jpg for photo.jpg.xmp)
    if [ -f "$base_name" ]; then
        media_file="$base_name"
    else
        # If photo.jpg doesn't exist, might be photo.xmp -> photo.cr2 case
        base_name_no_ext="${xmp_file%.xmp}"
        
        # Find file with same base name but different extension in same directory
        media_file=$(find "$(dirname "$xmp_file")" -maxdepth 1 -type f -name "$(basename "$base_name_no_ext").*" ! -name "*.xmp" | head -n 1)

        if [ -z "$media_file" ]; then
             log_warn "⏭️  Skip: No matching media file for $(basename "$xmp_file")"
             SKIPPED_COUNT=$((SKIPPED_COUNT + 1))
             clear_progress
             continue
        fi
    fi

    log_info "🎯 Target: $(basename "$media_file")"

    # Execute merge operation
    log_info "🔄 Merging metadata..."
    # The -P option already preserves the original file modification date/time.
    # However, adding touch -r explicitly ensures the timestamp is set from the XMP file
    # which might be desired if the media file's timestamp was already changed.
    exiftool -P -tagsfromfile "$xmp_file" -all:all "$media_file" > /dev/null 2>&1
    
    # Preserve original timestamps after exiftool modification
    # Use the XMP file's timestamp as reference for the media file
    touch -r "$xmp_file" "$media_file"

    if [ $? -eq 0 ]; then
        log_success "Merged: $(basename "$xmp_file") → $(basename "$media_file")"
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))

        if [ "$DELETE_XMP" = true ]; then
            rm "$xmp_file"
            log_info "🗑️  Deleted: $(basename "$xmp_file")"
        fi
    else
        log_error "Merge failed: $(basename "$xmp_file")"
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi

    
    clear_progress
done

echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║   📊 Merge Complete                          ║"
echo "╠══════════════════════════════════════════════╣"
printf "║  %-20s %20s  ║\n" "✅ Successful:" "$SUCCESS_COUNT"
printf "║  %-20s %20s  ║\n" "❌ Failed:" "$FAIL_COUNT"
printf "║  %-20s %20s  ║\n" "⏭️  Skipped:" "$SKIPPED_COUNT"
echo "╚══════════════════════════════════════════════╝"
