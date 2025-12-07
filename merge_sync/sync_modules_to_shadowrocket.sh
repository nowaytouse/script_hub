#!/usr/bin/env bash
# =============================================================================
# Sync Surge Modules to Shadowrocket
# Function: Convert Surge modules to Shadowrocket compatible format and sync
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SURGE_MODULE_DIR="${SCRIPT_DIR}/../../module/surge(main)"

# Please modify the following path to your actual Shadowrocket iCloud directory
# Example: /Users/YOUR_USERNAME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules
SHADOWROCKET_MODULE_DIR="/Users/YOUR_USERNAME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules"

echo -e "${BLUE}=== Surge -> Shadowrocket Module Sync ===${NC}"
echo "Source: $SURGE_MODULE_DIR"
echo "Target: $SHADOWROCKET_MODULE_DIR"
echo ""

# Check directories
if [ ! -d "$SURGE_MODULE_DIR" ]; then
    echo -e "${RED}Surge module directory not found${NC}"
    exit 1
fi

if [ ! -d "$SHADOWROCKET_MODULE_DIR" ]; then
    echo -e "${RED}Shadowrocket module directory not found${NC}"
    exit 1
fi

# Convert function: Surge -> Shadowrocket compatible
convert_to_shadowrocket() {
    local input_file="$1"
    local output_file="$2"
    
    # Copy and perform necessary conversions
    cat "$input_file" | \
    # Remove Surge-specific extended-matching parameter
    sed 's/,extended-matching//g' | \
    # Remove pre-matching parameter
    sed 's/,pre-matching//g' | \
    # Remove update-interval parameter (Shadowrocket doesn't support)
    sed 's/,"update-interval=[0-9]*"//g' | \
    # Convert REJECT-DROP to REJECT (Shadowrocket compatible)
    sed 's/REJECT-DROP/REJECT/g' | \
    # Convert REJECT-NO-DROP to REJECT
    sed 's/REJECT-NO-DROP/REJECT/g' | \
    # Remove %APPEND% prefix (Shadowrocket doesn't need)
    sed 's/%APPEND% //g' \
    > "$output_file"
}

# Modules to sync
MODULES=(
    "🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"
    "🚀💪General Enhanced⬆️⬆️ plus.sgmodule"
    "🔥 Firewall Port Blocker 🛡️🚫.sgmodule"
    "Encrypted DNS Module 🔒🛡️DNS.sgmodule"
    "URL Rewrite Module 🔄🌐.sgmodule"
)

SUCCESS=0
FAILED=0

for module in "${MODULES[@]}"; do
    src_file="${SURGE_MODULE_DIR}/${module}"
    
    if [ -f "$src_file" ]; then
        # Generate Shadowrocket compatible filename
        dst_name=$(echo "$module" | sed 's/[^a-zA-Z0-9._-]/_/g')
        dst_file="${SHADOWROCKET_MODULE_DIR}/${dst_name}"
        
        echo -e "${YELLOW}Syncing: ${module}${NC}"
        
        if convert_to_shadowrocket "$src_file" "$dst_file"; then
            echo -e "${GREEN}  Done -> ${dst_name}${NC}"
            ((SUCCESS++))
        else
            echo -e "${RED}  Failed${NC}"
            ((FAILED++))
        fi
    else
        echo -e "${RED}Skip: ${module} (not found)${NC}"
        ((FAILED++))
    fi
done

echo ""
echo -e "${BLUE}=== Sync Complete ===${NC}"
echo -e "Success: ${GREEN}${SUCCESS}${NC} | Failed: ${RED}${FAILED}${NC}"
