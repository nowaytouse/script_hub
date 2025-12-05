#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# 一键同步所有规则集 (One-Click Ruleset Sync)
# ═══════════════════════════════════════════════════════════════════════════════
# 功能：
# 1. 合并Surge广告拦截模块到AdBlock_Merged.list
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
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

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

# Step 1: 合并Surge广告拦截模块
log_section "Step 1: 合并Surge广告拦截模块"
if [[ -f "$SCRIPT_DIR/merge_adblock_modules.sh" ]]; then
    bash "$SCRIPT_DIR/merge_adblock_modules.sh"
    log_success "广告拦截模块合并完成"
else
    log_info "跳过广告拦截模块合并 (脚本不存在)"
fi

# Step 2: 转换所有规则到SingBox
log_section "Step 2: 转换所有规则到SingBox"
if [[ -f "$PROJECT_ROOT/scripts/network/batch_convert_to_singbox.sh" ]]; then
    bash "$PROJECT_ROOT/scripts/network/batch_convert_to_singbox.sh"
    log_success "SingBox规则转换完成"
else
    log_info "跳过SingBox转换 (脚本不存在)"
fi

# Step 3: 显示统计
log_section "同步统计"

SURGE_ADBLOCK="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/AdBlock_Merged.list"
SINGBOX_ADBLOCK="$PROJECT_ROOT/ruleset/SingBox/AdBlock_Merged_Singbox.srs"

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
echo "下一步："
echo "1. 检查生成的规则文件"
echo "2. 提交到Git: git add ruleset/ && git commit -m 'sync: 更新规则集'"
echo "3. 推送到远程: git push"
