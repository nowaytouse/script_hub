#!/usr/bin/env bash
# =============================================================================
# Sing-box & Mihomo Core Update Script
# Function: Auto download and update latest versions of sing-box and mihomo cores
# Updated: 2025-12-07
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

# Core installation paths
SINGBOX_PATH="/usr/local/bin/sing-box"
MIHOMO_PATH="/usr/local/bin/mihomo"

# Backup directory
BACKUP_DIR="$HOME/.local/share/proxy-cores/backup"
mkdir -p "$BACKUP_DIR"

# Detect system architecture
detect_arch() {
    local arch=$(uname -m)
    case "$arch" in
        x86_64|amd64) echo "amd64" ;;
        aarch64|arm64) echo "arm64" ;;
        armv7l) echo "armv7" ;;
        *) log_error "Unsupported architecture: $arch"; exit 1 ;;
    esac
}

# Detect operating system
detect_os() {
    case "$(uname -s)" in
        Darwin) echo "darwin" ;;
        Linux) echo "linux" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *) log_error "Unsupported operating system"; exit 1 ;;
    esac
}

# Get latest version number
get_latest_version() {
    local repo="$1"
    local version=$(curl -s "https://api.github.com/repos/$repo/releases/latest" | grep '"tag_name"' | sed -E 's/.*"v?([^"]+)".*/\1/')
    if [ -z "$version" ]; then
        log_error "Cannot get latest version for $repo"
        return 1
    fi
    echo "$version"
}

# Backup existing core
backup_core() {
    local core_path="$1"
    local core_name=$(basename "$core_path")
    
    if [ -f "$core_path" ]; then
        local version=$("$core_path" version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || echo "unknown")
        local backup_file="$BACKUP_DIR/${core_name}_${version}_$(date +%Y%m%d_%H%M%S)"
        
        log_info "Backing up $core_name v$version..."
        cp "$core_path" "$backup_file"
        log_success "Backed up to: $backup_file"
        
        # Keep only last 3 backups
        ls -t "$BACKUP_DIR/${core_name}_"* 2>/dev/null | tail -n +4 | xargs rm -f 2>/dev/null || true
    fi
}

# Update Sing-box
update_singbox() {
    log_info "Checking Sing-box updates..."
    
    local os=$(detect_os)
    local arch=$(detect_arch)
    local latest_version=$(get_latest_version "SagerNet/sing-box")
    
    if [ -z "$latest_version" ]; then
        log_error "Cannot get Sing-box latest version"
        return 1
    fi
    
    # Check current version
    if [ -f "$SINGBOX_PATH" ]; then
        local current_version=$("$SINGBOX_PATH" version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || echo "0.0.0")
        log_info "Current version: v$current_version"
        log_info "Latest version: v$latest_version"
        
        if [ "$current_version" = "$latest_version" ]; then
            log_success "Sing-box is already up to date"
            return 0
        fi
        
        # Backup old version
        backup_core "$SINGBOX_PATH"
    else
        log_info "First time installing Sing-box"
    fi
    
    # Download new version
    log_info "Downloading Sing-box v$latest_version..."
    local download_url="https://github.com/SagerNet/sing-box/releases/download/v${latest_version}/sing-box-${latest_version}-${os}-${arch}.tar.gz"
    local temp_dir=$(mktemp -d)
    
    if curl -L -o "$temp_dir/sing-box.tar.gz" "$download_url"; then
        log_success "Download complete"
        
        # Extract
        tar -xzf "$temp_dir/sing-box.tar.gz" -C "$temp_dir"
        
        # Install
        local binary=$(find "$temp_dir" -name "sing-box" -type f | head -1)
        if [ -n "$binary" ]; then
            sudo mv "$binary" "$SINGBOX_PATH"
            sudo chmod +x "$SINGBOX_PATH"
            log_success "Sing-box v$latest_version installed"
            
            # Verify
            "$SINGBOX_PATH" version
        else
            log_error "sing-box binary not found"
            rm -rf "$temp_dir"
            return 1
        fi
    else
        log_error "Download failed"
        rm -rf "$temp_dir"
        return 1
    fi
    
    rm -rf "$temp_dir"
}

# Update Mihomo
update_mihomo() {
    log_info "Checking Mihomo updates..."
    
    local os=$(detect_os)
    local arch=$(detect_arch)
    local latest_version=$(get_latest_version "MetaCubeX/mihomo")
    
    if [ -z "$latest_version" ]; then
        log_error "Cannot get Mihomo latest version"
        return 1
    fi
    
    # Check current version
    if [ -f "$MIHOMO_PATH" ]; then
        local current_version=$("$MIHOMO_PATH" -v 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1 || echo "0.0.0")
        log_info "Current version: v$current_version"
        log_info "Latest version: v$latest_version"
        
        if [ "$current_version" = "$latest_version" ]; then
            log_success "Mihomo is already up to date"
            return 0
        fi
        
        # Backup old version
        backup_core "$MIHOMO_PATH"
    else
        log_info "First time installing Mihomo"
    fi
    
    # Download new version
    log_info "Downloading Mihomo v$latest_version..."
    local download_url="https://github.com/MetaCubeX/mihomo/releases/download/v${latest_version}/mihomo-${os}-${arch}-v${latest_version}.gz"
    local temp_dir=$(mktemp -d)
    
    if curl -L -o "$temp_dir/mihomo.gz" "$download_url"; then
        log_success "Download complete"
        
        # Extract
        gunzip "$temp_dir/mihomo.gz"
        
        # Install
        if [ -f "$temp_dir/mihomo" ]; then
            sudo mv "$temp_dir/mihomo" "$MIHOMO_PATH"
            sudo chmod +x "$MIHOMO_PATH"
            log_success "Mihomo v$latest_version installed"
            
            # Verify
            "$MIHOMO_PATH" -v
        else
            log_error "mihomo binary not found"
            rm -rf "$temp_dir"
            return 1
        fi
    else
        log_error "Download failed"
        rm -rf "$temp_dir"
        return 1
    fi
    
    rm -rf "$temp_dir"
}

# Show help
show_help() {
    echo "Usage: $0 [options]"
    echo ""
    echo "Options:"
    echo "  --singbox-only    Update Sing-box only"
    echo "  --mihomo-only     Update Mihomo only"
    echo "  --check-only      Check versions only, don't update"
    echo "  -h, --help        Show help"
    echo ""
    echo "Examples:"
    echo "  $0                    # Update all cores"
    echo "  $0 --singbox-only     # Update Sing-box only"
    echo "  $0 --check-only       # Check versions only"
    exit 0
}

# Parse arguments
SINGBOX_ONLY=false
MIHOMO_ONLY=false
CHECK_ONLY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --singbox-only) SINGBOX_ONLY=true; shift ;;
        --mihomo-only) MIHOMO_ONLY=true; shift ;;
        --check-only) CHECK_ONLY=true; shift ;;
        -h|--help) show_help ;;
        *) log_error "Unknown option: $1"; exit 1 ;;
    esac
done

# Show banner
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       Sing-box & Mihomo Core Update Tool                     ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check permissions
if [ "$CHECK_ONLY" = false ]; then
    if ! sudo -n true 2>/dev/null; then
        log_warning "sudo permission required to install cores"
        sudo -v
    fi
fi

# Execute update
if [ "$CHECK_ONLY" = true ]; then
    log_info "Check mode (will not update)"
    echo ""
fi

if [ "$MIHOMO_ONLY" = false ]; then
    update_singbox || log_warning "Sing-box update failed"
    echo ""
fi

if [ "$SINGBOX_ONLY" = false ]; then
    update_mihomo || log_warning "Mihomo update failed"
    echo ""
fi

log_success "Core update complete"
