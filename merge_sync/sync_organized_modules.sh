#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# Sync Organized Modules to Shadowrocket
# 
# This script syncs the organized module structure from surge(main) to shadowrocket,
# preserving the subdirectory organization.
#
# ═══════════════════════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
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
    # Keep #!group for organization purposes (comment it out)
    if grep -q "^#!group=" "$target_file"; then
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -i '' 's/^#!group=/#!group (for organization): /' "$target_file"
        else
            sed -i 's/^#!group=/#!group (for organization): /' "$target_file"
        fi
    fi
    
    log_success "Synced: $relative_path"
    synced=$((synced + 1))
done

log_header "Sync Summary"
log_success "Total synced: $synced modules"
log_success "Done!"
