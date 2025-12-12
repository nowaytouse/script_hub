#!/opt/homebrew/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
# Module Verification Script
# 
# 验证所有模块的完整性和兼容性
# ═══════════════════════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MODULE_DIR="$PROJECT_ROOT/module/surge(main)"
SHADOWROCKET_MODULE_DIR="$PROJECT_ROOT/module/shadowrocket"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;36m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }
log_header() { echo -e "\n${CYAN}═══════════════════════════════════════════════════════════════${NC}"; echo -e "${CYAN}$1${NC}"; echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"; }

# ═══════════════════════════════════════════════════════════════════════════════
# 验证函数
# ═══════════════════════════════════════════════════════════════════════════════

check_module_metadata() {
    local module="$1"
    local filename=$(basename "$module")
    local issues=0
    
    # 检查必需的元数据
    if ! grep -q "^#!name=" "$module"; then
        log_warning "  缺少 #!name: $filename"
        issues=$((issues + 1))
    fi
    
    if ! grep -q "^#!desc=" "$module"; then
        log_warning "  缺少 #!desc: $filename"
        issues=$((issues + 1))
    fi
    
    # 检查分组标签
    if ! grep -q "^#!group=" "$module" && ! grep -q "^#!category=" "$module"; then
        log_warning "  缺少分组标签: $filename"
        issues=$((issues + 1))
    fi
    
    echo $issues
}

check_shadowrocket_compatibility() {
    local module="$1"
    local filename=$(basename "$module")
    local issues=0
    
    # 检查Surge专属字段
    if grep -q "^#!system=" "$module"; then
        local system=$(grep "^#!system=" "$module" | sed 's/^#!system=//')
        if [ "$system" = "mac" ] || [ "$system" = "ios" ]; then
            log_warning "  包含系统限制: $filename (system=$system)"
            issues=$((issues + 1))
        fi
    fi
    
    # 检查arguments（Shadowrocket不完全支持）
    if grep -q "^#!arguments=" "$module"; then
        log_info "  使用arguments参数: $filename (Shadowrocket可能不完全支持)"
    fi
    
    echo $issues
}

# ═══════════════════════════════════════════════════════════════════════════════
# 主验证流程
# ═══════════════════════════════════════════════════════════════════════════════

log_header "模块完整性验证"

total_modules=0
metadata_issues=0
compat_issues=0

log_info "检查Surge模块..."
for module in "$MODULE_DIR"/*/*.sgmodule "$MODULE_DIR"/*/*.module; do
    [ ! -f "$module" ] && continue
    total_modules=$((total_modules + 1))
    
    result=$(check_module_metadata "$module")
    metadata_issues=$((metadata_issues + result))
done

log_info "检查Shadowrocket兼容性..."
for module in "$MODULE_DIR"/*/*.sgmodule "$MODULE_DIR"/*/*.module; do
    [ ! -f "$module" ] && continue
    
    result=$(check_shadowrocket_compatibility "$module")
    compat_issues=$((compat_issues + result))
done

log_header "验证结果"
log_info "总模块数: $total_modules"
if [ $metadata_issues -eq 0 ]; then
    log_success "元数据检查: 全部通过"
else
    log_warning "元数据问题: $metadata_issues 个"
fi

if [ $compat_issues -eq 0 ]; then
    log_success "兼容性检查: 全部兼容"
else
    log_warning "兼容性问题: $compat_issues 个"
fi

log_header "分组统计"
log_info "amplify_nexus: $(ls "$MODULE_DIR"/amplify_nexus/ 2>/dev/null | wc -l | tr -d ' ') 个模块"
log_info "head_expanse: $(ls "$MODULE_DIR"/head_expanse/ 2>/dev/null | wc -l | tr -d ' ') 个模块"
log_info "narrow_pierce: $(ls "$MODULE_DIR"/narrow_pierce/ 2>/dev/null | wc -l | tr -d ' ') 个模块"

log_success "验证完成！"
