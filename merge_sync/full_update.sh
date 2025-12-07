#!/bin/bash
# =============================================================================
# 一键完整更新脚本 v3.1
# 功能: Git Pull + 同步MetaCubeX + 更新Sources + 增量合并 + 广告模块合并 + 模块同步 + 生成SRS + Git Push
# 更新: 2025-12-07
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 日志函数
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# 显示帮助
show_help() {
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  --with-core       同时更新 Sing-box & Mihomo 核心 (本地推荐)"
    echo "  --with-git        启用Git操作 (pull/push)"
    echo "  --skip-git        跳过Git操作"
    echo "  --skip-sync       跳过MetaCubeX同步"
    echo "  --skip-merge      跳过增量合并"
    echo "  --skip-adblock    跳过广告模块合并"
    echo "  --skip-module     跳过模块同步到iCloud"
    echo "  --skip-profile    跳过Surge配置同步"
    echo "  --skip-srs        跳过SRS生成"
    echo "  --verbose         显示详细输出"
    echo "  --quiet           静默模式 (最少输出)"
    echo "  --quick           快速模式 (跳过同步、模块和Git)"
    echo "  --full            完整模式 (包含Git操作)"
    echo "  --unattended      无人值守模式 (CI/CD专用，含Git，跳过iCloud)"
    echo "  --ci              CI模式 (同--unattended)"
    echo "  --cron            定时任务模式 (同--unattended)"
    echo "  -y, --yes         自动确认所有操作"
    echo "  -h, --help        显示帮助"
    echo ""
    echo "示例:"
    echo "  $0                    # 标准更新 (无Git, 无核心)"
    echo "  $0 --full             # 完整更新 (含Git pull/push)"
    echo "  $0 --with-core        # 本地全面更新 (含核心+Surge配置)"
    echo "  $0 --full --with-core # 最全面更新 (Git+核心+配置)"
    echo "  $0 --unattended       # 无人值守模式 (CI/CD, 跳过核心和配置)"
    echo "  $0 --quick            # 快速更新 (仅合并+SRS)"
    echo "  $0 --cron             # 定时任务模式"
    echo ""
    exit 0
}

# 解析参数
WITH_CORE=false
WITH_GIT=false
SKIP_GIT=false
SKIP_SYNC=false
SKIP_MERGE=false
SKIP_ADBLOCK=false
SKIP_MODULE=false
SKIP_PROFILE=false
SKIP_SRS=false
VERBOSE=false
QUIET=false
AUTO_YES=false
UNATTENDED=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --with-core) WITH_CORE=true; shift ;;
        --with-git) WITH_GIT=true; shift ;;
        --skip-git) SKIP_GIT=true; shift ;;
        --skip-sync) SKIP_SYNC=true; shift ;;
        --skip-merge) SKIP_MERGE=true; shift ;;
        --skip-adblock) SKIP_ADBLOCK=true; shift ;;
        --skip-module) SKIP_MODULE=true; shift ;;
        --skip-profile) SKIP_PROFILE=true; shift ;;
        --skip-srs) SKIP_SRS=true; shift ;;
        --verbose) VERBOSE=true; shift ;;
        --quiet) QUIET=true; shift ;;
        -y|--yes) AUTO_YES=true; shift ;;
        --quick) SKIP_SYNC=true; SKIP_MODULE=true; SKIP_GIT=true; shift ;;
        --full) WITH_GIT=true; shift ;;
        --unattended|--ci|--cron)
            # 无人值守模式: 启用Git, 跳过iCloud模块同步, 跳过核心更新, 静默输出, 自动确认
            UNATTENDED=true
            WITH_GIT=true
            WITH_CORE=false   # CI环境不更新核心
            SKIP_MODULE=true  # CI环境无iCloud
            SKIP_PROFILE=true # CI环境跳过Surge配置同步
            QUIET=true
            AUTO_YES=true
            shift ;;
        -h|--help) show_help ;;
        *) log_error "未知选项: $1"; exit 1 ;;
    esac
done

# 静默模式下重定义日志函数
if [ "$QUIET" = true ]; then
    log_info() { :; }  # 静默
    log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }  # 警告仍显示
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; }  # 错误仍显示
fi

# 显示banner (非静默模式)
if [ "$QUIET" = false ]; then
    echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║       Singbox 规则完整更新工具 v3.1                          ║${NC}"
    echo -e "${BLUE}║       Surge + MetaCubeX + SingBox + Module 全端同步          ║${NC}"
    echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo ""
fi

# 无人值守模式提示
if [ "$UNATTENDED" = true ] && [ "$QUIET" = false ]; then
    log_info "无人值守模式已启用 (Git: ON, Core: OFF, iCloud: OFF, Profile: OFF, Auto-confirm: ON)"
fi

# 步骤计数
STEP=0
TOTAL_STEPS=11  # 增加了空规则集检查+去重步骤

# ═══════════════════════════════════════════════════════════════
# 步骤1: Git Pull (获取远程更新)
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$WITH_GIT" = true ] && [ "$SKIP_GIT" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Git Pull (获取远程更新)...${NC}"
    cd "$PROJECT_ROOT"
    if git rev-parse --git-dir > /dev/null 2>&1; then
        # 检查是否有未提交的更改
        if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
            log_warning "检测到本地未提交的更改"
            if [ "$VERBOSE" = true ]; then
                git status --short
            fi
            log_info "尝试 stash 本地更改..."
            git stash push -m "auto-stash before full_update $(date +%Y%m%d_%H%M%S)" 2>/dev/null || true
        fi
        
        # 获取当前分支名
        CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "main")
        
        # 执行 git pull
        if [ "$VERBOSE" = true ]; then
            git pull --rebase origin "$CURRENT_BRANCH" || git pull origin "$CURRENT_BRANCH" || log_warning "Git pull 失败，继续执行"
        else
            git pull --rebase origin "$CURRENT_BRANCH" 2>&1 | grep -E "^(Already|Updating|Fast-forward|error:|fatal:)" || git pull origin "$CURRENT_BRANCH" 2>&1 | grep -E "^(Already|Updating|Fast-forward|error:|fatal:)" || log_warning "Git pull 失败"
        fi
        
        # 恢复 stash (如果有)
        if git stash list | grep -q "auto-stash before full_update"; then
            log_info "恢复本地更改..."
            git stash pop 2>/dev/null || log_warning "Stash pop 失败，请手动处理"
        fi
        
        log_success "Git Pull 完成"
    else
        log_warning "不是Git仓库，跳过"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 跳过Git Pull (使用 --with-git 或 --full 启用)${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# 步骤2: 更新 Sing-box & Mihomo 核心 (可选)
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$WITH_CORE" = true ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 更新 Sing-box & Mihomo 核心...${NC}"
    if [ -f "${SCRIPT_DIR}/update_cores.sh" ]; then
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/update_cores.sh"
        else
            "${SCRIPT_DIR}/update_cores.sh" 2>&1 | grep -E "^\[OK\]|\[INFO\]|\[WARN\]|当前版本|最新版本|下载|安装|完成" || true
        fi
        log_success "核心更新完成"
    else
        log_warning "跳过: update_cores.sh 不存在"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 跳过核心更新 (使用 --with-core 启用)${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# 步骤3: 同步MetaCubeX规则
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_SYNC" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 同步MetaCubeX规则...${NC}"
    if [ -f "${SCRIPT_DIR}/sync_metacubex_rules.sh" ]; then
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/sync_metacubex_rules.sh"
        else
            "${SCRIPT_DIR}/sync_metacubex_rules.sh" 2>&1 | grep -E "^(✅|❌|===|下载|更新)" || true
        fi
        log_success "MetaCubeX规则同步完成"
    else
        log_warning "跳过: sync_metacubex_rules.sh 不存在"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 跳过MetaCubeX同步${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# 步骤4: 更新Sources文件
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 更新Sources文件...${NC}"
if [ -f "${SCRIPT_DIR}/update_sources_metacubex.sh" ]; then
    if [ "$VERBOSE" = true ]; then
        "${SCRIPT_DIR}/update_sources_metacubex.sh"
    else
        "${SCRIPT_DIR}/update_sources_metacubex.sh" 2>&1 | grep -E "^(更新|跳过|===)" || true
    fi
    log_success "Sources文件更新完成"
else
    log_warning "跳过: update_sources_metacubex.sh 不存在"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# 步骤5: 增量合并规则
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_MERGE" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 增量合并规则...${NC}"
    if [ -f "${SCRIPT_DIR}/incremental_merge_all.sh" ]; then
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/incremental_merge_all.sh"
        else
            "${SCRIPT_DIR}/incremental_merge_all.sh" 2>&1 | grep -E "^\[OK\]|^合并:|^===|Before:|After:|Added:|跳过" || true
        fi
        log_success "规则增量合并完成"
    else
        log_warning "跳过: incremental_merge_all.sh 不存在"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 跳过增量合并${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# 步骤5.5: 空规则集检查 + 智能去重
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 空规则集检查 + 智能去重...${NC}"

# 检查空规则集
if [ -f "${SCRIPT_DIR}/check_empty_rulesets.sh" ]; then
    if [ "$VERBOSE" = true ]; then
        "${SCRIPT_DIR}/check_empty_rulesets.sh"
    else
        "${SCRIPT_DIR}/check_empty_rulesets.sh" 2>&1 | grep -E "^(⚠️|ℹ️|❌|Total|Empty)" || true
    fi
fi

# 运行智能去重 (广告 > 细分 > 兜底)
if [ -f "${SCRIPT_DIR}/smart_cleanup.py" ]; then
    log_info "运行智能去重 (优先级: 广告 > 细分网站 > 兜底规则)..."
    if [ "$VERBOSE" = true ]; then
        python3 "${SCRIPT_DIR}/smart_cleanup.py"
    else
        python3 "${SCRIPT_DIR}/smart_cleanup.py" 2>&1 | grep -E "^(Removed|Starting|Complete)" || true
    fi
    log_success "智能去重完成"
fi

# 更新规则集Header (添加策略建议)
if [ -f "${SCRIPT_DIR}/update_ruleset_headers.sh" ]; then
    log_info "更新规则集Header (添加策略建议)..."
    if [ "$VERBOSE" = true ]; then
        "${SCRIPT_DIR}/update_ruleset_headers.sh"
    else
        "${SCRIPT_DIR}/update_ruleset_headers.sh" 2>&1 | grep -E "^(✅|⚠️|╔|╚)" || true
    fi
    log_success "规则集Header更新完成"
else
    log_warning "跳过: update_ruleset_headers.sh 不存在"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# 步骤6: 广告模块合并
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_ADBLOCK" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 广告模块合并...${NC}"
    if [ -f "${SCRIPT_DIR}/merge_adblock_modules.sh" ]; then
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/merge_adblock_modules.sh"
        else
            "${SCRIPT_DIR}/merge_adblock_modules.sh" 2>&1 | grep -E "^\[✓\]|^\[INFO\]|^\[⚠\]|Processing:" || true
        fi
        log_success "广告模块合并完成"
    else
        log_warning "跳过: merge_adblock_modules.sh 不存在"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 跳过广告模块合并${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# 步骤7: 模块同步到iCloud
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_MODULE" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 模块同步到iCloud (Surge + Shadowrocket)...${NC}"
    if [ -f "${SCRIPT_DIR}/sync_modules_to_icloud.sh" ]; then
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/sync_modules_to_icloud.sh" --all
        else
            "${SCRIPT_DIR}/sync_modules_to_icloud.sh" --all 2>&1 | grep -E "^\[✓\]|^\[INFO\]|^\[⚠\]|Surge:|Shadowrocket:|modules" || true
        fi
        log_success "模块同步完成"
    else
        log_warning "跳过: sync_modules_to_icloud.sh 不存在"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 跳过模块同步${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# 步骤8: 同步Surge配置文件 (吸取用户规则 + 更新规则集)
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_PROFILE" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 同步Surge配置文件 (注释关键词智能分类)...${NC}"
    if [ -f "${SCRIPT_DIR}/sync_profile_to_template.sh" ]; then
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/sync_profile_to_template.sh"
        else
            "${SCRIPT_DIR}/sync_profile_to_template.sh" 2>&1 | grep -E "^\[OK\]|\[INFO\]|\[WARN\]|RULE-SET|用户规则|同步完成" || true
        fi
        log_success "Surge配置同步完成"
    else
        log_warning "跳过: sync_profile_to_template.sh 不存在"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 跳过Surge配置同步${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# 步骤9: 生成SRS文件
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_SRS" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 生成SRS文件 (Singbox二进制规则)...${NC}"
    if [ -f "${SCRIPT_DIR}/batch_convert_to_singbox.sh" ]; then
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/batch_convert_to_singbox.sh"
        else
            "${SCRIPT_DIR}/batch_convert_to_singbox.sh" 2>&1 | grep -E "^(✓|✗|===|Success:|Failed:|Processing:|Found)" || true
        fi
        log_success "SRS文件生成完成"
    else
        log_warning "跳过: batch_convert_to_singbox.sh 不存在"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 跳过SRS生成${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# 步骤10: Git Commit & Push (提交并推送更新)
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$WITH_GIT" = true ] && [ "$SKIP_GIT" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Git Commit & Push (提交并推送更新)...${NC}"
    cd "$PROJECT_ROOT"
    if git rev-parse --git-dir > /dev/null 2>&1; then
        # 检查是否有更改
        if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null || [ -n "$(git ls-files --others --exclude-standard)" ]; then
            # 添加所有更改
            git add -A
            
            # 生成提交信息
            COMMIT_MSG="chore(ruleset): auto-update $(date '+%Y-%m-%d %H:%M')"
            SURGE_COUNT=$(ls "${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)/"*.list 2>/dev/null | wc -l | tr -d ' ')
            SRS_COUNT=$(ls "${PROJECT_ROOT}/ruleset/SingBox/"*.srs 2>/dev/null | wc -l | tr -d ' ')
            COMMIT_MSG="$COMMIT_MSG - Surge:$SURGE_COUNT SRS:$SRS_COUNT"
            
            # 提交
            if [ "$VERBOSE" = true ]; then
                git commit -m "$COMMIT_MSG"
            else
                git commit -m "$COMMIT_MSG" 2>&1 | grep -E "^\[|files? changed|insertions|deletions" || true
            fi
            
            # 获取当前分支名
            CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "main")
            
            # 推送
            if [ "$VERBOSE" = true ]; then
                git push origin "$CURRENT_BRANCH" || git push || log_warning "Git push 失败"
            else
                git push origin "$CURRENT_BRANCH" 2>&1 | grep -E "^To|->|Everything up-to-date|error:|fatal:" || git push 2>&1 | grep -E "^To|->|Everything up-to-date|error:|fatal:" || log_warning "Git push 失败"
            fi
            
            log_success "Git Commit & Push 完成"
        else
            log_info "没有需要提交的更改"
        fi
    else
        log_warning "不是Git仓库，跳过"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] 跳过Git Push (使用 --with-git 或 --full 启用)${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# 统计
# ═══════════════════════════════════════════════════════════════
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    更新完成统计                              ║${NC}"
echo -e "${BLUE}╠══════════════════════════════════════════════════════════════╣${NC}"

METACUBEX_COUNT=$(ls "${PROJECT_ROOT}/ruleset/MetaCubeX/"*.list 2>/dev/null | wc -l | tr -d ' ')
SURGE_COUNT=$(ls "${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)/"*.list 2>/dev/null | wc -l | tr -d ' ')
SRS_COUNT=$(ls "${PROJECT_ROOT}/ruleset/SingBox/"*.srs 2>/dev/null | wc -l | tr -d ' ')
SOURCES_COUNT=$(ls "${PROJECT_ROOT}/ruleset/Sources/Links/"*_sources.txt 2>/dev/null | wc -l | tr -d ' ')
MODULE_COUNT=$(ls "${PROJECT_ROOT}/module/surge(main)/"*.sgmodule 2>/dev/null | wc -l | tr -d ' ')

printf "${BLUE}║  ${CYAN}MetaCubeX规则:${NC}  ${GREEN}%-5s${NC}                                    ${BLUE}║${NC}\n" "$METACUBEX_COUNT"
printf "${BLUE}║  ${CYAN}Surge规则:${NC}      ${GREEN}%-5s${NC}                                    ${BLUE}║${NC}\n" "$SURGE_COUNT"
printf "${BLUE}║  ${CYAN}SingBox SRS:${NC}    ${GREEN}%-5s${NC}                                    ${BLUE}║${NC}\n" "$SRS_COUNT"
printf "${BLUE}║  ${CYAN}Sources文件:${NC}    ${GREEN}%-5s${NC}                                    ${BLUE}║${NC}\n" "$SOURCES_COUNT"
printf "${BLUE}║  ${CYAN}Surge模块:${NC}      ${GREEN}%-5s${NC}                                    ${BLUE}║${NC}\n" "$MODULE_COUNT"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"

# 显示sing-box版本
LOCAL_SINGBOX="${SCRIPT_DIR}/config-manager-auto-update/bin/sing-box"
if [ -x "$LOCAL_SINGBOX" ]; then
    echo ""
    echo -e "${GREEN}本地sing-box: $("$LOCAL_SINGBOX" version | head -1)${NC}"
fi

# 显示缺失的SRS文件
echo ""
echo -e "${CYAN}=== 检查SRS覆盖率 ===${NC}"
MISSING_SRS=0
for list_file in "${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)/"*.list; do
    [ ! -f "$list_file" ] && continue
    base_name=$(basename "$list_file" .list)
    [[ "$base_name" == *.backup* ]] && continue
    srs_file="${PROJECT_ROOT}/ruleset/SingBox/${base_name}_Singbox.srs"
    if [ ! -f "$srs_file" ]; then
        echo -e "${YELLOW}  缺失: ${base_name}.list → ${base_name}_Singbox.srs${NC}"
        MISSING_SRS=$((MISSING_SRS + 1))
    fi
done

if [ $MISSING_SRS -eq 0 ]; then
    echo -e "${GREEN}  ✅ 所有Surge规则都有对应的SRS文件${NC}"
else
    echo -e "${YELLOW}  ⚠️ 缺失 $MISSING_SRS 个SRS文件${NC}"
fi

# 显示AdBlock_Merged规则数
if [ -f "${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)/AdBlock_Merged.list" ]; then
    ADBLOCK_COUNT=$(grep -v "^#" "${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)/AdBlock_Merged.list" | grep -v "^$" | wc -l | tr -d ' ')
    echo ""
    echo -e "${CYAN}=== 广告拦截规则 ===${NC}"
    echo -e "${GREEN}  AdBlock_Merged: $ADBLOCK_COUNT 条规则${NC}"
fi

echo ""
log_success "全部完成！"
echo ""
echo -e "${CYAN}提示:${NC}"
echo "  - 使用 --quick 快速更新 (跳过同步和模块)"
echo "  - 使用 --verbose 查看详细输出"
echo "  - 使用 --help 查看所有选项"
