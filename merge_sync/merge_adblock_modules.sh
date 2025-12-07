#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
# 广告拦截模块智能合并脚本 v3.4 (Pure Rules Only)
# ═══════════════════════════════════════════════════════════════════════════════

set -e

# PATHS
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SURGE_MODULE_DIR="$PROJECT_ROOT/module/surge(main)"
SHADOWROCKET_MODULE_DIR="/Users/YOUR_USERNAME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules"
TEMP_DIR="$PROJECT_ROOT/.temp_adblock_merge"
TARGET_MODULE="$SURGE_MODULE_DIR/🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"
ADBLOCK_MERGED_LIST="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock_Merged.list"

# TEMPORARY FILES
TEMP_RULES_REJECT="$TEMP_DIR/rules_reject.tmp"
TEMP_RULES_REJECT_DROP="$TEMP_DIR/rules_reject_drop.tmp"
TEMP_RULES_REJECT_NO_DROP="$TEMP_DIR/rules_reject_no_drop.tmp"
TEMP_RULES_DIRECT="$TEMP_DIR/rules_direct.tmp"

# FLAGS
interactive=false; list_only=false; auto_delete=false; auto_mode=false; specified_modules=()

# UTILS
log_info() { echo -e "\033[0;36m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[✓]\033[0m $1"; }
log_warning() { echo -e "\033[1;33m[⚠]\033[0m $1"; }
log_error() { echo -e "\033[0;31m[✗]\033[0m $1"; }

# INIT
rm -rf "$TEMP_DIR"; mkdir -p "$TEMP_DIR"
touch "$TEMP_RULES_REJECT" "$TEMP_RULES_REJECT_DROP" "$TEMP_RULES_REJECT_NO_DROP" "$TEMP_RULES_DIRECT"

extract_rules_surge_format() {
    local file="$1"; local new=0
    
    # 1. Rules Only (Strict Policy Filter: REJECT*, DIRECT)
    awk '/^\[Rule\]/{f=1;next}/^\[/{f=0}f' "$file" > "$TEMP_DIR/raw_rules.tmp" || true
    while read -r line; do
        [[ -z "$line" || "$line" =~ ^# ]] && continue
        # 🔥 跳过 RULE-SET 行（防止自引用循环）
        [[ "$line" =~ ^RULE-SET ]] && continue
        rule=$(echo "$line" | sed 's/  */ /g')
        if echo "$rule" | grep -q ",REJECT-DROP"; then
             grep -Fxq "$rule" "$TEMP_RULES_REJECT_DROP" || { echo "$rule" >> "$TEMP_RULES_REJECT_DROP"; ((new++)) || true; }
        elif echo "$rule" | grep -q ",REJECT-NO-DROP"; then
             grep -Fxq "$rule" "$TEMP_RULES_REJECT_NO_DROP" || { echo "$rule" >> "$TEMP_RULES_REJECT_NO_DROP"; ((new++)) || true; }
        elif echo "$rule" | grep -q ",REJECT"; then
             grep -Fxq "$rule" "$TEMP_RULES_REJECT" || { echo "$rule" >> "$TEMP_RULES_REJECT"; ((new++)) || true; }
        elif echo "$rule" | grep -q ",DIRECT"; then
             grep -Fxq "$rule" "$TEMP_RULES_DIRECT" || { echo "$rule" >> "$TEMP_RULES_DIRECT"; ((new++)) || true; }
        fi
    done < "$TEMP_DIR/raw_rules.tmp"
    
    echo $new
}

# SCANNER
scan_files() {
    local dir="$1"; local ext="$2"; 
    [[ ! -d "$dir" ]] && return
    find "$dir" -name "$ext" -maxdepth 1 -print0 | while IFS= read -r -d '' module; do
        [[ "$module" == "$TARGET_MODULE" ]] && continue
        if grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then
             log_info "Processing: $(basename "$module")"
             extract_rules_surge_format "$module" >/dev/null
        fi
    done
}

# GENERATOR
generate_module() {
    log_info "生成模块..."

    cat > "$TARGET_MODULE" << EOF
#!name=🚫 Universal Ad-Blocking Rules (Lite)
#!desc=Auto-merged: REJECT/DROP/DIRECT Rules Only.
#!category=『 🔝 Head Expanse › 首端扩域 』

[Rule]
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/AdBlock_Merged.list,REJECT,extended-matching,pre-matching,"update-interval=86400",no-resolve
RULE-SET,https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf,REJECT-NO-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve
RULE-SET,https://ruleset.skk.moe/List/non_ip/reject-drop.conf,REJECT-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve
RULE-SET,https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/BlockHttpDNS/BlockHttpDNS.list,REJECT-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve

EOF
    
    [[ -s "$TEMP_RULES_REJECT_DROP" ]] && echo "# DROP Rules" >> "$TARGET_MODULE" && sort -u "$TEMP_RULES_REJECT_DROP" >> "$TARGET_MODULE"
    # No Scripts, No Rewrite, No Host, No Map Local
}

# EXPORTERS
export_drop() {
    sort -u "$TEMP_RULES_REJECT_DROP" > "$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/reject-drop.conf" || true
    sort -u "$TEMP_RULES_REJECT_NO_DROP" > "$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/reject-no-drop.conf" || true
}
export_direct() {
    mkdir -p "$PROJECT_ROOT/ruleset/Sources/conf"
    if [[ -s "$TEMP_RULES_DIRECT" ]]; then sort -u "$TEMP_RULES_DIRECT" > "$PROJECT_ROOT/ruleset/Sources/conf/SurgeConf_ModulesDirect.list"; fi
}
export_merge_list() {
    if [[ -f "$ADBLOCK_MERGED_LIST" ]]; then
       # 🔥 过滤掉非法行：RULE-SET、DOMAIN带no-resolve、空行
       grep -v "#" "$ADBLOCK_MERGED_LIST" | grep -v "^$" | grep -v "^RULE-SET" > "$TEMP_DIR/old.tmp" || touch "$TEMP_DIR/old.tmp"
    else
       touch "$TEMP_DIR/old.tmp"
    fi
    sort -u "$TEMP_RULES_REJECT" > "$TEMP_DIR/new.tmp"
    cat "$TEMP_DIR/old.tmp" "$TEMP_DIR/new.tmp" | sort -u > "$ADBLOCK_MERGED_LIST.tmp"
    
    # 🔥 清理非法规则：DOMAIN/DOMAIN-SUFFIX/DOMAIN-KEYWORD 不能带 no-resolve
    # no-resolve 只能用于 IP-CIDR/IP-CIDR6/GEOIP 规则
    sed -i '' 's/^\(DOMAIN[^,]*,[^,]*\),no-resolve$/\1/' "$ADBLOCK_MERGED_LIST.tmp" 2>/dev/null || \
    sed -i 's/^\(DOMAIN[^,]*,[^,]*\),no-resolve$/\1/' "$ADBLOCK_MERGED_LIST.tmp"
    
    # 🔥 删除 RULE-SET 行（防止自引用）
    grep -v "^RULE-SET" "$ADBLOCK_MERGED_LIST.tmp" > "$ADBLOCK_MERGED_LIST.tmp2" && mv "$ADBLOCK_MERGED_LIST.tmp2" "$ADBLOCK_MERGED_LIST.tmp"
    
    mv "$ADBLOCK_MERGED_LIST.tmp" "$ADBLOCK_MERGED_LIST"
    
    local count=$(wc -l < "$ADBLOCK_MERGED_LIST" | tr -d ' ')
    local header="# Ruleset: AdBlock_Merged\n# Updated: $(date)\n# Total Rules: $count\n# ═══════════════════════════════════════════════════════════════\n"
    echo -e "$header" | cat - "$ADBLOCK_MERGED_LIST" > "$ADBLOCK_MERGED_LIST.tmp" && mv "$ADBLOCK_MERGED_LIST.tmp" "$ADBLOCK_MERGED_LIST"
}

# MAIN
if [[ -f "$TARGET_MODULE" ]]; then extract_rules_surge_format "$TARGET_MODULE" >/dev/null; fi 
scan_files "$SURGE_MODULE_DIR" "*.sgmodule"
scan_files "$SHADOWROCKET_MODULE_DIR" "*.module"

generate_module
export_drop; export_direct; export_merge_list

# Sync Singbox
if [[ -f "$SCRIPT_DIR/batch_convert_to_singbox.sh" ]]; then bash "$SCRIPT_DIR/batch_convert_to_singbox.sh" || true; fi

rm -rf "$TEMP_DIR"
log_success "AdBlock Merged (Rules Only)."
