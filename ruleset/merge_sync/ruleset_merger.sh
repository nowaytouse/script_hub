#!/opt/homebrew/bin/bash
# ============================================
# Script: Ruleset Merger - Proxy Rule Aggregator
# Version: 2.4
# Updated: 2025-12-04
# Description:
#   - Download rulesets from third-party URLs
#   - Auto-deduplicate rules
#   - Merge into target ruleset file
#   - Support DOMAIN-SUFFIX, DOMAIN-KEYWORD, IP-CIDR, etc.
#   - Support policy-specific rules (REJECT/REJECT-DROP/REJECT-NO-DROP)
#   - Auto-generate header with timestamp
#   - Support scheduled auto-update (cron)
#   - Auto-detect and preserve manual rules (no flag needed!)
# Usage:
#   ./ruleset_merger.sh [options]
#   Options:
#     -t, --target <file>     Target/base ruleset file (required)
#     -s, --source <URL>      Add third-party ruleset URL (can use multiple)
#     -f, --file <file>       Read rules from local file (can use multiple)
#     -l, --list <file>       Read URL list from file (format: URL|POLICY)
#     -o, --output <file>     Output to new file instead of overwriting target
#     -n, --name <name>       Ruleset name (for header info)
#     -k, --keep-comments     Keep all comment lines
#     -d, --dry-run           Show result only, don't write file
#     -g, --git-push          Auto git commit & push after update
#     -c, --cron              Install/manage cron jobs
#     -v, --verbose           Show verbose output
#     -h, --help              Show help message
# Policy Support:
#   Sources file format: URL|POLICY (e.g., https://example.com/rules.list|REJECT-DROP)
#   Supported policies: REJECT, REJECT-DROP, REJECT-NO-DROP
#   Default policy: REJECT (if not specified)
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
    cat << 'EOF'
Ruleset Merger - Proxy Rule Aggregator v2.4

Usage: ruleset_merger.sh [options]

Options:
  -t, --target <file>     Target/base ruleset file (required)
  -s, --source <URL>      Add third-party ruleset URL (can use multiple)
  -f, --file <file>       Read rules from local file (can use multiple)
  -l, --list <file>       Read URL list from file (format: URL|POLICY)
  -o, --output <file>     Output to new file instead of overwriting target
  -n, --name <name>       Ruleset name (for header, default: inferred from filename)
  -k, --keep-comments     Keep all comment lines
  -d, --dry-run           Show result only, don't write file
  -g, --git-push          Auto git commit & push after update
  -c, --cron              Install/manage cron jobs
  -v, --verbose           Show verbose output
  -h, --help              Show this help message

Policy Support:
  Sources file format: URL|POLICY (one per line)
  Example: https://example.com/rules.list|REJECT-DROP
  
  Supported policies:
    - REJECT         (default, standard rejection)
    - REJECT-DROP    (drop connection silently)
    - REJECT-NO-DROP (reject but don't drop connection)

Examples:
  ./ruleset_merger.sh -t base.list -o TikTok.list -n "TikTok" -s "URL1|REJECT"
  ./ruleset_merger.sh -t base.list -l sources.txt -o merged.list -g
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
    
    if [ ! -f "$TARGET_FILE" ]; then
        if [ -f "$SCRIPT_DIR/$TARGET_FILE" ]; then
            TARGET_FILE="$SCRIPT_DIR/$TARGET_FILE"
        else
            print_error "Target file not found: $TARGET_FILE"
            exit 1
        fi
    fi
    
    if [ ${#SOURCES[@]} -eq 0 ] && [ ${#LOCAL_FILES[@]} -eq 0 ] && [ -z "$URL_LIST_FILE" ]; then
        print_error "At least one source required (-s, -f, or -l)"
        exit 1
    fi
    
    if [ -n "$URL_LIST_FILE" ] && [ ! -f "$URL_LIST_FILE" ]; then
        if [ -f "$SCRIPT_DIR/$URL_LIST_FILE" ]; then
            URL_LIST_FILE="$SCRIPT_DIR/$URL_LIST_FILE"
        else
            print_error "URL list file not found: $URL_LIST_FILE"
            exit 1
        fi
    fi
    
    [ -z "$OUTPUT_FILE" ] && OUTPUT_FILE="$TARGET_FILE"
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
        curl -sL --connect-timeout 15 --max-time 60 "$url" -o "$output_file" 2>/dev/null && return 0
    elif command -v wget &>/dev/null; then
        wget -q --timeout=15 "$url" -O "$output_file" 2>/dev/null && return 0
    else
        print_error "curl or wget required"; exit 1
    fi
    return 1
}

# Extract valid rules (optimized for large files)
extract_rules() {
    local input="$1" output="$2"
    grep -E '^(DOMAIN-SUFFIX|DOMAIN-KEYWORD|DOMAIN|IP-CIDR|IP-CIDR6|GEOIP|URL-REGEX|USER-AGENT|PROCESS-NAME),' "$input" 2>/dev/null | \
        sed 's/[[:space:]]*$//' > "$output" || touch "$output"
}

# Generate ruleset header
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
# Generator: Ruleset Merger v2.4
# Git Hash: ${git_hash}
# ═══════════════════════════════════════════════════════════════
#
# Sources:
${sources_list}
#
# Usage:
#   - Auto-generated ruleset, DO NOT edit manually
#   - Rules grouped by type (DOMAIN-SUFFIX/DOMAIN/IP-CIDR/etc.)
#   - Policy is determined by RULE-SET line in config file
#   - Compatible with Surge/Shadowrocket/Clash/Quantumult X
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
    git push &>/dev/null && print_success "Pushed to remote" || print_warning "Push failed"
}

# Setup cron jobs
setup_cron() {
    echo ""
    echo "╔══════════════════════════════════════════╗"
    echo "║         Cron Job Manager                 ║"
    echo "╚══════════════════════════════════════════╝"
    echo ""
    
    echo "Select action:"
    echo "  1) View current cron jobs"
    echo "  2) Add new scheduled update"
    echo "  3) Remove cron jobs"
    echo "  4) Exit"
    echo ""
    read -p "Enter option [1-4]: " choice
    
    case $choice in
        1)
            print_info "Current ruleset cron jobs:"
            crontab -l 2>/dev/null | grep -E "ruleset_merger" || echo "  (none)"
            ;;
        2)
            read -p "Frequency [1=daily/2=weekly]: " freq
            local cron_time=""
            case $freq in
                1) cron_time="0 6 * * *" ;;
                2) cron_time="0 6 * * 0" ;;
                *) print_error "Invalid option"; exit 1 ;;
            esac
            (crontab -l 2>/dev/null; echo "$cron_time $SCRIPT_DIR/ruleset_merger.sh -t base.list -l sources.txt -o merged.list -g") | crontab -
            print_success "Cron job added: $cron_time"
            ;;
        3)
            crontab -l 2>/dev/null | grep -v "ruleset_merger" | crontab -
            print_success "Removed ruleset cron jobs"
            ;;
        4) exit 0 ;;
    esac
    exit 0
}

# Output rules by type (optimized) - 不添加策略标记
output_rules_by_type() {
    local input_file="$1"
    local policy="$2"  # 保留参数但不使用，避免破坏调用
    local output_file="$3"
    
    [ ! -s "$input_file" ] && return
    
    # Use awk for efficient categorization - 不在注释中添加策略标记
    awk '
    BEGIN { 
        ds_count=0; dk_count=0; d_count=0; ip_count=0; ip6_count=0; other_count=0
    }
    /^DOMAIN-SUFFIX,/ { ds[ds_count++]=$0; next }
    /^DOMAIN-KEYWORD,/ { dk[dk_count++]=$0; next }
    /^DOMAIN,/ { d[d_count++]=$0; next }
    /^IP-CIDR,/ { ip[ip_count++]=$0; next }
    /^IP-CIDR6,/ { ip6[ip6_count++]=$0; next }
    { other[other_count++]=$0 }
    END {
        if (ds_count > 0) {
            print "# ========== DOMAIN-SUFFIX =========="
            for (i=0; i<ds_count; i++) print ds[i]
            print ""
        }
        if (dk_count > 0) {
            print "# ========== DOMAIN-KEYWORD =========="
            for (i=0; i<dk_count; i++) print dk[i]
            print ""
        }
        if (d_count > 0) {
            print "# ========== DOMAIN =========="
            for (i=0; i<d_count; i++) print d[i]
            print ""
        }
        if (ip_count > 0) {
            print "# ========== IP-CIDR =========="
            for (i=0; i<ip_count; i++) print ip[i]
            print ""
        }
        if (ip6_count > 0) {
            print "# ========== IP-CIDR6 =========="
            for (i=0; i<ip6_count; i++) print ip6[i]
            print ""
        }
        if (other_count > 0) {
            print "# ========== OTHER =========="
            for (i=0; i<other_count; i++) print other[i]
            print ""
        }
    }
    ' "$input_file" >> "$output_file"
}

# Main merge logic (optimized for large datasets)
merge_rules() {
    create_temp_dir
    
    local all_new_rules="$TEMP_DIR/all_new_rules.txt"
    local existing_rules="$TEMP_DIR/existing_rules.txt"
    local final_output="$TEMP_DIR/final_output.txt"
    local sources_list_file="$TEMP_DIR/sources_list.txt"
    
    touch "$all_new_rules" "$sources_list_file"
    
    # Infer ruleset name
    [ -z "$RULESET_NAME" ] && RULESET_NAME=$(basename "$OUTPUT_FILE" .list | sed 's/_merged$//' | sed 's/_final$//')
    
    # Auto-detect manual rules file
    local auto_manual_file="${RULESET_NAME}_manual.txt"
    if [ -f "$auto_manual_file" ]; then
        MANUAL_FILE="$auto_manual_file"
    elif [ -f "$SCRIPT_DIR/$auto_manual_file" ]; then
        MANUAL_FILE="$SCRIPT_DIR/$auto_manual_file"
    fi
    
    # Extract existing rules
    print_info "Reading target: $TARGET_FILE"
    extract_rules "$TARGET_FILE" "$existing_rules"
    TOTAL_RULES_BEFORE=$(wc -l < "$existing_rules" | tr -d ' ')
    print_info "Existing rules: $TOTAL_RULES_BEFORE"
    
    # 🔥 提取sources文件头部的策略信息
    local expected_policy=""
    if [ -n "$URL_LIST_FILE" ] && [ -f "$URL_LIST_FILE" ]; then
        # 从头部注释提取策略信息
        while IFS= read -r line; do
            if [[ "$line" =~ ^#.*Policy\ Group:.*\(([A-Z]+)\) ]]; then
                expected_policy="${BASH_REMATCH[1]}"
                print_info "Expected policy for this ruleset: $expected_policy"
                break
            fi
            # 只读取前20行
            [ $((++line_count)) -gt 20 ] && break
        done < "$URL_LIST_FILE"
    fi
    
    # Read URL list file
    if [ -n "$URL_LIST_FILE" ]; then
        print_info "Reading URL list: $URL_LIST_FILE"
        while IFS= read -r line || [ -n "$line" ]; do
            line=$(echo "$line" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
            [ -z "$line" ] || [[ "$line" =~ ^# ]] && continue
            SOURCES+=("$line")
        done < "$URL_LIST_FILE"
    fi
    
    # Download and process rulesets (remote URLs and local files)
    local source_count=0
    local sources_dir=$(dirname "$URL_LIST_FILE")
    
    for source_line in "${SOURCES[@]}"; do
        source_count=$((source_count + 1))
        
        # Parse source and policy (format: SOURCE|POLICY)
        local source_path policy
        if [[ "$source_line" == *"|"* ]]; then
            source_path="${source_line%|*}"
            policy="${source_line##*|}"
        else
            source_path="$source_line"
            policy="REJECT"
        fi
        
        local temp_download="$TEMP_DIR/download_${source_count}.txt"
        local temp_extracted="$TEMP_DIR/extracted_${source_count}.txt"
        
        # Check if it's a local file (starts with ./ or / or doesn't start with http)
        local is_local=false
        local local_file=""
        
        if [[ "$source_path" == ./* ]]; then
            # Relative path from sources directory
            local_file="$sources_dir/${source_path#./}"
            is_local=true
        elif [[ "$source_path" == /* ]]; then
            # Absolute path
            local_file="$source_path"
            is_local=true
        elif [[ ! "$source_path" =~ ^https?:// ]]; then
            # Assume relative path if not URL
            local_file="$sources_dir/$source_path"
            is_local=true
        fi
        
        if $is_local; then
            # Process local file
            if [[ -f "$local_file" ]]; then
                print_info "Processing local [$source_count/${#SOURCES[@]}]: $(basename "$local_file") [$policy]"
                extract_rules "$local_file" "$temp_extracted"
                local rules_count=$(wc -l < "$temp_extracted" | tr -d ' ')
                print_verbose "  Extracted: $rules_count rules"
                # 不再添加策略标记，规则集本身不包含策略
                cat "$temp_extracted" >> "$all_new_rules"
                echo "#   - LOCAL: $(basename "$local_file") ($rules_count rules)" >> "$sources_list_file"
            else
                print_warning "Local file not found: $local_file"
                echo "#   - LOCAL: $source_path (NOT FOUND)" >> "$sources_list_file"
            fi
            continue
        fi
        
        # Process remote URL
        print_info "Processing [$source_count/${#SOURCES[@]}]: $(basename "$source_path")"
        
        if download_ruleset "$source_path" "$temp_download"; then
            extract_rules "$temp_download" "$temp_extracted"
            local rules_count=$(wc -l < "$temp_extracted" | tr -d ' ')
            print_verbose "  Extracted: $rules_count rules"
            
            # 不再添加策略标记，规则集本身不包含策略
            cat "$temp_extracted" >> "$all_new_rules"
            
            echo "#   - $(basename "$source_path") ($rules_count rules)" >> "$sources_list_file"
        else
            print_warning "Download failed: $source_path"
            echo "#   - $(basename "$source_path") (FAILED)" >> "$sources_list_file"
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
            echo "#   - [LOCAL] $(basename "$local_file")" >> "$sources_list_file"
        fi
    done
    
    # Process manual rules file
    if [ -n "$MANUAL_FILE" ] && [ -f "$MANUAL_FILE" ]; then
        local temp_manual="$TEMP_DIR/manual_extracted.txt"
        print_info "Processing manual rules: $(basename "$MANUAL_FILE")"
        extract_rules "$MANUAL_FILE" "$temp_manual"
        cat "$temp_manual" >> "$all_new_rules"
        echo "#   - [MANUAL] $(basename "$MANUAL_FILE") (auto-preserved)" >> "$sources_list_file"
    fi
    
    print_info "Deduplicating rules (this may take a moment)..."
    
    # 简化的去重逻辑 - 不再按策略分组
    local all_rules="$TEMP_DIR/all_rules.txt"
    
    # 合并新规则和现有规则
    cat "$all_new_rules" "$existing_rules" > "$all_rules"
    
    # 去重并排序
    sort -u "$all_rules" -o "$all_rules"
    
    # Calculate statistics
    TOTAL_RULES_AFTER=$(wc -l < "$all_rules" | tr -d ' ')
    TOTAL_RULES_ADDED=$((TOTAL_RULES_AFTER - TOTAL_RULES_BEFORE))
    [ $TOTAL_RULES_ADDED -lt 0 ] && TOTAL_RULES_ADDED=0
    
    # Generate output
    local sources_list=$(cat "$sources_list_file")
    generate_header "$RULESET_NAME" "$TOTAL_RULES_AFTER" "$sources_list" > "$final_output"
    
    # Add policy information
    cat >> "$final_output" << EOF
# 策略说明:
#   - 规则集本身不包含策略标记
#   - 策略由配置文件中的RULE-SET行决定
EOF
    
    # 🔥 显示预期策略（如果有）
    if [ -n "$expected_policy" ]; then
        cat >> "$final_output" << EOF
#   - ⚠️ 本规则集预期策略: $expected_policy
#   - 配置示例: RULE-SET,$RULESET_NAME.list,$expected_policy
EOF
    else
        cat >> "$final_output" << EOF
#   - 例如: RULE-SET,LAN.list,DIRECT 或 RULE-SET,AdBlock.list,REJECT
EOF
    fi
    
    cat >> "$final_output" << EOF
#
# ═══════════════════════════════════════════════════════════════

EOF
    
    # 🔥 策略混入检测 - 检测真正的IP-CIDR规则混入
    if [ -n "$expected_policy" ]; then
        print_info "Checking for policy conflicts..."
        
        local has_conflict=false
        local conflict_msg=""
        local conflict_rules=""
        
        # DIRECT规则集不应该包含广告域名关键词
        if [ "$expected_policy" = "DIRECT" ]; then
            conflict_rules=$(grep -E "DOMAIN.*(doubleclick|googleads|adservice\.google|ad\.doubleclick)" "$all_rules" 2>/dev/null || true)
            if [ -n "$conflict_rules" ]; then
                has_conflict=true
                conflict_msg="⚠️ CRITICAL: DIRECT ruleset contains ad-related domains!"
                print_error "$conflict_msg"
                print_error "Conflicting rules:"
                echo "$conflict_rules" | head -5 | while read line; do print_error "  $line"; done
            fi
        fi
        
        # REJECT规则集不应该包含私有IP段（IP-CIDR规则）
        if [ "$expected_policy" = "REJECT" ]; then
            conflict_rules=$(grep -E "^IP-CIDR,(192\.168\.|10\.|172\.(1[6-9]|2[0-9]|3[01])\.|127\.)" "$all_rules" 2>/dev/null || true)
            if [ -n "$conflict_rules" ]; then
                has_conflict=true
                conflict_msg="⚠️ CRITICAL: REJECT ruleset contains private IP ranges!"
                print_error "$conflict_msg"
                print_error "Conflicting rules:"
                echo "$conflict_rules" | head -5 | while read line; do print_error "  $line"; done
            fi
        fi
        
        # LAN规则集不应该包含广告域名
        if [[ "$RULESET_NAME" =~ ^LAN ]]; then
            conflict_rules=$(grep -E "DOMAIN.*(doubleclick|googleads|adservice\.google)" "$all_rules" 2>/dev/null || true)
            if [ -n "$conflict_rules" ]; then
                has_conflict=true
                conflict_msg="⚠️ CRITICAL: LAN ruleset contains ad-related domains!"
                print_error "$conflict_msg"
                print_error "Conflicting rules:"
                echo "$conflict_rules" | head -5 | while read line; do print_error "  $line"; done
            fi
        fi
        
        if [ "$has_conflict" = true ]; then
            echo "# ⚠️ POLICY CONFLICT DETECTED: $conflict_msg" >> "$final_output"
            echo "#    Please check source files for policy confusion!" >> "$final_output"
            echo "#" >> "$final_output"
            print_error "Policy conflict detected! Check output file for details."
            print_error "This indicates source files may have incorrect policy classification."
        fi
    fi
    
    # Output all rules
    output_rules_by_type "$all_rules" "" "$final_output"
    
    echo "# ========== END ==========" >> "$final_output"

    # Statistics - 显示文件名
    echo ""
    echo "╔══════════════════════════════════════════════════════════════╗"
    printf "║  📁 %-55s ║\n" "$RULESET_NAME"
    echo "╠══════════════════════════════════════════════════════════════╣"
    printf "║  Before:      %-45s ║\n" "$TOTAL_RULES_BEFORE"
    printf "║  Added:       %-45s ║\n" "$TOTAL_RULES_ADDED"
    printf "║  After:       %-45s ║\n" "$TOTAL_RULES_AFTER"
    echo "╚══════════════════════════════════════════════════════════════╝"
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
    echo "║              Version 2.4                 ║"
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
