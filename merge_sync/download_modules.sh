#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# Universal Module Downloader & Extractor
# 
# Features:
# - Download modules from URLs
# - Support proxy policies (PROXY, DIRECT, REJECT, etc.)
# - Support custom group assignment
# - Extract rules to rulesets
# - Sync to Shadowrocket format
#
# Usage:
#   ./download_modules.sh [options]
#   ./download_modules.sh --url "URL" --group "GROUP_NAME"
#   ./download_modules.sh --file "sources.txt"
#
# ═══════════════════════════════════════════════════════════════════════════════

# Don't exit on error - handle errors gracefully
set +e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TEMP_DIR="$PROJECT_ROOT/.temp_module_download"
MODULE_DIR="$PROJECT_ROOT/module/surge(main)"
SHADOWROCKET_MODULE_DIR="$PROJECT_ROOT/module/shadowrocket"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;36m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }
log_header() { echo -e "\n${CYAN}═══════════════════════════════════════════════════════════════${NC}"; echo -e "${CYAN}$1${NC}"; echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"; }

# ═══════════════════════════════════════════════════════════════════════════════
# Functions
# ═══════════════════════════════════════════════════════════════════════════════

process_module() {
    local temp_file="$1"
    local module_file="$2"
    local category="$3"
    
    local filename=$(basename "$module_file")
    
    # Surge uses #!category= for module grouping in UI (NOT #!group=)
    # Check if module already has #!category
    if grep -q "^#!category=" "$temp_file"; then
        # Replace existing category
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -i '' "s|^#!category=.*|#!category=$category|" "$temp_file"
        else
            sed -i "s|^#!category=.*|#!category=$category|" "$temp_file"
        fi
    else
        # Add category after #!name or at the beginning
        if grep -q "^#!name=" "$temp_file"; then
            # Insert after #!name line (macOS compatible)
            if [[ "$OSTYPE" == "darwin"* ]]; then
                sed -i '' "/^#!name=/a\\
#!category=$category
" "$temp_file"
            else
                sed -i "/^#!name=/a #!category=$category" "$temp_file"
            fi
        else
            # Add at the beginning
            local tmp_content=$(cat "$temp_file")
            echo -e "#!category=$category\n$tmp_content" > "$temp_file"
        fi
    fi
    
    # Remove any #!group= field (deprecated, Surge uses #!category=)
    if grep -q "^#!group=" "$temp_file"; then
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -i '' '/^#!group=/d' "$temp_file"
        else
            sed -i '/^#!group=/d' "$temp_file"
        fi
    fi
    
    # Remove duplicate #!category= lines (keep only the first one we set)
    # Some modules have their own #!category which would override ours
    # Strategy: First line should be our category, remove all others
    local category_count=$(grep -c "^#!category" "$temp_file" 2>/dev/null || echo "0")
    if [ "$category_count" -gt 1 ]; then
        log_warning "  Found $category_count #!category lines, keeping only the first one"
        # Use awk to keep only the first #!category line
        awk '
            /^#!category/ {
                if (!seen) {
                    print
                    seen = 1
                }
                next
            }
            { print }
        ' "$temp_file" > "${temp_file}.tmp" && mv "${temp_file}.tmp" "$temp_file"
    fi
    
    # Copy to module directory
    cp "$temp_file" "$module_file"
}

extract_rules_from_module() {
    local module_file="$1"
    local module_name="$2"
    
    # Extract [Rule] section
    local rules=$(awk '/^\[Rule\]/{f=1;next}/^\[/{f=0}f' "$module_file" 2>/dev/null | \
        grep -v '^#' | grep -v '^$' | grep -v '^RULE-SET' || true)
    
    if [ -n "$rules" ]; then
        local rule_count=$(echo "$rules" | wc -l | tr -d ' ')
        log_info "  Extracted $rule_count rules from $module_name"
        
        # Categorize rules by policy
        local proxy_rules=$(echo "$rules" | grep -E ",PROXY$|,PROXY," || true)
        local direct_rules=$(echo "$rules" | grep -E ",DIRECT$|,DIRECT," || true)
        local reject_rules=$(echo "$rules" | grep -E ",REJECT" || true)
        local other_rules=$(echo "$rules" | grep -vE ",PROXY|,DIRECT|,REJECT" || true)
        
        # Log categorization
        [ -n "$proxy_rules" ] && log_info "    - PROXY rules: $(echo "$proxy_rules" | wc -l | tr -d ' ')"
        [ -n "$direct_rules" ] && log_info "    - DIRECT rules: $(echo "$direct_rules" | wc -l | tr -d ' ')"
        [ -n "$reject_rules" ] && log_info "    - REJECT rules: $(echo "$reject_rules" | wc -l | tr -d ' ')"
        [ -n "$other_rules" ] && log_info "    - Other rules: $(echo "$other_rules" | wc -l | tr -d ' ')"
    fi
}

sync_to_shadowrocket() {
    log_info "Converting modules to Shadowrocket format..."
    
    local converted=0
    
    # Process modules from all subdirectories and root
    for module in "$MODULE_DIR"/*.sgmodule "$MODULE_DIR"/*.module "$MODULE_DIR"/*/*.sgmodule "$MODULE_DIR"/*/*.module; do
        [ ! -f "$module" ] && continue
        
        local filename=$(basename "$module")
        
        # Determine target path (preserve subdirectory structure)
        local relative_path="${module#$MODULE_DIR/}"
        local sr_file="$SHADOWROCKET_MODULE_DIR/$relative_path"
        local sr_dir=$(dirname "$sr_file")
        
        # Create subdirectory if needed
        mkdir -p "$sr_dir"
        
        # Copy and convert
        cp "$module" "$sr_file"
        
        # Shadowrocket-specific adjustments
        # Comment out #!category= (Shadowrocket doesn't use it)
        if grep -q "^#!category=" "$sr_file"; then
            if [[ "$OSTYPE" == "darwin"* ]]; then
                sed -i '' 's/^#!category=/#!category (Surge only): /' "$sr_file"
            else
                sed -i 's/^#!category=/#!category (Surge only): /' "$sr_file"
            fi
        fi
        
        converted=$((converted + 1))
    done
    
    log_success "Converted $converted modules to Shadowrocket format"
}

# ═══════════════════════════════════════════════════════════════════════════════
# Main Script
# ═══════════════════════════════════════════════════════════════════════════════

# Default values
DEFAULT_GROUP="『 🛠️ Amplify Nexus › 增幅枢纽 』"
EXTRACT_RULES=true
SYNC_SHADOWROCKET=true

# Parse arguments
URLS=()
GROUP="$DEFAULT_GROUP"
SOURCES_FILE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --url)
            URLS+=("$2")
            shift 2
            ;;
        --group)
            GROUP="$2"
            shift 2
            ;;
        --file)
            SOURCES_FILE="$2"
            shift 2
            ;;
        --no-extract)
            EXTRACT_RULES=false
            shift
            ;;
        --no-sync)
            SYNC_SHADOWROCKET=false
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --url URL          Add module URL to download"
            echo "  --group GROUP      Set group name for modules (default: $DEFAULT_GROUP)"
            echo "  --file FILE        Read URLs from file (one per line)"
            echo "  --no-extract       Don't extract rules to rulesets"
            echo "  --no-sync          Don't sync to Shadowrocket format"
            echo "  -h, --help         Show this help"
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Read URLs from file if specified
if [ -n "$SOURCES_FILE" ] && [ -f "$SOURCES_FILE" ]; then
    while IFS= read -r line; do
        # Skip comments and empty lines
        [[ "$line" =~ ^#.*$ ]] && continue
        [[ -z "$line" ]] && continue
        URLS+=("$line")
    done < "$SOURCES_FILE"
fi

# If no URLs provided, use default list
if [ ${#URLS[@]} -eq 0 ]; then
    URLS=(
        "https://whatshub.top/module/bili.module"
        "https://raw.githubusercontent.com/chavyleung/scripts/master/box/rewrite/boxjs.rewrite.surge.sgmodule"
        "https://raw.githubusercontent.com/sub-store-org/Sub-Store/master/config/Surge-Beta.sgmodule"
        "https://raw.githubusercontent.com/Semporia/TikTok-Unlock/master/Surge/TiKTok-US.sgmodule"
        "https://raw.githubusercontent.com/Coldvvater/Mononoke/refs/heads/master/Surge/Module/Tool/VVebo_Repair.sgmodule"
        "https://raw.githubusercontent.com/Maasea/sgmodule/refs/heads/master/YouTube.Enhance.sgmodule"
        "https://raw.githubusercontent.com/Repcz/Tool/refs/heads/X/Surge/Module/Function/QX-resource-preview.sgmodule"
        "https://raw.githubusercontent.com/Coldvvater/Mononoke/refs/heads/master/Surge/Module/Tool/Sub_Info.sgmodule"
        # ⚠️ REMOVED: DNS.sgmodule - 已合并到本地 🌐 DNS & Host Enhanced.sgmodule
        # "https://raw.githubusercontent.com/Repcz/Tool/refs/heads/X/Surge/Module/Function/DNS.sgmodule"
        "https://raw.githubusercontent.com/xream/scripts/main/surge/modules/network-info/net-lsp-x.sgmodule"
        "https://raw.githubusercontent.com/Rabbit-Spec/Surge/Master/Module/Panel/Timecard/Moore/Timecard.sgmodule"
        "https://raw.githubusercontent.com/ninjai/apple/refs/heads/main/sgmodule/iCloud_Private_Relay_Gateway.sgmodule"
        "https://github.com/NSRingo/WeatherKit/releases/latest/download/iRingo.WeatherKit.sgmodule"
        "https://github.com/NSRingo/GeoServices/releases/latest/download/iRingo.Location.sgmodule"
        "https://github.com/NSRingo/News/releases/latest/download/iRingo.News.sgmodule"
        "https://github.com/NSRingo/TV/releases/latest/download/iRingo.TV.sgmodule"
        "https://github.com/NSRingo/GeoServices/releases/latest/download/iRingo.Maps.sgmodule"
        # ⚠️ REMOVED: 🍟 DNS 分流.official.sgmodule - 已合并到本地 🌐 DNS & Host Enhanced.sgmodule
        # "https://raw.githubusercontent.com/QingRex/LoonKissSurge/refs/heads/main/Surge/Official/%F0%9F%8D%9F%20DNS%20%E5%88%86%E6%B5%81.official.sgmodule"
        "https://github.com/DualSubs/Universal/releases/latest/download/DualSubs.Universal.sgmodule"
        "https://github.com/BiliUniverse/Enhanced/releases/latest/download/BiliBili.Enhanced.sgmodule"
        "https://github.com/BiliUniverse/Global/releases/latest/download/BiliBili.Global.sgmodule"
        "https://github.com/BiliUniverse/Redirect/releases/latest/download/BiliBili.Redirect.sgmodule"
    )
fi

# ═══════════════════════════════════════════════════════════════════════════════
# Protected Local Modules (不会被远程下载覆盖)
# 这些模块是本地合并/优化版本，优先级高于远程版本
# ═══════════════════════════════════════════════════════════════════════════════
PROTECTED_MODULES=(
    "🌐 DNS & Host Enhanced.sgmodule"  # 合并自: DNS.sgmodule + 🍟 DNS 分流.official.sgmodule + 🚀💪General Enhanced⬆️⬆️ plus.sgmodule
)

log_header "Universal Module Downloader"
log_info "Group: $GROUP"
log_info "Total URLs: ${#URLS[@]}"
log_info "Extract rules: $EXTRACT_RULES"
log_info "Sync to Shadowrocket: $SYNC_SHADOWROCKET"

# Init
rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR" "$MODULE_DIR" "$SHADOWROCKET_MODULE_DIR"

# Statistics
downloaded=0
failed=0
total=${#URLS[@]}

log_header "Downloading Modules"

for url in "${URLS[@]}"; do
    [ -z "$url" ] && continue
    
    # Extract filename from URL (handle URL-encoded names)
    filename=$(basename "$url" | python3 -c "import sys, urllib.parse; print(urllib.parse.unquote(sys.stdin.read().strip()))" 2>/dev/null || basename "$url")
    
    # Ensure .sgmodule extension
    [[ "$filename" != *.sgmodule ]] && [[ "$filename" != *.module ]] && filename="${filename}.sgmodule"
    
    # Check if this module is protected (local merged version exists)
    is_protected=false
    for protected in "${PROTECTED_MODULES[@]}"; do
        if [[ "$filename" == "$protected" ]] || [[ "$filename" == "DNS.sgmodule" ]] || [[ "$filename" == *"DNS 分流"* ]] || [[ "$filename" == *"General Enhanced"* ]]; then
            is_protected=true
            break
        fi
    done
    
    if [ "$is_protected" = true ]; then
        log_warning "Skipping protected module: $filename (local merged version exists)"
        continue
    fi
    
    module_file="$MODULE_DIR/$filename"
    temp_file="$TEMP_DIR/$filename"
    
    log_info "Downloading: $filename"
    
    # Download with timeout and follow redirects
    if curl -L -s -m 60 -o "$temp_file" "$url" 2>/dev/null; then
        # Check if file is valid
        if [ -s "$temp_file" ] && ! grep -q "<!DOCTYPE html>" "$temp_file" 2>/dev/null; then
            # Process the module
            process_module "$temp_file" "$module_file" "$GROUP"
            
            # Extract rules if enabled
            if [ "$EXTRACT_RULES" = true ]; then
                extract_rules_from_module "$module_file" "$filename"
            fi
            
            downloaded=$((downloaded + 1))
            log_success "  Downloaded: $filename"
        else
            log_error "  Invalid file: $filename (may be 403/404)"
            failed=$((failed + 1))
        fi
    else
        log_error "  Download failed: $filename"
        failed=$((failed + 1))
    fi
done

log_header "Download Summary"
log_info "Total:      $total"
log_success "Success:    $downloaded"
[ $failed -gt 0 ] && log_error "Failed:     $failed" || log_info "Failed:     0"

# Sync to Shadowrocket if enabled
if [ "$SYNC_SHADOWROCKET" = true ]; then
    log_header "Syncing to Shadowrocket"
    sync_to_shadowrocket
fi

# Cleanup
rm -rf "$TEMP_DIR"

log_success "Done!"
