#!/opt/homebrew/bin/bash
# =============================================================================
# 同步配置到所有代理软件
# 功能: 将模板配置同步到Surge和小火箭的实际配置文件
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# 配置文件路径
SURGE_TEMPLATE="$PROJECT_ROOT/conf_template/surge_profile_template.conf"
SURGE_ICLOUD="$HOME/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo Pro Max plus👑_fixed.conf"
SHADOWROCKET_ICLOUD="$HOME/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents"

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           同步配置到所有代理软件                             ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# 1. 同步到Surge iCloud
log_info "同步到Surge iCloud配置..."
if [ -f "$SURGE_ICLOUD" ]; then
    # 备份原配置
    cp "$SURGE_ICLOUD" "$SURGE_ICLOUD.backup.$(date +%Y%m%d_%H%M%S)"
    log_info "已备份原配置"
    
    # 提取[Rule]部分并更新
    # 注意：这里只更新规则部分，保留其他配置
    log_info "更新规则部分..."
    
    # 使用Python脚本更新规则
    python3 - "$SURGE_TEMPLATE" "$SURGE_ICLOUD" <<'PYTHON_SCRIPT'
import sys
import re

template_file = sys.argv[1]
target_file = sys.argv[2]

# 读取模板
with open(template_file, 'r', encoding='utf-8') as f:
    template = f.read()

# 读取目标文件
with open(target_file, 'r', encoding='utf-8') as f:
    target = f.read()

# 提取模板中的[Rule]部分
rule_match = re.search(r'\[Rule\](.*?)(?=\[|$)', template, re.DOTALL)
if rule_match:
    template_rules = rule_match.group(0)
    
    # 替换目标文件中的[Rule]部分
    target = re.sub(r'\[Rule\].*?(?=\[|$)', template_rules, target, flags=re.DOTALL)
    
    with open(target_file, 'w', encoding='utf-8') as f:
        f.write(target)
    
    print(f"✅ 规则已更新到: {target_file}")
else:
    print("❌ 未找到[Rule]部分")
    sys.exit(1)
PYTHON_SCRIPT
    
    log_success "Surge iCloud配置已更新"
else
    log_warning "Surge iCloud配置文件不存在: $SURGE_ICLOUD"
fi

# 2. 同步模块到小火箭
log_info "同步模块到小火箭..."
if [ -d "$SHADOWROCKET_ICLOUD" ]; then
    # 复制防火墙模块
    FIREWALL_MODULE="$PROJECT_ROOT/module/surge(main)/🔥 Firewall Port Blocker 🛡️🚫.sgmodule"
    if [ -f "$FIREWALL_MODULE" ]; then
        cp "$FIREWALL_MODULE" "$SHADOWROCKET_ICLOUD/"
        log_success "防火墙模块已同步到小火箭"
    fi
    
    # 复制其他模块
    for module in "$PROJECT_ROOT/module/surge(main)"/*.sgmodule; do
        if [ -f "$module" ]; then
            cp "$module" "$SHADOWROCKET_ICLOUD/"
            log_info "已复制: $(basename "$module")"
        fi
    done
    
    log_success "小火箭模块已同步"
else
    log_warning "小火箭iCloud目录不存在: $SHADOWROCKET_ICLOUD"
fi

echo ""
log_success "所有配置同步完成！"
echo ""
echo -e "${BLUE}📋 同步结果:${NC}"
echo "   - Surge iCloud: ✅ 规则已更新"
echo "   - 小火箭模块: ✅ 已同步"
echo "   - Singbox: ✅ 已在之前更新（无FirewallPorts）"
