#!/bin/bash
# =============================================================================
# 一键完整更新脚本
# 功能: 同步MetaCubeX + 更新Sources + 增量合并 + 生成SRS + 更新Singbox核心
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo -e "${BLUE}╔══════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       Singbox 规则完整更新工具           ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════╝${NC}"
echo ""

# 步骤1: 更新Singbox核心 (可选)
if [ "$1" = "--with-core" ]; then
    echo -e "${YELLOW}[1/5] 更新Singbox核心...${NC}"
    if [ -f "${SCRIPT_DIR}/config-manager-auto-update/target/release/singbox-manager" ]; then
        "${SCRIPT_DIR}/config-manager-auto-update/target/release/singbox-manager" --once 2>&1 | grep -E "^(✅|❌|🔄|📥)" || true
    else
        echo -e "${RED}  跳过: singbox-manager未编译${NC}"
    fi
    echo ""
else
    echo -e "${YELLOW}[1/5] 跳过核心更新 (使用 --with-core 启用)${NC}"
    echo ""
fi

# 步骤2: 同步MetaCubeX规则
echo -e "${YELLOW}[2/5] 同步MetaCubeX规则...${NC}"
"${SCRIPT_DIR}/sync_metacubex_rules.sh" 2>&1 | grep -E "^(✅|❌|===)" || true
echo ""

# 步骤3: 更新Sources文件
echo -e "${YELLOW}[3/5] 更新Sources文件...${NC}"
"${SCRIPT_DIR}/update_sources_metacubex.sh" 2>&1 | grep -E "^(更新|跳过|===)" || true
echo ""

# 步骤4: 增量合并规则
echo -e "${YELLOW}[4/5] 增量合并规则...${NC}"
"${SCRIPT_DIR}/incremental_merge_all.sh" 2>&1 | grep -E "^\[OK\]|^合并:|^===|Before:|After:|Added:" || true
echo ""

# 步骤5: 生成SRS文件
echo -e "${YELLOW}[5/5] 生成SRS文件...${NC}"
"${SCRIPT_DIR}/batch_convert_to_singbox.sh" 2>&1 | grep -E "^(✓|✗|===|Success:|Failed:)" || true
echo ""

# 统计
echo -e "${BLUE}╔══════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║              更新完成                    ║${NC}"
echo -e "${BLUE}╠══════════════════════════════════════════╣${NC}"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
METACUBEX_COUNT=$(ls "${PROJECT_ROOT}/ruleset/MetaCubeX/"*.list 2>/dev/null | wc -l | tr -d ' ')
SURGE_COUNT=$(ls "${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)/"*.list 2>/dev/null | wc -l | tr -d ' ')
SRS_COUNT=$(ls "${PROJECT_ROOT}/ruleset/SingBox/"*.srs 2>/dev/null | wc -l | tr -d ' ')
echo -e "${BLUE}║  MetaCubeX规则: ${GREEN}${METACUBEX_COUNT}${BLUE}                        ║${NC}"
echo -e "${BLUE}║  Surge规则:     ${GREEN}${SURGE_COUNT}${BLUE}                        ║${NC}"
echo -e "${BLUE}║  SingBox SRS:   ${GREEN}${SRS_COUNT}${BLUE}                        ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════╝${NC}"

# 显示sing-box版本
LOCAL_SINGBOX="${SCRIPT_DIR}/config-manager-auto-update/bin/sing-box"
if [ -x "$LOCAL_SINGBOX" ]; then
    echo ""
    echo -e "${GREEN}本地sing-box: $("$LOCAL_SINGBOX" version | head -1)${NC}"
fi
