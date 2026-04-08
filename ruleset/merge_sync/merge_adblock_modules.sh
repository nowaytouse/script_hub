#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# AdBlock Module Smart Merge Script v5.1 (Consolidated Edition)
# 
# Merges multiple ad-blocking modules into Universal Ad-Blocking Rules:
# - 广告联盟去广告
# - 去除小程序和其他应用广告  
# - AD ByeBye+ 2.x
# - AllInOne
# - [Sukka] Enhance Better ADBlock for Surge
# - [Sukka] URL Rewrite
# - 可莉广告过滤器
# - 广告平台拦截器
#
# Features:
# - Multi-section extraction: [Rule], [URL Rewrite], [Map Local], [Script], [MITM]
# - Rule classification: REJECT, REJECT-DROP, REJECT-NO-DROP
# - Batch deduplication using sort -u
# - Whitelist filtering
# - Hash-based change detection for upstream updates
# - Upstream auto-sync with Sukka ruleset
# ═══════════════════════════════════════════════════════════════════════════════

set -e

# ═══════════════════════════════════════════════════════════════════════════════
# CONFIGURATION
# ═══════════════════════════════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SURGE_MODULE_DIR="$PROJECT_ROOT/module/surge(main)"
HEAD_EXPANSE_DIR="$SURGE_MODULE_DIR/head_expanse"
TEMP_DIR="$PROJECT_ROOT/.temp_adblock_merge"
TARGET_MODULE="$HEAD_EXPANSE_DIR/🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"
ADBLOCK_MERGED_LIST="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock.list"
CACHE_DIR="$PROJECT_ROOT/.cache"
HASH_FILE="$CACHE_DIR/adblock_hashes.txt"
WHITELIST_FILE="$PROJECT_ROOT/ruleset/Sources/adblock_whitelist.txt"
FIREWALL_MODULE="$HEAD_EXPANSE_DIR/🔥 Firewall Port Blocker 🛡️🚫.sgmodule"

# 🔥 Source modules to merge (relative to HEAD_EXPANSE_DIR)
SOURCE_MODULES=(
    "小程序和应用懒人去广告合集.official.sgmodule"
    "All-in-One-2.x.sgmodule"
    "AllInOne_Mock.sgmodule"
    "可莉广告过滤器.beta.sgmodule"
    "广告平台拦截器.sgmodule"
    "Adblock4limbo.sgmodule"
)

# 🔥 Sukka upstream rulesets (auto-update from skk.moe)
SUKKA_RULESETS=(
    "https://ruleset.skk.moe/List/non_ip/reject.conf"
    "https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf"
    "https://ruleset.skk.moe/List/non_ip/reject-drop.conf"
)

# ═══════════════════════════════════════════════════════════════════════════════
# LOGGING UTILITIES
# ═══════════════════════════════════════════════════════════════════════════════

log_info() { echo -e "\033[0;36m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[✓]\033[0m $1"; }
log_warning() { echo -e "\033[1;33m[⚠]\033[0m $1"; }
log_error() { echo -e "\033[0;31m[✗ ERROR]\033[0m $1" >&2; }
log_section() { echo -e "\n\033[1;35m═══ $1 ═══\033[0m"; }

# ═══════════════════════════════════════════════════════════════════════════════
# WHITELIST FUNCTIONS
# ═══════════════════════════════════════════════════════════════════════════════

load_whitelist() {
    if [ -f "$WHITELIST_FILE" ]; then
        grep -v '^#' "$WHITELIST_FILE" 2>/dev/null | grep -v '^$' | \
        sed 's/^DOMAIN-SUFFIX,//' | sed 's/^DOMAIN-KEYWORD,//' | sed 's/^DOMAIN,//' | \
        sort -u > "$TEMP_DIR/whitelist_domains.tmp"
        
        WHITELIST_COUNT=$(wc -l < "$TEMP_DIR/whitelist_domains.tmp" 2>/dev/null | tr -d ' ' || echo "0")
        log_info "Loaded $WHITELIST_COUNT whitelist patterns"
    else
        touch "$TEMP_DIR/whitelist_domains.tmp"
        log_warning "Whitelist file not found: $WHITELIST_FILE"
    fi
}

filter_whitelist() {
    local input_file="$1"
    local output_file="$2"
    
    if [ ! -s "$TEMP_DIR/whitelist_domains.tmp" ]; then
        cp "$input_file" "$output_file"
        return
    fi
    
    local whitelist_pattern=$(cat "$TEMP_DIR/whitelist_domains.tmp" | tr '\n' '|' | sed 's/|$//')
    
    if [ -n "$whitelist_pattern" ]; then
        grep -vE "$whitelist_pattern" "$input_file" > "$output_file" 2>/dev/null || cp "$input_file" "$output_file"
        
        local before=$(wc -l < "$input_file" 2>/dev/null | tr -d ' ' || echo "0")
        local after=$(wc -l < "$output_file" 2>/dev/null | tr -d ' ' || echo "0")
        local filtered=$((before - after))
        
        if [ "$filtered" -gt 0 ]; then
            log_warning "Whitelist filtered: $filtered rules removed"
        fi
    else
        cp "$input_file" "$output_file"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# RULE EXTRACTION FUNCTIONS
# ═══════════════════════════════════════════════════════════════════════════════

extract_section() {
    local file="$1"
    local section="$2"
    local output="$3"
    
    # Extract section content using awk
    awk -v sec="$section" '
        /^\[/ { in_section = ($0 ~ "^\\[" sec "\\]") }
        in_section && !/^\[/ && !/^#/ && !/^$/ { print }
    ' "$file" 2>/dev/null >> "$output" || true
}

extract_rules_from_module() {
    local module_file="$1"
    local module_name=$(basename "$module_file")
    
    if [ ! -f "$module_file" ]; then
        log_warning "Module not found: $module_name"
        return
    fi
    
    log_info "Processing: $module_name"
    
    # Extract [Rule] section and classify
    awk '/^\[Rule\]/{f=1;next}/^\[/{f=0}f' "$module_file" 2>/dev/null | \
    grep -v '^#' | grep -v '^$' | grep -v '^RULE-SET' | \
    sed 's/  */ /g' | while read -r line; do
        case "$line" in
            *,REJECT-DROP*) echo "$line" >> "$ALL_REJECT_DROP" ;;
            *,REJECT-NO-DROP*) echo "$line" >> "$ALL_REJECT_NO_DROP" ;;
            *,REJECT*) echo "$line" >> "$ALL_REJECT" ;;
            *,DIRECT*) echo "$line" >> "$ALL_DIRECT" ;;
        esac
    done
    
    # Extract [URL Rewrite] section
    extract_section "$module_file" "URL Rewrite" "$ALL_URL_REWRITE"
    
    # Extract [Map Local] section
    extract_section "$module_file" "Map Local" "$ALL_MAP_LOCAL"
    
    # Extract [Script] section
    extract_section "$module_file" "Script" "$ALL_SCRIPT"
    
    # Extract [MITM] hostname
    awk '/^\[MITM\]/{f=1;next}/^\[/{f=0}f && /^hostname/' "$module_file" 2>/dev/null | \
    sed 's/hostname = %APPEND% //' | sed 's/hostname = //' | \
    tr ',' '\n' | sed 's/^ *//' | sed 's/ *$//' | \
    grep -v '^$' >> "$ALL_MITM" 2>/dev/null || true
}

# ═══════════════════════════════════════════════════════════════════════════════
# MAIN EXECUTION
# ═══════════════════════════════════════════════════════════════════════════════

log_section "AdBlock Module Consolidation v5.0"

# Initialize
rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR" "$CACHE_DIR"
touch "$HASH_FILE"

# Temp files
ALL_REJECT="$TEMP_DIR/all_reject.tmp"
ALL_REJECT_DROP="$TEMP_DIR/all_reject_drop.tmp"
ALL_REJECT_NO_DROP="$TEMP_DIR/all_reject_no_drop.tmp"
ALL_DIRECT="$TEMP_DIR/all_direct.tmp"
ALL_URL_REWRITE="$TEMP_DIR/all_url_rewrite.tmp"
ALL_MAP_LOCAL="$TEMP_DIR/all_map_local.tmp"
ALL_SCRIPT="$TEMP_DIR/all_script.tmp"
ALL_MITM="$TEMP_DIR/all_mitm.tmp"

touch "$ALL_REJECT" "$ALL_REJECT_DROP" "$ALL_REJECT_NO_DROP" "$ALL_DIRECT"
touch "$ALL_URL_REWRITE" "$ALL_MAP_LOCAL" "$ALL_SCRIPT" "$ALL_MITM"

# Load whitelist
load_whitelist

# Check if update needed
needs_update=false
new_hashes=""

log_section "Checking for Updates"

for module_name in "${SOURCE_MODULES[@]}"; do
    module_path="$HEAD_EXPANSE_DIR/$module_name"
    if [ -f "$module_path" ]; then
        hash=$(md5 -q "$module_path" 2>/dev/null || md5sum "$module_path" | cut -d' ' -f1)
        new_hashes="${new_hashes}${module_name}|${hash}\n"
        
        old_hash=$(grep "^${module_name}|" "$HASH_FILE" 2>/dev/null | cut -d'|' -f2)
        if [ "$old_hash" != "$hash" ]; then
            needs_update=true
            log_info "Changed: $module_name"
        fi
    fi
done

for module in "$HEAD_EXPANSE_DIR"/*.sgmodule; do
    [ ! -f "$module" ] && continue
    [[ "$module" == "$TARGET_MODULE" ]] && continue
    [[ "$module" == "$FIREWALL_MODULE" ]] && continue

    module_name=$(basename "$module")
    # Skip if already in SOURCE_MODULES
    skip=false
    for src in "${SOURCE_MODULES[@]}"; do
        if [ "$src" == "$module_name" ]; then
            skip=true
            break
        fi
    done
    [ "$skip" = true ] && continue

    if grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then
        hash=$(md5 -q "$module" 2>/dev/null || md5sum "$module" | cut -d' ' -f1)
        new_hashes="${new_hashes}${module_name}|${hash}\n"

        old_hash=$(grep "^${module_name}|" "$HASH_FILE" 2>/dev/null | cut -d'|' -f2)
        if [ "$old_hash" != "$hash" ]; then
            needs_update=true
        fi
    fi
done

if [ ! -f "$TARGET_MODULE" ]; then
    needs_update=true
fi

if [ "$needs_update" = false ] && [ -f "$ADBLOCK_MERGED_LIST" ]; then
    log_info "No changes detected, skipping merge"
    exit 0
fi

log_section "Extracting Rules from Source Modules"

# Process priority source modules first
for module_name in "${SOURCE_MODULES[@]}"; do
    module_path="$HEAD_EXPANSE_DIR/$module_name"
    extract_rules_from_module "$module_path"
done

# Process other ad-related modules
for module in "$HEAD_EXPANSE_DIR"/*.sgmodule; do
    [ ! -f "$module" ] && continue
    [[ "$module" == "$TARGET_MODULE" ]] && continue
    [[ "$module" == "$FIREWALL_MODULE" ]] && continue

    module_name=$(basename "$module")
    skip=false
    for src in "${SOURCE_MODULES[@]}"; do
        if [ "$src" == "$module_name" ]; then
            skip=true
            break
        fi
    done
    [ "$skip" = true ] && continue

    if grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then
        extract_rules_from_module "$module"
    fi
done

log_section "Deduplicating Rules"

# Batch deduplication
sort -u "$ALL_REJECT" -o "$ALL_REJECT" 2>/dev/null || true
sort -u "$ALL_REJECT_DROP" -o "$ALL_REJECT_DROP" 2>/dev/null || true
sort -u "$ALL_REJECT_NO_DROP" -o "$ALL_REJECT_NO_DROP" 2>/dev/null || true
sort -u "$ALL_DIRECT" -o "$ALL_DIRECT" 2>/dev/null || true
sort -u "$ALL_URL_REWRITE" -o "$ALL_URL_REWRITE" 2>/dev/null || true
sort -u "$ALL_MAP_LOCAL" -o "$ALL_MAP_LOCAL" 2>/dev/null || true
sort -u "$ALL_SCRIPT" -o "$ALL_SCRIPT" 2>/dev/null || true
sort -u "$ALL_MITM" -o "$ALL_MITM" 2>/dev/null || true

# Count rules
reject_count=$(wc -l < "$ALL_REJECT" 2>/dev/null | tr -d ' ' || echo "0")
reject_drop_count=$(wc -l < "$ALL_REJECT_DROP" 2>/dev/null | tr -d ' ' || echo "0")
reject_no_drop_count=$(wc -l < "$ALL_REJECT_NO_DROP" 2>/dev/null | tr -d ' ' || echo "0")
url_rewrite_count=$(wc -l < "$ALL_URL_REWRITE" 2>/dev/null | tr -d ' ' || echo "0")
map_local_count=$(wc -l < "$ALL_MAP_LOCAL" 2>/dev/null | tr -d ' ' || echo "0")
script_count=$(wc -l < "$ALL_SCRIPT" 2>/dev/null | tr -d ' ' || echo "0")
mitm_count=$(wc -l < "$ALL_MITM" 2>/dev/null | tr -d ' ' || echo "0")

log_info "REJECT: $reject_count | DROP: $reject_drop_count | NO-DROP: $reject_no_drop_count"
log_info "URL Rewrite: $url_rewrite_count | Map Local: $map_local_count | Script: $script_count"
log_info "MITM Hosts: $mitm_count"

log_section "Generating Output Module"

# Generate module header
cat > "$TARGET_MODULE" << EOF
#!name=🚫 Universal Ad-Blocking Rules (Lite)
#!desc=Auto-merged from: 广告联盟 + 小程序去广告 + AD ByeBye+ 2.x + AllInOne + 可莉\\n\\n拦截广告平台、HTTPDNS、常见应用广告。包含 Rule/URL Rewrite/Map Local/Script/MITM 完整功能。所有规则集已内化，无外部依赖。\\n\\n⚠️ 隐私说明: kelee.one 是可莉的脚本托管服务，仅用于下载去广告脚本，不会上报用户数据。
#!author=可莉🅥, VirgilClyne, bunizao, blackmatrix7, limbopro, Auto-Merged
#!icon=https://raw.githubusercontent.com/luestr/IconResource/main/Other_icon/120px/KeLee.png
#!category=『 🔝 Head Expanse › 首端扩域 』
#!tag=去广告, 依赖, HTTPDNS
#!date=$(date +%Y-%m-%d\ %H:%M:%S)

[Rule]
# ═══════════════════════════════════════════════════════════════
# Self-hosted AdBlock rulesets (无外部依赖)
# Sources: 广告联盟, 小程序去广告, AD ByeBye+ 2.x, AllInOne, 可莉
# ═══════════════════════════════════════════════════════════════
# 主要REJECT规则 (本地托管)
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/AdBlock.list,REJECT,extended-matching,pre-matching,update-interval=86400,no-resolve
# REJECT-NO-DROP规则 (本地托管)
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/skk_upstream/reject-no-drop.conf,REJECT-NO-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve
# REJECT-DROP规则 (本地托管)
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/skk_upstream/reject-drop.conf,REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve
# BlockHttpDNS - 阻止HTTPDNS泄露 (本地托管)
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/skk_upstream/BlockHttpDNS.list,REJECT-DROP,extended-matching,pre-matching,update-interval=86400,no-resolve

EOF

# Add inline rules by category
if [ -s "$ALL_REJECT_DROP" ]; then
    echo "# Merged REJECT-DROP Rules ($reject_drop_count)" >> "$TARGET_MODULE"
    cat "$ALL_REJECT_DROP" >> "$TARGET_MODULE"
    echo "" >> "$TARGET_MODULE"
fi

if [ -s "$ALL_REJECT_NO_DROP" ]; then
    echo "# Merged REJECT-NO-DROP Rules ($reject_no_drop_count)" >> "$TARGET_MODULE"
    cat "$ALL_REJECT_NO_DROP" >> "$TARGET_MODULE"
    echo "" >> "$TARGET_MODULE"
fi

# Add URL Rewrite section
if [ -s "$ALL_URL_REWRITE" ]; then
    cat >> "$TARGET_MODULE" << EOF

[URL Rewrite]
# ═══════════════════════════════════════════════════════════════
# Merged URL Rewrite Rules ($url_rewrite_count)
# ═══════════════════════════════════════════════════════════════
EOF
    cat "$ALL_URL_REWRITE" >> "$TARGET_MODULE"
fi

# Add Map Local section
if [ -s "$ALL_MAP_LOCAL" ]; then
    cat >> "$TARGET_MODULE" << EOF

[Map Local]
# ═══════════════════════════════════════════════════════════════
# Merged Map Local Rules ($map_local_count)
# ═══════════════════════════════════════════════════════════════
EOF
    cat "$ALL_MAP_LOCAL" >> "$TARGET_MODULE"
fi

# Add Script section
if [ -s "$ALL_SCRIPT" ]; then
    cat >> "$TARGET_MODULE" << EOF

[Script]
# ═══════════════════════════════════════════════════════════════
# Merged Script Rules ($script_count)
# ═══════════════════════════════════════════════════════════════
EOF
    cat "$ALL_SCRIPT" >> "$TARGET_MODULE"
fi

# Add MITM section
if [ -s "$ALL_MITM" ]; then
    mitm_hosts=$(cat "$ALL_MITM" | sort -u | tr '\n' ',' | sed 's/,$//' | sed 's/,/, /g')
    cat >> "$TARGET_MODULE" << EOF

[MITM]
hostname = %APPEND% $mitm_hosts
EOF
fi

log_section "Generating AdBlock.list"

# AND规则拆分文件路径
AND_RULES_DECOMPOSED="$PROJECT_ROOT/ruleset/Sources/and_rules_decomposed.list"

# Merge with existing rules
if [ -f "$ADBLOCK_MERGED_LIST" ]; then
    grep -v "^#" "$ADBLOCK_MERGED_LIST" 2>/dev/null | grep -v "^$" | grep -v "^RULE-SET" > "$TEMP_DIR/old_adblock.tmp" || true
else
    touch "$TEMP_DIR/old_adblock.tmp"
fi

# 合并: 旧规则 + 新提取的REJECT规则 + AND规则拆分
cat "$TEMP_DIR/old_adblock.tmp" "$ALL_REJECT" 2>/dev/null | sort -u > "$TEMP_DIR/merged_adblock.tmp"

# 添加AND规则拆分（如果存在）
if [ -f "$AND_RULES_DECOMPOSED" ]; then
    log_info "Adding decomposed AND rules from: and_rules_decomposed.list"
    grep -v "^#" "$AND_RULES_DECOMPOSED" 2>/dev/null | grep -v "^$" >> "$TEMP_DIR/merged_adblock.tmp" || true
fi

# Clean and filter - 移除RULE-SET文件中不允许的参数
# RULE-SET文件只能包含纯规则，策略和参数在引用时指定
# 🔥 v5.2: 增强过滤 - 移除危险的 IP 规则
sed 's/,REJECT[^,]*//g; s/,extended-matching//g; s/,pre-matching//g; s/,no-resolve//g' "$TEMP_DIR/merged_adblock.tmp" 2>/dev/null | \
grep -v "^RULE-SET" | \
grep -v "^AND," | \
grep -v "^IP-CIDR,0\.0\.0\." | \
grep -v "^IP-CIDR,127\." | \
grep -v "^IP-CIDR,10\.0\.0\.0/8" | \
grep -v "^IP-CIDR,192\.168\.0\.0/16" | \
grep -v "^IP-CIDR,172\.16\.0\.0/12" | \
sort -u > "$TEMP_DIR/clean_adblock_pre.tmp" || true

filter_whitelist "$TEMP_DIR/clean_adblock_pre.tmp" "$TEMP_DIR/clean_adblock.tmp"

# Generate final list
final_count=$(wc -l < "$TEMP_DIR/clean_adblock.tmp" 2>/dev/null | tr -d ' ' || echo "0")
{
    echo "# Ruleset: AdBlock"
    echo "# Updated: $(date)"
    echo "# Total Rules: $final_count"
    echo "# Sources: 广告联盟, 小程序去广告, AD ByeBye+ 2.x, AllInOne, etc."
    echo "# ═══════════════════════════════════════════════════════════════"
    echo ""
    cat "$TEMP_DIR/clean_adblock.tmp"
} > "$ADBLOCK_MERGED_LIST"

# Export additional files
sort -u "$ALL_REJECT_DROP" > "$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/reject-drop.conf" 2>/dev/null || true
sort -u "$ALL_REJECT_NO_DROP" > "$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/reject-no-drop.conf" 2>/dev/null || true

if [ -s "$ALL_DIRECT" ]; then
    mkdir -p "$PROJECT_ROOT/ruleset/Sources/conf"
    sort -u "$ALL_DIRECT" > "$PROJECT_ROOT/ruleset/Sources/conf/SurgeConf_ModulesDirect.list"
fi

# Save hashes
echo -e "$new_hashes" > "$HASH_FILE"

# Cleanup
rm -rf "$TEMP_DIR"

log_section "Merge Complete"
log_success "Universal Ad-Blocking Rules: $final_count REJECT rules"
log_success "REJECT-DROP: $reject_drop_count | REJECT-NO-DROP: $reject_no_drop_count"
log_success "URL Rewrite: $url_rewrite_count | Map Local: $map_local_count | Script: $script_count"
log_success "MITM Hosts: $mitm_count"
