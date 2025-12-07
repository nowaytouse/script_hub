#!/usr/bin/env bash
# =============================================================================
# Sing-box & Mihomo Core Update Script (Wrapper)
# Function: Wrapper for config-manager-auto-update tool
# Updated: 2025-12-07
# Note: This script now delegates to the Rust-based config-manager-auto-update
#       tool to avoid code duplication
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Config-manager-auto-update tool paths
CONFIG_MANAGER_DIR="$PROJECT_ROOT/tools/config-manager-auto-update"
CONFIG_MANAGER_BINARY="$CONFIG_MANAGER_DIR/target/release/singbox-manager"
CONFIG_MANAGER_UPDATE_SCRIPT="$CONFIG_MANAGER_DIR/update.sh"

# Check if config-manager-auto-update tool exists
check_config_manager() {
    if [ ! -d "$CONFIG_MANAGER_DIR" ]; then
        log_error "config-manager-auto-update tool not found at: $CONFIG_MANAGER_DIR"
        log_info "Please ensure the tool is properly installed"
        return 1
    fi
    
    # Check if binary exists, if not try to build
    if [ ! -f "$CONFIG_MANAGER_BINARY" ]; then
        log_warning "Binary not found, attempting to build..."
        if command -v cargo >/dev/null 2>&1; then
            (cd "$CONFIG_MANAGER_DIR" && cargo build --release) || {
                log_error "Failed to build config-manager-auto-update"
                return 1
            }
        else
            log_error "Rust/Cargo not installed, cannot build binary"
            return 1
        fi
    fi
    
    return 0
}

# Update cores using config-manager-auto-update tool
update_cores() {
    log_info "Delegating to config-manager-auto-update tool..."
    
    if ! check_config_manager; then
        return 1
    fi
    
    # Run the update script
    if [ -f "$CONFIG_MANAGER_UPDATE_SCRIPT" ]; then
        bash "$CONFIG_MANAGER_UPDATE_SCRIPT"
    else
        # Directly call the binary
        "$CONFIG_MANAGER_BINARY" --once
    fi
}

# Show help
show_help() {
    echo "Usage: $0 [options]"
    echo ""
    echo "This is a wrapper script for the config-manager-auto-update tool."
    echo "It delegates all core update operations to the Rust-based tool."
    echo ""
    echo "Options:"
    echo "  -h, --help        Show help"
    echo ""
    echo "Note: For advanced options, use the config-manager-auto-update tool directly:"
    echo "  $CONFIG_MANAGER_DIR/update.sh"
    echo "  $CONFIG_MANAGER_BINARY --help"
    exit 0
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help) show_help ;;
        *) log_error "Unknown option: $1. Use --help for usage."; exit 1 ;;
    esac
done

# Show banner
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       Core Update Tool (Wrapper)                              ║${NC}"
echo -e "${BLUE}║       Powered by: config-manager-auto-update                  ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Execute update
update_cores || {
    log_error "Core update failed"
    exit 1
}

log_success "Core update complete"
