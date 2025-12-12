#!/opt/homebrew/bin/bash
# Download AdBlock modules from URLs in AdBlock_sources.txt
# Extract rules and merge into AdBlock.list

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SOURCES_FILE="$PROJECT_ROOT/ruleset/Sources/Links/AdBlock_sources.txt"
TEMP_DIR="$PROJECT_ROOT/.temp_adblock_download"
# 🔥 修复: 下载到正确的目录 head_expanse (广告拦截模块分类)
MODULE_DIR="$PROJECT_ROOT/module/surge(main)/head_expanse"
ADBLOCK_LIST="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock.list"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;36m'
NC='\033[0m' # No Color

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }

# Init
rm -rf "$TEMP_DIR"
mkdir -p "$TEMP_DIR" "$MODULE_DIR"

log_info "Downloading AdBlock modules..."

# Extract module URLs from sources file
module_urls=$(grep -E "^https?://.*\.sgmodule$|^https?://.*\.module$" "$SOURCES_FILE" 2>/dev/null || true)

if [ -z "$module_urls" ]; then
    log_warning "No module URLs found in $SOURCES_FILE"
    exit 0
fi

# Download each module
downloaded=0
failed=0
total=0

while IFS= read -r url; do
    [ -z "$url" ] && continue
    total=$((total + 1))
    
    # Extract filename from URL
    filename=$(basename "$url")
    module_file="$MODULE_DIR/$filename"
    temp_file="$TEMP_DIR/$filename"
    
    log_info "Downloading: $filename"
    
    # Download with timeout
    if curl -L -s -m 30 -o "$temp_file" "$url" 2>/dev/null; then
        # Check if file is valid (not empty, not HTML error page)
        if [ -s "$temp_file" ] && ! grep -q "<!DOCTYPE html>" "$temp_file" 2>/dev/null; then
            # Check if file changed
            if [ -f "$module_file" ]; then
                if ! diff -q "$temp_file" "$module_file" >/dev/null 2>&1; then
                    cp "$temp_file" "$module_file"
                    log_success "Updated: $filename"
                else
                    log_info "No change: $filename"
                fi
            else
                cp "$temp_file" "$module_file"
                log_success "Downloaded: $filename"
            fi
            downloaded=$((downloaded + 1))
        else
            log_error "Invalid file: $filename (may be 403/404)"
            failed=$((failed + 1))
        fi
    else
        log_error "Download failed: $filename"
        failed=$((failed + 1))
    fi
done <<< "$module_urls"

log_info ""
log_info "Download Summary:"
log_info "  Total:      $total"
log_success "  Success:    $downloaded"
[ $failed -gt 0 ] && log_error "  Failed:     $failed" || log_info "  Failed:     0"

# Extract rules from downloaded modules and create cleaned versions
log_info ""
log_info "Extracting rules and creating cleaned modules..."

ALL_RULES="$TEMP_DIR/all_module_rules.tmp"
touch "$ALL_RULES"

# 🔥 保护列表：这些模块不参与规则提取和清理
# 防火墙模块的规则应该单独保留，不合并到 AdBlock.list
PROTECTED_MODULES=(
    "🔥 Firewall Port Blocker 🛡️🚫.sgmodule"
    "🛡️ 广告拦截大合集.sgmodule"
    "🎯 App去广告大合集.sgmodule"
    "🚀 功能增强大合集.sgmodule"
)

for module in "$MODULE_DIR"/*.sgmodule "$MODULE_DIR"/*.module; do
    [ ! -f "$module" ] && continue
    
    module_name=$(basename "$module")
    
    # 检查是否在保护列表中
    is_protected=false
    for protected in "${PROTECTED_MODULES[@]}"; do
        if [[ "$module_name" == "$protected" ]]; then
            is_protected=true
            break
        fi
    done
    
    if [ "$is_protected" = true ]; then
        log_info "Skipping (protected): $module_name"
        continue
    fi
    
    log_info "Processing: $module_name"
    
    # Count original rules BEFORE any modification
    original_rules=$(grep -cE "^(DOMAIN|IP-CIDR|URL-REGEX|USER-AGENT|PROCESS-NAME)" "$module" 2>/dev/null | tr -d ' ' || echo "0")
    
    # Extract [Rule] section for merging into AdBlock.list
    awk '/^\[Rule\]/{f=1;next}/^\[/{f=0}f' "$module" 2>/dev/null | \
    grep -v '^#' | grep -v '^$' | grep -v '^RULE-SET' | \
    sed 's/  */ /g' | \
    grep -E "^(DOMAIN|IP-CIDR|IP-CIDR6|USER-AGENT|URL-REGEX|PROCESS-NAME|DOMAIN-REGEX|DOMAIN-SUFFIX|DOMAIN-KEYWORD|AND,|OR,|NOT,)" | \
    # 🔥 根源修复: 展开 AND/OR 复合规则为简单规则（跨平台兼容）
    python3 -c '
import sys, re
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    # 展开 AND/OR 复合规则
    if line.startswith("AND,((") or line.startswith("OR,(("):
        # 提取所有 DOMAIN-SUFFIX
        for m in re.findall(r"DOMAIN-SUFFIX,([^,\)\]]+)", line):
            s = m.strip()
            if s and "." in s:
                print(f"DOMAIN-SUFFIX,{s}")
        # 提取 DOMAIN-KEYWORD 中像域名的值
        for m in re.findall(r"DOMAIN-KEYWORD,-?([^,\)\]]+)", line):
            s = m.strip()
            if re.search(r"\.(com|net|org|io|cn|jp|kr|tw|hk|sg|uk|de|fr|ru|br|in|au|co|me|tv|cc|xyz|top|app|dev)$", s, re.I):
                print(f"DOMAIN-SUFFIX,{s}")
        continue
    # 过滤不完整规则和括号不匹配的行
    if line.endswith("DOMAIN-KEYWORD") or line.endswith("DOMAIN-SUFFIX") or line.endswith("DOMAIN"):
        continue
    if line.count("(") != line.count(")"):
        continue
    print(line)
' | \
    # Filter out invalid DOMAIN-REGEX rules
    grep -v '^DOMAIN-REGEX,\s*$' | \
    grep -v '^DOMAIN-REGEX,[^,]*$' | \
    # Fix IPv6 rules
    sed 's/^IP-CIDR,\([0-9a-fA-F:]*::[^,]*\)/IP-CIDR6,\1/' | \
    # Remove policies (REJECT/DIRECT) and options (extended-matching/pre-matching)
    sed 's/,\(REJECT\|DIRECT\|PROXY\|REJECT-DROP\|REJECT-TINYGIF\|REJECT-NO-DROP\|REJECT-IMG\)\(,.*\)*$//' | \
    sed 's/,extended-matching//g; s/,pre-matching//g' >> "$ALL_RULES" || true
    
    # Check if module has non-Rule sections (URL Rewrite, MITM, Script, Map Local, etc.)
    # 🔥 修复: 直接检查是否存在这些 section，而不是检查 [Rule] 之后的内容
    has_other_sections=$(grep -E "^\[(URL Rewrite|MITM|Script|Map Local|Body Rewrite|Header Rewrite)\]" "$module" 2>/dev/null | head -1)
    
    if [ -n "$has_other_sections" ]; then
        # Module has other sections - keep them but remove [Rule] section
        cleaned_module="$TEMP_DIR/cleaned_${module_name}"
        
        # 🔥 修复: 复制 header 时保留 #!category= 标签，排除已有的 NOTE 注释块
        # Surge 使用 #!category= 来分组模块（不是 #!group=）
        awk '
            /^\[Rule\]/ { exit }
            /^# ═══════════════/ { skip_block=1; next }
            /^# NOTE: All .* rules/ { next }
            /^# This cleaned version/ { next }
            /^#   - \[/ { next }
            /^# Use AdBlock.list/ { next }
            skip_block && /^$/ { skip_block=0; next }
            skip_block { next }
            { print }
        ' "$module" > "$cleaned_module"
        
        # 🔥 修复: 使用 #!category= 而不是 #!group=（Surge 正确的分组标签）
        # Add category classification based on module type (only if not already present)
        if ! grep -q "^#!category=" "$cleaned_module"; then
            module_display_name=$(grep "^#!name=" "$module" | head -1 | sed 's/#!name=//')
            
            # Priority: DNS > Ad blocking > Others
            if echo "$module_display_name" | grep -qi "httpdns\|dns"; then
                # 在 #!desc 后面插入 #!category
                if grep -q "^#!desc=" "$cleaned_module"; then
                    sed -i '' '/^#!desc=/a\
#!category=『 🛠️ Amplify Nexus › 增幅枢纽 』
' "$cleaned_module"
                else
                    echo "#!category=『 🛠️ Amplify Nexus › 增幅枢纽 』" >> "$cleaned_module"
                fi
            elif echo "$module_display_name" | grep -qi "广告\|adblock\|ad\|拦截\|limbo"; then
                if grep -q "^#!desc=" "$cleaned_module"; then
                    sed -i '' '/^#!desc=/a\
#!category=『 🔝 Head Expanse › 首端扩域 』
' "$cleaned_module"
                else
                    echo "#!category=『 🔝 Head Expanse › 首端扩域 』" >> "$cleaned_module"
                fi
            else
                if grep -q "^#!desc=" "$cleaned_module"; then
                    sed -i '' '/^#!desc=/a\
#!category=『 🎯 Narrow Pierce › 窄域穿刺 』
' "$cleaned_module"
                else
                    echo "#!category=『 🎯 Narrow Pierce › 窄域穿刺 』" >> "$cleaned_module"
                fi
            fi
        fi
        
        # 🔥 修复: 注释放在 header 之后、第一个 section 之前，避免跑到 MITM 区域
        # 先保存当前内容
        local header_content=$(cat "$cleaned_module")
        
        # 重新生成文件：header + 注释 + sections
        {
            echo "$header_content"
            echo ""
            echo "# ═══════════════════════════════════════════════════════════════"
            echo "# NOTE: All $original_rules rules from this module have been extracted to AdBlock.list"
            echo "# This cleaned version only contains non-Rule sections:"
            echo "#   - [URL Rewrite]"
            echo "#   - [MITM]"
            echo "#   - [Script]"
            echo "#   - Other module-specific features"
            echo "# Use AdBlock.list for all blocking rules"
            echo "# ═══════════════════════════════════════════════════════════════"
            echo ""
        } > "${cleaned_module}.tmp"
        mv "${cleaned_module}.tmp" "$cleaned_module"
        
        # Copy all sections AFTER [Rule] (excluding duplicate NOTE blocks)
        awk '
            BEGIN { in_rule=0; after_rule=0; skip_block=0 }
            /^\[Rule\]/ { in_rule=1; next }
            in_rule && /^\[/ { in_rule=0; after_rule=1 }
            !after_rule { next }
            /^# ═══════════════/ { skip_block=1; next }
            /^# NOTE: All .* rules/ { next }
            /^# This cleaned version/ { next }
            /^#   - \[/ { next }
            /^# Use AdBlock.list/ { next }
            skip_block && /^$/ { skip_block=0; next }
            skip_block { next }
            { print }
        ' "$module" >> "$cleaned_module" || true
        
        # Replace original module
        mv "$cleaned_module" "$module"
        log_success "  Cleaned: Extracted $original_rules rules, kept non-Rule sections"
    else
        # Module only has rules - delete it completely
        log_warning "  Removed: All $original_rules rules extracted to AdBlock.list (no other features)"
        rm -f "$module"
    fi
done


# Count extracted rules
rule_count=$(wc -l < "$ALL_RULES" 2>/dev/null | tr -d ' ' || echo "0")
log_info "Extracted $rule_count rules from modules"

if [ "$rule_count" -gt 0 ]; then
    # Merge with existing AdBlock.list
    log_info "Merging with existing AdBlock.list..."
    
    # Extract existing rules (without header) and clean AND/OR rules
    if [ -f "$ADBLOCK_LIST" ]; then
        grep -v "^#" "$ADBLOCK_LIST" | grep -v "^$" | \
        # 🔥 清理现有的 AND/OR 复合规则
        python3 -c '
import sys, re
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    # 展开 AND/OR 复合规则
    if line.startswith("AND,((") or line.startswith("OR,(("):
        for m in re.findall(r"DOMAIN-SUFFIX,([^,\)\]]+)", line):
            s = m.strip()
            if s and "." in s:
                print(f"DOMAIN-SUFFIX,{s}")
        for m in re.findall(r"DOMAIN-KEYWORD,-?([^,\)\]]+)", line):
            s = m.strip()
            if re.search(r"\.(com|net|org|io|cn|jp|kr|tw|hk|sg|uk|de|fr|ru|br|in|au|co|me|tv|cc|xyz|top|app|dev)$", s, re.I):
                print(f"DOMAIN-SUFFIX,{s}")
        continue
    # 过滤不完整规则
    if line.endswith("DOMAIN-KEYWORD") or line.endswith("DOMAIN-SUFFIX") or line.endswith("DOMAIN"):
        continue
    if line.count("(") != line.count(")"):
        continue
    print(line)
' > "$TEMP_DIR/existing_rules.tmp" 2>/dev/null || true
    else
        touch "$TEMP_DIR/existing_rules.tmp"
    fi
    
    # Merge and deduplicate
    cat "$TEMP_DIR/existing_rules.tmp" "$ALL_RULES" 2>/dev/null | \
    sort -u > "$TEMP_DIR/merged_rules.tmp"
    
    # Count final rules
    final_count=$(wc -l < "$TEMP_DIR/merged_rules.tmp" 2>/dev/null | tr -d ' ' || echo "0")
    
    # Generate new AdBlock.list with header
    {
        echo "# Ruleset: AdBlock"
        echo "# Updated: $(date)"
        echo "# Total Rules: $final_count"
        echo "# Includes: Module rules + Source rules"
        echo "# ═══════════════════════════════════════════════════════════════"
        echo ""
        cat "$TEMP_DIR/merged_rules.tmp"
    } > "$ADBLOCK_LIST"
    
    log_success "AdBlock.list updated: $final_count total rules"
else
    log_warning "No rules extracted from modules"
fi

# Cleanup
rm -rf "$TEMP_DIR"

log_success "Done!"

