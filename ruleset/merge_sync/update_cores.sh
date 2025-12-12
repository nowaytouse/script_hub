#!/opt/homebrew/bin/bash
# =============================================================================
# Sing-box & Mihomo Core Update Script (Simple Version)
# Function: Download latest versions without using GitHub API
# Updated: 2025-12-07
# Note: Uses HTML scraping instead of API to avoid rate limits
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Installation paths
SINGBOX_SYSTEM_PATH="/usr/local/bin/sing-box"
SINGBOX_LOCAL_PATH="$PROJECT_ROOT/tools/config-manager-auto-update/bin/sing-box"
MIHOMO_SYSTEM_PATH="/usr/local/bin/mihomo"
MIHOMO_LOCAL_PATH="$PROJECT_ROOT/tools/config-manager-auto-update/bin/mihomo"

# Detect system info
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    darwin) OS="darwin" ;;
    linux) OS="linux" ;;
    *) log_error "Unsupported OS: $OS"; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64) ARCH="amd64" ;;
    aarch64|arm64) ARCH="arm64" ;;
    armv7l) ARCH="armv7" ;;
    *) log_error "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Get latest version from GitHub releases page (no API)
get_latest_version() {
    local repo="$1"
    local include_prerelease="${2:-false}"
    
    if [ "$include_prerelease" = "true" ]; then
        # Get all releases including pre-release
        curl -sL "https://github.com/$repo/releases" 2>/dev/null | \
            grep -oE 'href="/'"$repo"'/releases/tag/v[0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?"' | \
            head -1 | \
            grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?' | \
            sed 's/^v//'
    else
        # Get latest stable release only
        curl -sL "https://github.com/$repo/releases/latest" 2>/dev/null | \
            grep -oE 'href="/'"$repo"'/releases/tag/v[0-9]+\.[0-9]+\.[0-9]+"' | \
            head -1 | \
            grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+' | \
            sed 's/^v//'
    fi
}

# Update Sing-box
update_singbox() {
    log_info "Checking Sing-box updates..."
    log_info "Fetching latest version from GitHub..."
    
    local latest_version=$(get_latest_version "SagerNet/sing-box" "$INCLUDE_PRERELEASE")
    
    if [ -z "$latest_version" ]; then
        log_error "Cannot get latest version"
        return 1
    fi
    
    # Check current versions
    local system_version="0.0.0"
    local local_version="0.0.0"
    
    if [ -f "$SINGBOX_SYSTEM_PATH" ]; then
        system_version=$("$SINGBOX_SYSTEM_PATH" version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?' | head -1 || echo "0.0.0")
    fi
    
    if [ -f "$SINGBOX_LOCAL_PATH" ]; then
        local_version=$("$SINGBOX_LOCAL_PATH" version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?' | head -1 || echo "0.0.0")
    fi
    
    log_info "System version: v$system_version"
    log_info "Local version: v$local_version"
    log_info "Latest version: v$latest_version"
    
    # Check if update needed
    if [ "$system_version" = "$latest_version" ] && [ "$local_version" = "$latest_version" ]; then
        log_success "Sing-box is already up to date"
        return 0
    fi
    
    # Download
    log_info "Downloading Sing-box v$latest_version..."
    local download_url="https://github.com/SagerNet/sing-box/releases/download/v${latest_version}/sing-box-${latest_version}-${OS}-${ARCH}.tar.gz"
    local temp_dir=$(mktemp -d)
    
    if ! curl -L -o "$temp_dir/sing-box.tar.gz" "$download_url"; then
        log_error "Download failed"
        rm -rf "$temp_dir"
        return 1
    fi
    
    log_success "Download complete"
    
    # Extract
    tar -xzf "$temp_dir/sing-box.tar.gz" -C "$temp_dir"
    local binary=$(find "$temp_dir" -name "sing-box" -type f | head -1)
    
    if [ -z "$binary" ]; then
        log_error "Binary not found in archive"
        rm -rf "$temp_dir"
        return 1
    fi
    
    # Install to system path (with sudo)
    if sudo cp "$binary" "$SINGBOX_SYSTEM_PATH" 2>/dev/null; then
        sudo chmod +x "$SINGBOX_SYSTEM_PATH"
        log_success "System sing-box v$latest_version installed"
    else
        log_warning "Cannot install to system path (no sudo)"
    fi
    
    # Install to local path (no sudo)
    mkdir -p "$(dirname "$SINGBOX_LOCAL_PATH")"
    cp "$binary" "$SINGBOX_LOCAL_PATH"
    chmod +x "$SINGBOX_LOCAL_PATH"
    log_success "Local sing-box v$latest_version installed"
    
    # Verify
    echo ""
    log_info "Verification:"
    if [ -f "$SINGBOX_SYSTEM_PATH" ]; then
        "$SINGBOX_SYSTEM_PATH" version | head -1
    fi
    "$SINGBOX_LOCAL_PATH" version | head -1
    
    rm -rf "$temp_dir"
}

# Update Mihomo
update_mihomo() {
    log_info "Checking Mihomo updates..."
    log_info "Fetching latest version from GitHub..."
    
    local latest_version=$(get_latest_version "MetaCubeX/mihomo" "false")
    
    if [ -z "$latest_version" ]; then
        log_error "Cannot get latest version"
        return 1
    fi
    
    # Check current versions
    local system_version="0.0.0"
    local local_version="0.0.0"
    
    if [ -f "$MIHOMO_SYSTEM_PATH" ]; then
        system_version=$("$MIHOMO_SYSTEM_PATH" -v 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1 || echo "0.0.0")
    fi
    
    if [ -f "$MIHOMO_LOCAL_PATH" ]; then
        local_version=$("$MIHOMO_LOCAL_PATH" -v 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1 || echo "0.0.0")
    fi
    
    log_info "System version: v$system_version"
    log_info "Local version: v$local_version"
    log_info "Latest version: v$latest_version"
    
    # Check if update needed
    if [ "$system_version" = "$latest_version" ] && [ "$local_version" = "$latest_version" ]; then
        log_success "Mihomo is already up to date"
        return 0
    fi
    
    # Download
    log_info "Downloading Mihomo v$latest_version..."
    local download_url="https://github.com/MetaCubeX/mihomo/releases/download/v${latest_version}/mihomo-${OS}-${ARCH}-v${latest_version}.gz"
    local temp_dir=$(mktemp -d)
    
    if ! curl -L -o "$temp_dir/mihomo.gz" "$download_url"; then
        log_error "Download failed"
        rm -rf "$temp_dir"
        return 1
    fi
    
    log_success "Download complete"
    
    # Extract
    gunzip "$temp_dir/mihomo.gz"
    
    if [ ! -f "$temp_dir/mihomo" ]; then
        log_error "Binary not found after extraction"
        rm -rf "$temp_dir"
        return 1
    fi
    
    # Install to system path (with sudo)
    if sudo cp "$temp_dir/mihomo" "$MIHOMO_SYSTEM_PATH" 2>/dev/null; then
        sudo chmod +x "$MIHOMO_SYSTEM_PATH"
        log_success "System mihomo v$latest_version installed"
    else
        log_warning "Cannot install to system path (no sudo)"
    fi
    
    # Install to local path (no sudo)
    mkdir -p "$(dirname "$MIHOMO_LOCAL_PATH")"
    cp "$temp_dir/mihomo" "$MIHOMO_LOCAL_PATH"
    chmod +x "$MIHOMO_LOCAL_PATH"
    log_success "Local mihomo v$latest_version installed"
    
    # Verify
    echo ""
    log_info "Verification:"
    if [ -f "$MIHOMO_SYSTEM_PATH" ]; then
        "$MIHOMO_SYSTEM_PATH" -v | head -1
    fi
    "$MIHOMO_LOCAL_PATH" -v | head -1
    
    rm -rf "$temp_dir"
}

# Show help
show_help() {
    echo "Usage: $0 [options]"
    echo ""
    echo "Options:"
    echo "  --singbox-only    Update Sing-box only"
    echo "  --mihomo-only     Update Mihomo only"
    echo "  --prerelease      Include pre-release versions (alpha/beta) [DEFAULT]"
    echo "  --stable          Use stable versions only (not recommended for sing-box)"
    echo "  -h, --help        Show help"
    echo ""
    echo "Examples:"
    echo "  $0                    # Update all cores (stable)"
    echo "  $0 --singbox-only     # Update Sing-box only"
    echo "  $0 --prerelease       # Update to latest pre-release"
    exit 0
}

# Parse arguments
SINGBOX_ONLY=false
MIHOMO_ONLY=false
# 🔥 默认使用预览版 (sing-box 1.13.0+ 需要预览版)
INCLUDE_PRERELEASE=true

while [[ $# -gt 0 ]]; do
    case $1 in
        --singbox-only) SINGBOX_ONLY=true; shift ;;
        --mihomo-only) MIHOMO_ONLY=true; shift ;;
        --prerelease|--alpha) INCLUDE_PRERELEASE=true; shift ;;
        --stable) INCLUDE_PRERELEASE=false; shift ;;
        -h|--help) show_help ;;
        *) log_error "Unknown option: $1"; exit 1 ;;
    esac
done

# Show banner
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       Core Update Tool (Simple Version)                      ║${NC}"
echo -e "${BLUE}║       No API - Direct HTML Scraping                          ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Execute update
if [ "$MIHOMO_ONLY" = false ]; then
    update_singbox || log_warning "Sing-box update failed"
    echo ""
fi

if [ "$SINGBOX_ONLY" = false ]; then
    update_mihomo || log_warning "Mihomo update failed"
    echo ""
fi

log_success "Core update complete"
