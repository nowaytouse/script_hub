#!/usr/bin/env bash
# =============================================================================
# MetaCubeX Sources Incremental Update Script v2.0
# Function: Detect MetaCubeX rule changes, truly incremental update to Sources
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SOURCES_DIR="${PROJECT_ROOT}/ruleset/Sources/Links"
METACUBEX_DIR="${PROJECT_ROOT}/ruleset/MetaCubeX"
CACHE_FILE="${PROJECT_ROOT}/.cache/metacubex_hashes.txt"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# Ensure cache directory exists
mkdir -p "$(dirname "$CACHE_FILE")"
touch "$CACHE_FILE"

echo -e "${CYAN}=== MetaCubeX Sources Incremental Update ===${NC}"

# Statistics
updated=0
added=0
unchanged=0
missing=0

# Mapping: Sources name -> MetaCubeX filename
declare -A MAPPING=(
    ["Telegram"]="telegram"
    ["Discord"]="discord"
    ["Google"]="google"
    ["Apple"]="apple"
    ["Microsoft"]="microsoft"
    ["Bilibili"]="bilibili"
    ["AI"]="category-ai-cn"
    ["Gaming"]="category-games"
    ["AdBlock"]="category-ads-all"
    ["Instagram"]="instagram"
    ["Twitter"]="twitter"
    ["Spotify"]="spotify"
    ["Steam"]="steam"
    ["YouTube"]="youtube"
    ["GitHub"]="github"
    ["Netflix"]="netflix"
    ["TikTok"]="tiktok"
    ["PayPal"]="paypal"
    ["SocialMedia"]="facebook"
    ["Reddit"]="reddit"
    ["GlobalProxy"]="cloudflare"
)

for name in "${!MAPPING[@]}"; do
    metacubex="${MAPPING[$name]}"
    sources_file="${SOURCES_DIR}/${name}_sources.txt"
    metacubex_file="${METACUBEX_DIR}/MetaCubeX_${metacubex}.list"
    
    # Check if files exist
    if [ ! -f "$sources_file" ]; then
        ((missing++))
        continue
    fi
    
    if [ ! -f "$metacubex_file" ]; then
        ((missing++))
        continue
    fi
    
    # Calculate MetaCubeX file hash
    current_hash=$(md5 -q "$metacubex_file" 2>/dev/null || md5sum "$metacubex_file" | cut -d' ' -f1)
    cached_hash=$(grep "^${name}:" "$CACHE_FILE" 2>/dev/null | cut -d':' -f2)
    
    # Check if Sources already contains MetaCubeX reference
    has_reference=$(grep -c "MetaCubeX_${metacubex}.list" "$sources_file" 2>/dev/null | tr -d '[:space:]' || echo "0")
    [ -z "$has_reference" ] && has_reference=0
    
    if [ "$has_reference" -eq 0 ]; then
        # First time adding
        echo -e "${GREEN}+ Add:${NC} $name <- MetaCubeX_${metacubex}.list"
        echo "" >> "$sources_file"
        echo "# MetaCubeX local rules" >> "$sources_file"
        echo "../MetaCubeX/MetaCubeX_${metacubex}.list" >> "$sources_file"
        # Update cache
        grep -v "^${name}:" "$CACHE_FILE" > "$CACHE_FILE.tmp" 2>/dev/null || true
        echo "${name}:${current_hash}" >> "$CACHE_FILE.tmp"
        mv "$CACHE_FILE.tmp" "$CACHE_FILE"
        ((added++))
    elif [ "$current_hash" != "$cached_hash" ]; then
        # MetaCubeX file updated
        rule_count=$(grep -v '^#' "$metacubex_file" | grep -v '^$' | wc -l | tr -d ' ')
        echo -e "${YELLOW}↻ Update:${NC} $name (MetaCubeX changed, ${rule_count} rules)"
        # Update cache
        grep -v "^${name}:" "$CACHE_FILE" > "$CACHE_FILE.tmp" 2>/dev/null || true
        echo "${name}:${current_hash}" >> "$CACHE_FILE.tmp"
        mv "$CACHE_FILE.tmp" "$CACHE_FILE"
        ((updated++))
    else
        # No changes
        ((unchanged++))
    fi
done

echo ""
echo -e "${CYAN}=== Statistics ===${NC}"
echo -e "  Added: ${GREEN}${added}${NC}"
echo -e "  Updated: ${YELLOW}${updated}${NC}"
echo -e "  Unchanged: ${unchanged}"
[ $missing -gt 0 ] && echo -e "  Missing: ${missing}"

# If there are updates, hint that rules need to be re-merged
if [ $((added + updated)) -gt 0 ]; then
    echo ""
    echo -e "${CYAN}Changes detected, subsequent steps will re-merge rules${NC}"
fi
