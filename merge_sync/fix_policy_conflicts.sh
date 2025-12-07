#!/bin/bash
# ============================================
# 策略冲突自动修复脚本
# 修复规则集之间的策略混入问题
# ============================================

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULESET_DIR="${SCRIPT_DIR}/../ruleset/Surge(Shadowkroket)"

echo "╔══════════════════════════════════════════╗"
echo "║     策略冲突自动修复                     ║"
echo "╚══════════════════════════════════════════╝"
echo ""

# 1. 从Google.list移除广告域名，添加到AdBlock
echo -e "${BLUE}━━━ 1. 修复Google.list中的广告域名 ━━━${NC}"

ad_domains=$(grep -E "doubleclick|googleads|adservice\.google|pagead|adsystem" "$RULESET_DIR/Google.list" 2>/dev/null || true)

if [ -n "$ad_domains" ]; then
    echo "   发现广告域名:"
    echo "$ad_domains" | while read line; do echo "     $line"; done
    echo ""
    
    # 创建临时文件
    temp_google=$(mktemp)
    temp_adblock=$(mktemp)
    
    # 从Google.list移除广告域名
    grep -vE "doubleclick|googleads|adservice\.google|pagead|adsystem" "$RULESET_DIR/Google.list" > "$temp_google"
    
    # 添加到AdBlock.list（在规则部分末尾）
    # 先复制原文件
    cp "$RULESET_DIR/AdBlock.list" "$temp_adblock"
    
    # 添加注释和规则
    echo "" >> "$temp_adblock"
    echo "# ========== Google Ads (moved from Google.list) ==========" >> "$temp_adblock"
    echo "$ad_domains" >> "$temp_adblock"
    
    # 替换原文件
    mv "$temp_google" "$RULESET_DIR/Google.list"
    mv "$temp_adblock" "$RULESET_DIR/AdBlock.list"
    
    echo -e "   ${GREEN}✓ 已将广告域名从Google.list移到AdBlock.list${NC}"
else
    echo -e "   ${GREEN}✓ Google.list无广告域名${NC}"
fi

echo ""

# 2. 更新AdBlock.list（如果存在）
echo -e "${BLUE}━━━ 2. 同步AdBlock.list ━━━${NC}"

if [ -f "$RULESET_DIR/AdBlock.list" ]; then
    # AdBlock应该包含所有AdBlock规则
    # 这里只添加注释，实际合并由merge_adblock_modules.sh处理
    echo -e "   ${YELLOW}ℹ️  AdBlock.list需要重新生成${NC}"
    echo -e "   ${YELLOW}   运行: bash merge_sync/merge_adblock_modules.sh${NC}"
else
    echo -e "   ${GREEN}✓ AdBlock.list不存在，跳过${NC}"
fi

echo ""

# 3. 运行smart_cleanup去重
echo -e "${BLUE}━━━ 3. 运行smart_cleanup去重 ━━━${NC}"

if [ -f "${SCRIPT_DIR}/smart_cleanup.py" ]; then
    python3 "${SCRIPT_DIR}/smart_cleanup.py"
    echo -e "   ${GREEN}✓ 去重完成${NC}"
else
    echo -e "   ${YELLOW}⚠️  smart_cleanup.py不存在，跳过${NC}"
fi

echo ""

# 4. 重新检测冲突
echo -e "${BLUE}━━━ 4. 重新检测冲突 ━━━${NC}"

if [ -f "${SCRIPT_DIR}/detect_policy_conflicts.sh" ]; then
    bash "${SCRIPT_DIR}/detect_policy_conflicts.sh" || true
else
    echo -e "   ${YELLOW}⚠️  detect_policy_conflicts.sh不存在，跳过${NC}"
fi

echo ""
echo "╔══════════════════════════════════════════╗"
echo "║            修复完成                      ║"
echo "╠══════════════════════════════════════════╣"
echo "║  建议:                                   ║"
echo "║  1. 检查修复结果                         ║"
echo "║  2. 重新生成SRS文件                      ║"
echo "║  3. 提交到Git                            ║"
echo "╚══════════════════════════════════════════╝"
