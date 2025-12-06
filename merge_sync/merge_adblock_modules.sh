#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# 广告拦截模块智能合并脚本 v3.2 (Ad-Blocking Module Intelligent Merger)
# ═══════════════════════════════════════════════════════════════════════════════
# 功能：
# 1. 从多个代理软件提取广告拦截规则
# 2. 智能识别不同格式的规则
# 3. 智能分类：REJECT、REJECT-DROP、REJECT-NO-DROP
# 4. 增量合并，自动去重
# 5. URL Rewrite 规则单独处理
# 6. Host 规则单独处理
# 7. 支持命令行参数和交互式模式
# 8. 自动同步到小火箭模块
# 9. 自动导出模块中的直连规则到 SurgeConf_ModulesDirect.list
# 10. 自动导出 REJECT-DROP/NO-DROP 规则到独立文件 (防止配置报错)
# ═══════════════════════════════════════════════════════════════════════════════

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# 配置路径
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SURGE_MODULE_DIR="$PROJECT_ROOT/module/surge(main)"

# Shadowrocket Module Directory (Optional)
SHADOWROCKET_MODULE_DIR="/Users/YOUR_USERNAME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules"

TEMP_DIR="$PROJECT_ROOT/.temp_adblock_merge"

# 目标模块
TARGET_MODULE="$SURGE_MODULE_DIR/🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"

# 巨大的合并规则文件
ADBLOCK_MERGED_LIST="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock_Merged.list"

# 临时文件
TEMP_RULES_REJECT="$TEMP_DIR/rules_reject.tmp"
TEMP_RULES_REJECT_DROP="$TEMP_DIR/rules_reject_drop.tmp"
TEMP_RULES_REJECT_NO_DROP="$TEMP_DIR/rules_reject_no_drop.tmp"
TEMP_RULES_DIRECT="$TEMP_DIR/rules_direct.tmp"
TEMP_URL_REWRITE="$TEMP_DIR/url_rewrite.tmp"
TEMP_HOST="$TEMP_DIR/host.tmp"
TEMP_MITM="$TEMP_DIR/mitm.tmp"

# 命令行参数
INTERACTIVE_MODE=false
LIST_ONLY_MODE=false
AUTO_DELETE=false
AUTO_MODE=false
SPECIFIED_MODULES=()

# 统计信息
TOTAL_NEW_RULES=0
TOTAL_NEW_DIRECT=0
PROCESSED_MODULES=0
MODULES_TO_DELETE=()

# 日志函数
log_info() { echo -e "${CYAN}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }
log_section() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
}

# 解析命令行参数
parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --interactive|-i) INTERACTIVE_MODE=true; shift ;;
            --list-only|-l) LIST_ONLY_MODE=true; shift ;;
            --auto-delete|-d) AUTO_DELETE=true; shift ;;
            --auto|-a) AUTO_MODE=true; shift ;;
            --help|-h) show_help; exit 0 ;;
            --no-backup) shift ;;
            *)
                if [[ -f "$1" ]]; then SPECIFIED_MODULES+=("$1"); else log_error "文件不存在: $1"; exit 1; fi
                shift ;;
        esac
    done
}

show_help() { echo "使用方法: $0 [选项] [模块文件...]"; }

detect_module_format() {
    local file="$1"
    local format="unknown"
    case "${file##*.}" in
        sgmodule) format="surge" ;;
        module) format="shadowrocket" ;;
        yaml|yml) format="clash" ;;
        plugin) format="loon" ;;
        conf)
            if grep -q "^\[Rule\]" "$file" 2>/dev/null; then format="surge"
            elif grep -q "^rules:" "$file" 2>/dev/null; then format="clash"
            elif grep -q "^hostname =" "$file" 2>/dev/null; then format="quantumult_x"
            fi ;;
        snippet) format="quantumult_x" ;;
    esac
    echo "$format"
}

# Extraction Functions (Condensed)
extract_rules_from_clash() {
    local file="$1"; local new_rules=0
    log_info "处理 Clash: $(basename "$file")"
    awk '/^rules:/{flag=1;next}/^[a-zA-Z-]+:/{flag=0}flag && /^[[:space:]]*- .*REJECT/{print}' "$file" | while read -r line; do
        rule=$(echo "$line" | sed 's/^[[:space:]]*- //; s/  */ /g')
        if echo "$rule" | grep -q ",REJECT$"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT"; then echo "$rule" >> "$TEMP_RULES_REJECT"; ((new_rules++)); fi
        fi
    done
    [[ $new_rules -gt 0 ]] && log_success "提取 $new_rules 条"; echo $new_rules
}

extract_rules_from_quantumult_x() {
    local file="$1"; local new_rules=0
    log_info "处理 QX: $(basename "$file")"
    grep "reject" "$file" | grep -v "^#" | grep -v "^;" | while read -r line; do
        local rule=$(echo "$line" | sed 's/host-suffix/DOMAIN-SUFFIX/g; s/host/DOMAIN/g; s/ip-cidr/IP-CIDR/g; s/user-agent/USER-AGENT/g; s/reject/REJECT/g; s/, /,/g; s/ ,/,/g')
        if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT"; then echo "$rule" >> "$TEMP_RULES_REJECT"; ((new_rules++)); fi
    done
    [[ $new_rules -gt 0 ]] && log_success "提取 $new_rules 条"; echo $new_rules
}

extract_rules_from_loon() {
    local file="$1"; local new_rules=0
    log_info "处理 Loon: $(basename "$file")"
    awk '/^\[Rule\]/{flag=1;next}/^\[/{flag=0}flag && /REJECT/{print}' "$file" | while read -r line; do
        if [[ ! "$line" =~ ^# ]] && [[ ! -z "$line" ]]; then
            local rule=$(echo "$line" | sed 's/  */ /g')
            if echo "$rule" | grep -q ",REJECT,"; then
                if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT"; then echo "$rule" >> "$TEMP_RULES_REJECT"; ((new_rules++)); fi
            fi
        fi
    done
    [[ $new_rules -gt 0 ]] && log_success "提取 $new_rules 条"; echo $new_rules
}

create_temp_dir() { rm -rf "$TEMP_DIR"; mkdir -p "$TEMP_DIR"; }
cleanup_temp_dir() { rm -rf "$TEMP_DIR"; }

extract_existing_rules() {
    log_section "提取现有规则"
    if [[ ! -f "$TARGET_MODULE" ]]; then log_error "目标模块不存在"; exit 1; fi
    
    # Extract Rule Section
    awk '/^\[Rule\]/{flag=1;next}/^\[/{flag=0}flag && /^(DOMAIN|IP-CIDR|USER-AGENT|URL-REGEX)/{print}' "$TARGET_MODULE" > "$TEMP_DIR/existing_rules.tmp"
    
    grep ",REJECT," "$TEMP_DIR/existing_rules.tmp" 2>/dev/null | grep -v "REJECT-DROP" | grep -v "REJECT-NO-DROP" > "$TEMP_RULES_REJECT" || touch "$TEMP_RULES_REJECT"
    grep ",REJECT-DROP," "$TEMP_DIR/existing_rules.tmp" 2>/dev/null > "$TEMP_RULES_REJECT_DROP" || touch "$TEMP_RULES_REJECT_DROP"
    grep ",REJECT-NO-DROP," "$TEMP_DIR/existing_rules.tmp" 2>/dev/null > "$TEMP_RULES_REJECT_NO_DROP" || touch "$TEMP_RULES_REJECT_NO_DROP"
    grep ",DIRECT" "$TEMP_DIR/existing_rules.tmp" 2>/dev/null > "$TEMP_RULES_DIRECT" || touch "$TEMP_RULES_DIRECT"
    
    # URL Rewrite
    awk '/^\[URL Rewrite\]/{flag=1;next}/^\[/{flag=0}flag && /reject/{print}' "$TARGET_MODULE" > "$TEMP_URL_REWRITE" || touch "$TEMP_URL_REWRITE"
    
    # Host
    awk '/^\[Host\]/{flag=1;next}/^\[/{flag=0}flag && /= 0.0.0.0/{print}' "$TARGET_MODULE" > "$TEMP_HOST" || touch "$TEMP_HOST"
    
    # MITM
    awk '/^\[MITM\]/{flag=1;next}/^\[/{flag=0}flag && /^hostname/{print}' "$TARGET_MODULE" > "$TEMP_MITM" || touch "$TEMP_MITM"
    
    log_success "现有规则统计: REJECT=$(wc -l < "$TEMP_RULES_REJECT"), DROP=$(wc -l < "$TEMP_RULES_REJECT_DROP")"
}

extract_rules_from_module() {
    local module_file="$1"; local module_name=$(basename "$module_file")
    log_info "处理模块: $module_name"
    if [[ ! -f "$module_file" ]]; then log_warning "文件不存在"; return; fi
    local format=$(detect_module_format "$module_file")
    local new_rules=0
    case "$format" in
        clash) new_rules=$(extract_rules_from_clash "$module_file" | tail -1) || new_rules=0 ;;
        quantumult_x) new_rules=$(extract_rules_from_quantumult_x "$module_file" | tail -1) || new_rules=0 ;;
        loon) new_rules=$(extract_rules_from_loon "$module_file" | tail -1) || new_rules=0 ;;
        surge|shadowrocket|unknown) new_rules=$(extract_rules_surge_format "$module_file" | tail -1) || new_rules=0 ;;
    esac
    [[ "$new_rules" =~ ^[0-9]+$ ]] || new_rules=0
    ((TOTAL_NEW_RULES += new_rules)) || true
    ((PROCESSED_MODULES++)) || true
}

extract_rules_surge_format() {
    local module_file="$1"; local new_rules=0
    > "$TEMP_DIR/new_rules.tmp"
    awk '/^\[Rule\]/{flag=1;next}/^\[/{flag=0}flag && /^(DOMAIN|IP-CIDR|USER-AGENT|URL-REGEX|DEST-PORT|SRC-PORT|IP-ASN|GEOIP|PROCESS-NAME)/{print}' "$module_file" >> "$TEMP_DIR/new_rules.tmp" || true
    while read -r rule; do
        if [[ -z "$rule" ]] || [[ "$rule" =~ ^# ]]; then continue; fi
        rule=$(echo "$rule" | sed 's/  */ /g')
        if echo "$rule" | grep -q ",REJECT-DROP"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT_DROP"; then echo "$rule" >> "$TEMP_RULES_REJECT_DROP"; ((new_rules++)); fi
        elif echo "$rule" | grep -q ",REJECT-NO-DROP"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT_NO_DROP"; then echo "$rule" >> "$TEMP_RULES_REJECT_NO_DROP"; ((new_rules++)); fi
        elif echo "$rule" | grep -q ",REJECT"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT"; then echo "$rule" >> "$TEMP_RULES_REJECT"; ((new_rules++)); fi
        elif echo "$rule" | grep -q ",DIRECT"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_DIRECT"; then echo "$rule" >> "$TEMP_RULES_DIRECT"; ((new_rules++)); ((TOTAL_NEW_DIRECT++)); fi
        fi
    done < "$TEMP_DIR/new_rules.tmp"
    
    # URL Rewrite, Host, MITM (Simplified grep)
    grep -A 1000 "^\[URL Rewrite\]" "$module_file" 2>/dev/null | grep -B 1000 "^\[" | grep "reject" | while read -r rule; do
         rule=$(echo "$rule" | sed 's/  */ /g')
         if ! grep -Fxq "$rule" "$TEMP_URL_REWRITE"; then echo "$rule" >> "$TEMP_URL_REWRITE"; ((new_rules++)); fi
    done
    grep -A 1000 "^\[Host\]" "$module_file" 2>/dev/null | grep -B 1000 "^\[" | grep "= 0.0.0.0" | while read -r rule; do
         rule=$(echo "$rule" | sed 's/  */ /g')
         if ! grep -Fxq "$rule" "$TEMP_HOST"; then echo "$rule" >> "$TEMP_HOST"; ((new_rules++)); fi
    done
    grep -A 100 "^\[MITM\]" "$module_file" 2>/dev/null | grep "^hostname" > "$TEMP_DIR/new_mitm.tmp"
    if [[ -s "$TEMP_DIR/new_mitm.tmp" ]]; then
       cat "$TEMP_MITM" >> "$TEMP_DIR/new_mitm.tmp"
       echo "hostname = %APPEND% $(grep "hostname" "$TEMP_DIR/new_mitm.tmp" | sed 's/.*= *//g; s/%APPEND%//g' | tr ',' '\n' | sed 's/ //g' | sort -u | tr '\n' ',' | sed 's/,$//; s/,/, /g')" > "$TEMP_MITM"
    fi
    echo $new_rules
}

scan_and_merge_modules() {
    log_section "扫描并合并模块"
    # (Same scanning logic as before, condensed)
    if [[ -d "$SURGE_MODULE_DIR" ]]; then
        for module in "$SURGE_MODULE_DIR"/*.sgmodule "$SURGE_MODULE_DIR"/*.conf; do
            if [[ -f "$module" ]] && [[ "$module" != "$TARGET_MODULE" ]]; then
                 if grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then extract_rules_from_module "$module"; fi
            fi
        done
    fi
    local INGESTED_list="$PROJECT_ROOT/ruleset/Sources/conf/SurgeConf_AdBlock.list"
    if [[ -f "$INGESTED_list" ]]; then
        log_info "处理本地吸纳规则"
        while read -r rule; do
             if [[ "$rule" =~ ^(DOMAIN|IP-CIDR|USER-AGENT|URL-REGEX|DEST-PORT|SRC-PORT|IP-ASN|GEOIP|PROCESS-NAME) ]]; then
                clean_rule=$(echo "$rule" | sed 's/  */ /g')
                if echo "$clean_rule" | grep -q ",REJECT-DROP"; then
                    if ! grep -Fxq "$clean_rule" "$TEMP_RULES_REJECT_DROP"; then echo "$clean_rule" >> "$TEMP_RULES_REJECT_DROP"; fi
                elif echo "$clean_rule" | grep -q ",REJECT-NO-DROP"; then
                     if ! grep -Fxq "$clean_rule" "$TEMP_RULES_REJECT_NO_DROP"; then echo "$clean_rule" >> "$TEMP_RULES_REJECT_NO_DROP"; fi
                elif echo "$clean_rule" | grep -q ",REJECT"; then
                     if ! grep -Fxq "$clean_rule" "$TEMP_RULES_REJECT"; then echo "$clean_rule" >> "$TEMP_RULES_REJECT"; fi
                fi
             fi
        done < "$INGESTED_list"
    fi
}

generate_new_module() {
    log_section "生成新模块文件"
    local reject_drop_count=$(wc -l < "$TEMP_RULES_REJECT_DROP" | tr -d ' ')
    cat > "$TARGET_MODULE" << EOF
#!name=🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style)
#!version=$(date +%Y.%m.%d)
#!desc=Modular ad-blocking with Host sinkhole + Online rulesets. Low-memory optimized. 🧩💾⚡
#!category=『 🔝 Head Expanse › 首端扩域 』

[Rule]
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/AdBlock_Merged.list,REJECT,extended-matching,pre-matching,"update-interval=86400",no-resolve
RULE-SET,https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf,REJECT-NO-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve
RULE-SET,https://ruleset.skk.moe/List/non_ip/reject-drop.conf,REJECT-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve
RULE-SET,https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/BlockHttpDNS/BlockHttpDNS.list,REJECT-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve

EOF
    # Append Drop Rules if needed (optional inline, but we also export them now)
    if [[ $reject_drop_count -gt 0 ]]; then
        echo "# REJECT-DROP Rules (Inline Backup)" >> "$TARGET_MODULE"
        sort -u "$TEMP_RULES_REJECT_DROP" >> "$TARGET_MODULE"
    fi
    # Append Rewrite/Host/MITM as before...
     if [[ -s "$TEMP_URL_REWRITE" ]]; then echo "[URL Rewrite]" >> "$TARGET_MODULE"; sort -u "$TEMP_URL_REWRITE" >> "$TARGET_MODULE"; fi
     if [[ -s "$TEMP_HOST" ]]; then echo "[Host]" >> "$TARGET_MODULE"; sort -u "$TEMP_HOST" >> "$TARGET_MODULE"; fi
     if [[ -s "$TEMP_MITM" ]]; then echo "[MITM]" >> "$TARGET_MODULE"; cat "$TEMP_MITM" >> "$TARGET_MODULE"; fi
    log_success "新模块文件已生成"
}

export_drop_rules() {
    log_section "导出 DROP/NO-DROP 规则"
    local DROP_FILE="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/reject-drop.conf"
    local NO_DROP_FILE="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/reject-no-drop.conf"
    
    sort -u "$TEMP_RULES_REJECT_DROP" > "$DROP_FILE"
    log_success "已导出 REJECT-DROP 规则到: $(basename "$DROP_FILE")"
    
    sort -u "$TEMP_RULES_REJECT_NO_DROP" > "$NO_DROP_FILE"
    log_success "已导出 REJECT-NO-DROP 规则到: $(basename "$NO_DROP_FILE")"
}

merge_to_direct_list() {
    if [[ -s "$TEMP_RULES_DIRECT" ]]; then
        local MODULES_DIRECT_LIST="$PROJECT_ROOT/ruleset/Sources/conf/SurgeConf_ModulesDirect.list"
        mkdir -p "$(dirname "$MODULES_DIRECT_LIST")"
        sort -u "$TEMP_RULES_DIRECT" > "$MODULES_DIRECT_LIST"
        log_success "已导出 $(wc -l < "$MODULES_DIRECT_LIST" | tr -d ' ') 条DIRECT规则到: SurgeConf_ModulesDirect.list"
    fi
}

merge_to_adblock_list() {
    log_section "合并规则到 AdBlock_Merged.list"
    cp "$ADBLOCK_MERGED_LIST" "$ADBLOCK_MERGED_LIST.backup"
    grep -v "^#" "$ADBLOCK_MERGED_LIST" | grep -v "^$" > "$TEMP_DIR/existing.tmp"
    sort -u "$TEMP_RULES_REJECT" > "$TEMP_DIR/new.tmp"
    cat > "$ADBLOCK_MERGED_LIST" << EOF
# ═══════════════════════════════════════════════════════════════
# Ruleset: AdBlock_Merged
# Updated: $(date +"%Y-%m-%d %H:%M:%S UTC")
# Total Rules: $(cat "$TEMP_DIR/existing.tmp" "$TEMP_DIR/new.tmp" | sort -u | wc -l | tr -d ' ')
# Generator: Ruleset Merger v3.2
# ═══════════════════════════════════════════════════════════════

EOF
    cat "$TEMP_DIR/existing.tmp" "$TEMP_DIR/new.tmp" | sort -u >> "$ADBLOCK_MERGED_LIST"
    log_success "AdBlock_Merged.list 更新完毕"
}

# Dummy interactive for now
interactive_select_modules() { log_info "Interactive mode"; }

main() {
    parse_arguments "$@"
    log_section "广告拦截模块智能合并 v3.2"
    create_temp_dir
    extract_existing_rules
    scan_and_merge_modules
    generate_new_module
    export_drop_rules  # NEW
    merge_to_adblock_list
    merge_to_direct_list
    if [[ -f "$SCRIPT_DIR/batch_convert_to_singbox.sh" ]]; then bash "$SCRIPT_DIR/batch_convert_to_singbox.sh"; fi
    if [[ "$AUTO_DELETE" == true ]]; then for m in "${SPECIFIED_MODULES[@]}"; do rm -f "$m"; done; fi
    cleanup_temp_dir
    log_success "完成"
}

main "$@"
