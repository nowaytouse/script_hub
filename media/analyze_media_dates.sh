#!/usr/bin/env bash
# ============================================================================
# 📅 Media Date Analyzer - Deep EXIF/XMP Date Analysis
# ============================================================================
# Priority: XMP-photoshop:DateCreated > XMP:CreateDate > DateTimeOriginal
# Excludes unreliable FileModifyDate (download/copy time)
# ============================================================================

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

# Configuration
CURRENT_YEAR=$(date +%Y)
MIN_VALID_YEAR=1990
MAX_VALID_YEAR=$((CURRENT_YEAR + 1))

TARGET_DIR="${1:-}"

if [[ -z "$TARGET_DIR" ]]; then
    echo -e "${RED}❌ Usage: $(basename "$0") <directory>${NC}"
    exit 1
fi

if [[ ! -d "$TARGET_DIR" ]]; then
    echo -e "${RED}❌ Directory not found: $TARGET_DIR${NC}"
    exit 1
fi

echo -e "${CYAN}📅 Media Date Analyzer (Deep XMP Analysis)${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "📁 Directory: ${YELLOW}$TARGET_DIR${NC}"

# Create temp file
TEMP_JSON=$(mktemp)
trap "rm -f $TEMP_JSON" EXIT

# Count files
echo -e "\n🔍 Scanning files..."
FILE_COUNT=$(find "$TARGET_DIR" -type f \( \
    -iname "*.jpg" -o -iname "*.jpeg" -o -iname "*.png" -o \
    -iname "*.gif" -o -iname "*.webp" -o -iname "*.mp4" -o \
    -iname "*.mov" -o -iname "*.jfif" -o -iname "*.heic" \
\) 2>/dev/null | wc -l | tr -d ' ')

echo -e "📊 Found ${GREEN}$FILE_COUNT${NC} media files"

if [[ $FILE_COUNT -eq 0 ]]; then
    echo -e "${YELLOW}⚠️  No media files found${NC}"
    exit 0
fi

# Extract ALL date fields using exiftool JSON (deep extraction)
echo -e "\n⏳ Deep extracting XMP/EXIF dates..."

# Priority fields (most reliable to least):
# 1. XMP-photoshop:DateCreated - Photoshop original creation
# 2. XMP-xmp:CreateDate - XMP creation date
# 3. XMP-xmp:MetadataDate - When metadata was last modified
# 4. XMP-xmpMM:HistoryWhen - Edit history timestamps
# 5. DateTimeOriginal - Camera original
# 6. CreateDate - Generic creation
# 7. ModifyDate - Last modification (NOT FileModifyDate!)

exiftool -r -j -G1 \
    -XMP-photoshop:DateCreated \
    -XMP-xmp:CreateDate \
    -XMP-xmp:MetadataDate \
    -XMP-xmp:ModifyDate \
    -XMP-xmpMM:HistoryWhen \
    -EXIF:DateTimeOriginal \
    -EXIF:CreateDate \
    -EXIF:ModifyDate \
    -FileName \
    -ext jpg -ext jpeg -ext png -ext gif -ext webp -ext mp4 -ext mov -ext jfif -ext heic \
    "$TARGET_DIR" 2>/dev/null > "$TEMP_JSON"

# Process with jq for high performance
echo -e "\n📊 Analyzing dates (excluding FileModifyDate)..."

ANALYSIS=$(jq -r --arg min "$MIN_VALID_YEAR" --arg max "$MAX_VALID_YEAR" '
def extract_year:
    if . == null or . == "" or . == "-" then null
    elif type == "array" then .[0] | extract_year
    elif startswith("0000") then null
    else capture("^(?<y>[0-9]{4})").y // null
    end;

def normalize_date:
    if . == null or . == "" or . == "-" then null
    elif type == "array" then .[0] | normalize_date
    elif startswith("0000") then null
    else gsub("[+].*$"; "") | gsub("T"; " ")
    end;

def valid_year:
    if . == null then false
    else (. | tonumber) >= ($min | tonumber) and (. | tonumber) <= ($max | tonumber)
    end;

[.[] | {
    file: .FileName,
    # XMP dates (most reliable for artwork)
    xmp_ps_created: (.["XMP-photoshop:DateCreated"] // ""),
    xmp_created: (.["XMP-xmp:CreateDate"] // ""),
    xmp_metadata: (.["XMP-xmp:MetadataDate"] // ""),
    xmp_modified: (.["XMP-xmp:ModifyDate"] // ""),
    xmp_history: (.["XMP-xmpMM:HistoryWhen"] // ""),
    # EXIF dates
    exif_original: (.["EXIF:DateTimeOriginal"] // ""),
    exif_created: (.["EXIF:CreateDate"] // ""),
    exif_modified: (.["EXIF:ModifyDate"] // "")
} |
# Find best date with strict priority (NO FileModifyDate!)
. + {
    best_date: (
        # Priority 1: XMP-photoshop DateCreated (Photoshop artwork)
        if (.xmp_ps_created | extract_year) != null and ((.xmp_ps_created | extract_year) | valid_year) 
            then .xmp_ps_created | normalize_date
        # Priority 2: XMP CreateDate
        elif (.xmp_created | extract_year) != null and ((.xmp_created | extract_year) | valid_year)
            then .xmp_created | normalize_date
        # Priority 3: XMP History (first entry = creation)
        elif (.xmp_history | extract_year) != null and ((.xmp_history | extract_year) | valid_year)
            then .xmp_history | normalize_date
        # Priority 4: EXIF DateTimeOriginal
        elif (.exif_original | extract_year) != null and ((.exif_original | extract_year) | valid_year)
            then .exif_original | normalize_date
        # Priority 5: EXIF CreateDate
        elif (.exif_created | extract_year) != null and ((.exif_created | extract_year) | valid_year)
            then .exif_created | normalize_date
        # Priority 6: XMP MetadataDate (fallback)
        elif (.xmp_metadata | extract_year) != null and ((.xmp_metadata | extract_year) | valid_year)
            then .xmp_metadata | normalize_date
        else null
        end
    ),
    date_source: (
        if (.xmp_ps_created | extract_year) != null and ((.xmp_ps_created | extract_year) | valid_year) 
            then "XMP-Photoshop"
        elif (.xmp_created | extract_year) != null and ((.xmp_created | extract_year) | valid_year)
            then "XMP-CreateDate"
        elif (.xmp_history | extract_year) != null and ((.xmp_history | extract_year) | valid_year)
            then "XMP-History"
        elif (.exif_original | extract_year) != null and ((.exif_original | extract_year) | valid_year)
            then "EXIF-Original"
        elif (.exif_created | extract_year) != null and ((.exif_created | extract_year) | valid_year)
            then "EXIF-CreateDate"
        elif (.xmp_metadata | extract_year) != null and ((.xmp_metadata | extract_year) | valid_year)
            then "XMP-Metadata"
        else "None"
        end
    )
}]
| {
    with_dates: [.[] | select(.best_date != null)],
    without_dates: [.[] | select(.best_date == null)]
}
| {
    total_with_dates: (.with_dates | length),
    total_without_dates: (.without_dates | length),
    sorted: (.with_dates | sort_by(.best_date)),
    by_source: (.with_dates | group_by(.date_source) | map({source: .[0].date_source, count: length})),
    by_year: (.with_dates | group_by(.best_date[0:4]) | map({year: .[0].best_date[0:4], count: length}) | sort_by(.year)),
    by_month: (.with_dates | group_by(.best_date[0:7]) | map({month: .[0].best_date[0:7], count: length}) | sort_by(.month)),
    by_day: (.with_dates | group_by(.best_date[0:10]) | map({day: .[0].best_date[0:10], count: length}) | sort_by(.day))
}
| . + {
    earliest: (if (.sorted | length) > 0 then .sorted[0] else null end),
    latest: (if (.sorted | length) > 0 then .sorted[-1] else null end)
}
' "$TEMP_JSON")

# Extract results
VALID_COUNT=$(echo "$ANALYSIS" | jq -r '.total_with_dates')
NO_DATE_COUNT=$(echo "$ANALYSIS" | jq -r '.total_without_dates')
EARLIEST_DATE=$(echo "$ANALYSIS" | jq -r '.earliest.best_date // "N/A"')
EARLIEST_FILE=$(echo "$ANALYSIS" | jq -r '.earliest.file // "N/A"')
EARLIEST_SRC=$(echo "$ANALYSIS" | jq -r '.earliest.date_source // "N/A"')
LATEST_DATE=$(echo "$ANALYSIS" | jq -r '.latest.best_date // "N/A"')
LATEST_FILE=$(echo "$ANALYSIS" | jq -r '.latest.file // "N/A"')
LATEST_SRC=$(echo "$ANALYSIS" | jq -r '.latest.date_source // "N/A"')

echo -e "\n${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}📊 Deep Analysis Results${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

echo -e "\n📈 Statistics:"
echo -e "   Total files:           ${YELLOW}$FILE_COUNT${NC}"
echo -e "   With reliable dates:   ${GREEN}$VALID_COUNT${NC}"
echo -e "   Without dates:         ${RED}$NO_DATE_COUNT${NC}"

echo -e "\n📋 Date Source Distribution:"
echo "$ANALYSIS" | jq -r '.by_source[] | "   \(.source): \(.count) files"'

if [[ $VALID_COUNT -gt 0 ]]; then
    echo -e "\n${MAGENTA}📅 TRUE Date Range (Original Creation Time):${NC}"
    echo -e "   ${GREEN}Earliest:${NC} $EARLIEST_DATE"
    echo -e "   ${CYAN}File:${NC}     $EARLIEST_FILE"
    echo -e "   ${CYAN}Source:${NC}   $EARLIEST_SRC"
    echo -e ""
    echo -e "   ${GREEN}Latest:${NC}   $LATEST_DATE"
    echo -e "   ${CYAN}File:${NC}     $LATEST_FILE"
    echo -e "   ${CYAN}Source:${NC}   $LATEST_SRC"
    
    echo -e "\n📆 Distribution by Year:"
    echo "$ANALYSIS" | jq -r '.by_year[] | "\(.year)|\(.count)"' | while IFS='|' read -r year count; do
        pct=$((count * 100 / VALID_COUNT))
        bar_len=$((pct / 3 + 1))
        bar=""
        for ((i=0; i<bar_len; i++)); do bar+="█"; done
        printf "   %s: %4d files (%2d%%) %s\n" "$year" "$count" "$pct" "$bar"
    done
    
    echo -e "\n📆 Distribution by Month (Top 15):"
    echo "$ANALYSIS" | jq -r '[.by_month[] | {month, count}] | sort_by(-.count) | .[0:15][] | "\(.month)|\(.count)"' | while IFS='|' read -r month count; do
        printf "   %s: %4d files\n" "$month" "$count"
    done
    
    echo -e "\n📆 Distribution by Day (Top 10):"
    echo "$ANALYSIS" | jq -r '[.by_day[] | {day, count}] | sort_by(-.count) | .[0:10][] | "\(.day)|\(.count)"' | while IFS='|' read -r day count; do
        printf "   %s: %4d files\n" "$day" "$count"
    done
fi

echo -e "\n${GREEN}✅ Deep analysis complete!${NC}"
