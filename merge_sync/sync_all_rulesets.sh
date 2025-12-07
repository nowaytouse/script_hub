#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# 一键同步所有规则集 (One-Click Ruleset Sync)
# ═══════════════════════════════════════════════════════════════════════════════
# 功能：
# 1. 合并Surge广告拦截模块到AdBlock.list
# 2. 转换所有Surge规则到SingBox格式
# 3. 显示同步统计
# ═══════════════════════════════════════════════════════════════════════════════

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

log_section() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
}

log_info() {
    echo -e "${CYAN}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

log_section "一键同步所有规则集"

# Step 1: 合并普通规则集 (Sources -> Ruleset)
log_section "Step 1: 合并普通规则集"
if [[ -f "$SCRIPT_DIR/merge_all_rulesets.sh" ]]; then
    bash "$SCRIPT_DIR/merge_all_rulesets.sh"
    log_success "普通规则集合并完成"
else
    log_error "merge_all_rulesets.sh 不存在"
fi

# Step 2: 合并Surge广告拦截模块 (AdBlock)
log_section "Step 2: 合并Surge广告拦截模块"
if [[ -f "$SCRIPT_DIR/merge_adblock_modules.sh" ]]; then
    bash "$SCRIPT_DIR/merge_adblock_modules.sh" --auto --no-backup # Disable internal backup if handled elsewhere, or let it handle rotation
    log_success "广告拦截模块合并完成"
else
    log_info "跳过广告拦截模块合并 (脚本不存在)"
fi

# Step 2: 转换所有规则到SingBox
log_section "Step 2: 转换所有规则到SingBox"
if [[ -f "$SCRIPT_DIR/batch_convert_to_singbox.sh" ]]; then
    bash "$SCRIPT_DIR/batch_convert_to_singbox.sh"
    log_success "SingBox规则转换完成"
else
    log_info "跳过SingBox转换 (脚本不存在)"
fi

# Step 3: 显示统计
log_section "同步统计"

SURGE_ADBLOCK="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock.list"
SINGBOX_ADBLOCK="$PROJECT_ROOT/ruleset/SingBox/AdBlock_Singbox.srs"

if [[ -f "$SURGE_ADBLOCK" ]]; then
    SURGE_COUNT=$(grep -v "^#" "$SURGE_ADBLOCK" | grep -v "^$" | wc -l | tr -d ' ')
    SURGE_SIZE=$(ls -lh "$SURGE_ADBLOCK" | awk '{print $5}')
    echo -e "${GREEN}Surge AdBlock:${NC}"
    echo "  - 规则数: $SURGE_COUNT"
    echo "  - 文件大小: $SURGE_SIZE"
    echo "  - 更新时间: $(stat -f "%Sm" -t "%Y-%m-%d %H:%M:%S" "$SURGE_ADBLOCK")"
fi

echo ""

if [[ -f "$SINGBOX_ADBLOCK" ]]; then
    SINGBOX_SIZE=$(ls -lh "$SINGBOX_ADBLOCK" | awk '{print $5}')
    echo -e "${GREEN}SingBox AdBlock:${NC}"
    echo "  - 文件大小: $SINGBOX_SIZE (二进制格式)"
    echo "  - 更新时间: $(stat -f "%Sm" -t "%Y-%m-%d %H:%M:%S" "$SINGBOX_ADBLOCK")"
fi

echo ""

# 统计所有SingBox规则集
SINGBOX_DIR="$PROJECT_ROOT/ruleset/SingBox"
if [[ -d "$SINGBOX_DIR" ]]; then
    SINGBOX_TOTAL=$(find "$SINGBOX_DIR" -name "*.srs" | wc -l | tr -d ' ')
    SINGBOX_TOTAL_SIZE=$(du -sh "$SINGBOX_DIR" | awk '{print $1}')
    echo -e "${GREEN}SingBox规则集总计:${NC}"
    echo "  - 规则集数量: $SINGBOX_TOTAL"
    echo "  - 总大小: $SINGBOX_TOTAL_SIZE"
fi

log_section "完成"
log_success "所有规则集同步完成！"
echo ""
log_section "Git同步"

cd "$PROJECT_ROOT"
log_info "执行 Git Pull..."
if git pull; then
    log_success "Git Pull 成功"
else
    log_warning "Git Pull 失败或有冲突"
fi

log_info "添加变更..."
git add ruleset/ module/

log_info "提交变更..."
timestamp=$(date "+%Y-%m-%d %H:%M:%S")
if git commit -m "sync: 自动同步规则集 $timestamp"; then
    log_success "提交成功"
    
    log_info "推送到远程..."
    if git push; then
        log_success "推送成功"
    else
        log_warning "推送失败"
    fi
else
    log_info "无变更需要提交"
fi

log_section "完成"
log_success "所有规则集同步及Git操作完成！"
