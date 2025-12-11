#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# 广告拦截模块智能合并脚本 (Ad-Blocking Module Intelligent Merger)
# ═══════════════════════════════════════════════════════════════════════════════
# 功能：
# 1. 从多个代理软件（Surge、小火箭等）提取广告拦截规则
# 2. 智能分类：REJECT、REJECT-DROP、REJECT-NO-DROP
# 3. 增量合并，自动去重
# 4. URL Rewrite 规则单独处理
# 5. Host 规则单独处理
# 6. 自动同步到小火箭模块
# ═══════════════════════════════════════════════════════════════════════════════

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 配置路径
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SURGE_MODULE_DIR="$PROJECT_ROOT/module/surge(main)"
SHADOWROCKET_MODULE_DIR="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules"
TEMP_DIR="$PROJECT_ROOT/.temp_adblock_merge"

# 目标模块
TARGET_MODULE="$SURGE_MODULE_DIR/🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"

# 巨大的合并规则文件
ADBLOCK_MERGED_LIST="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock_Merged.list"

# 临时文件
TEMP_RULES_REJECT="$TEMP_DIR/rules_reject.tmp"
TEMP_RULES_REJECT_DROP="$TEMP_DIR/rules_reject_drop.tmp"
TEMP_RULES_REJECT_NO_DROP="$TEMP_DIR/rules_reject_no_drop.tmp"
TEMP_URL_REWRITE="$TEMP_DIR/url_rewrite.tmp"
TEMP_HOST="$TEMP_DIR/host.tmp"
TEMP_MITM="$TEMP_DIR/mitm.tmp"

# 日志函数
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

# 创建临时目录
create_temp_dir() {
    log_info "创建临时目录..."
    rm -rf "$TEMP_DIR"
    mkdir -p "$TEMP_DIR"
}

# 清理临时目录
cleanup_temp_dir() {
    log_info "清理临时目录..."
    rm -rf "$TEMP_DIR"
}

# 提取现有规则到临时文件
extract_existing_rules() {
    log_section "提取现有规则"
    
    if [[ ! -f "$TARGET_MODULE" ]]; then
        log_error "目标模块不存在: $TARGET_MODULE"
        exit 1
    fi
    
    # 提取 [Rule] 部分 - 使用更简单的方法
    log_info "提取 Rule 规则..."
    local in_rule_section=0
    while IFS= read -r line; do
        if [[ "$line" =~ ^\[Rule\] ]]; then
            in_rule_section=1
            continue
        elif [[ "$line" =~ ^\[.*\] ]] && [[ $in_rule_section -eq 1 ]]; then
            break
        fi
        
        if [[ $in_rule_section -eq 1 ]] && [[ ! "$line" =~ ^# ]] && [[ ! -z "$line" ]]; then
            if [[ "$line" =~ ^(DOMAIN|IP-CIDR|USER-AGENT|URL-REGEX) ]]; then
                echo "$line" >> "$TEMP_DIR/existing_rules.tmp"
            fi
        fi
    done < "$TARGET_MODULE"
    
    touch "$TEMP_DIR/existing_rules.tmp"
    
    # 按策略分类
    grep ",REJECT," "$TEMP_DIR/existing_rules.tmp" 2>/dev/null | grep -v "REJECT-DROP" | grep -v "REJECT-NO-DROP" > "$TEMP_RULES_REJECT" || touch "$TEMP_RULES_REJECT"
    grep ",REJECT-DROP," "$TEMP_DIR/existing_rules.tmp" 2>/dev/null > "$TEMP_RULES_REJECT_DROP" || touch "$TEMP_RULES_REJECT_DROP"
    grep ",REJECT-NO-DROP," "$TEMP_DIR/existing_rules.tmp" 2>/dev/null > "$TEMP_RULES_REJECT_NO_DROP" || touch "$TEMP_RULES_REJECT_NO_DROP"
    
    # 提取 [URL Rewrite] 部分
    log_info "提取 URL Rewrite 规则..."
    local in_url_section=0
    while IFS= read -r line; do
        if [[ "$line" =~ ^\[URL\ Rewrite\] ]]; then
            in_url_section=1
            continue
        elif [[ "$line" =~ ^\[.*\] ]] && [[ $in_url_section -eq 1 ]]; then
            break
        fi
        
        if [[ $in_url_section -eq 1 ]] && [[ ! "$line" =~ ^# ]] && [[ ! -z "$line" ]]; then
            if [[ "$line" =~ reject ]]; then
                echo "$line" >> "$TEMP_URL_REWRITE"
            fi
        fi
    done < "$TARGET_MODULE"
    
    touch "$TEMP_URL_REWRITE"
    
    # 提取 [Host] 部分
    log_info "提取 Host 规则..."
    local in_host_section=0
    while IFS= read -r line; do
        if [[ "$line" =~ ^\[Host\] ]]; then
            in_host_section=1
            continue
        elif [[ "$line" =~ ^\[.*\] ]] && [[ $in_host_section -eq 1 ]]; then
            break
        fi
        
        if [[ $in_host_section -eq 1 ]] && [[ ! "$line" =~ ^# ]] && [[ ! -z "$line" ]]; then
            if [[ "$line" =~ "= 0.0.0.0" ]]; then
                echo "$line" >> "$TEMP_HOST"
            fi
        fi
    done < "$TARGET_MODULE"
    
    touch "$TEMP_HOST"
    
    # 提取 [MITM] hostname
    log_info "提取 MITM hostname..."
    local in_mitm_section=0
    while IFS= read -r line; do
        if [[ "$line" =~ ^\[MITM\] ]]; then
            in_mitm_section=1
            continue
        elif [[ "$line" =~ ^\[.*\] ]] && [[ $in_mitm_section -eq 1 ]]; then
            break
        fi
        
        if [[ $in_mitm_section -eq 1 ]] && [[ "$line" =~ ^hostname ]]; then
            echo "$line" >> "$TEMP_MITM"
        fi
    done < "$TARGET_MODULE"
    
    touch "$TEMP_MITM"
    
    local reject_count=$(wc -l < "$TEMP_RULES_REJECT" | tr -d ' ')
    local reject_drop_count=$(wc -l < "$TEMP_RULES_REJECT_DROP" | tr -d ' ')
    local reject_no_drop_count=$(wc -l < "$TEMP_RULES_REJECT_NO_DROP" | tr -d ' ')
    local url_rewrite_count=$(wc -l < "$TEMP_URL_REWRITE" | tr -d ' ')
    local host_count=$(wc -l < "$TEMP_HOST" | tr -d ' ')
    
    log_success "现有规则统计:"
    echo "  - REJECT: $reject_count"
    echo "  - REJECT-DROP: $reject_drop_count"
    echo "  - REJECT-NO-DROP: $reject_no_drop_count"
    echo "  - URL Rewrite: $url_rewrite_count"
    echo "  - Host: $host_count"
}

# 从指定模块提取规则
extract_rules_from_module() {
    local module_file="$1"
    local module_name=$(basename "$module_file")
    
    log_info "处理模块: $module_name"
    
    if [[ ! -f "$module_file" ]]; then
        log_warning "模块文件不存在: $module_file"
        return
    fi
    
    # 检查文件大小，跳过过大的文件（可能有问题）
    local file_size=$(stat -f%z "$module_file" 2>/dev/null || echo "0")
    if [[ $file_size -gt 100000 ]]; then
        log_warning "文件过大，跳过: $module_name ($(($file_size / 1024))KB)"
        return
    fi
    
    local new_rules=0
    
    # 提取 Rule 规则 - 使用 grep 更高效
    > "$TEMP_DIR/new_rules.tmp"
    grep -A 1000 "^\[Rule\]" "$module_file" 2>/dev/null | grep -B 1000 "^\[" | grep -E "^(DOMAIN|IP-CIDR|USER-AGENT|URL-REGEX)" >> "$TEMP_DIR/new_rules.tmp" || true
    
    # 按策略分类并去重
    while IFS= read -r rule; do
        if [[ -z "$rule" ]] || [[ "$rule" =~ ^# ]]; then
            continue
        fi
        
        # 标准化规则（移除多余空格）
        rule=$(echo "$rule" | sed 's/  */ /g')
        
        if echo "$rule" | grep -q ",REJECT-DROP,"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT_DROP"; then
                echo "$rule" >> "$TEMP_RULES_REJECT_DROP"
                ((new_rules++))
            fi
        elif echo "$rule" | grep -q ",REJECT-NO-DROP,"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT_NO_DROP"; then
                echo "$rule" >> "$TEMP_RULES_REJECT_NO_DROP"
                ((new_rules++))
            fi
        elif echo "$rule" | grep -q ",REJECT,"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT"; then
                echo "$rule" >> "$TEMP_RULES_REJECT"
                ((new_rules++))
            fi
        fi
    done < "$TEMP_DIR/new_rules.tmp"
    
    # 提取 URL Rewrite 规则 - 使用 grep 更高效
    > "$TEMP_DIR/new_url_rewrite.tmp"
    grep -A 1000 "^\[URL Rewrite\]" "$module_file" 2>/dev/null | grep -B 1000 "^\[" | grep -v "^#" | grep -v "^\[" | grep "reject" >> "$TEMP_DIR/new_url_rewrite.tmp" || true
    
    while IFS= read -r rule; do
        if [[ -z "$rule" ]] || [[ "$rule" =~ ^# ]]; then
            continue
        fi
        rule=$(echo "$rule" | sed 's/  */ /g')
        if ! grep -Fxq "$rule" "$TEMP_URL_REWRITE"; then
            echo "$rule" >> "$TEMP_URL_REWRITE"
            ((new_rules++))
        fi
    done < "$TEMP_DIR/new_url_rewrite.tmp"
    
    # 提取 Host 规则 - 使用 grep 更高效
    > "$TEMP_DIR/new_host.tmp"
    grep -A 1000 "^\[Host\]" "$module_file" 2>/dev/null | grep -B 1000 "^\[" | grep "= 0.0.0.0" >> "$TEMP_DIR/new_host.tmp" || true
    
    while IFS= read -r rule; do
        if [[ -z "$rule" ]] || [[ "$rule" =~ ^# ]]; then
            continue
        fi
        rule=$(echo "$rule" | sed 's/  */ /g')
        if ! grep -Fxq "$rule" "$TEMP_HOST"; then
            echo "$rule" >> "$TEMP_HOST"
            ((new_rules++))
        fi
    done < "$TEMP_DIR/new_host.tmp"
    
    # 提取 MITM hostname - 使用 grep 更高效
    > "$TEMP_DIR/new_mitm.tmp"
    grep -A 100 "^\[MITM\]" "$module_file" 2>/dev/null | grep "^hostname" >> "$TEMP_DIR/new_mitm.tmp" || true
    
    # 合并 MITM hostname（去重）
    if [[ -s "$TEMP_DIR/new_mitm.tmp" ]]; then
        # 提取所有 hostname
        local existing_hosts=$(grep "hostname" "$TEMP_MITM" 2>/dev/null | sed 's/hostname = %APPEND% //g' | sed 's/hostname = //g' | tr ',' '\n' | sed 's/^ *//g' | sed 's/ *$//g' | sort -u)
        local new_hosts=$(grep "hostname" "$TEMP_DIR/new_mitm.tmp" | sed 's/hostname = %APPEND% //g' | sed 's/hostname = //g' | tr ',' '\n' | sed 's/^ *//g' | sed 's/ *$//g' | sort -u)
        
        # 合并去重
        local all_hosts=$(echo -e "$existing_hosts\n$new_hosts" | sort -u | grep -v '^$')
        
        # 重新生成 hostname 行
        echo "hostname = %APPEND% $(echo "$all_hosts" | tr '\n' ',' | sed 's/,$//' | sed 's/,/, /g')" > "$TEMP_MITM"
    fi
    
    if [[ $new_rules -gt 0 ]]; then
        log_success "从 $module_name 新增 $new_rules 条规则"
    else
        log_info "从 $module_name 未发现新规则"
    fi
}

# 扫描并处理所有模块
scan_and_merge_modules() {
    log_section "扫描并合并模块"
    
    # 处理 Surge 模块
    log_info "扫描 Surge 模块目录..."
    if [[ -d "$SURGE_MODULE_DIR" ]]; then
        for module in "$SURGE_MODULE_DIR"/*.sgmodule; do
            if [[ -f "$module" ]] && [[ "$module" != "$TARGET_MODULE" ]]; then
                # 只处理包含广告拦截相关的模块
                if grep -qi "ad\|reject\|block" "$module"; then
                    extract_rules_from_module "$module"
                fi
            fi
        done
    fi
    
    # 处理小火箭模块（暂时禁用，因为有兼容性问题）
    log_warning "小火箭模块扫描已禁用（存在兼容性问题）"
    log_info "如需处理小火箭模块，请手动指定模块文件"
    
    # TODO: 修复小火箭模块处理的兼容性问题
    # if [[ -d "$SHADOWROCKET_MODULE_DIR" ]]; then
    #     local processed=0
    #     for module in "$SHADOWROCKET_MODULE_DIR"/*.sgmodule "$SHADOWROCKET_MODULE_DIR"/*.module; do
    #         if [[ -f "$module" ]]; then
    #             local basename_module=$(basename "$module")
    #             
    #             # 跳过已同步的模块（以__开头或包含特定名称）
    #             if [[ "$basename_module" =~ ^__ ]] || [[ "$basename_module" =~ (Encrypted_DNS|URL_Rewrite|Firewall|General_Enhanced|Universal_Ad-Blocking) ]]; then
    #                 continue
    #             fi
    #             
    #             # 只处理包含广告拦截相关的模块
    #             if grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then
    #                 extract_rules_from_module "$module"
    #                 ((processed++))
    #             fi
    #         fi
    #     done
    #     log_info "小火箭模块处理完成，共处理 $processed 个模块"
    # fi
}

# 生成新的模块文件
generate_new_module() {
    log_section "生成新模块文件"
    
    local reject_count=$(wc -l < "$TEMP_RULES_REJECT" | tr -d ' ')
    local reject_drop_count=$(wc -l < "$TEMP_RULES_REJECT_DROP" | tr -d ' ')
    local reject_no_drop_count=$(wc -l < "$TEMP_RULES_REJECT_NO_DROP" | tr -d ' ')
    local url_rewrite_count=$(wc -l < "$TEMP_URL_REWRITE" | tr -d ' ')
    local host_count=$(wc -l < "$TEMP_HOST" | tr -d ' ')
    local total_rules=$((reject_count + reject_drop_count + reject_no_drop_count))
    
    log_info "最终规则统计:"
    echo "  - REJECT: $reject_count"
    echo "  - REJECT-DROP: $reject_drop_count"
    echo "  - REJECT-NO-DROP: $reject_no_drop_count"
    echo "  - URL Rewrite: $url_rewrite_count"
    echo "  - Host: $host_count"
    echo "  - 总计: $total_rules 条分流规则"
    
    # 备份原文件
    if [[ -f "$TARGET_MODULE" ]]; then
        cp "$TARGET_MODULE" "$TARGET_MODULE.backup.$(date +%Y%m%d_%H%M%S)"
        log_success "已备份原模块文件"
    fi
    
    # 生成新模块
    local current_date=$(date +%Y.%m.%d)
    
    cat > "$TARGET_MODULE" << EOF
#!name=🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style)
#!version=$current_date
#!desc=Modular ad-blocking with Host sinkhole + Online rulesets. Low-memory optimized. 🧩💾⚡
#!author=nyamiiko
#!homepage=https://github.com/nowaytouse/script_hub
#!category=『 🔝 Head Expanse › 首端扩域 』

[Rule]
# ═══════════════════════════════════════════════════════════════
# Universal Ad-Blocking (Merged - 235k+ rules, deduplicated)
# Updated: $(date +%Y-%m-%d) | REJECT rules are in AdBlock_Merged.list
# Note: All REJECT rules are merged into the big list file below
# ═══════════════════════════════════════════════════════════════
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge(Shadowkroket)/AdBlock_Merged.list,REJECT,extended-matching,pre-matching,"update-interval=86400",no-resolve

# ═══════════════════════════════════════════════════════════════
# Policy-Specific Rules (Upstream - Preserve DROP/NO-DROP)
# ═══════════════════════════════════════════════════════════════
RULE-SET,https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf,REJECT-NO-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve
RULE-SET,https://ruleset.skk.moe/List/non_ip/reject-drop.conf,REJECT-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve
RULE-SET,https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/BlockHttpDNS/BlockHttpDNS.list,REJECT-DROP,extended-matching,pre-matching,"update-interval=86400",no-resolve

EOF
    
    # 不再添加 REJECT 规则到模块中（已合并到 AdBlock_Merged.list）
    
    # 添加 REJECT-DROP 规则
    if [[ $reject_drop_count -gt 0 ]]; then
        cat >> "$TARGET_MODULE" << EOF

# ═══════════════════════════════════════════════════════════════
# REJECT-DROP Rules (${reject_drop_count} rules)
# ═══════════════════════════════════════════════════════════════
EOF
        sort -u "$TEMP_RULES_REJECT_DROP" >> "$TARGET_MODULE"
    fi
    
    # 添加 REJECT-NO-DROP 规则
    if [[ $reject_no_drop_count -gt 0 ]]; then
        cat >> "$TARGET_MODULE" << EOF

# ═══════════════════════════════════════════════════════════════
# REJECT-NO-DROP Rules (${reject_no_drop_count} rules)
# ═══════════════════════════════════════════════════════════════
EOF
        sort -u "$TEMP_RULES_REJECT_NO_DROP" >> "$TARGET_MODULE"
    fi
    
    # 添加 URL Rewrite 规则
    if [[ $url_rewrite_count -gt 0 ]]; then
        cat >> "$TARGET_MODULE" << EOF

[URL Rewrite]
# ═══════════════════════════════════════════════════════════════
# URL Rewrite Rules (${url_rewrite_count} rules)
# Auto-merged from multiple sources
# ═══════════════════════════════════════════════════════════════
EOF
        sort -u "$TEMP_URL_REWRITE" >> "$TARGET_MODULE"
    fi
    
    # 添加 Host 规则
    if [[ $host_count -gt 0 ]]; then
        cat >> "$TARGET_MODULE" << EOF

[Host]
# ═══════════════════════════════════════════════════════════════
# Ad/Tracking Domain Sinkhole (${host_count} domains)
# Resolve to 0.0.0.0
# ═══════════════════════════════════════════════════════════════
EOF
        sort -u "$TEMP_HOST" >> "$TARGET_MODULE"
    fi
    
    # 添加 MITM
    if [[ -s "$TEMP_MITM" ]]; then
        cat >> "$TARGET_MODULE" << EOF

[MITM]
EOF
        cat "$TEMP_MITM" >> "$TARGET_MODULE"
    fi
    
    log_success "新模块文件已生成"
}

# 合并规则到巨大的 AdBlock_Merged.list 文件
merge_to_adblock_list() {
    log_section "合并规则到 AdBlock_Merged.list"
    
    if [[ ! -f "$ADBLOCK_MERGED_LIST" ]]; then
        log_error "AdBlock_Merged.list 文件不存在: $ADBLOCK_MERGED_LIST"
        return
    fi
    
    # 备份原文件
    cp "$ADBLOCK_MERGED_LIST" "$ADBLOCK_MERGED_LIST.backup.$(date +%Y%m%d_%H%M%S)"
    log_success "已备份 AdBlock_Merged.list"
    
    # 提取现有规则（跳过注释和空行）
    log_info "提取现有规则..."
    grep -v "^#" "$ADBLOCK_MERGED_LIST" | grep -v "^$" > "$TEMP_DIR/existing_adblock_rules.tmp"
    local existing_count=$(wc -l < "$TEMP_DIR/existing_adblock_rules.tmp" | tr -d ' ')
    log_info "现有规则: $existing_count 条"
    
    # 准备新规则（只合并 REJECT 规则，不包括 REJECT-DROP 和 REJECT-NO-DROP）
    log_info "准备新规则..."
    > "$TEMP_DIR/new_adblock_rules.tmp"
    
    # 从临时文件中提取 REJECT 规则，转换为 .list 格式
    while IFS= read -r rule; do
        if [[ -z "$rule" ]] || [[ "$rule" =~ ^# ]]; then
            continue
        fi
        
        # 保留 extended-matching, pre-matching 参数（小火箭测试版即将支持）
        # 只移除多余空格
        rule=$(echo "$rule" | sed 's/  */ /g')
        
        # 检查是否已存在
        if ! grep -Fxq "$rule" "$TEMP_DIR/existing_adblock_rules.tmp"; then
            echo "$rule" >> "$TEMP_DIR/new_adblock_rules.tmp"
        fi
    done < "$TEMP_RULES_REJECT"
    
    local new_count=$(wc -l < "$TEMP_DIR/new_adblock_rules.tmp" | tr -d ' ')
    
    if [[ $new_count -eq 0 ]]; then
        log_info "没有新规则需要添加"
        return
    fi
    
    log_success "发现 $new_count 条新规则"
    
    # 合并规则
    log_info "合并规则到 AdBlock_Merged.list..."
    
    # 提取文件头部（注释部分）
    grep "^#" "$ADBLOCK_MERGED_LIST" > "$TEMP_DIR/adblock_header.tmp"
    
    # 更新统计信息
    local total_rules=$((existing_count + new_count))
    local current_date=$(date +"%Y-%m-%d %H:%M:%S UTC")
    
    # 生成新文件
    cat > "$ADBLOCK_MERGED_LIST" << EOF
# ═══════════════════════════════════════════════════════════════
# Ruleset: AdBlock_Merged
# Updated: $current_date
# Total Rules: $total_rules
# Generator: Ruleset Merger v2.4 + Module Merger
# ═══════════════════════════════════════════════════════════════
#
# Last Merge: Added $new_count rules from modules
#
EOF
    
    # 添加原有的 Sources 注释（如果有）
    grep "^# Sources:" "$TEMP_DIR/adblock_header.tmp" -A 100 | grep "^#   -" >> "$ADBLOCK_MERGED_LIST" || true
    
    echo "" >> "$ADBLOCK_MERGED_LIST"
    echo "# ═══════════════════════════════════════════════════════════════" >> "$ADBLOCK_MERGED_LIST"
    echo "# Rules from Modules (Added: $current_date)" >> "$ADBLOCK_MERGED_LIST"
    echo "# ═══════════════════════════════════════════════════════════════" >> "$ADBLOCK_MERGED_LIST"
    
    # 添加新规则（排序）
    sort -u "$TEMP_DIR/new_adblock_rules.tmp" >> "$ADBLOCK_MERGED_LIST"
    
    echo "" >> "$ADBLOCK_MERGED_LIST"
    echo "# ═══════════════════════════════════════════════════════════════" >> "$ADBLOCK_MERGED_LIST"
    echo "# Original Rules" >> "$ADBLOCK_MERGED_LIST"
    echo "# ═══════════════════════════════════════════════════════════════" >> "$ADBLOCK_MERGED_LIST"
    
    # 添加原有规则（排序）
    sort -u "$TEMP_DIR/existing_adblock_rules.tmp" >> "$ADBLOCK_MERGED_LIST"
    
    log_success "已合并到 AdBlock_Merged.list"
    log_info "总规则数: $existing_count + $new_count = $total_rules"
}

# 同步到小火箭
sync_to_shadowrocket() {
    log_section "同步到小火箭"
    
    if [[ ! -d "$SHADOWROCKET_MODULE_DIR" ]]; then
        log_warning "小火箭模块目录不存在，跳过同步"
        return
    fi
    
    log_info "调用小火箭同步脚本..."
    if [[ -f "$SCRIPT_DIR/sync_modules_to_shadowrocket.sh" ]]; then
        bash "$SCRIPT_DIR/sync_modules_to_shadowrocket.sh" "$TARGET_MODULE"
        log_success "已同步到小火箭"
    else
        log_warning "小火箭同步脚本不存在: $SCRIPT_DIR/sync_modules_to_shadowrocket.sh"
    fi
}

# 主函数
main() {
    log_section "广告拦截模块智能合并"
    echo "目标模块: $TARGET_MODULE"
    echo ""
    
    # 创建临时目录
    create_temp_dir
    
    # 提取现有规则
    extract_existing_rules
    
    # 扫描并合并所有模块
    scan_and_merge_modules
    
    # 生成新模块
    generate_new_module
    
    # 合并规则到 AdBlock_Merged.list
    merge_to_adblock_list
    
    # 同步到小火箭
    sync_to_shadowrocket
    
    # 清理临时目录
    cleanup_temp_dir
    
    log_section "完成"
    log_success "广告拦截模块合并完成！"
    echo ""
    echo "下一步："
    echo "1. 检查生成的模块: $TARGET_MODULE"
    echo "2. 如有问题，可恢复备份: $TARGET_MODULE.backup.*"
    echo "3. 提交到 Git: git add . && git commit -m 'feat: 合并广告拦截模块规则'"
}

# 执行主函数
main "$@"
