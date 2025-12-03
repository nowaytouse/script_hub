#!/bin/bash

# ============================================================================
# Archive Script - Split and Compress Large Directories
# 归档脚本 - 分割并压缩大型目录
# ============================================================================
#
# Functionality:
# - Splits files in a directory into ~500MB chunks
# - Creates .zip archives for each chunk
# - Preserves directory structure
#
# Usage:
#   ./archive_and_upload.sh <source_directory>
#
# Example:
#   ./archive_and_upload.sh ./my_large_files
#
# ============================================================================

set -e

# Configuration
MAX_SIZE=$((500 * 1024 * 1024))  # 500 MB in bytes

# Check parameters
if [ -z "$1" ]; then
    echo "Usage: $0 <source_directory>"
    echo ""
    echo "Example:"
    echo "  $0 ./my_large_files"
    exit 1
fi

SOURCE_DIR="$1"

if [ ! -d "$SOURCE_DIR" ]; then
    echo "Error: Source directory '$SOURCE_DIR' does not exist."
    exit 1
fi

# Main logic
echo "=============================================="
echo "  Archive Script"
echo "=============================================="
echo ""
echo "Source directory: $SOURCE_DIR"
echo "Max archive size: ~500 MB"
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
        echo "Creating archive '$ARCHIVE_NAME' (~$(($current_size / 1024 / 1024)) MB, $total_files files)..."

        # Create zip archive from file list
        zip -q -@ "$ARCHIVE_NAME" < "$TMP_FILE_LIST"
        
        if [ $? -eq 0 ]; then
            echo "✅ Created: $ARCHIVE_NAME"
            ls -lh "$ARCHIVE_NAME"
        else
            echo "❌ Error: Failed to create archive '$ARCHIVE_NAME'"
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
    echo "Creating final archive '$ARCHIVE_NAME' (~$(($current_size / 1024 / 1024)) MB, $total_files files)..."
    
    zip -q -@ "$ARCHIVE_NAME" < "$TMP_FILE_LIST"

    if [ $? -eq 0 ]; then
        echo "✅ Created: $ARCHIVE_NAME"
        ls -lh "$ARCHIVE_NAME"
    else
        echo "❌ Error: Failed to create archive '$ARCHIVE_NAME'"
    fi
fi

echo ""
echo "=============================================="
echo "  Archive Complete"
echo "=============================================="
echo "Total parts created: $part"
echo "Archives location: $(pwd)"
echo "=============================================="
