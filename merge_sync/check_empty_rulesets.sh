#!/bin/bash

# ═══════════════════════════════════════════════════════════════
# Empty Ruleset Checker
# 检查并报告空规则集的状态
# ═══════════════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULESET_DIR="$SCRIPT_DIR/../ruleset/Surge(Shadowkroket)"
SOURCES_DIR="$SCRIPT_DIR/../ruleset/Sources/Links"

echo "╔══════════════════════════════════════════╗"
echo "║     Empty Ruleset Checker v1.0           ║"
echo "╚══════════════════════════════════════════╝"
echo ""

# 统计变量
total_rulesets=0
empty_rulesets=0
truly_empty=0
has_sources=0

# 检查所有.list文件
for ruleset_file in "$RULESET_DIR"/*.list; do
    filename=$(basename "$ruleset_file")
    ruleset_name="${filename%.list}"
    total_rulesets=$((total_rulesets + 1))
    
    # 计算规则数量（排除注释和空行）
    rule_count=$(grep -v '^#' "$ruleset_file" | grep -v '^$' | grep -v '^\s*$' | wc -l | tr -d ' ')
    
    if [ "$rule_count" -eq 0 ]; then
        empty_rulesets=$((empty_rulesets + 1))
        
        # 检查是否有对应的sources文件
        sources_file="$SOURCES_DIR/${ruleset_name}_sources.txt"
        if [ -f "$sources_file" ]; then
            sources_count=$(grep -v '^#' "$sources_file" | grep -v '^$' | wc -l | tr -d ' ')
            if [ "$sources_count" -gt 0 ]; then
                echo "⚠️  $filename: 空规则集，但有 $sources_count 个sources (需要合并)"
                has_sources=$((has_sources + 1))
            else
                echo "❌ $filename: 空规则集，sources也为空"
                truly_empty=$((truly_empty + 1))
            fi
        else
            echo "❌ $filename: 空规则集，无sources文件"
            truly_empty=$((truly_empty + 1))
        fi
    fi
done

echo ""
echo "╔══════════════════════════════════════════╗"
echo "║            检查结果                      ║"
echo "╠══════════════════════════════════════════╣"
printf "║  Total rulesets:  %-21s ║\n" "$total_rulesets"
printf "║  Empty rulesets:  %-21s ║\n" "$empty_rulesets"
printf "║  Has sources:     %-21s ║\n" "$has_sources"
printf "║  Truly empty:     %-21s ║\n" "$truly_empty"
echo "╚══════════════════════════════════════════╝"

if [ "$truly_empty" -gt 0 ]; then
    echo ""
    echo "ℹ️  建议: 运行 cleanup_empty_rulesets.sh 清理真正空的规则集"
fi

if [ "$has_sources" -gt 0 ]; then
    echo ""
    echo "ℹ️  建议: 运行 incremental_merge_all.sh 合并sources到规则集"
fi
