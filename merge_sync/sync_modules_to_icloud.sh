#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# Module Sync Script - Surge Modules to iCloud (Surge + Shadowrocket)
# ═══════════════════════════════════════════════════════════════════════════════
# Features:
# 1. Sync Surge modules to Surge iCloud directory
# 2. Sync and convert modules to Shadowrocket iCloud (compatibility conversion)
# 3. Auto-exclude sensitive files
# 4. Support selective or batch sync
# ═══════════════════════════════════════════════════════════════════════════════

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Path configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SOURCE_DIR="$PROJECT_ROOT/module/surge(main)"

# iCloud directory configuration
SURGE_ICLOUD_DIR="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents"
SHADOWROCKET_ICLOUD_DIR="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents"

# Sensitive keywords (for exclusion)
SENSITIVE_KEYWORDS=(
    "private"
    "secret"
    "password"
    "token"
    "api-key"
    "YOUR_"
)

# Logging functions
log_info() {
    echo -e "${CYAN}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[⚠]${NC} $1"
}

log_error() {
    echo -e "${RED}[✗]${NC} $1"
}

log_section() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
}

# Check if directories exist
check_directories() {
    log_section "Checking Directories"
    
    if [[ ! -d "$SOURCE_DIR" ]]; then
        log_error "Source directory not found: $SOURCE_DIR"
        exit 1
    fi
    log_success "Source: $SOURCE_DIR"
    
    if [[ ! -d "$SURGE_ICLOUD_DIR" ]]; then
        log_warning "Surge iCloud not found: $SURGE_ICLOUD_DIR"
        log_info "Will skip Surge sync"
        SURGE_AVAILABLE=false
    else
        log_success "Surge iCloud: $SURGE_ICLOUD_DIR"
        SURGE_AVAILABLE=true
    fi
    
    if [[ ! -d "$SHADOWROCKET_ICLOUD_DIR" ]]; then
        log_warning "Shadowrocket iCloud not found: $SHADOWROCKET_ICLOUD_DIR"
        log_info "Will skip Shadowrocket sync"
        SHADOWROCKET_AVAILABLE=false
    else
        log_success "Shadowrocket iCloud: $SHADOWROCKET_ICLOUD_DIR"
        SHADOWROCKET_AVAILABLE=true
    fi
    
    if [[ "$SURGE_AVAILABLE" == false ]] && [[ "$SHADOWROCKET_AVAILABLE" == false ]]; then
        log_error "All target directories unavailable, cannot sync"
        exit 1
    fi
}

# Check if file contains sensitive keywords
is_sensitive_file() {
    local filename="$1"
    
    for keyword in "${SENSITIVE_KEYWORDS[@]}"; do
        if [[ "$filename" == *"$keyword"* ]]; then
            return 0  # Is sensitive
        fi
    done
    
    return 1  # Not sensitive
}

# Sync single module to Surge iCloud
sync_to_surge() {
    local module_file="$1"
    local module_name=$(basename "$module_file")
    
    # Check if sensitive
    if is_sensitive_file "$module_name"; then
        log_warning "Skipped sensitive: $module_name"
        return
    fi
    
    # Copy to Surge iCloud
    cp "$module_file" "$SURGE_ICLOUD_DIR/$module_name"
    log_success "Surge: $module_name"
}

# Sync single module to Shadowrocket iCloud
sync_to_shadowrocket() {
    local module_file="$1"
    local module_name=$(basename "$module_file")
    
    # Check if sensitive
    if is_sensitive_file "$module_name"; then
        log_warning "Skipped sensitive: $module_name"
        return
    fi
    
    # Convert and copy to Shadowrocket iCloud
    local output_file="$SHADOWROCKET_ICLOUD_DIR/$module_name"
    
    # Use sed for compatibility conversion (single pass)
    sed -e 's/REJECT-DROP/REJECT/g' \
        -e 's/REJECT-NO-DROP/REJECT/g' \
        -e 's/hostname = %APPEND% /hostname = /g' \
        "$module_file" > "$output_file"
    
    if [[ $? -eq 0 ]]; then
        log_success "Shadowrocket: $module_name"
    else
        log_warning "Shadowrocket conversion failed: $module_name"
    fi
}

# Sync all modules
sync_all_modules() {
    log_section "Syncing All Modules"
    
    local surge_count=0
    local shadowrocket_count=0
    local skipped_count=0
    
    for module_file in "$SOURCE_DIR"/*.sgmodule; do
        if [[ ! -f "$module_file" ]]; then
            continue
        fi
        
        local module_name=$(basename "$module_file")
        
        # Check if sensitive
        if is_sensitive_file "$module_name"; then
            log_warning "Skipped sensitive: $module_name"
            ((skipped_count++))
            continue
        fi
        
        log_info "Processing: $module_name"
        
        # Sync to Surge
        if [[ "$SURGE_AVAILABLE" == true ]]; then
            sync_to_surge "$module_file"
            ((surge_count++))
        fi
        
        # Sync to Shadowrocket
        if [[ "$SHADOWROCKET_AVAILABLE" == true ]]; then
            sync_to_shadowrocket "$module_file"
            ((shadowrocket_count++))
        fi
        
        echo ""
    done
    
    log_section "Sync Statistics"
    if [[ "$SURGE_AVAILABLE" == true ]]; then
        echo "Surge: $surge_count modules"
    fi
    if [[ "$SHADOWROCKET_AVAILABLE" == true ]]; then
        echo "Shadowrocket: $shadowrocket_count modules"
    fi
    echo "Skipped: $skipped_count sensitive files"
}

# Sync specific module
sync_specific_module() {
    local module_name="$1"
    local module_file="$SOURCE_DIR/$module_name"
    
    if [[ ! -f "$module_file" ]]; then
        log_error "Module file not found: $module_name"
        exit 1
    fi
    
    # Check if sensitive
    if is_sensitive_file "$module_name"; then
        log_error "Cannot sync sensitive file: $module_name"
        exit 1
    fi
    
    log_section "Syncing Specific Module: $module_name"
    
    # Sync to Surge
    if [[ "$SURGE_AVAILABLE" == true ]]; then
        sync_to_surge "$module_file"
    fi
    
    # Sync to Shadowrocket
    if [[ "$SHADOWROCKET_AVAILABLE" == true ]]; then
        sync_to_shadowrocket "$module_file"
    fi
}

# List all syncable modules
list_modules() {
    log_section "Syncable Modules List"
    
    local count=0
    local sensitive_count=0
    
    for module_file in "$SOURCE_DIR"/*.sgmodule; do
        if [[ ! -f "$module_file" ]]; then
            continue
        fi
        
        local module_name=$(basename "$module_file")
        
        if is_sensitive_file "$module_name"; then
            echo -e "${YELLOW}[Sensitive]${NC} $module_name"
            ((sensitive_count++))
        else
            echo -e "${GREEN}[Syncable]${NC} $module_name"
            ((count++))
        fi
    done
    
    echo ""
    echo "Syncable: $count modules"
    echo "Sensitive: $sensitive_count files (will be skipped)"
}

# Clean duplicate modules
clean_duplicate_modules() {
    log_section "Cleaning Duplicate Modules"
    
    local cleaned=0
    
    # Clean duplicates in Surge iCloud
    if [[ "$SURGE_AVAILABLE" == true ]]; then
        log_info "Checking Surge iCloud duplicates..."
        
        # Known duplicate modules list
        local duplicates=(
            "🔐加密dns.sgmodule"  # Duplicate of "Encrypted DNS Module 🔒🛡️DNS.sgmodule"
        )
        
        for dup in "${duplicates[@]}"; do
            local dup_file="$SURGE_ICLOUD_DIR/$dup"
            if [[ -f "$dup_file" ]]; then
                rm "$dup_file"
                log_success "Removed duplicate: $dup"
                ((cleaned++))
            fi
        done
    fi
    
    # Clean old synced files in Shadowrocket
    if [[ "$SHADOWROCKET_AVAILABLE" == true ]]; then
        log_info "Cleaning Shadowrocket old sync files..."
        for old_file in "$SHADOWROCKET_ICLOUD_DIR"/__*.sgmodule; do
            if [[ -f "$old_file" ]]; then
                rm "$old_file"
                log_info "Removed old file: $(basename "$old_file")"
                ((cleaned++))
            fi
        done
    fi
    
    if [[ $cleaned -eq 0 ]]; then
        log_info "No duplicates or old files found"
    else
        log_success "Total cleaned: $cleaned files"
    fi
}

# Show help information
show_help() {
    cat << EOF
${BLUE}═══════════════════════════════════════════════════════════════${NC}
  Module Sync Script - Surge Modules to iCloud
${BLUE}═══════════════════════════════════════════════════════════════${NC}

Usage:
  $0 [options] [module_name]

Options:
  -a, --all       Sync all modules (default)
  -l, --list      List all syncable modules
  -c, --clean     Clean duplicate/old files
  -h, --help      Show this help message

Examples:
  $0                                    # Sync all modules
  $0 --all                              # Sync all modules
  $0 "URL Rewrite Module 🔄🌐.sgmodule"  # Sync specific module
  $0 --list                             # List all modules
  $0 --clean                            # Clean old files

Sync Targets:
  - Surge iCloud: $SURGE_ICLOUD_DIR
  - Shadowrocket: $SHADOWROCKET_ICLOUD_DIR

Sensitive File Exclusion:
  Files containing these keywords will be skipped:
  ${SENSITIVE_KEYWORDS[@]}

Compatibility Conversion:
  Shadowrocket modules are auto-converted:
  - REJECT-DROP → REJECT
  - REJECT-NO-DROP → REJECT
  - hostname %APPEND% → hostname

EOF
}

# Main function
main() {
    log_section "Module Sync Script"
    
    # Check directories
    check_directories
    
    # Parse arguments
    case "${1:-}" in
        -h|--help)
            show_help
            exit 0
            ;;
        -l|--list)
            list_modules
            exit 0
            ;;
        -c|--clean)
            clean_duplicate_modules
            exit 0
            ;;
        -a|--all|"")
            clean_duplicate_modules
            sync_all_modules
            ;;
        *)
            # Sync specific module
            sync_specific_module "$1"
            ;;
    esac
    
    log_section "Complete"
    log_success "Module sync completed!"
    echo ""
    echo "Next steps:"
    echo "1. Open Surge or Shadowrocket app"
    echo "2. Refresh module list"
    echo "3. Enable desired modules"
}

# Execute main function
main "$@"
