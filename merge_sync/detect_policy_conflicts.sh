#!/bin/bash
# ============================================
# 策略冲突检测脚本
# 检测规则集之间的策略混入问题
# ============================================

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULESET_DIR="${SCRIPT_DIR}/../ruleset/Surge(Shadowkroket)"

total_conflicts=0

echo "╔══════════════════════════════════════════╗"
echo "║     策略冲突检测 - Policy Conflict      ║"
echo "╚══════════════════════════════════════════╝"
echo ""

# 1. 检查广告域名混入非AdBlock规则集
echo "━━━ 1. 检测广告域名混入 ━━━"
ad_keywords="doubleclick|googleads|adservice\.google|ad\.doubleclick|pagead|adsystem"

for ruleset in "$RULESET_DIR"/*.list; do
    name=$(basename "$ruleset")
    # 跳过AdBlock相关规则集
    if [[ "$name" =~ AdBlock|NSFW ]]; then continue; fi
    
    # 检查是否包含广告域名
    conflicts=$(grep -v "^#" "$ruleset" | grep -E "DOMAIN.*($ad_keywords)" 2>/dev/null | wc -l | tr -d ' ')
    
    if [ "$conflicts" -gt 0 ]; then
        echo -e "${RED}⚠️  $name: $conflicts 条广告域名（应该在AdBlock中）${NC}"
        grep -v "^#" "$ruleset" | grep -E "DOMAIN.*($ad_keywords)" | head -3
        echo ""
        ((total_conflicts += conflicts))
    fi
done

# 2. 检查真正的LAN地址混入REJECT规则集
echo "━━━ 2. 检测真正的LAN地址混入REJECT规则集 ━━━"
echo "   ℹ️  注意: 以下是合法的REJECT规则，不是错误："
echo "   ℹ️    - 广告服务器IP（如10.72.25.0/24）"
echo "   ℹ️    - 云服务元数据地址（169.254.169.254 - AWS/Azure）"
echo "   ℹ️  只检测明显错误的本地网络地址（192.168.0.0/16, 127.0.0.0/8）"
echo ""

for ruleset in "$RULESET_DIR"/*.list; do
    name=$(basename "$ruleset")
    # 只检查AdBlock相关规则集
    if [[ ! "$name" =~ AdBlock|NSFW ]]; then continue; fi
    
    # 🔥 只检测明显错误的本地网络地址
    # 192.168.0.0/16 - 家庭网络（不应该在REJECT中）
    # 127.0.0.0/8 - 本地回环（不应该在REJECT中）
    # 排除: 169.254.169.254（AWS元数据服务，应该在REJECT中）
    conflicts=$(grep -v "^#" "$ruleset" | grep -E "^IP-CIDR,(192\.168\.|127\.)" 2>/dev/null | wc -l | tr -d ' ')
    
    if [ "$conflicts" -gt 0 ]; then
        echo -e "${RED}⚠️  $name: $conflicts 条本地网络地址（不应该在REJECT中）${NC}"
        grep -v "^#" "$ruleset" | grep -E "^IP-CIDR,(192\.168\.|127\.)" | head -3
        echo ""
        ((total_conflicts += conflicts))
    fi
done

# 3. 检查规则重复（跨规则集）
echo "━━━ 3. 检测规则重复 ━━━"

# 创建临时文件存储所有规则
temp_dir=$(mktemp -d)
trap "rm -rf $temp_dir" EXIT

for ruleset in "$RULESET_DIR"/*.list; do
    name=$(basename "$ruleset" .list)
    grep -v "^#" "$ruleset" | grep -E "^(DOMAIN|IP-CIDR)" > "$temp_dir/$name.rules" 2>/dev/null || touch "$temp_dir/$name.rules"
done

# 检查重复
for file1 in "$temp_dir"/*.rules; do
    name1=$(basename "$file1" .rules)
    
    for file2 in "$temp_dir"/*.rules; do
        name2=$(basename "$file2" .rules)
        
        # 跳过自己和已检查的组合
        if [[ "$name1" == "$name2" ]] || [[ "$name1" > "$name2" ]]; then
            continue
        fi
        
        # 计算重复规则数
        duplicates=$(comm -12 <(sort "$file1") <(sort "$file2") | wc -l | tr -d ' ')
        
        if [ "$duplicates" -gt 5 ]; then
            echo -e "${YELLOW}⚠️  $name1 ↔ $name2: $duplicates 条重复规则${NC}"
            ((total_conflicts += duplicates))
        fi
    done
done

echo ""
echo "╔══════════════════════════════════════════╗"
echo "║            检测结果                      ║"
echo "╠══════════════════════════════════════════╣"
if [ $total_conflicts -eq 0 ]; then
    echo -e "║  ${GREEN}✅ 未发现策略冲突${NC}                    ║"
else
    echo -e "║  ${RED}⚠️  发现 $total_conflicts 处冲突${NC}                  ║"
    echo "║                                          ║"
    echo "║  建议:                                   ║"
    echo "║  1. 检查源文件策略分类                   ║"
    echo "║  2. 运行 smart_cleanup.py 去重           ║"
    echo "║  3. 手动修复策略混入问题                 ║"
fi
echo "╚══════════════════════════════════════════╝"

exit $total_conflicts
