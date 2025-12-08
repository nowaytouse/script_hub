#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# Organize Modules by Group into Subfolders
# 
# This script reads the #!group= metadata from each module and organizes them
# into subfolders based on their group assignment.
#
# Groups:
# - 『 🛠️ Amplify Nexus › 增幅枢纽 』 -> amplify_nexus/
# - 『 🔝 Head Expanse › 首端扩域 』 -> head_expanse/
# - 『 🎯 Narrow Pierce › 窄域穿刺 』 -> narrow_pierce/
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

# Group name to folder mapping
declare -A GROUP_FOLDERS=(
    ["『 🛠️ Amplify Nexus › 增幅枢纽 』"]="amplify_nexus"
    ["『 🔝 Head Expanse › 首端扩域 』"]="head_expanse"
    ["『 🎯 Narrow Pierce › 窄域穿刺 』"]="narrow_pierce"
)

# ═══════════════════════════════════════════════════════════════════════════════
# Functions
# ═══════════════════════════════════════════════════════════════════════════════

get_module_group() {
    local module_file="$1"
    
    # Extract #!group= line
    local group=$(grep "^#!group=" "$module_file" 2>/dev/null | head -1 | sed 's/^#!group=//')
    
    echo "$group"
}

organize_modules() {
    local base_dir="$1"
    local dir_name=$(basename "$base_dir")
    
    log_header "Organizing $dir_name Modules"
    
    # Create subdirectories
    for folder in "${GROUP_FOLDERS[@]}"; do
        mkdir -p "$base_dir/$folder"
        log_info "Created: $base_dir/$folder"
    done
    
    # Statistics
    local moved=0
    local no_group=0
    local total=0
    
    # Process each module
    for module in "$base_dir"/*.sgmodule "$base_dir"/*.module; do
        [ ! -f "$module" ] && continue
        
        # Skip if it's in a subdirectory (check if parent dir is one of our group folders)
        local parent_dir=$(basename "$(dirname "$module")")
        for folder in "${GROUP_FOLDERS[@]}"; do
            if [ "$parent_dir" = "$folder" ]; then
                continue 2
            fi
        done
        
        total=$((total + 1))
        
        local filename=$(basename "$module")
        local group=$(get_module_group "$module")
        
        if [ -z "$group" ]; then
            log_warning "No group found: $filename"
            no_group=$((no_group + 1))
            continue
        fi
        
        # Find matching folder
        local target_folder=""
        for group_name in "${!GROUP_FOLDERS[@]}"; do
            if [ "$group" = "$group_name" ]; then
                target_folder="${GROUP_FOLDERS[$group_name]}"
                break
            fi
        done
        
        if [ -z "$target_folder" ]; then
            log_warning "Unknown group '$group': $filename"
            no_group=$((no_group + 1))
            continue
        fi
        
        # Move to subfolder
        local target_path="$base_dir/$target_folder/$filename"
        
        if [ -f "$target_path" ]; then
            log_warning "Already exists: $target_folder/$filename (skipping)"
            continue
        fi
        
        mv "$module" "$target_path"
        log_success "Moved: $filename -> $target_folder/"
        moved=$((moved + 1))
    done
    
    log_header "$dir_name Summary"
    log_info "Total modules:     $total"
    log_success "Moved:             $moved"
    [ $no_group -gt 0 ] && log_warning "No group/Unknown:  $no_group" || log_info "No group/Unknown:  0"
}

# ═══════════════════════════════════════════════════════════════════════════════
# Main Script
# ═══════════════════════════════════════════════════════════════════════════════

log_header "Module Organization by Group"

# Organize Surge modules
organize_modules "$MODULE_DIR"

# Organize Shadowrocket modules
organize_modules "$SHADOWROCKET_MODULE_DIR"

log_success "Done!"
