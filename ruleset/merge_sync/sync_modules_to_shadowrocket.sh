#!/opt/homebrew/bin/bash
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

# Shadowrocket iCloud directory (modules are in root, not in Modules subfolder)
SHADOWROCKET_MODULE_DIR="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents"

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
# 🔥 修复: 保留 #!category= 标签（小火箭也支持分组）
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
    
    # 在 #!desc 中添加 [🚀SR] 标记（如果没有的话）
    if ! grep -q '\[🚀SR\]' "$output_file"; then
        sed -i '' 's/^#!desc=/#!desc=[🚀SR] /' "$output_file"
    fi
}

# Sync all modules from all categories
SUCCESS=0
FAILED=0
SKIPPED=0

for category in amplify_nexus head_expanse narrow_pierce; do
    category_dir="${SURGE_MODULE_DIR}/${category}"
    
    if [ ! -d "$category_dir" ]; then
        echo -e "${YELLOW}Category not found: ${category}${NC}"
        continue
    fi
    
    echo -e "\n${BLUE}=== Category: ${category} ===${NC}"
    
    for src_file in "$category_dir"/*.sgmodule; do
        [ -f "$src_file" ] || continue
        
        module=$(basename "$src_file")
        dst_file="${SHADOWROCKET_MODULE_DIR}/${module}"
        
        echo -e "${YELLOW}Syncing: ${module}${NC}"
        
        if convert_to_shadowrocket "$src_file" "$dst_file"; then
            echo -e "${GREEN}  ✓ Done${NC}"
            ((SUCCESS++))
        else
            echo -e "${RED}  ✗ Failed${NC}"
            ((FAILED++))
        fi
    done
done

echo ""
echo -e "${BLUE}=== Sync Complete ===${NC}"
echo -e "Success: ${GREEN}${SUCCESS}${NC} | Failed: ${RED}${FAILED}${NC}"
