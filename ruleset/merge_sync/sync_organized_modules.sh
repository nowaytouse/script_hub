#!/opt/homebrew/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
# Sync Organized Modules to Shadowrocket
# 
# This script syncs the organized module structure from surge(main) to shadowrocket,
# preserving the subdirectory organization.
#
# ═══════════════════════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
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

log_header "Syncing Organized Modules to Shadowrocket"

# Clean shadowrocket directory
log_info "Cleaning shadowrocket directory..."
rm -rf "$SHADOWROCKET_MODULE_DIR"/*
mkdir -p "$SHADOWROCKET_MODULE_DIR"

synced=0

# Sync all modules (including subdirectories)
for module in "$MODULE_DIR"/*.sgmodule "$MODULE_DIR"/*.module "$MODULE_DIR"/*/*.sgmodule "$MODULE_DIR"/*/*.module; do
    [ ! -f "$module" ] && continue
    
    # Get relative path
    relative_path="${module#$MODULE_DIR/}"
    target_file="$SHADOWROCKET_MODULE_DIR/$relative_path"
    target_dir=$(dirname "$target_file")
    
    # Create subdirectory if needed
    mkdir -p "$target_dir"
    
    # Copy module
    cp "$module" "$target_file"
    
    # Shadowrocket-specific adjustments
    # 🔥 修复: 保留 #!category= 标签（小火箭也支持分组）
    # 只需要移除 Surge 特有的参数
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # 移除 Surge 特有参数
        sed -i '' 's/,extended-matching//g' "$target_file"
        sed -i '' 's/,pre-matching//g' "$target_file"
        sed -i '' 's/,"update-interval=[0-9]*"//g' "$target_file"
        # 转换 REJECT-DROP/REJECT-NO-DROP 为 REJECT
        sed -i '' 's/REJECT-DROP/REJECT/g' "$target_file"
        sed -i '' 's/REJECT-NO-DROP/REJECT/g' "$target_file"
        # 移除 %APPEND% 前缀
        sed -i '' 's/%APPEND% //g' "$target_file"
        # 在 #!desc 中添加 [🚀SR] 标记（如果没有的话）
        if ! grep -q '\[🚀SR\]' "$target_file"; then
            sed -i '' 's/^#!desc=/#!desc=[🚀SR] /' "$target_file"
        fi
    else
        sed -i 's/,extended-matching//g' "$target_file"
        sed -i 's/,pre-matching//g' "$target_file"
        sed -i 's/,"update-interval=[0-9]*"//g' "$target_file"
        sed -i 's/REJECT-DROP/REJECT/g' "$target_file"
        sed -i 's/REJECT-NO-DROP/REJECT/g' "$target_file"
        sed -i 's/%APPEND% //g' "$target_file"
        if ! grep -q '\[🚀SR\]' "$target_file"; then
            sed -i 's/^#!desc=/#!desc=[🚀SR] /' "$target_file"
        fi
    fi
    
    log_success "Synced: $relative_path"
    synced=$((synced + 1))
done

log_header "Sync Summary"
log_success "Total synced: $synced modules"
log_success "Done!"
