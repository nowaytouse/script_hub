#!/usr/bin/env bash
# Download AdBlock modules from URLs in AdBlock_sources.txt
# Extract rules and merge into AdBlock.list

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SOURCES_FILE="$PROJECT_ROOT/ruleset/Sources/Links/AdBlock_sources.txt"
TEMP_DIR="$PROJECT_ROOT/.temp_adblock_download"
MODULE_DIR="$PROJECT_ROOT/module/adblock_external"
ADBLOCK_LIST="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock.list"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;36m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }

# Init
rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR" "$MODULE_DIR"

log_info "Downloading AdBlock modules..."

# Extract module URLs from sources file
module_urls=$(grep -E "^https?://.*\.sgmodule$|^https?://.*\.module$" "$SOURCES_FILE" 2>/dev/null || true)

if [ -z "$module_urls" ]; then
    log_warning "No module URLs found in $SOURCES_FILE"
    exit 0
fi

# Download each module
downloaded=0
failed=0
total=0

while IFS= read -r url; do
    [ -z "$url" ] && continue
    total=$((total + 1))
    
    # Extract filename from URL
    filename=$(basename "$url")
    module_file="$MODULE_DIR/$filename"
    temp_file="$TEMP_DIR/$filename"
    
    log_info "Downloading: $filename"
    
    # Download with timeout
    if curl -L -s -m 30 -o "$temp_file" "$url" 2>/dev/null; then
        # Check if file is valid (not empty, not HTML error page)
        if [ -s "$temp_file" ] && ! grep -q "<!DOCTYPE html>" "$temp_file" 2>/dev/null; then
            # Check if file changed
            if [ -f "$module_file" ]; then
                if ! diff -q "$temp_file" "$module_file" >/dev/null 2>&1; then
                    cp "$temp_file" "$module_file"
                    log_success "Updated: $filename"
                else
                    log_info "No change: $filename"
                fi
            else
                cp "$temp_file" "$module_file"
                log_success "Downloaded: $filename"
            fi
            downloaded=$((downloaded + 1))
        else
            log_error "Invalid file: $filename (may be 403/404)"
            failed=$((failed + 1))
        fi
    else
        log_error "Download failed: $filename"
        failed=$((failed + 1))
    fi
done <<< "$module_urls"

log_info ""
log_info "Download Summary:"
log_info "  Total:      $total"
log_success "  Success:    $downloaded"
[ $failed -gt 0 ] && log_error "  Failed:     $failed" || log_info "  Failed:     0"

# Extract rules from downloaded modules
log_info ""
log_info "Extracting rules from modules..."

ALL_RULES="$TEMP_DIR/all_module_rules.tmp"
touch "$ALL_RULES"

for module in "$MODULE_DIR"/*.sgmodule "$MODULE_DIR"/*.module; do
    [ ! -f "$module" ] && continue
    
    log_info "Processing: $(basename "$module")"
    
    # Extract [Rule] section
    awk '/^\[Rule\]/{f=1;next}/^\[/{f=0}f' "$module" 2>/dev/null | \
    grep -v '^#' | grep -v '^$' | grep -v '^RULE-SET' | \
    sed 's/  */ /g' | \
    grep -E "^DOMAIN|^IP-CIDR|^USER-AGENT|^URL-REGEX|^PROCESS-NAME" >> "$ALL_RULES" || true
done

# Count extracted rules
rule_count=$(wc -l < "$ALL_RULES" 2>/dev/null | tr -d ' ' || echo "0")
log_info "Extracted $rule_count rules from modules"

if [ "$rule_count" -gt 0 ]; then
    # Merge with existing AdBlock.list
    log_info "Merging with existing AdBlock.list..."
    
    # Extract existing rules (without header)
    if [ -f "$ADBLOCK_LIST" ]; then
        grep -v "^#" "$ADBLOCK_LIST" | grep -v "^$" > "$TEMP_DIR/existing_rules.tmp" 2>/dev/null || true
    else
        touch "$TEMP_DIR/existing_rules.tmp"
    fi
    
    # Merge and deduplicate
    cat "$TEMP_DIR/existing_rules.tmp" "$ALL_RULES" 2>/dev/null | \
    sort -u > "$TEMP_DIR/merged_rules.tmp"
    
    # Count final rules
    final_count=$(wc -l < "$TEMP_DIR/merged_rules.tmp" 2>/dev/null | tr -d ' ' || echo "0")
    
    # Generate new AdBlock.list with header
    {
        echo "# Ruleset: AdBlock"
        echo "# Updated: $(date)"
        echo "# Total Rules: $final_count"
        echo "# Includes: Module rules + Source rules"
        echo "# ═══════════════════════════════════════════════════════════════"
        echo ""
        cat "$TEMP_DIR/merged_rules.tmp"
    } > "$ADBLOCK_LIST"
    
    log_success "AdBlock.list updated: $final_count total rules"
else
    log_warning "No rules extracted from modules"
fi

# Cleanup
rm -rf "$TEMP_DIR"

log_success "Done!"

