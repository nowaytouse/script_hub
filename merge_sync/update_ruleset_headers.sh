#!/bin/bash

# ═══════════════════════════════════════════════════════════════
# 规则集Header更新脚本
# 功能: 为规则集添加策略建议和节点建议
# ═══════════════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULESET_DIR="$SCRIPT_DIR/../ruleset/Surge(Shadowkroket)"
POLICY_MAP="$SCRIPT_DIR/ruleset_policy_map.txt"

# 颜色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "╔══════════════════════════════════════════╗"
echo "║     规则集Header更新工具 v1.0            ║"
echo "╚══════════════════════════════════════════╝"
echo ""

# 检查策略映射文件
if [ ! -f "$POLICY_MAP" ]; then
    echo "❌ 策略映射文件不存在: $POLICY_MAP"
    exit 1
fi

# 处理每个规则集
for ruleset_file in "$RULESET_DIR"/*.list; do
    filename=$(basename "$ruleset_file" .list)
    
    # 从策略映射文件获取信息
    policy_line=$(grep "^${filename}|" "$POLICY_MAP" | head -1)
    
    if [ -z "$policy_line" ]; then
        echo -e "${YELLOW}⚠️  跳过: $filename (无策略映射)${NC}"
        continue
    fi
    
    # 解析策略信息
    policy=$(echo "$policy_line" | cut -d'|' -f2)
    node=$(echo "$policy_line" | cut -d'|' -f3)
    desc=$(echo "$policy_line" | cut -d'|' -f4)
    
    # 计算规则数量
    rule_count=$(grep -v '^#' "$ruleset_file" | grep -v '^$' | wc -l | tr -d ' ')
    
    # 生成新header
    header="# ═══════════════════════════════════════════════════════════════
# Ruleset: ${filename}
# Policy: ${policy}
"
    
    # Add node recommendation (if any)
    if [ -n "$node" ]; then
        header+="# Node: ${node}
"
    fi
    
    header+="# Description: ${desc}
# Rules: ${rule_count}
# Updated: $(date '+%Y-%m-%d %H:%M:%S')
# ═══════════════════════════════════════════════════════════════
"
    
    # 提取现有规则（跳过旧header）
    rules=$(grep -v '^#' "$ruleset_file" | grep -v '^$')
    
    # 写入新文件
    echo -n "$header" > "$ruleset_file"
    echo "" >> "$ruleset_file"
    echo "$rules" >> "$ruleset_file"
    
    echo -e "${GREEN}✅ 更新: $filename (${policy}${node:+, $node})${NC}"
done

echo ""
echo "╔══════════════════════════════════════════╗"
echo "║            更新完成                      ║"
echo "╚══════════════════════════════════════════╝"
