#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# 一键规则同步脚本 (All-in-One Rule Sync) v3.0
# ═══════════════════════════════════════════════════════════════════════════════
# 功能：按最佳顺序执行所有规则处理任务
# 1. 汲取远程sgmodule规则（REJECT + DIRECT）
# 2. 吐出剩余内容为精简模块
# 3. 提取本地模块规则
# 4. 去重合并到规则集
# 5. 转换SRS规则（Sing-box）
# 6. 同步到iCloud
# 7. Git提交推送
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
SURGE_MODULE_DIR="$PROJECT_ROOT/module/surge(main)"
ADBLOCK_MERGED_LIST="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock.list"
CHINA_DIRECT_LIST="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/ChinaDirect.list"

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
🚀 一键规则同步脚本 v3.0

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
    1. 汲取远程sgmodule规则（REJECT + DIRECT）
       - REJECT规则 → 合并到 AdBlock.list
       - DIRECT规则 → 合并到 ChinaDirect.list
    2. 吐出精简模块（删除已吸取规则后的剩余内容）
       - 保留 URL Rewrite、MITM、Script 等
       - 输出到 module/surge(main)/__Extracted_*.sgmodule
    3. 提取本地模块规则
    4. 去重合并到规则集
    5. 转换SRS规则（Sing-box）
    6. 同步到iCloud
    7. Git提交推送

sgmodule源文件: ruleset/Sources/AdBlock_sgmodule_sources.txt
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
║        🚀 一键规则同步脚本 v3.0                               ║
║        All-in-One Rule Synchronization                        ║
╚═══════════════════════════════════════════════════════════════╝
EOF
    echo -e "${NC}"
    
    if $AUTO_MODE; then
        echo -e "${GREEN}🤖 无人值守模式${NC}"
    else
        echo "执行顺序："
        echo "  0️⃣  汲取远程sgmodule规则（REJECT+DIRECT）+ 吐出精简模块"
        echo "  1️⃣  提取本地模块规则"
        echo "  2️⃣  转换SRS规则"
        echo "  3️⃣  同步到iCloud"
        echo "  4️⃣  Git提交推送"
    fi
    echo ""
}

# 步骤0: 汲取远程sgmodule规则（REJECT + DIRECT）并吐出精简模块
step_fetch_sgmodules() {
    log_step "0" "汲取远程sgmodule规则 + 吐出精简模块"
    
    if [[ ! -f "$SGMODULE_SOURCES" ]]; then
        log_warning "sgmodule源文件不存在: $SGMODULE_SOURCES"
        log_info "跳过远程规则汲取"
        return 0
    fi
    
    local temp_dir=$(mktemp -d)
    local reject_rules_file="$temp_dir/reject_rules.txt"
    local direct_rules_file="$temp_dir/direct_rules.txt"
    local count=0
    local total_reject=0
    local total_direct=0
    
    log_info "读取sgmodule源列表..."
    
    while IFS= read -r line || [[ -n "$line" ]]; do
        line=$(echo "$line" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        [[ -z "$line" ]] || [[ "$line" =~ ^# ]] && continue
        
        local url="$line"
        ((count++))
        
        local module_basename=$(basename "$url" .sgmodule)
        log_info "[$count] 下载: $module_basename"
        
        # 下载sgmodule
        local module_content
        module_content=$(curl -sL --connect-timeout 10 --max-time 30 "$url" 2>/dev/null) || {
            log_warning "下载失败: $url"
            continue
        }
        
        # 临时文件存储各部分
        local temp_header="$temp_dir/header_${count}.txt"
        local temp_rule="$temp_dir/rule_${count}.txt"
        local temp_url_rewrite="$temp_dir/url_rewrite_${count}.txt"
        local temp_mitm="$temp_dir/mitm_${count}.txt"
        local temp_script="$temp_dir/script_${count}.txt"
        local temp_other="$temp_dir/other_${count}.txt"
        
        # 解析模块内容
        local current_section="header"
        local module_reject=0
        local module_direct=0
        local module_other_rules=0
        
        while IFS= read -r module_line; do
            # 检测section
            if [[ "$module_line" =~ ^\[Rule\] ]]; then
                current_section="rule"
                continue
            elif [[ "$module_line" =~ ^\[URL\ Rewrite\] ]]; then
                current_section="url_rewrite"
                echo "[URL Rewrite]" >> "$temp_url_rewrite"
                continue
            elif [[ "$module_line" =~ ^\[MITM\] ]]; then
                current_section="mitm"
                echo "[MITM]" >> "$temp_mitm"
                continue
            elif [[ "$module_line" =~ ^\[Script\] ]]; then
                current_section="script"
                echo "[Script]" >> "$temp_script"
                continue
            elif [[ "$module_line" =~ ^\[.*\] ]]; then
                current_section="other"
                echo "$module_line" >> "$temp_other"
                continue
            fi
            
            case "$current_section" in
                header)
                    echo "$module_line" >> "$temp_header"
                    ;;
                rule)
                    if [[ -n "$module_line" ]] && [[ ! "$module_line" =~ ^# ]]; then
                        # 提取REJECT规则
                        if [[ "$module_line" =~ ,REJECT ]]; then
                            local rule_type rule_value
                            rule_type=$(echo "$module_line" | cut -d',' -f1)
                            rule_value=$(echo "$module_line" | cut -d',' -f2)
                            if [[ -n "$rule_type" ]] && [[ -n "$rule_value" ]]; then
                                echo "${rule_type},${rule_value},REJECT" >> "$reject_rules_file"
                                ((module_reject++))
                                ((total_reject++))
                            fi
                        # 提取DIRECT规则
                        elif [[ "$module_line" =~ ,DIRECT ]]; then
                            local rule_type rule_value
                            rule_type=$(echo "$module_line" | cut -d',' -f1)
                            rule_value=$(echo "$module_line" | cut -d',' -f2)
                            if [[ -n "$rule_type" ]] && [[ -n "$rule_value" ]]; then
                                echo "${rule_type},${rule_value},DIRECT" >> "$direct_rules_file"
                                ((module_direct++))
                                ((total_direct++))
                            fi
                        else
                            # 保留其他规则（如PROXY等）到精简模块
                            echo "$module_line" >> "$temp_rule"
                            ((module_other_rules++))
                        fi
                    elif [[ "$module_line" =~ ^# ]]; then
                        # 保留注释
                        echo "$module_line" >> "$temp_rule"
                    fi
                    ;;
                url_rewrite)
                    echo "$module_line" >> "$temp_url_rewrite"
                    ;;
                mitm)
                    echo "$module_line" >> "$temp_mitm"
                    ;;
                script)
                    echo "$module_line" >> "$temp_script"
                    ;;
                other)
                    echo "$module_line" >> "$temp_other"
                    ;;
            esac
        done <<< "$module_content"
        
        log_success "  吸取: REJECT=$module_reject, DIRECT=$module_direct"
        
        # 生成精简模块（吐出剩余内容）
        local output_module="$SURGE_MODULE_DIR/__Extracted_${module_basename}.sgmodule"
        
        # 检查是否有剩余内容需要吐出
        local has_remaining=false
        [[ -s "$temp_rule" ]] && has_remaining=true
        [[ -s "$temp_url_rewrite" ]] && has_remaining=true
        [[ -s "$temp_mitm" ]] && has_remaining=true
        [[ -s "$temp_script" ]] && has_remaining=true
        
        if $has_remaining; then
            log_info "  吐出精简模块: $(basename "$output_module")"
            
            # 写入头部（修改描述）
            {
                if [[ -s "$temp_header" ]]; then
                    # 修改desc行，标注已提取规则
                    sed "s/^#!desc=.*/#!desc=[已提取REJECT=${module_reject}+DIRECT=${module_direct}] 原模块精简版/" "$temp_header"
                else
                    echo "#!name=__Extracted_${module_basename}"
                    echo "#!desc=[已提取REJECT=${module_reject}+DIRECT=${module_direct}] 原模块精简版"
                fi
                
                echo ""
                echo "# ═══════════════════════════════════════════════════════════════"
                echo "# 此模块由 sync_all_rules.sh 自动生成"
                echo "# 原始URL: $url"
                echo "# 已提取: REJECT规则 $module_reject 条, DIRECT规则 $module_direct 条"
                echo "# 生成时间: $(date '+%Y-%m-%d %H:%M:%S')"
                echo "# ═══════════════════════════════════════════════════════════════"
                echo ""
                
                # 写入剩余Rule（如果有）
                if [[ -s "$temp_rule" ]] && grep -qv "^#\|^$" "$temp_rule" 2>/dev/null; then
                    echo "[Rule]"
                    cat "$temp_rule"
                    echo ""
                fi
                
                # 写入URL Rewrite
                [[ -s "$temp_url_rewrite" ]] && cat "$temp_url_rewrite" && echo ""
                
                # 写入MITM
                [[ -s "$temp_mitm" ]] && cat "$temp_mitm" && echo ""
                
                # 写入Script
                [[ -s "$temp_script" ]] && cat "$temp_script" && echo ""
                
                # 写入其他section
                [[ -s "$temp_other" ]] && cat "$temp_other"
                
            } > "$output_module"
            
            log_success "  已生成: $output_module"
        else
            log_info "  无剩余内容需要吐出"
        fi
        
    done < "$SGMODULE_SOURCES"
    
    # 汇总统计
    echo ""
    log_success "═══ 汲取汇总 ═══"
    log_success "处理模块: $count 个"
    
    # 处理REJECT规则
    if [[ -f "$reject_rules_file" ]] && [[ -s "$reject_rules_file" ]]; then
        local unique_reject=$(sort -u "$reject_rules_file" | wc -l | tr -d ' ')
        log_success "REJECT规则: $unique_reject 条（去重后）"
        
        # 合并到AdBlock.list
        if [[ -f "$ADBLOCK_MERGED_LIST" ]]; then
            log_info "合并REJECT规则到 AdBlock.list..."
            local before_count=$(grep -cv "^#\|^$" "$ADBLOCK_MERGED_LIST" 2>/dev/null || echo "0")
            
            # 追加新规则并去重
            cat "$reject_rules_file" >> "$ADBLOCK_MERGED_LIST"
            
            # 提取规则部分，去重，重新生成
            local temp_merged="$temp_dir/merged_adblock.txt"
            grep -v "^#\|^$" "$ADBLOCK_MERGED_LIST" | sort -u > "$temp_merged"
            local after_count=$(wc -l < "$temp_merged" | tr -d ' ')
            local added=$((after_count - before_count))
            
            # 重新生成文件（保留头部）
            {
                head -30 "$ADBLOCK_MERGED_LIST" | grep "^#"
                echo ""
                cat "$temp_merged"
            } > "$ADBLOCK_MERGED_LIST.new"
            mv "$ADBLOCK_MERGED_LIST.new" "$ADBLOCK_MERGED_LIST"
            
            log_success "  新增 $added 条规则到 AdBlock.list"
        fi
        
        cp "$reject_rules_file" "$PROJECT_ROOT/.temp_sgmodule_reject_rules.txt"
    fi
    
    # 处理DIRECT规则
    if [[ -f "$direct_rules_file" ]] && [[ -s "$direct_rules_file" ]]; then
        local unique_direct=$(sort -u "$direct_rules_file" | wc -l | tr -d ' ')
        log_success "DIRECT规则: $unique_direct 条（去重后）"
        
        # 合并到ChinaDirect.list（如果存在）
        if [[ -f "$CHINA_DIRECT_LIST" ]]; then
            log_info "合并DIRECT规则到 ChinaDirect.list..."
            local before_count=$(grep -cv "^#\|^$" "$CHINA_DIRECT_LIST" 2>/dev/null || echo "0")
            
            # 追加新规则并去重
            cat "$direct_rules_file" >> "$CHINA_DIRECT_LIST"
            
            # 提取规则部分，去重，重新生成
            local temp_merged="$temp_dir/merged_direct.txt"
            grep -v "^#\|^$" "$CHINA_DIRECT_LIST" | sort -u > "$temp_merged"
            local after_count=$(wc -l < "$temp_merged" | tr -d ' ')
            local added=$((after_count - before_count))
            
            # 重新生成文件（保留头部）
            {
                head -30 "$CHINA_DIRECT_LIST" | grep "^#"
                echo ""
                cat "$temp_merged"
            } > "$CHINA_DIRECT_LIST.new"
            mv "$CHINA_DIRECT_LIST.new" "$CHINA_DIRECT_LIST"
            
            log_success "  新增 $added 条规则到 ChinaDirect.list"
        else
            log_warning "ChinaDirect.list 不存在，跳过DIRECT规则合并"
        fi
        
        cp "$direct_rules_file" "$PROJECT_ROOT/.temp_sgmodule_direct_rules.txt"
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
    
    local adblock_list="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock.list"
    local direct_list="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/ChinaDirect.list"
    local srs_dir="$PROJECT_ROOT/ruleset/SingBox"
    
    echo -e "${GREEN}📊 规则集统计:${NC}"
    
    if [[ -f "$adblock_list" ]]; then
        local adblock_count=$(grep -cv "^#\|^$" "$adblock_list" 2>/dev/null || echo "0")
        echo "  • AdBlock.list: $adblock_count 条规则"
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
    rm -f "$PROJECT_ROOT/.temp_sgmodule_reject_rules.txt" 2>/dev/null || true
    rm -f "$PROJECT_ROOT/.temp_sgmodule_direct_rules.txt" 2>/dev/null || true
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
