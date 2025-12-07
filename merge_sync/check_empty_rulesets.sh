#!/bin/bash

# ═══════════════════════════════════════════════════════════════
# Empty Ruleset Checker
# 检查并报告空规则集的状态
# ═══════════════════════════════════════════════════════════════

RULESET_DIR="ruleset/Surge(Shadowkroket)"
SOURCES_DIR="ruleset/Sources/Links"

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
        
        # 检查是否有sources文件
        sources_file="$SOURCES_DIR/${ruleset_name}_sources.txt"
        
        if [ -f "$sources_file" ]; then
            has_sources=$((has_sources + 1))
            
            # 检查sources文件内容
            source_urls=$(grep -v '^#' "$sources_file" | grep -v '^$' | grep -E '^(http|\.\./)' | wc -l | tr -d ' ')
            
            if [ "$source_urls" -gt 0 ]; then
                echo "⚠️  $filename (0 rules)"
                echo "    Sources: $sources_file ($source_urls sources)"
                echo "    Status: Sources exist but returned empty"
                echo ""
            else
                truly_empty=$((truly_empty + 1))
                echo "ℹ️  $filename (0 rules)"
                echo "    Sources: $sources_file (intentionally empty)"
                echo "    Status: OK - Documented as empty"
                echo ""
            fi
        else
            truly_empty=$((truly_empty + 1))
            echo "❌ $filename (0 rules)"
            echo "    Sources: NOT FOUND"
            echo "    Status: Missing sources file"
            echo ""
        fi
    fi
done

echo "╔══════════════════════════════════════════╗"
echo "║            Summary                       ║"
echo "╠══════════════════════════════════════════╣"
printf "║  Total Rulesets:     %-18s ║\n" "$total_rulesets"
printf "║  Empty Rulesets:     %-18s ║\n" "$empty_rulesets"
printf "║  With Sources:       %-18s ║\n" "$has_sources"
printf "║  Truly Empty:        %-18s ║\n" "$truly_empty"
echo "╚══════════════════════════════════════════╝"
echo ""

# 建议
if [ "$empty_rulesets" -gt 0 ]; then
    echo "📋 Recommendations:"
    echo ""
    echo "1. Review sources files for empty rulesets"
    echo "2. Check if upstream sources are still available (404?)"
    echo "3. Consider using alternative sources or GEOIP rules"
    echo "4. Document intentionally empty rulesets in sources file"
    echo ""
fi

exit 0
