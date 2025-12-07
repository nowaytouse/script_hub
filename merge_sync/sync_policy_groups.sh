#!/bin/bash
# =============================================================================
# 策略组同步脚本
# 功能: 对比Surge和Singbox的策略组，确保一致性
# 创建: 2025-12-07
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

SURGE_CONFIG="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo Pro Max plus👑_fixed.conf"
SINGBOX_CONFIG="substore/Singbox_substore_1.13.0+.json"

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           策略组同步检查工具                                 ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

if [ ! -f "$SURGE_CONFIG" ]; then
    log_error "Surge配置文件不存在: $SURGE_CONFIG"
    exit 1
fi

if [ ! -f "$SINGBOX_CONFIG" ]; then
    log_error "Singbox配置文件不存在: $SINGBOX_CONFIG"
    exit 1
fi

# 提取Surge策略组
log_info "提取Surge策略组..."
SURGE_GROUPS=$(grep "^[^#].*=" "$SURGE_CONFIG" | grep -E "select|url-test|fallback|load-balance" | cut -d'=' -f1 | sed 's/ *$//' | sort)
SURGE_COUNT=$(echo "$SURGE_GROUPS" | wc -l | tr -d ' ')

echo "$SURGE_GROUPS" > /tmp/surge_groups.txt

# 提取Singbox策略组
log_info "提取Singbox策略组..."
python3 - "$SINGBOX_CONFIG" <<'PYTHON_SCRIPT'
import json
import sys

with open(sys.argv[1], 'r') as f:
    config = json.load(f)

outbounds = config.get('outbounds', [])
policy_groups = []

for ob in outbounds:
    ob_type = ob.get('type')
    if ob_type in ['selector', 'urltest']:
        policy_groups.append(ob.get('tag'))

for pg in sorted(policy_groups):
    print(pg)
PYTHON_SCRIPT

SINGBOX_GROUPS=$(python3 - "$SINGBOX_CONFIG" <<'PYTHON_SCRIPT2'
import json
import sys

with open(sys.argv[1], 'r') as f:
    config = json.load(f)

outbounds = config.get('outbounds', [])
for ob in outbounds:
    if ob.get('type') in ['selector', 'urltest']:
        print(ob.get('tag'))
PYTHON_SCRIPT2
)

echo "$SINGBOX_GROUPS" | sort > /tmp/singbox_groups.txt
SINGBOX_COUNT=$(echo "$SINGBOX_GROUPS" | wc -l | tr -d ' ')

# 对比
log_info "对比策略组..."
echo ""
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    策略组统计                                ║${NC}"
echo -e "${BLUE}╠══════════════════════════════════════════════════════════════╣${NC}"
echo -e "${BLUE}║  ${GREEN}Surge策略组数:${NC}    $SURGE_COUNT                                    ${BLUE}║${NC}"
echo -e "${BLUE}║  ${GREEN}Singbox策略组数:${NC}  $SINGBOX_COUNT                                    ${BLUE}║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# 找出差异
log_info "查找差异..."
MISSING_IN_SINGBOX=$(comm -23 /tmp/surge_groups.txt /tmp/singbox_groups.txt)
EXTRA_IN_SINGBOX=$(comm -13 /tmp/surge_groups.txt /tmp/singbox_groups.txt)

if [ -n "$MISSING_IN_SINGBOX" ]; then
    log_warning "Singbox中缺失的策略组:"
    echo "$MISSING_IN_SINGBOX" | while read group; do
        echo "  - $group"
    done
    echo ""
fi

if [ -n "$EXTRA_IN_SINGBOX" ]; then
    log_info "Singbox中额外的策略组:"
    echo "$EXTRA_IN_SINGBOX" | while read group; do
        echo "  - $group"
    done
    echo ""
fi

if [ -z "$MISSING_IN_SINGBOX" ] && [ -z "$EXTRA_IN_SINGBOX" ]; then
    log_success "✅ 策略组完全同步！"
else
    log_warning "策略组存在差异"
    echo ""
    log_info "注意: Surge和Singbox的策略组配置方式不同"
    log_info "  - Surge: 基于文本配置"
    log_info "  - Singbox: 基于JSON配置"
    log_info "  - 某些差异是正常的（如DIRECT、REJECT等内置策略）"
fi

# 清理临时文件
rm -f /tmp/surge_groups.txt /tmp/singbox_groups.txt

echo ""
log_success "策略组检查完成"
