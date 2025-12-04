#!/bin/bash

# ============================================================================
# 📦 Archive Script - Split and Compress Large Directories
# ============================================================================
#
# Splits files in a directory into ~500MB chunks and creates .zip archives.
#
# Features:
#   ✅ Automatic file splitting by size (~500MB per archive)
#   ✅ Preserves directory structure
#   ✅ Progress reporting with emoji indicators
#
# Usage:
#   ./archive_and_upload.sh <source_directory>
#
# Example:
#   ./archive_and_upload.sh ./my_large_files
#
# ============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

# Logging
log_info()    { echo -e "${BLUE}ℹ️  [INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}✅ [OK]${NC} $1"; }
log_error()   { echo -e "${RED}❌ [ERROR]${NC} $1"; }

# Configuration
MAX_SIZE=$((500 * 1024 * 1024))  # 500 MB in bytes

# Check parameters
if [ -z "$1" ]; then
    echo "📦 Archive Script"
    echo ""
    echo "Usage: $0 <source_directory>"
    echo ""
    echo "Example:"
    echo "  $0 ./my_large_files"
    exit 1
fi

SOURCE_DIR="$1"

if [ ! -d "$SOURCE_DIR" ]; then
    log_error "Source directory does not exist: $SOURCE_DIR"
    exit 1
fi

# Main logic
echo "╔══════════════════════════════════════════════╗"
echo "║   📦 Archive Script                          ║"
echo "╚══════════════════════════════════════════════╝"
echo ""
log_info "📁 Source: $SOURCE_DIR"
log_info "📊 Max archive size: ~500 MB"
echo ""

# Create temporary file list
TMP_FILE_LIST=$(mktemp)
FIND_OUTPUT=$(mktemp)

# Cleanup temporary files on exit
trap 'rm -f "$TMP_FILE_LIST" "$FIND_OUTPUT"' EXIT

# Find all files and store paths with sizes
find "$SOURCE_DIR" -type f -print0 | xargs -0 du -b > "$FIND_OUTPUT"

current_size=0
part=1
total_files=0

while read -r size file; do
    echo "$file" >> "$TMP_FILE_LIST"
    current_size=$((current_size + size))
    ((total_files++)) || true

    if (( current_size >= MAX_SIZE )); then
        ARCHIVE_NAME="archive_part_${part}.zip"
        log_info "📦 Creating: $ARCHIVE_NAME (~$(($current_size / 1024 / 1024)) MB, $total_files files)..."

        # Create zip archive from file list
        zip -q -@ "$ARCHIVE_NAME" < "$TMP_FILE_LIST"
        
        if [ $? -eq 0 ]; then
            log_success "Created: $ARCHIVE_NAME"
            ls -lh "$ARCHIVE_NAME"
        else
            log_error "Failed to create: $ARCHIVE_NAME"
        fi

        # Reset
        > "$TMP_FILE_LIST"
        current_size=0
        total_files=0
        part=$((part + 1))
    fi
done < <(awk '{print $1, $2}' "$FIND_OUTPUT")

# Process remaining files (if any)
if [ -s "$TMP_FILE_LIST" ]; then
    ARCHIVE_NAME="archive_part_${part}.zip"
    log_info "📦 Creating final: $ARCHIVE_NAME (~$(($current_size / 1024 / 1024)) MB, $total_files files)..."
    
    zip -q -@ "$ARCHIVE_NAME" < "$TMP_FILE_LIST"

    if [ $? -eq 0 ]; then
        log_success "Created: $ARCHIVE_NAME"
        ls -lh "$ARCHIVE_NAME"
    else
        log_error "Failed to create: $ARCHIVE_NAME"
    fi
fi

echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║   📊 Archive Complete                        ║"
echo "╠══════════════════════════════════════════════╣"
printf "║  %-20s %20s  ║\n" "📦 Parts created:" "$part"
printf "║  %-20s %20s  ║\n" "📁 Location:" "$(pwd)"
echo "╚══════════════════════════════════════════════╝"
