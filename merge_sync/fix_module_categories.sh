#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
# 修复模块分组字段 - 从根源修复
# 
# Surge使用 #!category= 来显示分组（不是 #!group=）
# 此脚本将：
# 1. 将 #!group= 的值复制到 #!category=
# 2. 删除 #!group= 字段（彻底清理）
# 3. 为没有分类的模块根据目录推断分类
# ═══════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
MODULE_DIR="$PROJECT_ROOT/module/surge(main)"
SHADOWROCKET_DIR="$PROJECT_ROOT/module/shadowrocket"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }

echo "═══════════════════════════════════════════════════════════════"
echo "修复模块分组字段 (从根源修复)"
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "Surge 使用 #!category= 字段显示分组（不是 #!group=）"
echo ""

fixed=0
total=0

process_module() {
    local module="$1"
    local filename=$(basename "$module")
    local dir=$(basename "$(dirname "$module")")
    local changed=false
    
    # 1. 如果有 #!group= 但没有 #!category=，复制值到 #!category=
    if grep -q "^#!group=" "$module" && ! grep -q "^#!category=" "$module"; then
        group_value=$(grep "^#!group=" "$module" | head -1 | sed 's/^#!group=//')
        
        # 在 #!group= 后添加 #!category=
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -i '' "/^#!group=/a\\
#!category=$group_value
" "$module"
        else
            sed -i "/^#!group=/a #!category=$group_value" "$module"
        fi
        changed=true
        log_success "添加 #!category: $filename"
    fi
    
    # 2. 如果有 #!group= 和 #!category=，确保 #!category= 有正确的值
    if grep -q "^#!group=" "$module" && grep -q "^#!category=" "$module"; then
        group_value=$(grep "^#!group=" "$module" | head -1 | sed 's/^#!group=//')
        category_value=$(grep "^#!category=" "$module" | head -1 | sed 's/^#!category=//')
        
        # 如果 category 值不是我们的分组名，用 group 值覆盖
        if [[ "$category_value" != "『"* ]]; then
            if [[ "$OSTYPE" == "darwin"* ]]; then
                sed -i '' "s|^#!category=.*|#!category=$group_value|" "$module"
            else
                sed -i "s|^#!category=.*|#!category=$group_value|" "$module"
            fi
            changed=true
            log_success "更新 #!category: $filename"
        fi
    fi
    
    # 3. 删除 #!group= 字段（彻底清理）
    if grep -q "^#!group=" "$module"; then
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -i '' '/^#!group=/d' "$module"
        else
            sed -i '/^#!group=/d' "$module"
        fi
        changed=true
        log_info "删除 #!group: $filename"
    fi
    
    # 4. 如果没有 #!category=，根据目录推断
    if ! grep -q "^#!category=" "$module"; then
        case "$dir" in
            amplify_nexus)
                category="『 🛠️ Amplify Nexus › 增幅枢纽 』"
                ;;
            head_expanse)
                category="『 🔝 Head Expanse › 首端扩域 』"
                ;;
            narrow_pierce)
                category="『 🎯 Narrow Pierce › 窄域穿刺 』"
                ;;
            *)
                return
                ;;
        esac
        
        # 在 #!name= 后添加 #!category=
        if grep -q "^#!name=" "$module"; then
            if [[ "$OSTYPE" == "darwin"* ]]; then
                sed -i '' "/^#!name=/a\\
#!category=$category
" "$module"
            else
                sed -i "/^#!name=/a #!category=$category" "$module"
            fi
        else
            echo -e "#!category=$category\n$(cat "$module")" > "$module"
        fi
        changed=true
        log_success "推断 #!category: $filename"
    fi
    
    if [ "$changed" = true ]; then
        fixed=$((fixed + 1))
    fi
}

# 处理 Surge 模块
log_info "处理 Surge 模块..."
for module in "$MODULE_DIR"/*/*.sgmodule "$MODULE_DIR"/*/*.module "$MODULE_DIR"/*.sgmodule "$MODULE_DIR"/*.module; do
    [ ! -f "$module" ] && continue
    total=$((total + 1))
    process_module "$module"
done

# 处理 Shadowrocket 模块（同样删除 #!group=，注释掉 #!category=）
log_info ""
log_info "处理 Shadowrocket 模块..."
for module in "$SHADOWROCKET_DIR"/*/*.sgmodule "$SHADOWROCKET_DIR"/*/*.module "$SHADOWROCKET_DIR"/*.sgmodule "$SHADOWROCKET_DIR"/*.module; do
    [ ! -f "$module" ] && continue
    
    filename=$(basename "$module")
    
    # 删除 #!group=
    if grep -q "^#!group=" "$module"; then
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -i '' '/^#!group=/d' "$module"
        else
            sed -i '/^#!group=/d' "$module"
        fi
        log_info "删除 #!group (SR): $filename"
    fi
    
    # 注释掉 #!category=（Shadowrocket 不使用）
    if grep -q "^#!category=" "$module"; then
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -i '' 's/^#!category=/#!category (Surge only): /' "$module"
        else
            sed -i 's/^#!category=/#!category (Surge only): /' "$module"
        fi
        log_info "注释 #!category (SR): $filename"
    fi
done

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "修复完成"
echo "═══════════════════════════════════════════════════════════════"
echo "Surge 模块总数: $total"
echo "已修复: $fixed"
echo ""
log_success "所有模块已从根源修复！"
log_info "- Surge: 使用 #!category= 字段"
log_info "- Shadowrocket: #!category= 已注释"
log_info "- #!group= 字段已全部删除"
