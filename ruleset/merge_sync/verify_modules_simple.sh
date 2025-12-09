#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
# 模块验证脚本 (简化版)
# ═══════════════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MODULE_DIR="$PROJECT_ROOT/module/surge(main)"

echo "═══════════════════════════════════════════════════════════════"
echo "模块完整性验证"
echo "═══════════════════════════════════════════════════════════════"
echo ""

total=0
missing_name=0
missing_desc=0
missing_group=0
has_system=0
has_arguments=0

for module in "$MODULE_DIR"/*/*.sgmodule "$MODULE_DIR"/*/*.module; do
    [ ! -f "$module" ] && continue
    total=$((total + 1))
    filename=$(basename "$module")
    
    # 检查元数据
    grep -q "^#!name=" "$module" || { echo "⚠️  缺少 #!name: $filename"; missing_name=$((missing_name + 1)); }
    grep -q "^#!desc=" "$module" || { echo "⚠️  缺少 #!desc: $filename"; missing_desc=$((missing_desc + 1)); }
    
    # 检查分组
    if ! grep -q "^#!group=" "$module" && ! grep -q "^#!category=" "$module"; then
        echo "⚠️  缺少分组: $filename"
        missing_group=$((missing_group + 1))
    fi
    
    # 检查兼容性
    if grep -q "^#!system=" "$module"; then
        system=$(grep "^#!system=" "$module" | head -1 | sed 's/^#!system=//')
        echo "ℹ️  系统限制: $filename (system=$system)"
        has_system=$((has_system + 1))
    fi
    
    if grep -q "^#!arguments=" "$module"; then
        has_arguments=$((has_arguments + 1))
    fi
done

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "验证结果"
echo "═══════════════════════════════════════════════════════════════"
echo "总模块数: $total"
echo "缺少 #!name: $missing_name"
echo "缺少 #!desc: $missing_desc"
echo "缺少分组标签: $missing_group"
echo "有系统限制: $has_system"
echo "使用arguments: $has_arguments"
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "分组统计"
echo "═══════════════════════════════════════════════════════════════"
echo "amplify_nexus: $(ls "$MODULE_DIR"/amplify_nexus/ 2>/dev/null | wc -l | tr -d ' ') 个模块"
echo "head_expanse: $(ls "$MODULE_DIR"/head_expanse/ 2>/dev/null | wc -l | tr -d ' ') 个模块"
echo "narrow_pierce: $(ls "$MODULE_DIR"/narrow_pierce/ 2>/dev/null | wc -l | tr -d ' ') 个模块"
echo ""
echo "✅ 验证完成！"
