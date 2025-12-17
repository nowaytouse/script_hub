#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# AdBlock Module Smart Merge Script v4.2 (with Whitelist Support)
# Optimization: Use sort -u instead of line-by-line grep dedup, batch processing
# New: Whitelist mechanism to prevent false positives (e.g., WeChat redirect)
# ═══════════════════════════════════════════════════════════════════════════════

set -e

# PATHS
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SURGE_MODULE_DIR="$PROJECT_ROOT/module/surge(main)"
TEMP_DIR="$PROJECT_ROOT/.temp_adblock_merge"
TARGET_MODULE="$SURGE_MODULE_DIR/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"
ADBLOCK_MERGED_LIST="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock.list"
CACHE_DIR="$PROJECT_ROOT/.cache"
HASH_FILE="$CACHE_DIR/adblock_hashes.txt"
WHITELIST_FILE="$PROJECT_ROOT/ruleset/Sources/adblock_whitelist.txt"

# UTILS
log_info() { echo -e "\033[0;36m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[✓]\033[0m $1"; }
log_warning() { echo -e "\033[1;33m[⚠]\033[0m $1"; }
log_error() { echo -e "\033[0;31m[✗]\033[0m $1"; }

# 🔥 Load whitelist patterns for filtering
load_whitelist() {
    if [ -f "$WHITELIST_FILE" ]; then
        # Extract domain patterns from whitelist (ignore comments and empty lines)
        grep -v '^#' "$WHITELIST_FILE" | grep -v '^$' | \
        sed 's/^DOMAIN-SUFFIX,//' | sed 's/^DOMAIN-KEYWORD,//' | sed 's/^DOMAIN,//' | \
        sort -u > "$TEMP_DIR/whitelist_domains.tmp"
        
        WHITELIST_COUNT=$(wc -l < "$TEMP_DIR/whitelist_domains.tmp" 2>/dev/null | tr -d ' ' || echo "0")
        log_info "Loaded $WHITELIST_COUNT whitelist patterns from adblock_whitelist.txt"
    else
        touch "$TEMP_DIR/whitelist_domains.tmp"
        log_warning "Whitelist file not found: $WHITELIST_FILE"
    fi
}

# 🔥 Filter rules against whitelist
filter_whitelist() {
    local input_file="$1"
    local output_file="$2"
    
    if [ ! -s "$TEMP_DIR/whitelist_domains.tmp" ]; then
        # No whitelist, just copy
        cp "$input_file" "$output_file"
        return
    fi
    
    # Build grep pattern from whitelist
    local whitelist_pattern=$(cat "$TEMP_DIR/whitelist_domains.tmp" | tr '\n' '|' | sed 's/|$//')
    
    if [ -n "$whitelist_pattern" ]; then
        # Filter out rules matching whitelist domains
        grep -vE "$whitelist_pattern" "$input_file" > "$output_file" 2>/dev/null || cp "$input_file" "$output_file"
        
        # Count filtered rules
        local before=$(wc -l < "$input_file" 2>/dev/null | tr -d ' ' || echo "0")
        local after=$(wc -l < "$output_file" 2>/dev/null | tr -d ' ' || echo "0")
        local filtered=$((before - after))
        
        if [ "$filtered" -gt 0 ]; then
            log_warning "Whitelist filtered: $filtered rules removed (before: $before, after: $after)"
        fi
    else
        cp "$input_file" "$output_file"
    fi
}

# INIT
rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR" "$CACHE_DIR"
touch "$HASH_FILE"

# Load whitelist
load_whitelist

# Check if update needed
needs_update=false
new_hashes=""

# Collect all module files and check hashes
for module in "$SURGE_MODULE_DIR"/*.sgmodule; do
    [ ! -f "$module" ] && continue
    [[ "$module" == "$TARGET_MODULE" ]] && continue
    
    # Only process modules containing ad-related keywords
    if grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then
        name=$(basename "$module")
        hash=$(md5 -q "$module" 2>/dev/null || md5sum "$module" | cut -d' ' -f1)
        new_hashes="${new_hashes}${name}|${hash}\n"
        
        # Check old hash
        old_hash=$(grep "^${name}|" "$HASH_FILE" 2>/dev/null | cut -d'|' -f2)
        if [ "$old_hash" != "$hash" ]; then
            needs_update=true
        fi
    fi
done

# Check if target module exists
if [ ! -f "$TARGET_MODULE" ]; then
    needs_update=true
fi

# If no update needed, exit
if [ "$needs_update" = false ] && [ -f "$ADBLOCK_MERGED_LIST" ]; then
    log_info "No changes in AdBlock modules, skipping merge"
    exit 0
fi

log_info "Changes detected, starting merge..."

# Temp files
ALL_REJECT="$TEMP_DIR/all_reject.tmp"
ALL_REJECT_DROP="$TEMP_DIR/all_reject_drop.tmp"
ALL_REJECT_NO_DROP="$TEMP_DIR/all_reject_no_drop.tmp"
ALL_DIRECT="$TEMP_DIR/all_direct.tmp"

touch "$ALL_REJECT" "$ALL_REJECT_DROP" "$ALL_REJECT_NO_DROP" "$ALL_DIRECT"

# Batch extract rules (process all modules at once)
for module in "$SURGE_MODULE_DIR"/*.sgmodule; do
    [ ! -f "$module" ] && continue
    [[ "$module" == "$TARGET_MODULE" ]] && continue
    
    # Only process modules containing ad-related keywords
    if grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then
        log_info "Processing: $(basename "$module")"
        
        # Extract [Rule] section and classify
        awk '/^\[Rule\]/{f=1;next}/^\[/{f=0}f' "$module" 2>/dev/null | \
        grep -v '^#' | grep -v '^$' | grep -v '^RULE-SET' | \
        sed 's/  */ /g' | while read -r line; do
            case "$line" in
                *,REJECT-DROP*) echo "$line" >> "$ALL_REJECT_DROP" ;;
                *,REJECT-NO-DROP*) echo "$line" >> "$ALL_REJECT_NO_DROP" ;;
                *,REJECT*) echo "$line" >> "$ALL_REJECT" ;;
                *,DIRECT*) echo "$line" >> "$ALL_DIRECT" ;;
            esac
        done
    fi
done

# Extract rules from existing target module
if [ -f "$TARGET_MODULE" ]; then
    awk '/^\[Rule\]/{f=1;next}/^\[/{f=0}f' "$TARGET_MODULE" 2>/dev/null | \
    grep -v '^#' | grep -v '^$' | grep -v '^RULE-SET' | \
    sed 's/  */ /g' | while read -r line; do
        case "$line" in
            *,REJECT-DROP*) echo "$line" >> "$ALL_REJECT_DROP" ;;
            *,REJECT-NO-DROP*) echo "$line" >> "$ALL_REJECT_NO_DROP" ;;
            *,REJECT*) echo "$line" >> "$ALL_REJECT" ;;
            *,DIRECT*) echo "$line" >> "$ALL_DIRECT" ;;
        esac
    done
fi

# Use sort -u for batch dedup (100x faster than line-by-line grep)
sort -u "$ALL_REJECT" -o "$ALL_REJECT" 2>/dev/null || true
sort -u "$ALL_REJECT_DROP" -o "$ALL_REJECT_DROP" 2>/dev/null || true
sort -u "$ALL_REJECT_NO_DROP" -o "$ALL_REJECT_NO_DROP" 2>/dev/null || true
sort -u "$ALL_DIRECT" -o "$ALL_DIRECT" 2>/dev/null || true

# ⚠️ IMPORTANT: Do NOT overwrite TARGET_MODULE!
# The module file is manually maintained with URL Rewrite, Body Rewrite, Map Local, MITM configs.
# This script ONLY updates the AdBlock.list ruleset file.
log_info "Skipping module generation (manually maintained)"

# Export rule files
sort -u "$ALL_REJECT_DROP" > "$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/reject-drop.conf" 2>/dev/null || true
sort -u "$ALL_REJECT_NO_DROP" > "$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/reject-no-drop.conf" 2>/dev/null || true

# Export DIRECT rules
if [ -s "$ALL_DIRECT" ]; then
    mkdir -p "$PROJECT_ROOT/ruleset/Sources/conf"
    sort -u "$ALL_DIRECT" > "$PROJECT_ROOT/ruleset/Sources/conf/SurgeConf_ModulesDirect.list"
fi

# Export merged AdBlock list
if [ -f "$ADBLOCK_MERGED_LIST" ]; then
    # Keep old rules
    grep -v "^#" "$ADBLOCK_MERGED_LIST" | grep -v "^$" | grep -v "^RULE-SET" > "$TEMP_DIR/old_adblock.tmp" 2>/dev/null || true
else
    touch "$TEMP_DIR/old_adblock.tmp"
fi

# Merge old and new rules, dedup
cat "$TEMP_DIR/old_adblock.tmp" "$ALL_REJECT" 2>/dev/null | sort -u > "$TEMP_DIR/merged_adblock.tmp"

# Clean invalid rules
sed 's/^\(DOMAIN[^,]*,[^,]*\),no-resolve$/\1/' "$TEMP_DIR/merged_adblock.tmp" 2>/dev/null | \
grep -v "^RULE-SET" > "$TEMP_DIR/clean_adblock_pre.tmp" || true

# 🔥 Apply whitelist filter (remove rules that match whitelist domains)
filter_whitelist "$TEMP_DIR/clean_adblock_pre.tmp" "$TEMP_DIR/clean_adblock.tmp"

# Add header
rule_count=$(wc -l < "$TEMP_DIR/clean_adblock.tmp" 2>/dev/null | tr -d ' ' || echo "0")
{
    echo "# Ruleset: AdBlock"
    echo "# Updated: $(date)"
    echo "# Total Rules: $rule_count"
    echo "# ═══════════════════════════════════════════════════════════════"
    echo ""
    cat "$TEMP_DIR/clean_adblock.tmp"
} > "$ADBLOCK_MERGED_LIST"

# Save new hashes
echo -e "$new_hashes" > "$HASH_FILE"

# Cleanup
rm -rf "$TEMP_DIR"

log_success "AdBlock Merged: $rule_count rules"
