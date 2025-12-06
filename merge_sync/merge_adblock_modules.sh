#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# 广告拦截模块智能合并脚本 v3.0 (Ad-Blocking Module Intelligent Merger)
# ═══════════════════════════════════════════════════════════════════════════════
# 功能：
# 1. 从多个代理软件（Surge、Shadowrocket、Clash、Quantumult X等）提取广告拦截规则
# 2. 智能识别不同格式的规则（自动检测格式）
# 3. 智能分类：REJECT、REJECT-DROP、REJECT-NO-DROP
# 4. 增量合并，自动去重
# 5. URL Rewrite 规则单独处理
# 6. Host 规则单独处理
# 7. 支持命令行参数和交互式模式
# 8. 自动同步到小火箭模块
# ═══════════════════════════════════════════════════════════════════════════════
# 
# 使用方法：
#   1. 自动扫描模式（默认）:
#      ./merge_adblock_modules.sh
#
#   2. 指定单个模块:
#      ./merge_adblock_modules.sh /path/to/module.sgmodule
#
#   3. 指定多个模块:
#      ./merge_adblock_modules.sh module1.sgmodule module2.conf module3.yaml
#
#   4. 交互式选择模式:
#      ./merge_adblock_modules.sh --interactive
#
#   5. 仅合并到 AdBlock_Merged.list（不更新模块）:
#      ./merge_adblock_modules.sh --list-only
#
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
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SURGE_MODULE_DIR="$PROJECT_ROOT/module/surge(main)"

# ⚠️ 请修改以下路径为你的实际 Shadowrocket iCloud 目录（可选，如不需要同步到Shadowrocket可留空）
# 示例: /Users/YOUR_USERNAME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules
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

# 目标规则集
CHINA_DIRECT_LIST="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/ChinaDirect.list"

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

# 解析命令行参数
parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --interactive|-i)
                INTERACTIVE_MODE=true
                shift
                ;;
            --list-only|-l)
                LIST_ONLY_MODE=true
                shift
                ;;
            --auto-delete|-d)
                AUTO_DELETE=true
                shift
                ;;
            --auto|-a)
                AUTO_MODE=true
                shift
                ;;
            --help|-h)
                show_help
                exit 0
                ;;
            *)
                # 检查是否是文件路径
                if [[ -f "$1" ]]; then
                    SPECIFIED_MODULES+=("$1")
                else
                    log_error "文件不存在: $1"
                    exit 1
                fi
                shift
                ;;
        esac
    done
}

# 显示帮助信息
show_help() {
    cat << EOF
广告拦截模块智能合并脚本 v3.0

使用方法:
  $0 [选项] [模块文件...]

选项:
  -i, --interactive    交互式选择模块
  -l, --list-only      仅合并到规则列表（不更新模块）
  -d, --auto-delete    自动删除已处理的模块（需确认）
  -a, --auto           无人值守模式（跳过所有交互）
  -h, --help           显示此帮助信息

示例:
  # 自动扫描并合并所有模块
  $0

  # 指定单个模块
  $0 /path/to/module.sgmodule

  # 指定多个模块
  $0 module1.sgmodule module2.conf module3.yaml

  # 交互式选择模块
  $0 --interactive

  # 仅合并到规则列表
  $0 --list-only module.sgmodule

支持的格式:
  - Surge (.sgmodule, .conf)
  - Shadowrocket (.module, .conf)
  - Clash (.yaml, .yml)
  - Quantumult X (.conf, .snippet)
  - Loon (.plugin)

EOF
}

# 检测模块格式
detect_module_format() {
    local file="$1"
    local format="unknown"
    
    # 根据文件扩展名初步判断
    case "${file##*.}" in
        sgmodule)
            format="surge"
            ;;
        module)
            format="shadowrocket"
            ;;
        yaml|yml)
            format="clash"
            ;;
        plugin)
            format="loon"
            ;;
        conf)
            # 需要检查内容来区分 Surge/Shadowrocket/Quantumult X
            if grep -q "^\[Rule\]" "$file" 2>/dev/null; then
                format="surge"
            elif grep -q "^rules:" "$file" 2>/dev/null; then
                format="clash"
            elif grep -q "^hostname =" "$file" 2>/dev/null; then
                format="quantumult_x"
            fi
            ;;
        snippet)
            format="quantumult_x"
            ;;
    esac
    
    echo "$format"
}

# 从 Clash 格式提取规则
extract_rules_from_clash() {
    local file="$1"
    local new_rules=0
    
    log_info "处理 Clash 格式: $(basename "$file")"
    
    # 提取 rules 部分
    local in_rules=false
    while IFS= read -r line; do
        if [[ "$line" =~ ^rules: ]]; then
            in_rules=true
            continue
        elif [[ "$line" =~ ^[a-zA-Z-]+: ]] && [[ "$in_rules" == true ]]; then
            break
        fi
        
        if [[ "$in_rules" == true ]] && [[ "$line" =~ ^[[:space:]]*- ]]; then
            # 移除前导空格和 "- "
            local rule=$(echo "$line" | sed 's/^[[:space:]]*- //')
            
            # 转换 Clash 格式到 Surge 格式
            # Clash: DOMAIN-SUFFIX,example.com,REJECT
            # Surge: DOMAIN-SUFFIX,example.com,REJECT
            if [[ "$rule" =~ REJECT ]]; then
                # 标准化规则
                rule=$(echo "$rule" | sed 's/  */ /g')
                
                # 分类存储
                if echo "$rule" | grep -q ",REJECT$"; then
                    if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT"; then
                        echo "$rule" >> "$TEMP_RULES_REJECT"
                        ((new_rules++))
                    fi
                fi
            fi
        fi
    done < "$file"
    
    if [[ $new_rules -gt 0 ]]; then
        log_success "从 Clash 模块提取 $new_rules 条规则"
    fi
    
    echo $new_rules
}

# 从 Quantumult X 格式提取规则
extract_rules_from_quantumult_x() {
    local file="$1"
    local new_rules=0
    
    log_info "处理 Quantumult X 格式: $(basename "$file")"
    
    # Quantumult X 格式: host-suffix, example.com, reject
    while IFS= read -r line; do
        if [[ -z "$line" ]] || [[ "$line" =~ ^# ]] || [[ "$line" =~ ^\; ]]; then
            continue
        fi
        
        if [[ "$line" =~ reject ]]; then
            # 转换 Quantumult X 格式到 Surge 格式
            # QX: host-suffix, example.com, reject
            # Surge: DOMAIN-SUFFIX,example.com,REJECT
            
            local rule=$(echo "$line" | sed 's/host-suffix/DOMAIN-SUFFIX/g' | \
                         sed 's/host/DOMAIN/g' | \
                         sed 's/ip-cidr/IP-CIDR/g' | \
                         sed 's/user-agent/USER-AGENT/g' | \
                         sed 's/reject/REJECT/g' | \
                         sed 's/, /,/g' | \
                         sed 's/ ,/,/g')
            
            if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT"; then
                echo "$rule" >> "$TEMP_RULES_REJECT"
                ((new_rules++))
            fi
        fi
    done < "$file"
    
    if [[ $new_rules -gt 0 ]]; then
        log_success "从 Quantumult X 模块提取 $new_rules 条规则"
    fi
    
    echo $new_rules
}

# 从 Loon 格式提取规则
extract_rules_from_loon() {
    local file="$1"
    local new_rules=0
    
    log_info "处理 Loon 格式: $(basename "$file")"
    
    # Loon 格式与 Surge 类似
    local in_rule_section=false
    while IFS= read -r line; do
        if [[ "$line" =~ ^\[Rule\] ]]; then
            in_rule_section=true
            continue
        elif [[ "$line" =~ ^\[.*\] ]] && [[ "$in_rule_section" == true ]]; then
            break
        fi
        
        if [[ "$in_rule_section" == true ]] && [[ ! "$line" =~ ^# ]] && [[ ! -z "$line" ]]; then
            if [[ "$line" =~ REJECT ]]; then
                local rule=$(echo "$line" | sed 's/  */ /g')
                
                if echo "$rule" | grep -q ",REJECT,"; then
                    if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT"; then
                        echo "$rule" >> "$TEMP_RULES_REJECT"
                        ((new_rules++))
                    fi
                fi
            fi
        fi
    done < "$file"
    
    if [[ $new_rules -gt 0 ]]; then
        log_success "从 Loon 模块提取 $new_rules 条规则"
    fi
    
    echo $new_rules
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
    grep ",DIRECT" "$TEMP_DIR/existing_rules.tmp" 2>/dev/null > "$TEMP_RULES_DIRECT" || touch "$TEMP_RULES_DIRECT"
    
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

# 从指定模块提取规则（智能识别格式）
extract_rules_from_module() {
    local module_file="$1"
    local module_name=$(basename "$module_file")
    
    log_info "处理模块: $module_name"
    
    if [[ ! -f "$module_file" ]]; then
        log_warning "模块文件不存在: $module_file"
        return
    fi
    
    # 检查文件大小，跳过过大的文件（可能有问题）
    local file_size=$(stat -f%z "$module_file" 2>/dev/null || stat -c%s "$module_file" 2>/dev/null || echo "0")
    if [[ $file_size -gt 500000 ]]; then
        log_warning "文件过大，跳过: $module_name ($(($file_size / 1024))KB)"
        return
    fi
    
    # 检测模块格式
    local format=$(detect_module_format "$module_file")
    log_info "检测到格式: $format"
    
    local new_rules=0
    
    # 根据格式调用不同的提取函数
    case "$format" in
        clash)
            new_rules=$(extract_rules_from_clash "$module_file" | tail -1) || new_rules=0
            ;;
        quantumult_x)
            new_rules=$(extract_rules_from_quantumult_x "$module_file" | tail -1) || new_rules=0
            ;;
        loon)
            new_rules=$(extract_rules_from_loon "$module_file" | tail -1) || new_rules=0
            ;;
        surge|shadowrocket|unknown)
            # 使用原有的 Surge/Shadowrocket 提取逻辑
            new_rules=$(extract_rules_surge_format "$module_file" | tail -1) || new_rules=0
            ;;
    esac
    
    # 确保new_rules是数字
    [[ "$new_rules" =~ ^[0-9]+$ ]] || new_rules=0
    
    ((TOTAL_NEW_RULES += new_rules)) || true
    ((PROCESSED_MODULES++)) || true
}

# 从 Surge/Shadowrocket 格式提取规则（原有逻辑）
extract_rules_surge_format() {
    local module_file="$1"
    local new_rules=0
    
    # 提取 Rule 规则 - 使用 awk 更准确
    > "$TEMP_DIR/new_rules.tmp"
    awk '/^\[Rule\]/{flag=1;next}/^\[/{flag=0}flag && /^(DOMAIN|IP-CIDR|USER-AGENT|URL-REGEX|DEST-PORT|SRC-PORT|IP-ASN|GEOIP|PROCESS-NAME)/{print}' "$module_file" >> "$TEMP_DIR/new_rules.tmp" || true
    
    # 按策略分类并去重
    while IFS= read -r rule; do
        if [[ -z "$rule" ]] || [[ "$rule" =~ ^# ]]; then
            continue
        fi
        
        # 标准化规则（移除多余空格）
        rule=$(echo "$rule" | sed 's/  */ /g')
        
        if echo "$rule" | grep -q ",REJECT-DROP"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT_DROP"; then
                echo "$rule" >> "$TEMP_RULES_REJECT_DROP"
                ((new_rules++))
            fi
        elif echo "$rule" | grep -q ",REJECT-NO-DROP"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT_NO_DROP"; then
                echo "$rule" >> "$TEMP_RULES_REJECT_NO_DROP"
                ((new_rules++))
            fi
        elif echo "$rule" | grep -q ",REJECT"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_REJECT"; then
                echo "$rule" >> "$TEMP_RULES_REJECT"
                ((new_rules++))
            fi
        elif echo "$rule" | grep -q ",DIRECT"; then
            if ! grep -Fxq "$rule" "$TEMP_RULES_DIRECT"; then
                echo "$rule" >> "$TEMP_RULES_DIRECT"
                ((new_rules++))
                ((TOTAL_NEW_DIRECT++))
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
    
    echo $new_rules
}

# 交互式选择模块
interactive_select_modules() {
    log_section "交互式选择模块"
    
    local all_modules=()
    local module_formats=()
    
    # 扫描所有可能的模块目录
    log_info "扫描模块文件..."
    
    # Surge 模块
    if [[ -d "$SURGE_MODULE_DIR" ]]; then
        while IFS= read -r -d '' module; do
            if [[ "$module" != "$TARGET_MODULE" ]] && grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then
                all_modules+=("$module")
                module_formats+=("$(detect_module_format "$module")")
            fi
        done < <(find "$SURGE_MODULE_DIR" -type f \( -name "*.sgmodule" -o -name "*.conf" \) -print0)
    fi
    
    # Shadowrocket 模块
    if [[ -d "$SHADOWROCKET_MODULE_DIR" ]]; then
        while IFS= read -r -d '' module; do
            local basename_module=$(basename "$module")
            if [[ ! "$basename_module" =~ ^__ ]] && grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then
                all_modules+=("$module")
                module_formats+=("$(detect_module_format "$module")")
            fi
        done < <(find "$SHADOWROCKET_MODULE_DIR" -type f \( -name "*.module" -o -name "*.conf" -o -name "*.sgmodule" \) -print0 2>/dev/null)
    fi
    
    # 其他可能的目录（Clash、Quantumult X等）
    for dir in "$PROJECT_ROOT/module"/*/ "$PROJECT_ROOT/conf"*/*/; do
        if [[ -d "$dir" ]]; then
            while IFS= read -r -d '' module; do
                if grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then
                    all_modules+=("$module")
                    module_formats+=("$(detect_module_format "$module")")
                fi
            done < <(find "$dir" -type f \( -name "*.yaml" -o -name "*.yml" -o -name "*.conf" -o -name "*.plugin" -o -name "*.snippet" \) -print0 2>/dev/null)
        fi
    done
    
    if [[ ${#all_modules[@]} -eq 0 ]]; then
        log_error "未找到任何广告拦截模块"
        return
    fi
    
    log_success "找到 ${#all_modules[@]} 个模块"
    echo ""
    echo "请选择要合并的模块（输入序号，多个序号用空格分隔，输入 'all' 选择全部，输入 'q' 退出）:"
    echo ""
    
    # 显示模块列表
    for i in "${!all_modules[@]}"; do
        local module="${all_modules[$i]}"
        local format="${module_formats[$i]}"
        local module_name=$(basename "$module")
        local module_dir=$(dirname "$module" | sed "s|$PROJECT_ROOT/||")
        printf "%3d) [%-12s] %s\n" $((i+1)) "$format" "$module_dir/$module_name"
    done
    
    echo ""
    read -p "请输入选择: " selection
    
    if [[ "$selection" == "q" ]]; then
        log_info "用户取消操作"
        exit 0
    elif [[ "$selection" == "all" ]]; then
        SPECIFIED_MODULES=("${all_modules[@]}")
        log_success "已选择全部 ${#all_modules[@]} 个模块"
    else
        # 解析用户输入的序号
        for num in $selection; do
            if [[ "$num" =~ ^[0-9]+$ ]] && [[ $num -ge 1 ]] && [[ $num -le ${#all_modules[@]} ]]; then
                SPECIFIED_MODULES+=("${all_modules[$((num-1))]}")
            else
                log_warning "无效的序号: $num"
            fi
        done
        
        if [[ ${#SPECIFIED_MODULES[@]} -eq 0 ]]; then
            log_error "未选择任何有效的模块"
            exit 1
        fi
        
        log_success "已选择 ${#SPECIFIED_MODULES[@]} 个模块"
    fi
}

# 扫描并处理所有模块
scan_and_merge_modules() {
    log_section "扫描并合并模块"
    
    # 如果指定了模块，只处理指定的模块
    if [[ ${#SPECIFIED_MODULES[@]} -gt 0 ]]; then
        log_info "处理指定的 ${#SPECIFIED_MODULES[@]} 个模块..."
        for module in "${SPECIFIED_MODULES[@]}"; do
            extract_rules_from_module "$module"
        done
        return
    fi
    
    # 否则自动扫描
    log_info "自动扫描模式..."
    
    # 处理 Surge 模块
    log_info "扫描 Surge 模块目录..."
    if [[ -d "$SURGE_MODULE_DIR" ]]; then
        for module in "$SURGE_MODULE_DIR"/*.sgmodule "$SURGE_MODULE_DIR"/*.conf; do
            if [[ -f "$module" ]] && [[ "$module" != "$TARGET_MODULE" ]]; then
                # 只处理包含广告拦截相关的模块
                if grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then
                    extract_rules_from_module "$module"
                fi
            fi
        done
    fi
    
    # 处理小火箭模块
    if [[ -d "$SHADOWROCKET_MODULE_DIR" ]]; then
        log_info "扫描 Shadowrocket 模块目录..."
        for module in "$SHADOWROCKET_MODULE_DIR"/*.module "$SHADOWROCKET_MODULE_DIR"/*.conf "$SHADOWROCKET_MODULE_DIR"/*.sgmodule; do
            if [[ -f "$module" ]]; then
                local basename_module=$(basename "$module")
                
                # 跳过已同步的模块
                if [[ "$basename_module" =~ ^__ ]] || [[ "$basename_module" =~ (Encrypted_DNS|URL_Rewrite|Firewall|General_Enhanced|Universal_Ad-Blocking) ]]; then
                    continue
                fi
                
                # 只处理包含广告拦截相关的模块
                if grep -qi "ad\|reject\|block" "$module" 2>/dev/null; then
                    extract_rules_from_module "$module"
                fi
            fi
        done
    fi
    
    log_info "自动扫描完成，共处理 $PROCESSED_MODULES 个模块"
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
        if [[ "$AUTO_MODE" == "false" && "$CI" != "true" ]]; then
            cp "$TARGET_MODULE" "$TARGET_MODULE.backup.$(date +%Y%m%d_%H%M%S)"
            log_success "已备份原模块文件"
        else
            log_info "跳过备份 (自动模式/CI)"
        fi
    fi
    
    # 生成新模块
    local current_date=$(date +%Y.%m.%d)
    
    cat > "$TARGET_MODULE" << EOF
#!name=🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style)
#!version=$current_date
#!desc=Modular ad-blocking with Host sinkhole + Online rulesets. Low-memory optimized. 🧩💾⚡
#!author=YOUR_AUTHOR_NAME
#!homepage=https://github.com/YOUR_USERNAME/YOUR_REPO
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

# 合并DIRECT规则到ChinaDirect.list
merge_to_direct_list() {
    if [[ ! -s "$TEMP_RULES_DIRECT" ]]; then
        log_info "无DIRECT规则需要合并"
        return
    fi
    
    log_section "合并DIRECT规则到ChinaDirect.list"
    
    if [[ ! -f "$CHINA_DIRECT_LIST" ]]; then
        log_warning "ChinaDirect.list不存在，跳过"
        return
    fi
    
    cp "$CHINA_DIRECT_LIST" "$CHINA_DIRECT_LIST.backup.$(date +%Y%m%d_%H%M%S)"
    
    local existing=$(grep -v "^#" "$CHINA_DIRECT_LIST" | grep -v "^$" | wc -l | tr -d ' ')
    local new_count=0
    
    while IFS= read -r rule; do
        if ! grep -Fxq "$rule" "$CHINA_DIRECT_LIST"; then
            echo "$rule" >> "$CHINA_DIRECT_LIST"
            ((new_count++))
        fi
    done < "$TEMP_RULES_DIRECT"
    
    log_success "新增 $new_count 条DIRECT规则（总计: $((existing + new_count))）"
}

# 安全删除已处理模块
safe_delete_modules() {
    if [[ ${#MODULES_TO_DELETE[@]} -eq 0 ]]; then
        return
    fi
    
    log_section "删除已处理模块"
    
    echo "以下模块已提取规则，可以删除："
    for i in "${!MODULES_TO_DELETE[@]}"; do
        echo "  $((i+1)). ${MODULES_TO_DELETE[$i]}"
    done
    
    if [[ "$AUTO_DELETE" == false ]] && [[ "$AUTO_MODE" == false ]]; then
        read -p "确认删除？(y/N): " confirm
        if [[ "$confirm" != "y" ]] && [[ "$confirm" != "Y" ]]; then
            log_info "取消删除"
            return
        fi
    fi
    
    for module in "${MODULES_TO_DELETE[@]}"; do
        rm -f "$module"
        log_success "已删除: $(basename "$module")"
    done
}

# 同步SRS规则
sync_srs_rules() {
    log_section "同步SRS规则（Sing-box）"
    
    local srs_script="$PROJECT_ROOT/scripts/network/batch_convert_to_singbox.sh"
    
    if [[ ! -f "$srs_script" ]]; then
        log_warning "SRS转换脚本不存在: $srs_script"
        return
    fi
    
    log_info "调用SRS转换脚本..."
    bash "$srs_script"
    log_success "SRS规则已更新"
}

# 主函数
main() {
    # 解析命令行参数
    parse_arguments "$@"
    
    log_section "广告拦截模块智能合并 v3.0"
    
    if [[ "$LIST_ONLY_MODE" == false ]]; then
        echo "目标模块: $TARGET_MODULE"
    fi
    echo "目标规则列表: $ADBLOCK_MERGED_LIST"
    echo ""
    
    # 创建临时目录
    create_temp_dir
    
    # 交互式选择模式
    if [[ "$INTERACTIVE_MODE" == true ]]; then
        interactive_select_modules
    fi
    
    # 提取现有规则
    extract_existing_rules
    
    # 扫描并合并所有模块
    scan_and_merge_modules
    
    # 显示统计信息
    log_section "统计信息"
    log_info "处理模块数: $PROCESSED_MODULES"
    log_info "新增规则数: $TOTAL_NEW_RULES"
    
    # 生成新模块（除非是 list-only 模式）
    if [[ "$LIST_ONLY_MODE" == false ]]; then
        generate_new_module
    else
        log_info "跳过模块生成（list-only 模式）"
    fi
    
    # 合并规则到 AdBlock_Merged.list
    merge_to_adblock_list
    
    # 合并DIRECT规则
    merge_to_direct_list
    
    # 同步SRS规则
    sync_srs_rules
    
    # 安全删除已处理模块
    if [[ "$AUTO_DELETE" == true ]] || [[ ${#SPECIFIED_MODULES[@]} -gt 0 ]]; then
        MODULES_TO_DELETE=("${SPECIFIED_MODULES[@]}")
        safe_delete_modules
    fi
    
    # 清理临时目录
    cleanup_temp_dir
    
    log_section "完成"
    log_success "广告拦截模块合并完成！"
    echo ""
    echo "📊 最终统计:"
    echo "  - 处理模块: $PROCESSED_MODULES 个"
    echo "  - 新增REJECT规则: $TOTAL_NEW_RULES 条"
    echo "  - 新增DIRECT规则: $TOTAL_NEW_DIRECT 条"
    echo ""
    
    if [[ "$LIST_ONLY_MODE" == false ]]; then
        echo "📝 下一步："
        echo "  1. 检查生成的模块: $TARGET_MODULE"
        echo "  2. 检查规则列表: $ADBLOCK_MERGED_LIST"
        echo "  3. 如有问题，可恢复备份: *.backup.*"
        echo "  4. 提交到 Git: git add . && git commit -m 'feat: 合并广告拦截模块规则'"
    else
        echo "📝 下一步："
        echo "  1. 检查规则列表: $ADBLOCK_MERGED_LIST"
        echo "  2. 如有问题，可恢复备份: $ADBLOCK_MERGED_LIST.backup.*"
        echo "  3. 提交到 Git: git add . && git commit -m 'feat: 更新广告拦截规则列表'"
    fi
}

# 执行主函数
main "$@"
