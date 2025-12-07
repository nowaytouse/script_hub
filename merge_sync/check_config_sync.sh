#!/bin/bash
# =============================================================================
# 配置同步检查脚本
# 功能: 检查 Surge、Singbox、Shadowrocket 三个配置是否完全同步
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

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       配置同步检查工具                                        ║${NC}"
echo -e "${BLUE}║       Surge vs Singbox vs Shadowrocket                       ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# 配置文件路径
SURGE_TEMPLATE="$PROJECT_ROOT/ruleset/Sources/surge_rules_complete.conf"
SINGBOX_CONFIG="$PROJECT_ROOT/substore/Singbox_substore_1.13.0+.json"

# ═══════════════════════════════════════════════════════════════
# 步骤1: 提取 Surge 规则集列表
# ═══════════════════════════════════════════════════════════════
log_info "步骤1: 提取 Surge 规则集列表..."

surge_rulesets=()
while IFS= read -r line; do
    # 匹配 RULE-SET 行
    if [[ "$line" =~ RULE-SET.*https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge\(Shadowkroket\)/([^.]+)\.list ]]; then
        ruleset_name="${BASH_REMATCH[1]}"
        surge_rulesets+=("$ruleset_name")
    fi
done < "$SURGE_TEMPLATE"

log_success "Surge 规则集数量: ${#surge_rulesets[@]}"

# ═══════════════════════════════════════════════════════════════
# 步骤2: 提取 Singbox 规则集列表
# ═══════════════════════════════════════════════════════════════
log_info "步骤2: 提取 Singbox 规则集列表..."

singbox_rulesets=()
while IFS= read -r line; do
    # 匹配 surge-xxx 规则集
    if [[ "$line" =~ \"surge-([^\"]+)\" ]]; then
        ruleset_name="${BASH_REMATCH[1]}"
        # 去重
        if [[ ! " ${singbox_rulesets[@]} " =~ " ${ruleset_name} " ]]; then
            singbox_rulesets+=("$ruleset_name")
        fi
    fi
done < "$SINGBOX_CONFIG"

log_success "Singbox 规则集数量: ${#singbox_rulesets[@]}"

# ═══════════════════════════════════════════════════════════════
# 步骤3: 对比规则集
# ═══════════════════════════════════════════════════════════════
log_info "步骤3: 对比规则集..."
echo ""

# 转换为小写并排序
surge_sorted=($(printf '%s\n' "${surge_rulesets[@]}" | tr '[:upper:]' '[:lower:]' | sort))
singbox_sorted=($(printf '%s\n' "${singbox_rulesets[@]}" | tr '[:upper:]' '[:lower:]' | sort))

# 检查 Surge 中有但 Singbox 中没有的
missing_in_singbox=()
for ruleset in "${surge_sorted[@]}"; do
    if [[ ! " ${singbox_sorted[@]} " =~ " ${ruleset} " ]]; then
        missing_in_singbox+=("$ruleset")
    fi
done

# 检查 Singbox 中有但 Surge 中没有的
extra_in_singbox=()
for ruleset in "${singbox_sorted[@]}"; do
    if [[ ! " ${surge_sorted[@]} " =~ " ${ruleset} " ]]; then
        extra_in_singbox+=("$ruleset")
    fi
done

# ═══════════════════════════════════════════════════════════════
# 步骤4: 显示结果
# ═══════════════════════════════════════════════════════════════
echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║                    对比结果                                  ║${NC}"
echo -e "${CYAN}╠══════════════════════════════════════════════════════════════╣${NC}"
printf "${CYAN}║${NC}  Surge 规则集:     %-40s ${CYAN}║${NC}\n" "${#surge_rulesets[@]}"
printf "${CYAN}║${NC}  Singbox 规则集:   %-40s ${CYAN}║${NC}\n" "${#singbox_rulesets[@]}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

if [ ${#missing_in_singbox[@]} -eq 0 ] && [ ${#extra_in_singbox[@]} -eq 0 ]; then
    log_success "✅ 完全同步！所有规则集都已匹配"
else
    if [ ${#missing_in_singbox[@]} -gt 0 ]; then
        log_warning "⚠️  Singbox 中缺少的规则集 (${#missing_in_singbox[@]}个):"
        for ruleset in "${missing_in_singbox[@]}"; do
            echo "   - $ruleset"
        done
        echo ""
    fi
    
    if [ ${#extra_in_singbox[@]} -gt 0 ]; then
        log_info "ℹ️  Singbox 中额外的规则集 (${#extra_in_singbox[@]}个):"
        for ruleset in "${extra_in_singbox[@]}"; do
            echo "   - $ruleset"
        done
        echo ""
    fi
fi

# ═══════════════════════════════════════════════════════════════
# 步骤5: 检查关键规则集
# ═══════════════════════════════════════════════════════════════
log_info "步骤5: 检查关键规则集..."
echo ""

key_rulesets=(
    "adblock"
    "chinadirect"
    "globalproxy"
    "lan"
    "manual"
    "ai"
    "telegram"
    "netflix"
    "youtube"
    "google"
)

echo "关键规则集检查:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
printf "%-20s | %-10s | %-10s\n" "规则集" "Surge" "Singbox"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

for key in "${key_rulesets[@]}"; do
    surge_has="❌"
    singbox_has="❌"
    
    # 检查 Surge
    for ruleset in "${surge_sorted[@]}"; do
        if [[ "$ruleset" == "$key" ]]; then
            surge_has="✅"
            break
        fi
    done
    
    # 检查 Singbox
    for ruleset in "${singbox_sorted[@]}"; do
        if [[ "$ruleset" == "$key" ]]; then
            singbox_has="✅"
            break
        fi
    done
    
    printf "%-20s | %-10s | %-10s\n" "$key" "$surge_has" "$singbox_has"
done

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# ═══════════════════════════════════════════════════════════════
# 步骤6: 检查规则顺序（DNS防泄漏）
# ═══════════════════════════════════════════════════════════════
log_info "步骤6: 检查规则顺序（DNS防泄漏）..."
echo ""

# 检查 ChinaDirect 是否在 GlobalProxy 之前
chinadirect_pos=-1
globalproxy_pos=-1

for i in "${!surge_rulesets[@]}"; do
    ruleset_lower=$(echo "${surge_rulesets[$i]}" | tr '[:upper:]' '[:lower:]')
    if [[ "$ruleset_lower" == "chinadirect" ]]; then
        chinadirect_pos=$i
    fi
    if [[ "$ruleset_lower" == "globalproxy" ]]; then
        globalproxy_pos=$i
    fi
done

if [ $chinadirect_pos -ge 0 ] && [ $globalproxy_pos -ge 0 ]; then
    if [ $chinadirect_pos -lt $globalproxy_pos ]; then
        log_success "✅ DNS防泄漏顺序正确: ChinaDirect (位置 $chinadirect_pos) 在 GlobalProxy (位置 $globalproxy_pos) 之前"
    else
        log_error "❌ DNS防泄漏顺序错误: ChinaDirect (位置 $chinadirect_pos) 在 GlobalProxy (位置 $globalproxy_pos) 之后"
    fi
else
    log_warning "⚠️  无法检查规则顺序: ChinaDirect 或 GlobalProxy 未找到"
fi

echo ""

# ═══════════════════════════════════════════════════════════════
# ═══════════════════════════════════════════════════════════════
# 任务4: 验证关键规则集配置
# ═══════════════════════════════════════════════════════════════
log_info "任务4: 验证关键规则集配置..."

# 检查 cnip 规则集（DNS防泄漏关键配置）
python3 - "$SINGBOX_CONFIG" <<'PYTHON_SCRIPT4'
import json
import sys

config_file = sys.argv[1]

with open(config_file, 'r', encoding='utf-8') as f:
    config = json.load(f)

# 检查 cnip 规则集定义
rule_sets = config.get('route', {}).get('rule_set', [])
cnip_defined = any(rs['tag'] == 'cnip' for rs in rule_sets)

# 检查 cnip 引用
inbounds = config.get('inbounds', [])
cnip_in_inbound = any(
    'route_exclude_address_set' in ib and ib['route_exclude_address_set'] == 'cnip'
    for ib in inbounds
)

rules = config.get('route', {}).get('rules', [])
cnip_in_rules = any(
    'rule_set' in rule and rule['rule_set'] == 'cnip'
    for rule in rules
)

print("\n关键规则集检查:")
print(f"  cnip 定义: {'✅ 已定义' if cnip_defined else '❌ 未定义'}")
print(f"  cnip inbound引用: {'✅ 已引用' if cnip_in_inbound else '❌ 未引用'}")
print(f"  cnip rules引用: {'✅ 已引用' if cnip_in_rules else '❌ 未引用'}")

if not cnip_defined:
    print("\n❌ 错误: cnip 规则集未定义！")
    print("   这会导致 Singbox 启动失败")
    print("   请运行: ./merge_sync/sync_all_configs.sh")
    sys.exit(1)

if cnip_defined and (cnip_in_inbound or cnip_in_rules):
    print("\n✅ cnip 规则集配置正确")
    print("   用途: DNS防泄漏 + 中国IP直连")

PYTHON_SCRIPT4

echo ""

# 总结
# ═══════════════════════════════════════════════════════════════
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    检查完成                                  ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"

if [ ${#missing_in_singbox[@]} -eq 0 ] && [ ${#extra_in_singbox[@]} -eq 0 ]; then
    log_success "所有配置完全同步！"
    exit 0
else
    log_warning "配置存在差异，请运行 sync_all_configs.sh 进行同步"
    exit 1
fi
