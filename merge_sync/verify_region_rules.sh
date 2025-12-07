#!/bin/bash
# =============================================================================
# 地区规则分配验证脚本
# 功能: 验证Surge、Singbox、Shadowrocket的地区流媒体规则分配是否正确
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

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           地区规则分配验证工具                               ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# 定义正确的地区规则映射
declare -A REGION_RULES=(
    ["StreamJP"]="🇯🇵"
    ["StreamUS"]="🇺🇸"
    ["StreamKR"]="🇰🇷"
    ["StreamHK"]="🇭🇰"
    ["StreamTW"]="🇹🇼"
    ["StreamEU"]="🇬🇧"
)

TOTAL_ERRORS=0

# ============================================================================
# 验证Surge配置
# ============================================================================
log_info "验证Surge配置..."
SURGE_CONFIG="ruleset/Sources/surge_rules_complete.conf"

if [ ! -f "$SURGE_CONFIG" ]; then
    log_error "Surge配置文件不存在: $SURGE_CONFIG"
    exit 1
fi

SURGE_ERRORS=0
for rule in "${!REGION_RULES[@]}"; do
    region="${REGION_RULES[$rule]}"
    
    # 检查规则是否存在且分配正确
    if grep -q "RULE-SET.*${rule}.list" "$SURGE_CONFIG"; then
        line=$(grep "RULE-SET.*${rule}.list" "$SURGE_CONFIG")
        if echo "$line" | grep -q "$region"; then
            echo "  ✅ $rule → $region"
        else
            echo "  ❌ $rule 分配错误"
            echo "     行: $line"
            SURGE_ERRORS=$((SURGE_ERRORS + 1))
        fi
    else
        log_warning "  ⚠️  $rule 规则未找到"
    fi
done

if [ $SURGE_ERRORS -eq 0 ]; then
    log_success "Surge配置验证通过"
else
    log_error "Surge配置发现 $SURGE_ERRORS 个错误"
    TOTAL_ERRORS=$((TOTAL_ERRORS + SURGE_ERRORS))
fi

echo ""

# ============================================================================
# 验证Singbox配置
# ============================================================================
log_info "验证Singbox配置..."
SINGBOX_CONFIG="substore/Singbox_substore_1.13.0+.json"

if [ ! -f "$SINGBOX_CONFIG" ]; then
    log_error "Singbox配置文件不存在: $SINGBOX_CONFIG"
    exit 1
fi

python3 - "$SINGBOX_CONFIG" <<'PYTHON_SCRIPT'
import json
import sys

config_file = sys.argv[1]
with open(config_file, 'r', encoding='utf-8') as f:
    config = json.load(f)

# 定义正确的映射
region_rules = {
    'surge-streamjp': '🇯🇵',
    'surge-streamus': '🇺🇸',
    'surge-streamkr': '🇰🇷',
    'surge-streamhk': '🇭🇰',
    'surge-streamtw': '🇹🇼',
    'surge-streameu': '🇬🇧'
}

errors = 0
for rule in config.get('route', {}).get('rules', []):
    rule_set = rule.get('rule_set')
    outbound = rule.get('outbound', '')
    
    if isinstance(rule_set, str) and rule_set in region_rules:
        expected_region = region_rules[rule_set]
        if expected_region in outbound:
            print(f"  ✅ {rule_set} → {outbound}")
        else:
            print(f"  ❌ {rule_set} 分配错误")
            print(f"     当前: {outbound}")
            print(f"     应包含: {expected_region}")
            errors += 1

sys.exit(errors)
PYTHON_SCRIPT

SINGBOX_ERRORS=$?
if [ $SINGBOX_ERRORS -eq 0 ]; then
    log_success "Singbox配置验证通过"
else
    log_error "Singbox配置发现 $SINGBOX_ERRORS 个错误"
    TOTAL_ERRORS=$((TOTAL_ERRORS + SINGBOX_ERRORS))
fi

echo ""

# ============================================================================
# 总结
# ============================================================================
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    验证总结                                  ║${NC}"
echo -e "${BLUE}╠══════════════════════════════════════════════════════════════╣${NC}"

if [ $TOTAL_ERRORS -eq 0 ]; then
    echo -e "${BLUE}║  ${GREEN}✅ 所有配置验证通过！${NC}                                    ${BLUE}║${NC}"
    echo -e "${BLUE}║  ${GREEN}   地区规则分配完全正确${NC}                                ${BLUE}║${NC}"
else
    echo -e "${BLUE}║  ${RED}❌ 发现 $TOTAL_ERRORS 个错误${NC}                                      ${BLUE}║${NC}"
    echo -e "${BLUE}║  ${RED}   请检查并修复配置文件${NC}                                ${BLUE}║${NC}"
fi

echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

exit $TOTAL_ERRORS
