#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
# Ruleset Header Update Script v2.1 (Performance Optimized - macOS Compatible)
# Function: Add policy suggestions and node recommendations to rulesets
# Optimization: Batch processing + Hash-based incremental detection
# ═══════════════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
RULESET_DIR="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)"
POLICY_MAP="$SCRIPT_DIR/ruleset_policy_map.txt"
CACHE_DIR="$PROJECT_ROOT/.cache"
HASH_FILE="$CACHE_DIR/header_hashes.txt"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Quiet mode detection
QUIET=${QUIET:-false}
if [ "$QUIET" = true ]; then
    exec 3>/dev/null
else
    exec 3>&1
fi

echo "╔══════════════════════════════════════════╗" >&3
echo "║   Ruleset Header Update Tool v2.1        ║" >&3
echo "╚══════════════════════════════════════════╝" >&3

# Check policy mapping file
if [ ! -f "$POLICY_MAP" ]; then
    echo "Policy mapping file not found: $POLICY_MAP"
    exit 1
fi

# Create cache directory
mkdir -p "$CACHE_DIR"
touch "$HASH_FILE"

# Statistics
updated=0
skipped=0
no_policy=0

# Current date (calculate once)
CURRENT_DATE=$(date '+%Y-%m-%d %H:%M:%S')

# Process each ruleset
for ruleset_file in "$RULESET_DIR"/*.list; do
    [ ! -f "$ruleset_file" ] && continue
    filename=$(basename "$ruleset_file" .list)
    
    # Skip backup files
    [[ "$filename" == *.backup* ]] && continue
    
    # Get info from policy mapping file
    policy_line=$(grep "^${filename}|" "$POLICY_MAP" | head -1)
    
    if [ -z "$policy_line" ]; then
        ((no_policy++)) || true
        continue
    fi
    
    # Parse policy info
    policy=$(echo "$policy_line" | cut -d'|' -f2)
    node=$(echo "$policy_line" | cut -d'|' -f3)
    desc=$(echo "$policy_line" | cut -d'|' -f4)
    
    # Calculate rule content hash (non-comment lines only, sorted)
    content_hash=$(grep -v '^#' "$ruleset_file" 2>/dev/null | grep -v '^$' | sort | md5 -q 2>/dev/null || md5sum | cut -d' ' -f1)
    
    # Incremental detection: check old hash
    old_hash=$(grep "^${filename}|" "$HASH_FILE" 2>/dev/null | cut -d'|' -f2)
    
    if [ "$old_hash" = "$content_hash" ]; then
        ((skipped++)) || true
        continue
    fi
    
    # Calculate rule count
    rule_count=$(grep -cv '^#\|^$' "$ruleset_file" 2>/dev/null || echo "0")
    
    # Generate new header
    header="# ═══════════════════════════════════════════════════════════════
# Ruleset: ${filename}
# Policy: ${policy}"
    
    # Add node recommendation (if any)
    if [ -n "$node" ]; then
        header+="
# Node: ${node}"
    fi
    
    header+="
# Description: ${desc}
# Rules: ${rule_count}
# Updated: ${CURRENT_DATE}
# ═══════════════════════════════════════════════════════════════
"
    
    # Extract existing rules (skip old header)
    rules=$(grep -v '^#' "$ruleset_file" | grep -v '^$')
    
    # Write new file
    {
        echo -n "$header"
        echo ""
        echo "$rules"
    } > "$ruleset_file"
    
    # Update hash file (remove old record, add new record)
    grep -v "^${filename}|" "$HASH_FILE" > "$HASH_FILE.tmp" 2>/dev/null || true
    echo "${filename}|${content_hash}" >> "$HASH_FILE.tmp"
    mv "$HASH_FILE.tmp" "$HASH_FILE"
    
    echo -e "${GREEN}Updated: $filename (${policy}${node:+, $node})${NC}" >&3
    ((updated++)) || true
done

echo "" >&3
echo "╔══════════════════════════════════════════╗" >&3
echo "║            Update Complete               ║" >&3
printf "║  Updated: %-3d  Skipped: %-3d  NoMap: %-3d ║\n" "$updated" "$skipped" "$no_policy" >&3
echo "╚══════════════════════════════════════════╝" >&3
