#!/usr/bin/env bash
# =============================================================================
# Incremental Merge Rulesets v3.1 - Ultra Fast Edition
# Core Optimization: 
#   - Only merge local files, skip remote downloads
#   - Hash-based incremental updates
#   - Parallel processing support
#   - Remote sync handled by sync_metacubex_rules.sh
# =============================================================================

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SURGE_DIR="${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)"
SOURCES_DIR="${PROJECT_ROOT}/ruleset/Sources/Links"
METACUBEX_DIR="${PROJECT_ROOT}/ruleset/MetaCubeX"
CACHE_FILE="${PROJECT_ROOT}/.cache/merge_hashes.txt"

# Arguments
PARALLEL=false
MAX_JOBS=4
FORCE_ALL=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --parallel|-p) PARALLEL=true; shift ;;
        --jobs|-j) MAX_JOBS="$2"; shift 2 ;;
        --force) FORCE_ALL=true; shift ;;
        *) shift ;;
    esac
done

mkdir -p "$(dirname "$CACHE_FILE")"
touch "$CACHE_FILE"

echo -e "${CYAN}=== Incremental Merge Rulesets v3.1 ===${NC}"
[ "$PARALLEL" = true ] && echo "Parallel mode: $MAX_JOBS jobs"

# Statistics (use temp files for parallel mode)
STATS_DIR=$(mktemp -d)
echo "0" > "$STATS_DIR/merged"
echo "0" > "$STATS_DIR/skipped"

# Fast merge function - only process local files
fast_merge() {
    local name="$1"
    local sources_file="${SOURCES_DIR}/${name}_sources.txt"
    local target_file="${SURGE_DIR}/${name}.list"
    
    [ ! -f "$sources_file" ] && return 1
    
    # Calculate sources file hash + MetaCubeX file hash
    local metacubex_file="${METACUBEX_DIR}/${name}.list"
    local hash_input="$sources_file"
    [ -f "$metacubex_file" ] && hash_input="$sources_file $metacubex_file"
    
    local current_hash=$(cat $hash_input 2>/dev/null | md5 -q 2>/dev/null || cat $hash_input | md5sum | cut -d' ' -f1)
    local cached_hash=$(grep "^${name}:" "$CACHE_FILE" 2>/dev/null | cut -d':' -f2)
    
    # Skip if no changes (unless forced)
    if [ "$FORCE_ALL" = false ] && [ -f "$target_file" ] && [ "$current_hash" = "$cached_hash" ]; then
        echo "1" >> "$STATS_DIR/skipped"
        return 0
    fi
    
    echo -ne "${YELLOW}↻${NC} $name... "
    
    # Collect local rules
    local temp_rules=$(mktemp)
    local sources_dir=$(dirname "$sources_file")
    
    # Keep existing rules (non-comment lines)
    # Filter out AND/OR/NOT rules (only valid in config, not ruleset files)
    # Filter out invalid DOMAIN-REGEX and fix IPv6 CIDR
    [ -f "$target_file" ] && grep -v '^#' "$target_file" | grep -v '^$' | \
        grep -v '^AND,' | grep -v '^OR,' | grep -v '^NOT,' | \
        grep -v '^DOMAIN-REGEX,$' | grep -v '^DOMAIN-REGEX,[^,]*$' | \
        sed 's/^IP-CIDR,\([0-9a-fA-F:]*::[^,]*\)/IP-CIDR6,\1/' | \
        # Remove policies and options from rules
        sed 's/,\(REJECT\|DIRECT\|PROXY\|REJECT-DROP\|REJECT-TINYGIF\|REJECT-NO-DROP\|REJECT-IMG\)\(,.*\)*$//' | \
        sed 's/,extended-matching//g; s/,pre-matching//g' >> "$temp_rules" 2>/dev/null
    
    # Add MetaCubeX rules if available
    [ -f "$metacubex_file" ] && grep -E '^(DOMAIN|IP-CIDR|PROCESS-NAME|URL-REGEX|USER-AGENT)' "$metacubex_file" >> "$temp_rules" 2>/dev/null
    
    # Read local files from sources
    while IFS= read -r line; do
        [[ -z "$line" || "$line" =~ ^# ]] && continue
        local path="${line%|*}"
        
        # Only process local files (skip URLs starting with http)
        if [[ "$path" =~ ^https?:// ]]; then
            continue
        fi
        
        # Parse local path
        local local_file=""
        if [[ "$path" == ../* || "$path" == ./* ]]; then
            local_file="$sources_dir/$path"
        else
            local_file="$sources_dir/$path"
        fi
        
        # Extract rules with filtering
        if [ -f "$local_file" ]; then
            grep -E '^(DOMAIN|IP-CIDR|PROCESS-NAME|URL-REGEX|USER-AGENT)' "$local_file" 2>/dev/null | \
                grep -v '^AND,' | grep -v '^OR,' | grep -v '^NOT,' | \
                grep -v '^DOMAIN-REGEX,$' | grep -v '^DOMAIN-REGEX,[^,]*$' | \
                sed 's/^IP-CIDR,\([0-9a-fA-F:]*::[^,]*\)/IP-CIDR6,\1/' | \
        # Remove policies and options from rules
        sed 's/,\(REJECT\|DIRECT\|PROXY\|REJECT-DROP\|REJECT-TINYGIF\|REJECT-NO-DROP\|REJECT-IMG\)\(,.*\)*$//' | \
        sed 's/,extended-matching//g; s/,pre-matching//g' >> "$temp_rules" 2>/dev/null
        fi
    done < "$sources_file"
    
    # Dedup and generate output
    local rule_count=$(sort -u "$temp_rules" | wc -l | tr -d ' ')
    
    # Generate header
    cat > "$target_file" << EOF
# ═══════════════════════════════════════════════════════════════
# Ruleset: ${name}
# Updated: $(date '+%Y-%m-%d %H:%M')
# Rules: ${rule_count}
# ═══════════════════════════════════════════════════════════════

EOF
    
    # Add deduplicated rules
    sort -u "$temp_rules" >> "$target_file"
    rm -f "$temp_rules"
    
    # Update cache (atomic with lock for parallel mode)
    (
        flock -x 200 2>/dev/null || true
        grep -v "^${name}:" "$CACHE_FILE" > "$CACHE_FILE.tmp" 2>/dev/null || true
        echo "${name}:${current_hash}" >> "$CACHE_FILE.tmp"
        mv "$CACHE_FILE.tmp" "$CACHE_FILE"
    ) 200>"$CACHE_FILE.lock" 2>/dev/null
    
    echo -e "${GREEN}✓${NC} (${rule_count} rules)"
    echo "1" >> "$STATS_DIR/merged"
}

# All rulesets
RULESETS=(
    GlobalMedia AI Gaming GlobalProxy Microsoft Discord Fediverse NSFW LAN
    SocialMedia Telegram TikTok Twitter Instagram Reddit WeChat
    YouTube Netflix Disney Spotify Bahamut AppleNews
    Google Bing Apple GitHub PayPal Tesla Binance
    Steam Epic
    ChinaDirect Bilibili QQ Tencent XiaoHongShu NetEaseMusic GoogleCN
    CDN Speedtest
    StreamJP StreamUS StreamKR StreamHK StreamTW StreamEU
    AIProcess DirectProcess DownloadProcess GamingProcess
    AdBlock
)

# Process rulesets
if [ "$PARALLEL" = true ]; then
    # Parallel processing
    job_count=0
    for name in "${RULESETS[@]}"; do
        fast_merge "$name" &
        job_count=$((job_count + 1))
        if [ $job_count -ge $MAX_JOBS ]; then
            wait -n 2>/dev/null || wait
            job_count=$((job_count - 1))
        fi
    done
    wait
else
    # Sequential processing
    for name in "${RULESETS[@]}"; do
        fast_merge "$name"
    done
fi

# Calculate statistics
merged=$(wc -l < "$STATS_DIR/merged" | tr -d ' ')
skipped=$(wc -l < "$STATS_DIR/skipped" | tr -d ' ')
rm -rf "$STATS_DIR"

echo ""
echo -e "${CYAN}=== Complete ===${NC}"
echo -e "  Merged: ${GREEN}${merged}${NC}"
echo -e "  Skipped: ${skipped} (no changes)"
