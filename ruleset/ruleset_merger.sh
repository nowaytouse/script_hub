#!/bin/bash
# ============================================
# Script: Ruleset Merger - Proxy Rule Aggregator
# Version: 2.2
# Updated: 2025-12-03
# Description:
#   - Download rulesets from third-party URLs
#   - Auto-deduplicate rules
#   - Merge into target ruleset file
#   - Support DOMAIN-SUFFIX, DOMAIN-KEYWORD, IP-CIDR, etc.
#   - Auto-generate header with timestamp
#   - Support scheduled auto-update (cron)
#   - Auto-detect and preserve manual rules (no flag needed!)
# Usage:
#   ./ruleset_merger.sh [options]
#   Options:
#     -t, --target <file>     Target/base ruleset file (required)
#     -s, --source <URL>      Add third-party ruleset URL (can use multiple)
#     -f, --file <file>       Read rules from local file (can use multiple)
#     -l, --list <file>       Read URL list from file (one URL per line)
#     -o, --output <file>     Output to new file instead of overwriting target
#     -n, --name <name>       Ruleset name (for header info)
#     -k, --keep-comments     Keep all comment lines
#     -d, --dry-run           Show result only, don't write file
#     -g, --git-push          Auto git commit & push after update
#     -c, --cron              Install/manage cron jobs
#     -v, --verbose           Show verbose output
#     -h, --help              Show help message
# Manual Rules:
#   Create <name>_manual.txt in same directory (e.g., Telegram_manual.txt)
#   These rules will be automatically preserved across updates!
# Examples:
#   ./ruleset_merger.sh -t base.list -o TikTok.list -n "TikTok" -s "URL1" -s "URL2"
#   ./ruleset_merger.sh -t base.list -l sources.txt -o merged.list -g
#   ./ruleset_merger.sh --cron
# ============================================

set -e

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Default values
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_FILE=""
OUTPUT_FILE=""
RULESET_NAME=""
MANUAL_FILE=""
SOURCES=()
LOCAL_FILES=()
URL_LIST_FILE=""
KEEP_COMMENTS=false
DRY_RUN=false
VERBOSE=false
GIT_PUSH=false
SETUP_CRON=false
TEMP_DIR=""

# Statistics
TOTAL_RULES_BEFORE=0
TOTAL_RULES_ADDED=0
TOTAL_RULES_DUPLICATE=0
TOTAL_RULES_AFTER=0


# Print colored messages
print_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[OK]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }
print_verbose() { [ "$VERBOSE" = true ] && echo -e "${CYAN}[DEBUG]${NC} $1" || true; }

# Show help
show_help() {
    cat << EOF
Ruleset Merger - Proxy Rule Aggregator v2.0

Usage: $(basename "$0") [options]

Options:
  -t, --target <file>     Target/base ruleset file (required)
  -s, --source <URL>      Add third-party ruleset URL (can use multiple)
  -f, --file <file>       Read rules from local file (can use multiple)
  -l, --list <file>       Read URL list from file (one URL per line)
  -o, --output <file>     Output to new file instead of overwriting target
  -n, --name <name>       Ruleset name (for header, default: inferred from filename)
  -k, --keep-comments     Keep all comment lines
  -d, --dry-run           Show result only, don't write file
  -g, --git-push          Auto git commit & push after update
  -c, --cron              Install/manage cron jobs
  -v, --verbose           Show verbose output
  -h, --help              Show this help message

Manual Rules (Auto-Detected):
  Create <name>_manual.txt in same directory
  Example: Telegram_manual.txt for Telegram.list
  These rules are automatically preserved across updates!

Examples:
  # Merge TikTok rulesets
  $(basename "$0") -t base.list -o TikTok.list -n "TikTok" \\
    -s "https://raw.githubusercontent.com/user/repo/TikTok.list"

  # Merge with auto-detected manual rules (create TikTok_manual.txt first)
  $(basename "$0") -t base.list -l sources.txt -o TikTok.list -g

  # Install daily auto-update (cron)
  $(basename "$0") --cron

Supported Rule Types:
  DOMAIN-SUFFIX, DOMAIN-KEYWORD, DOMAIN, IP-CIDR, IP-CIDR6,
  GEOIP, URL-REGEX, USER-AGENT, PROCESS-NAME

EOF
}

# Cleanup on exit
cleanup() {
    [ -n "$TEMP_DIR" ] && [ -d "$TEMP_DIR" ] && rm -rf "$TEMP_DIR"
}
trap cleanup EXIT

# Parse arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -t|--target) TARGET_FILE="$2"; shift 2 ;;
            -s|--source) SOURCES+=("$2"); shift 2 ;;
            -f|--file) LOCAL_FILES+=("$2"); shift 2 ;;
            -l|--list) URL_LIST_FILE="$2"; shift 2 ;;
            -o|--output) OUTPUT_FILE="$2"; shift 2 ;;
            -n|--name) RULESET_NAME="$2"; shift 2 ;;
            -k|--keep-comments) KEEP_COMMENTS=true; shift ;;
            -d|--dry-run) DRY_RUN=true; shift ;;
            -g|--git-push) GIT_PUSH=true; shift ;;
            -c|--cron) SETUP_CRON=true; shift ;;
            -v|--verbose) VERBOSE=true; shift ;;
            -h|--help) show_help; exit 0 ;;
            *) print_error "Unknown option: $1"; exit 1 ;;
        esac
    done
}

# Validate arguments
validate_args() {
    if [ -z "$TARGET_FILE" ]; then
        print_error "Target file required (-t)"
        exit 1
    fi
    
    # Find target file
    if [ ! -f "$TARGET_FILE" ]; then
        if [ -f "$SCRIPT_DIR/$TARGET_FILE" ]; then
            TARGET_FILE="$SCRIPT_DIR/$TARGET_FILE"
        else
            print_error "Target file not found: $TARGET_FILE"
            exit 1
        fi
    fi
    
    # Check sources
    if [ ${#SOURCES[@]} -eq 0 ] && [ ${#LOCAL_FILES[@]} -eq 0 ] && [ -z "$URL_LIST_FILE" ]; then
        print_error "At least one source required (-s, -f, or -l)"
        exit 1
    fi
    
    # Check URL list file
    if [ -n "$URL_LIST_FILE" ] && [ ! -f "$URL_LIST_FILE" ]; then
        if [ -f "$SCRIPT_DIR/$URL_LIST_FILE" ]; then
            URL_LIST_FILE="$SCRIPT_DIR/$URL_LIST_FILE"
        else
            print_error "URL list file not found: $URL_LIST_FILE"
            exit 1
        fi
    fi
    
    if [ -z "$OUTPUT_FILE" ]; then
        OUTPUT_FILE="$TARGET_FILE"
    fi
    
    return 0
}

# Create temp directory
create_temp_dir() {
    TEMP_DIR=$(mktemp -d)
    print_verbose "Created temp dir: $TEMP_DIR"
}

# Download ruleset
download_ruleset() {
    local url="$1" output_file="$2"
    print_verbose "Downloading: $url"
    
    if command -v curl &>/dev/null; then
        curl -sL --connect-timeout 10 --max-time 30 "$url" -o "$output_file" 2>/dev/null && return 0
    elif command -v wget &>/dev/null; then
        wget -q --timeout=10 "$url" -O "$output_file" 2>/dev/null && return 0
    else
        print_error "curl or wget required"; exit 1
    fi
    return 1
}

# Extract valid rules
extract_rules() {
    local input="$1" output="$2"
    grep -E '^(DOMAIN-SUFFIX|DOMAIN-KEYWORD|DOMAIN|IP-CIDR|IP-CIDR6|GEOIP|URL-REGEX|USER-AGENT|PROCESS-NAME),' "$input" 2>/dev/null | \
        sed 's/[[:space:]]*$//' | sort -u > "$output"
}


# Generate ruleset header (English)
generate_header() {
    local name="$1" total_rules="$2" sources_list="$3"
    local update_date=$(date "+%Y-%m-%d")
    local update_time=$(date -u "+%H:%M:%S UTC")
    local git_hash=$(git rev-parse --short HEAD 2>/dev/null || echo "N/A")
    
    cat << EOF
# ═══════════════════════════════════════════════════════════════
# Ruleset: ${name}
# Updated: ${update_date} ${update_time}
# Total Rules: ${total_rules}
# Generator: Ruleset Merger v2.0
# Git Hash: ${git_hash}
# ═══════════════════════════════════════════════════════════════
#
# Sources:
${sources_list}
#
# Usage:
#   - Auto-generated ruleset, DO NOT edit manually
#   - Rules have no action (REJECT/PROXY/DIRECT), configure as needed
#   - Compatible with Surge/Shadowrocket/Clash/Quantumult X
#   - Manual rules in ${name}_manual.txt are auto-preserved
#
# Auto-Update:
#   - Cron job pulls upstream updates daily
#   - Manual: ./ruleset_merger.sh -l sources.txt -o ${name}.list
#
# ═══════════════════════════════════════════════════════════════

EOF
}

# Git commit and push
git_commit_push() {
    local output_file="$1" ruleset_name="$2" rules_count="$3"
    [ "$GIT_PUSH" != true ] && return 0
    
    print_info "Git commit..."
    local commit_msg="chore(ruleset): update ${ruleset_name} [${rules_count} rules] - $(date '+%Y-%m-%d %H:%M')"
    
    git rev-parse --git-dir &>/dev/null || { print_warning "Not a git repo, skip"; return 0; }
    git add "$output_file" 2>/dev/null || true
    git diff --cached --quiet && { print_info "No changes, skip commit"; return 0; }
    
    git commit -m "$commit_msg" &>/dev/null && print_success "Committed: $commit_msg" || print_warning "Commit failed"
    git push &>/dev/null && print_success "Pushed to remote" || print_warning "Push failed, please push manually"
}

# Setup cron jobs
setup_cron() {
    echo ""
    echo "╔══════════════════════════════════════════╗"
    echo "║         Cron Job Manager                 ║"
    echo "╚══════════════════════════════════════════╝"
    echo ""
    
    local cron_script="$SCRIPT_DIR/cron_update.sh"
    local cron_config="$SCRIPT_DIR/cron_jobs.conf"
    
    echo "Select action:"
    echo "  1) View current cron jobs"
    echo "  2) Add new scheduled update"
    echo "  3) Remove cron jobs"
    echo "  4) Generate example config"
    echo "  5) Exit"
    echo ""
    read -p "Enter option [1-5]: " choice
    
    case $choice in
        1)
            echo ""
            print_info "Current ruleset cron jobs:"
            crontab -l 2>/dev/null | grep -E "ruleset_merger|cron_update" || echo "  (none)"
            ;;
        2)
            echo ""
            read -p "Frequency [1=daily/2=weekly/3=custom]: " freq
            local cron_time=""
            case $freq in
                1) cron_time="0 6 * * *" ;;
                2) cron_time="0 6 * * 0" ;;
                3) read -p "Enter cron expression (e.g. '0 */6 * * *'): " cron_time ;;
                *) print_error "Invalid option"; exit 1 ;;
            esac
            
            cat > "$cron_script" << 'CRONEOF'
#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_FILE="$SCRIPT_DIR/cron_update.log"
echo "========== $(date) ==========" >> "$LOG_FILE"
if [ -f "$SCRIPT_DIR/cron_jobs.conf" ]; then
    while IFS='|' read -r name target sources output; do
        [[ "$name" =~ ^#.*$ || -z "$name" ]] && continue
        echo "Updating: $name" >> "$LOG_FILE"
        "$SCRIPT_DIR/ruleset_merger.sh" -t "$target" -l "$sources" -o "$output" -n "$name" -g >> "$LOG_FILE" 2>&1
    done < "$SCRIPT_DIR/cron_jobs.conf"
fi
echo "Done" >> "$LOG_FILE"
CRONEOF
            chmod +x "$cron_script"
            (crontab -l 2>/dev/null | grep -v "cron_update.sh"; echo "$cron_time $cron_script") | crontab -
            print_success "Cron job added: $cron_time"
            
            [ ! -f "$cron_config" ] && cat > "$cron_config" << 'CONFEOF'
# Cron update config
# Format: name|base_file|sources_file|output_file
TikTok|base.list|TikTok_sources.txt|TikTok.list
Google|base.list|Google_sources.txt|Google.list
CONFEOF
            ;;
        3)
            crontab -l 2>/dev/null | grep -v "cron_update.sh" | crontab -
            print_success "Removed ruleset cron jobs"
            ;;
        4)
            cat > "$SCRIPT_DIR/example_sources.txt" << 'SRCEOF'
# Example sources list (one URL per line, # for comments)
https://raw.githubusercontent.com/Coldvvater/Mononoke/master/Surge/Rules/TikTok.list
https://raw.githubusercontent.com/Semporia/TikTok-Unlock/master/Surge/TikTok.list
SRCEOF
            print_success "Created: $SCRIPT_DIR/example_sources.txt"
            ;;
        5) exit 0 ;;
    esac
    exit 0
}


# Main merge logic
merge_rules() {
    create_temp_dir
    
    local all_new_rules="$TEMP_DIR/all_new_rules.txt"
    local existing_rules="$TEMP_DIR/existing_rules.txt"
    local merged_rules="$TEMP_DIR/merged_rules.txt"
    local final_output="$TEMP_DIR/final_output.txt"
    local sources_list_file="$TEMP_DIR/sources_list.txt"
    
    touch "$all_new_rules" "$sources_list_file"
    
    # Infer ruleset name
    [ -z "$RULESET_NAME" ] && RULESET_NAME=$(basename "$OUTPUT_FILE" .list | sed 's/_merged$//' | sed 's/_final$//')
    
    # Auto-detect manual rules file
    local auto_manual_file="${RULESET_NAME}_manual.txt"
    if [ -f "$auto_manual_file" ]; then
        MANUAL_FILE="$auto_manual_file"
        print_verbose "Auto-detected manual rules: $MANUAL_FILE"
    elif [ -f "$SCRIPT_DIR/$auto_manual_file" ]; then
        MANUAL_FILE="$SCRIPT_DIR/$auto_manual_file"
        print_verbose "Auto-detected manual rules: $MANUAL_FILE"
    fi
    
    # Extract existing rules
    print_info "Reading target: $TARGET_FILE"
    extract_rules "$TARGET_FILE" "$existing_rules"
    TOTAL_RULES_BEFORE=$(wc -l < "$existing_rules" | tr -d ' ')
    print_info "Existing rules: $TOTAL_RULES_BEFORE"
    
    # Read URL list file
    if [ -n "$URL_LIST_FILE" ]; then
        print_info "Reading URL list: $URL_LIST_FILE"
        while IFS= read -r url || [ -n "$url" ]; do
            url=$(echo "$url" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
            [ -n "$url" ] && [[ ! "$url" =~ ^# ]] && SOURCES+=("$url")
        done < "$URL_LIST_FILE"
    fi
    
    # Download and process remote rulesets
    local source_count=0
    for url in "${SOURCES[@]}"; do
        source_count=$((source_count + 1))
        local temp_download="$TEMP_DIR/download_${source_count}.txt"
        local temp_extracted="$TEMP_DIR/extracted_${source_count}.txt"
        
        print_info "Processing [$source_count/${#SOURCES[@]}]: $url"
        
        if download_ruleset "$url" "$temp_download"; then
            extract_rules "$temp_download" "$temp_extracted"
            local rules_count=$(wc -l < "$temp_extracted" | tr -d ' ')
            print_verbose "  Extracted: $rules_count rules"
            cat "$temp_extracted" >> "$all_new_rules"
            echo "#   - $url" >> "$sources_list_file"
        else
            print_warning "Download failed: $url"
            echo "#   - $url (FAILED)" >> "$sources_list_file"
        fi
    done
    
    # Process local files
    for local_file in "${LOCAL_FILES[@]}"; do
        if [ -f "$local_file" ]; then
            local temp_extracted="$TEMP_DIR/local_extracted_${source_count}.txt"
            source_count=$((source_count + 1))
            print_info "Processing local: $local_file"
            extract_rules "$local_file" "$temp_extracted"
            cat "$temp_extracted" >> "$all_new_rules"
            echo "#   - [LOCAL] $local_file" >> "$sources_list_file"
        else
            print_warning "Local file not found: $local_file"
        fi
    done
    
    # Process persistent manual rules file (auto-detected)
    if [ -n "$MANUAL_FILE" ] && [ -f "$MANUAL_FILE" ]; then
        local temp_manual="$TEMP_DIR/manual_extracted.txt"
        print_info "Processing manual rules: $(basename "$MANUAL_FILE")"
        extract_rules "$MANUAL_FILE" "$temp_manual"
        local manual_count=$(wc -l < "$temp_manual" | tr -d ' ')
        print_verbose "  Manual rules: $manual_count"
        cat "$temp_manual" >> "$all_new_rules"
        echo "#   - [MANUAL] $(basename "$MANUAL_FILE") (auto-preserved)" >> "$sources_list_file"
    fi
    
    # Deduplicate
    sort -u "$all_new_rules" -o "$all_new_rules"
    local new_rules_count=$(wc -l < "$all_new_rules" | tr -d ' ')
    print_info "New rules (deduplicated): $new_rules_count"
    
    # Merge and deduplicate
    cat "$existing_rules" "$all_new_rules" | sort -u > "$merged_rules"
    TOTAL_RULES_AFTER=$(wc -l < "$merged_rules" | tr -d ' ')
    TOTAL_RULES_ADDED=$((TOTAL_RULES_AFTER - TOTAL_RULES_BEFORE))
    TOTAL_RULES_DUPLICATE=$((new_rules_count - TOTAL_RULES_ADDED))
    
    # Generate output
    local sources_list=$(cat "$sources_list_file")
    generate_header "$RULESET_NAME" "$TOTAL_RULES_AFTER" "$sources_list" > "$final_output"
    
    # Categorize rules
    local domain_suffix=$(grep '^DOMAIN-SUFFIX,' "$merged_rules" | sort -t',' -k2 || true)
    local domain_keyword=$(grep '^DOMAIN-KEYWORD,' "$merged_rules" | sort -t',' -k2 || true)
    local domain=$(grep '^DOMAIN,' "$merged_rules" | sort -t',' -k2 || true)
    local ip_cidr=$(grep '^IP-CIDR,' "$merged_rules" | sort -t',' -k2 || true)
    local ip_cidr6=$(grep '^IP-CIDR6,' "$merged_rules" | sort -t',' -k2 || true)
    local other=$(grep -v -E '^(DOMAIN-SUFFIX|DOMAIN-KEYWORD|DOMAIN|IP-CIDR|IP-CIDR6),' "$merged_rules" || true)
    
    [ -n "$domain_suffix" ] && { echo "# ========== DOMAIN-SUFFIX ==========" >> "$final_output"; echo "$domain_suffix" >> "$final_output"; echo "" >> "$final_output"; }
    [ -n "$domain_keyword" ] && { echo "# ========== DOMAIN-KEYWORD ==========" >> "$final_output"; echo "$domain_keyword" >> "$final_output"; echo "" >> "$final_output"; }
    [ -n "$domain" ] && { echo "# ========== DOMAIN ==========" >> "$final_output"; echo "$domain" >> "$final_output"; echo "" >> "$final_output"; }
    [ -n "$ip_cidr" ] && { echo "# ========== IP-CIDR ==========" >> "$final_output"; echo "$ip_cidr" >> "$final_output"; echo "" >> "$final_output"; }
    [ -n "$ip_cidr6" ] && { echo "# ========== IP-CIDR6 ==========" >> "$final_output"; echo "$ip_cidr6" >> "$final_output"; echo "" >> "$final_output"; }
    [ -n "$other" ] && { echo "# ========== OTHER ==========" >> "$final_output"; echo "$other" >> "$final_output"; echo "" >> "$final_output"; }
    
    echo "# ========== END ==========" >> "$final_output"
    
    # Statistics
    echo ""
    echo "╔══════════════════════════════════════════╗"
    echo "║            Merge Statistics              ║"
    echo "╠══════════════════════════════════════════╣"
    printf "║  Before:      %-25s ║\n" "$TOTAL_RULES_BEFORE"
    printf "║  Added:       %-25s ║\n" "$TOTAL_RULES_ADDED"
    printf "║  Duplicates:  %-25s ║\n" "$TOTAL_RULES_DUPLICATE"
    printf "║  After:       %-25s ║\n" "$TOTAL_RULES_AFTER"
    echo "╚══════════════════════════════════════════╝"
    echo ""
    
    # Write output
    if [ "$DRY_RUN" = true ]; then
        print_info "Dry run - not writing file"
        [ "$VERBOSE" = true ] && { echo "===== Preview (first 50 lines) ====="; head -n 50 "$final_output"; echo "..."; }
    else
        cp "$final_output" "$OUTPUT_FILE"
        print_success "Ruleset updated: $OUTPUT_FILE"
        git_commit_push "$OUTPUT_FILE" "$RULESET_NAME" "$TOTAL_RULES_AFTER"
    fi
}

# Main
main() {
    echo ""
    echo "╔══════════════════════════════════════════╗"
    echo "║     Ruleset Merger - Rule Aggregator     ║"
    echo "║              Version 2.0                 ║"
    echo "╚══════════════════════════════════════════╝"
    echo ""
    
    parse_args "$@"
    [ "$SETUP_CRON" = true ] && { setup_cron; exit 0; }
    validate_args
    merge_rules
    
    echo ""
    print_success "Done!"
}

main "$@"
