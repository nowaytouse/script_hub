#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# 一键规则同步脚本 (All-in-One Rule Sync)
# ═══════════════════════════════════════════════════════════════════════════════
# 功能：按最佳顺序执行所有规则处理任务
# 1. 提取模块规则（REJECT + DIRECT）
# 2. 去重合并到规则集
# 3. 转换SRS规则（Sing-box）
# 4. 同步到iCloud（可选）
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

# 日志函数
log_step() {
    echo ""
    echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${MAGENTA}  步骤 $1: $2${NC}"
    echo -e "${MAGENTA}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

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

# 显示欢迎信息
show_welcome() {
    clear
    echo -e "${BLUE}"
    cat << "EOF"
╔═══════════════════════════════════════════════════════════════╗
║                                                               ║
║        🚀 一键规则同步脚本 v1.0                               ║
║        All-in-One Rule Synchronization                        ║
║                                                               ║
╚═══════════════════════════════════════════════════════════════╝
EOF
    echo -e "${NC}"
    echo "本脚本将按以下顺序执行："
    echo "  1️⃣  提取模块规则（REJECT + DIRECT）"
    echo "  2️⃣  去重合并到规则集"
    echo "  3️⃣  转换SRS规则（Sing-box）"
    echo "  4️⃣  同步到iCloud（可选）"
    echo ""
}

# 步骤1: 提取并合并模块规则
step_merge_rules() {
    log_step "1" "提取并合并模块规则"
    
    if [[ ! -f "$SCRIPT_DIR/merge_adblock_modules.sh" ]]; then
        log_error "合并脚本不存在: merge_adblock_modules.sh"
        exit 1
    fi
    
    log_info "调用规则合并脚本..."
    bash "$SCRIPT_DIR/merge_adblock_modules.sh" "$@"
    
    log_success "规则提取和合并完成"
}

# 步骤2: 转换SRS规则
step_convert_srs() {
    log_step "2" "转换SRS规则（Sing-box）"
    
    local srs_script="$PROJECT_ROOT/scripts/network/batch_convert_to_singbox.sh"
    
    if [[ ! -f "$srs_script" ]]; then
        log_warning "SRS转换脚本不存在: $srs_script"
        log_info "跳过SRS转换"
        return
    fi
    
    log_info "转换Surge规则到SRS格式..."
    bash "$srs_script"
    
    log_success "SRS规则转换完成"
}

# 步骤3: 同步到iCloud
step_sync_icloud() {
    log_step "3" "同步到iCloud（可选）"
    
    if [[ ! -f "$SCRIPT_DIR/sync_modules_to_icloud.sh" ]]; then
        log_warning "iCloud同步脚本不存在"
        return
    fi
    
    read -p "是否同步到iCloud? (y/N): " sync_confirm
    if [[ "$sync_confirm" != "y" ]] && [[ "$sync_confirm" != "Y" ]]; then
        log_info "跳过iCloud同步"
        return
    fi
    
    log_info "同步到iCloud..."
    bash "$SCRIPT_DIR/sync_modules_to_icloud.sh"
    
    log_success "iCloud同步完成"
}

# 显示统计信息
show_statistics() {
    log_step "✓" "完成统计"
    
    local adblock_list="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock_Merged.list"
    local direct_list="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/ChinaDirect.list"
    local srs_dir="$PROJECT_ROOT/ruleset/SingBox"
    
    echo -e "${GREEN}📊 规则集统计:${NC}"
    
    if [[ -f "$adblock_list" ]]; then
        local adblock_count=$(grep -v "^#" "$adblock_list" | grep -v "^$" | wc -l | tr -d ' ')
        echo "  • AdBlock_Merged.list: $adblock_count 条规则"
    fi
    
    if [[ -f "$direct_list" ]]; then
        local direct_count=$(grep -v "^#" "$direct_list" | grep -v "^$" | wc -l | tr -d ' ')
        echo "  • ChinaDirect.list: $direct_count 条规则"
    fi
    
    if [[ -d "$srs_dir" ]]; then
        local srs_count=$(find "$srs_dir" -name "*.srs" -o -name "*.json" | wc -l | tr -d ' ')
        echo "  • SRS规则文件: $srs_count 个"
    fi
    
    echo ""
    echo -e "${GREEN}✨ 所有任务已完成！${NC}"
}

# 步骤4: Git提交
step_git_commit() {
    log_step "4" "Git提交"
    
    read -p "是否提交到Git? (y/N): " git_confirm
    if [[ "$git_confirm" != "y" ]] && [[ "$git_confirm" != "Y" ]]; then
        log_info "跳过Git提交"
        return
    fi
    
    log_info "提交更改到Git..."
    git add ruleset/ module/ .temp_adblock_merge 2>/dev/null || true
    git add -u
    
    local commit_msg="feat: 更新规则集 $(date +%Y-%m-%d)"
    git commit -m "$commit_msg" || {
        log_warning "没有需要提交的更改"
        return
    }
    
    read -p "是否推送到远程? (y/N): " push_confirm
    if [[ "$push_confirm" == "y" ]] || [[ "$push_confirm" == "Y" ]]; then
        git push && log_success "已推送到远程" || log_warning "推送失败"
    fi
    
    log_success "Git提交完成"
}

# 错误处理
handle_error() {
    log_error "执行过程中发生错误！"
    echo ""
    echo "可能的原因："
    echo "  • 脚本文件不存在或无执行权限"
    echo "  • 规则集文件路径错误"
    echo "  • 磁盘空间不足"
    echo ""
    echo "请检查错误信息并重试"
    exit 1
}

# 主函数
main() {
    # 设置错误处理
    trap handle_error ERR
    
    # 显示欢迎信息
    show_welcome
    
    # 确认执行
    read -p "按Enter键开始，或Ctrl+C取消... " confirm
    
    # 记录开始时间
    local start_time=$(date +%s)
    
    # 执行步骤
    step_merge_rules "$@"
    step_convert_srs
    step_sync_icloud
    step_git_commit
    
    # 计算耗时
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    # 显示统计
    show_statistics
    
    echo -e "${CYAN}⏱️  总耗时: ${duration}秒${NC}"
    
    # 显示下一步
    show_next_steps
}

# 执行主函数
main "$@"
