#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# 一键规则同步脚本 (All-in-One Rule Sync) v2.0
# ═══════════════════════════════════════════════════════════════════════════════
# 功能：按最佳顺序执行所有规则处理任务
# 1. 汲取远程sgmodule规则
# 2. 提取模块规则（REJECT + DIRECT）
# 3. 去重合并到规则集
# 4. 转换SRS规则（Sing-box）
# 5. 同步到iCloud
# 6. Git提交推送
#
# 用法：
#   ./sync_all_rules.sh           # 交互模式
#   ./sync_all_rules.sh --auto    # 无人值守模式（全自动）
#   ./sync_all_rules.sh --no-git  # 跳过Git操作
#   ./sync_all_rules.sh --help    # 显示帮助
# ═══════════════════════════════════════════════════════════════════════════════

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

# 路径配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SOURCES_DIR="$PROJECT_ROOT/ruleset/Sources"
SGMODULE_SOURCES="$SOURCES_DIR/AdBlock_sgmodule_sources.txt"

# 模式配置
AUTO_MODE=false
NO_GIT=false
NO_ICLOUD=false

# 日志函数
log_step() {
    echo ""
    echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${MAGENTA}  步骤 $1: $2${NC}"
    echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

log_info() { echo -e "${CYAN}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }

# 显示帮助
show_help() {
    cat << EOF
🚀 一键规则同步脚本 v2.0

用法: $(basename "$0") [选项]

选项:
    --auto, -a      无人值守模式（全自动，无交互）
    --no-git        跳过Git操作
    --no-icloud     跳过iCloud同步
    --help, -h      显示此帮助信息

示例:
    $(basename "$0")              # 交互模式
    $(basename "$0") --auto       # 全自动模式（适合cron）
    $(basename "$0") --auto --no-git  # 自动但不提交Git

功能:
    1. 汲取远程sgmodule规则（从AdBlock_sgmodule_sources.txt）
    2. 提取本地模块规则（REJECT + DIRECT）
    3. 去重合并到规则集
    4. 转换SRS规则（Sing-box）
    5. 同步到iCloud
    6. Git提交推送
EOF
    exit 0
}

# 解析参数
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --auto|-a) AUTO_MODE=true; shift ;;
            --no-git) NO_GIT=true; shift ;;
            --no-icloud) NO_ICLOUD=true; shift ;;
            --help|-h) show_help ;;
            *) shift ;;
        esac
    done
}

# 确认函数（自动模式下自动确认）
confirm() {
    local prompt="$1"
    local default="${2:-n}"
    
    if $AUTO_MODE; then
        return 0  # 自动模式下总是确认
    fi
    
    read -p "$prompt (y/N): " response
    [[ "$response" == "y" ]] || [[ "$response" == "Y" ]]
}

# 显示欢迎信息
show_welcome() {
    echo -e "${BLUE}"
    cat << "EOF"
╔═══════════════════════════════════════════════════════════════╗
║        🚀 一键规则同步脚本 v2.0                               ║
║        All-in-One Rule Synchronization                        ║
╚═══════════════════════════════════════════════════════════════╝
EOF
    echo -e "${NC}"
    
    if $AUTO_MODE; then
        echo -e "${GREEN}🤖 无人值守模式${NC}"
    else
        echo "执行顺序："
        echo "  1️⃣  汲取远程sgmodule规则"
        echo "  2️⃣  提取本地模块规则"
        echo "  3️⃣  转换SRS规则"
        echo "  4️⃣  同步到iCloud"
        echo "  5️⃣  Git提交推送"
    fi
    echo ""
}

# 步骤0: 汲取远程sgmodule规则（仅REJECT策略）
step_fetch_sgmodules() {
    log_step "0" "汲取远程sgmodule规则（仅REJECT策略）"
    
    if [[ ! -f "$SGMODULE_SOURCES" ]]; then
        log_warning "sgmodule源文件不存在: $SGMODULE_SOURCES"
        log_info "跳过远程规则汲取"
        return 0
    fi
    
    local temp_dir=$(mktemp -d)
    local rules_file="$temp_dir/extracted_rules.txt"
    local count=0
    local total_rules=0
    local skipped_rules=0
    
    log_info "读取sgmodule源列表..."
    
    while IFS= read -r line || [[ -n "$line" ]]; do
        line=$(echo "$line" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        [[ -z "$line" ]] || [[ "$line" =~ ^# ]] && continue
        
        local url="$line"
        ((count++))
        
        log_info "[$count] 下载: $(basename "$url")"
        
        # 下载sgmodule
        local module_content
        module_content=$(curl -sL --connect-timeout 10 --max-time 30 "$url" 2>/dev/null) || {
            log_warning "下载失败: $url"
            continue
        }
        
        # 提取Rule部分的规则（仅REJECT策略）
        local in_rule_section=false
        local module_rules=0
        local module_skipped=0
        
        while IFS= read -r module_line; do
            if [[ "$module_line" =~ ^\[Rule\] ]]; then
                in_rule_section=true
                continue
            elif [[ "$module_line" =~ ^\[ ]]; then
                in_rule_section=false
                continue
            fi
            
            if $in_rule_section && [[ -n "$module_line" ]] && [[ ! "$module_line" =~ ^# ]]; then
                # 只提取包含REJECT策略的规则（REJECT, REJECT-DROP, REJECT-NO-DROP）
                if [[ "$module_line" =~ ,REJECT ]]; then
                    # 提取规则类型和值（保留完整规则格式）
                    local rule_type rule_value
                    rule_type=$(echo "$module_line" | cut -d',' -f1)
                    rule_value=$(echo "$module_line" | cut -d',' -f2)
                    
                    if [[ -n "$rule_type" ]] && [[ -n "$rule_value" ]]; then
                        # 输出格式: DOMAIN-SUFFIX,example.com,REJECT
                        echo "${rule_type},${rule_value},REJECT" >> "$rules_file"
                        ((module_rules++))
                        ((total_rules++))
                    fi
                else
                    # 跳过非REJECT规则（如DIRECT, PROXY等）
                    ((module_skipped++))
                    ((skipped_rules++))
                fi
            fi
        done <<< "$module_content"
        
        if [[ $module_rules -gt 0 ]]; then
            log_success "  提取 $module_rules 条REJECT规则（跳过 $module_skipped 条非REJECT规则）"
        else
            log_info "  未发现REJECT规则（跳过 $module_skipped 条非REJECT规则）"
        fi
        
    done < "$SGMODULE_SOURCES"
    
    if [[ -f "$rules_file" ]] && [[ $total_rules -gt 0 ]]; then
        # 去重并追加到AdBlock规则
        local unique_rules=$(sort -u "$rules_file" | wc -l | tr -d ' ')
        log_success "汲取完成: $count 个模块"
        log_success "  • REJECT规则: $unique_rules 条（去重后）"
        log_info "  • 跳过非REJECT: $skipped_rules 条"
        
        # 保存到临时文件供后续合并使用
        cp "$rules_file" "$PROJECT_ROOT/.temp_sgmodule_rules.txt"
    else
        log_info "没有汲取到REJECT规则"
    fi
    
    rm -rf "$temp_dir"
}

# 步骤1: 提取并合并模块规则
step_merge_rules() {
    log_step "1" "提取并合并模块规则"
    
    if [[ ! -f "$SCRIPT_DIR/merge_adblock_modules.sh" ]]; then
        log_error "合并脚本不存在: merge_adblock_modules.sh"
        return 1
    fi
    
    log_info "调用规则合并脚本..."
    bash "$SCRIPT_DIR/merge_adblock_modules.sh" --auto
    
    log_success "规则提取和合并完成"
}

# 步骤2: 转换SRS规则
step_convert_srs() {
    log_step "2" "转换SRS规则（Sing-box）"
    
    local srs_script="$PROJECT_ROOT/scripts/network/batch_convert_to_singbox.sh"
    
    if [[ ! -f "$srs_script" ]]; then
        log_warning "SRS转换脚本不存在"
        return 0
    fi
    
    log_info "转换Surge规则到SRS格式..."
    bash "$srs_script"
    
    log_success "SRS规则转换完成"
}

# 步骤3: 同步到iCloud
step_sync_icloud() {
    log_step "3" "同步到iCloud"
    
    if $NO_ICLOUD; then
        log_info "跳过iCloud同步（--no-icloud）"
        return 0
    fi
    
    if [[ ! -f "$SCRIPT_DIR/sync_modules_to_icloud.sh" ]]; then
        log_warning "iCloud同步脚本不存在"
        return 0
    fi
    
    if ! $AUTO_MODE && ! confirm "是否同步到iCloud?"; then
        log_info "跳过iCloud同步"
        return 0
    fi
    
    log_info "同步到iCloud..."
    bash "$SCRIPT_DIR/sync_modules_to_icloud.sh"
    
    log_success "iCloud同步完成"
}

# 步骤4: Git提交
step_git_commit() {
    log_step "4" "Git提交"
    
    if $NO_GIT; then
        log_info "跳过Git操作（--no-git）"
        return 0
    fi
    
    if ! $AUTO_MODE && ! confirm "是否提交到Git?"; then
        log_info "跳过Git提交"
        return 0
    fi
    
    log_info "提交更改到Git..."
    
    # 添加文件
    git add ruleset/ module/ 2>/dev/null || true
    git add -u 2>/dev/null || true
    
    # 检查是否有更改
    if git diff --cached --quiet; then
        log_info "没有需要提交的更改"
        return 0
    fi
    
    local commit_msg="feat: 更新规则集 $(date +%Y-%m-%d)"
    git commit -m "$commit_msg"
    
    if $AUTO_MODE || confirm "是否推送到远程?"; then
        git push && log_success "已推送到远程" || log_warning "推送失败"
    fi
    
    log_success "Git提交完成"
}

# 显示统计信息
show_statistics() {
    log_step "✓" "完成统计"
    
    local adblock_list="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock_Merged.list"
    local direct_list="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/ChinaDirect.list"
    local srs_dir="$PROJECT_ROOT/ruleset/SingBox"
    
    echo -e "${GREEN}📊 规则集统计:${NC}"
    
    if [[ -f "$adblock_list" ]]; then
        local adblock_count=$(grep -cv "^#\|^$" "$adblock_list" 2>/dev/null || echo "0")
        echo "  • AdBlock_Merged.list: $adblock_count 条规则"
    fi
    
    if [[ -f "$direct_list" ]]; then
        local direct_count=$(grep -cv "^#\|^$" "$direct_list" 2>/dev/null || echo "0")
        echo "  • ChinaDirect.list: $direct_count 条规则"
    fi
    
    if [[ -d "$srs_dir" ]]; then
        local srs_count=$(find "$srs_dir" -name "*.srs" 2>/dev/null | wc -l | tr -d ' ')
        echo "  • SRS规则文件: $srs_count 个"
    fi
    
    echo ""
    echo -e "${GREEN}✨ 所有任务已完成！${NC}"
}

# 错误处理
handle_error() {
    log_error "执行过程中发生错误！"
    echo ""
    echo "可能的原因："
    echo "  • 网络连接问题"
    echo "  • 脚本文件不存在或无执行权限"
    echo "  • 规则集文件路径错误"
    echo ""
    echo "请检查错误信息并重试"
    exit 1
}

# 清理临时文件
cleanup() {
    rm -f "$PROJECT_ROOT/.temp_sgmodule_rules.txt" 2>/dev/null || true
}

# 主函数
main() {
    # 解析参数
    parse_args "$@"
    
    # 设置错误处理和清理
    trap handle_error ERR
    trap cleanup EXIT
    
    # 显示欢迎信息
    show_welcome
    
    # 交互模式下等待确认
    if ! $AUTO_MODE; then
        read -p "按Enter键开始，或Ctrl+C取消... " _
    fi
    
    # 记录开始时间
    local start_time=$(date +%s)
    
    # 执行步骤
    step_fetch_sgmodules
    step_merge_rules
    step_convert_srs
    step_sync_icloud
    step_git_commit
    
    # 计算耗时
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    # 显示统计
    show_statistics
    
    echo -e "${CYAN}⏱️  总耗时: ${duration}秒${NC}"
}

# 执行主函数
main "$@"
