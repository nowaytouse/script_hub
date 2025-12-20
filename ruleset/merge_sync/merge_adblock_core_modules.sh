#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# AdBlock Core Modules Merger v1.0
# 
# 合并核心广告拦截模块为单一模块:
#   - 可莉广告过滤器 (Keli Ad Filter)
#   - 广告平台拦截器 (Ad Platform Blocker)
#   - HTTPDNS拦截器 (HTTPDNS Blocker)
#
# 输出: 🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule
# ═══════════════════════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MODULE_DIR="$PROJECT_ROOT/module/surge(main)"
HEAD_EXPANSE_DIR="$MODULE_DIR/head_expanse"
TEMP_DIR="$PROJECT_ROOT/.temp_adblock_core_merge"
OUTPUT_MODULE="$HEAD_EXPANSE_DIR/🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }

# Source modules to merge
SOURCE_MODULES=(
    "$HEAD_EXPANSE_DIR/可莉广告过滤器.beta.sgmodule"
    "$HEAD_EXPANSE_DIR/广告平台拦截器.sgmodule"
    "$HEAD_EXPANSE_DIR/blockHTTPDNS.module"
)

# Init
rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR"

log_info "═══════════════════════════════════════════════════════════════"
log_info "AdBlock Core Modules Merger"
log_info "═══════════════════════════════════════════════════════════════"

# Temp files for each section
RULES_FILE="$TEMP_DIR/rules.tmp"
URL_REWRITE_FILE="$TEMP_DIR/url_rewrite.tmp"
BODY_REWRITE_FILE="$TEMP_DIR/body_rewrite.tmp"
MAP_LOCAL_FILE="$TEMP_DIR/map_local.tmp"
MITM_FILE="$TEMP_DIR/mitm.tmp"

touch "$RULES_FILE" "$URL_REWRITE_FILE" "$BODY_REWRITE_FILE" "$MAP_LOCAL_FILE" "$MITM_FILE"

# BiliBili exclusion pattern (already in dedicated module)
BILIBILI_PATTERN="bilibili|biliapi|bilivideo|哔哩"

# Process each source module
for module in "${SOURCE_MODULES[@]}"; do
    if [ ! -f "$module" ]; then
        log_warning "Module not found: $(basename "$module")"
        continue
    fi
    
    module_name=$(basename "$module")
    log_info "Processing: $module_name"
    
    # Extract [Rule] section (exclude BiliBili)
    awk '/^\[Rule\]/{f=1;next}/^\[/{f=0}f' "$module" 2>/dev/null | \
        grep -v '^#' | grep -viE "$BILIBILI_PATTERN" | grep -v '^$' >> "$RULES_FILE" || true
    
    # Extract [URL Rewrite] section (exclude BiliBili)
    awk '/^\[URL Rewrite\]/{f=1;next}/^\[/{f=0}f' "$module" 2>/dev/null | \
        grep -v '^#' | grep -viE "$BILIBILI_PATTERN" | grep -v '^$' >> "$URL_REWRITE_FILE" || true
    
    # Extract [Body Rewrite] section (exclude BiliBili)
    awk '/^\[Body Rewrite\]/{f=1;next}/^\[/{f=0}f' "$module" 2>/dev/null | \
        grep -v '^#' | grep -viE "$BILIBILI_PATTERN" | grep -v '^$' >> "$BODY_REWRITE_FILE" || true
    
    # Extract [Map Local] section (exclude BiliBili)
    awk '/^\[Map Local\]/{f=1;next}/^\[/{f=0}f' "$module" 2>/dev/null | \
        grep -v '^#' | grep -viE "$BILIBILI_PATTERN" | grep -v '^$' >> "$MAP_LOCAL_FILE" || true
    
    # Extract MITM hostnames (exclude BiliBili)
    grep -E "^hostname\s*=" "$module" 2>/dev/null | \
        sed 's/hostname\s*=\s*%APPEND%\s*//' | \
        tr ',' '\n' | tr -d ' ' | grep -viE "$BILIBILI_PATTERN" >> "$MITM_FILE" || true
done

# Deduplicate all sections
sort -u "$RULES_FILE" -o "$RULES_FILE" 2>/dev/null || true
sort -u "$URL_REWRITE_FILE" -o "$URL_REWRITE_FILE" 2>/dev/null || true
sort -u "$BODY_REWRITE_FILE" -o "$BODY_REWRITE_FILE" 2>/dev/null || true
sort -u "$MAP_LOCAL_FILE" -o "$MAP_LOCAL_FILE" 2>/dev/null || true
sort -u "$MITM_FILE" -o "$MITM_FILE" 2>/dev/null || true

# Count rules
RULE_COUNT=$(wc -l < "$RULES_FILE" 2>/dev/null | tr -d ' ' || echo "0")
URL_REWRITE_COUNT=$(wc -l < "$URL_REWRITE_FILE" 2>/dev/null | tr -d ' ' || echo "0")
BODY_REWRITE_COUNT=$(wc -l < "$BODY_REWRITE_FILE" 2>/dev/null | tr -d ' ' || echo "0")
MAP_LOCAL_COUNT=$(wc -l < "$MAP_LOCAL_FILE" 2>/dev/null | tr -d ' ' || echo "0")
MITM_COUNT=$(wc -l < "$MITM_FILE" 2>/dev/null | tr -d ' ' || echo "0")

log_info "Merged statistics:"
log_info "  Rules: $RULE_COUNT"
log_info "  URL Rewrite: $URL_REWRITE_COUNT"
log_info "  Body Rewrite: $BODY_REWRITE_COUNT"
log_info "  Map Local: $MAP_LOCAL_COUNT"
log_info "  MITM Hosts: $MITM_COUNT"

# Generate merged module
log_info "Generating merged module..."

cat > "$OUTPUT_MODULE" << 'EOF'
#!name=🚫 Universal Ad-Blocking Rules (Lite)
#!desc=Auto-merged from: 可莉广告过滤器 + 广告平台拦截器 + HTTPDNS拦截器\n\n拦截广告平台、HTTPDNS、常见应用广告。包含 Rule/URL Rewrite/Body Rewrite/Map Local/MITM 完整功能。
#!author=可莉🅥, VirgilClyne, Auto-Merged
#!icon=https://raw.githubusercontent.com/luestr/IconResource/main/Other_icon/120px/KeLee.png
#!category=『 🔝 Head Expanse › 首端扩域 』
#!tag=去广告, 依赖, HTTPDNS
EOF

echo "#!date=$(date '+%Y-%m-%d %H:%M:%S')" >> "$OUTPUT_MODULE"
echo "" >> "$OUTPUT_MODULE"

# Add external RULE-SET references
cat >> "$OUTPUT_MODULE" << 'EOF'
[Rule]
# External AdBlock rulesets (high priority)
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/AdBlock.list,REJECT,extended-matching,pre-matching,"update-interval=86400",no-resolve
RULE-SET,https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf,REJECT-NO-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve
RULE-SET,https://ruleset.skk.moe/List/non_ip/reject-drop.conf,REJECT-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve
RULE-SET,https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/BlockHttpDNS/BlockHttpDNS.list,REJECT-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve

# Merged inline rules
EOF

# Add rules
cat "$RULES_FILE" >> "$OUTPUT_MODULE"

# Add URL Rewrite section
if [ -s "$URL_REWRITE_FILE" ]; then
    echo "" >> "$OUTPUT_MODULE"
    echo "[URL Rewrite]" >> "$OUTPUT_MODULE"
    cat "$URL_REWRITE_FILE" >> "$OUTPUT_MODULE"
fi

# Add Body Rewrite section
if [ -s "$BODY_REWRITE_FILE" ]; then
    echo "" >> "$OUTPUT_MODULE"
    echo "[Body Rewrite]" >> "$OUTPUT_MODULE"
    cat "$BODY_REWRITE_FILE" >> "$OUTPUT_MODULE"
fi

# Add Map Local section
if [ -s "$MAP_LOCAL_FILE" ]; then
    echo "" >> "$OUTPUT_MODULE"
    echo "[Map Local]" >> "$OUTPUT_MODULE"
    cat "$MAP_LOCAL_FILE" >> "$OUTPUT_MODULE"
fi

# Add MITM section
if [ -s "$MITM_FILE" ]; then
    echo "" >> "$OUTPUT_MODULE"
    echo "[MITM]" >> "$OUTPUT_MODULE"
    # Convert to comma-separated list
    MITM_HOSTS=$(cat "$MITM_FILE" | tr '\n' ',' | sed 's/,$//' | sed 's/,,/,/g')
    echo "hostname = %APPEND% $MITM_HOSTS" >> "$OUTPUT_MODULE"
fi

# Cleanup
rm -rf "$TEMP_DIR"

log_success "═══════════════════════════════════════════════════════════════"
log_success "Merged module created: $(basename "$OUTPUT_MODULE")"
log_success "Total: $RULE_COUNT rules, $URL_REWRITE_COUNT rewrites, $MAP_LOCAL_COUNT maps"
log_success "═══════════════════════════════════════════════════════════════"

# Optionally delete source modules (commented out for safety)
# log_warning "Source modules can be deleted manually if no longer needed:"
# for module in "${SOURCE_MODULES[@]}"; do
#     [ -f "$module" ] && echo "  rm \"$module\""
# done
