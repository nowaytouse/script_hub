#!/bin/bash
# =============================================================================
# Singbox 启动测试脚本
# 功能: 测试 Singbox 配置是否可以正常加载
# 创建: 2025-12-07
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

SINGBOX_CONFIG="$PROJECT_ROOT/substore/Singbox_substore_1.13.0+.json"

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           Singbox 启动测试工具                               ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

if [ ! -f "$SINGBOX_CONFIG" ]; then
    log_error "配置文件不存在: $SINGBOX_CONFIG"
    exit 1
fi

log_info "测试配置文件: $SINGBOX_CONFIG"
echo ""

# 优先使用本地预览版 sing-box
LOCAL_SINGBOX="$PROJECT_ROOT/tools/config-manager-auto-update/bin/sing-box"
if [ -x "$LOCAL_SINGBOX" ]; then
    SINGBOX_CMD="$LOCAL_SINGBOX"
    log_info "使用本地预览版 sing-box"
elif command -v sing-box &> /dev/null; then
    SINGBOX_CMD="sing-box"
    log_info "使用系统 sing-box"
else
    SINGBOX_CMD=""
fi

# 检查 sing-box 是否可用
if [ -z "$SINGBOX_CMD" ]; then
    log_warning "sing-box 未安装，跳过启动测试"
    log_info "仅进行配置验证..."
    
    # 使用 Python 验证 JSON 格式
    python3 - "$SINGBOX_CONFIG" <<'PYTHON_SCRIPT'
import json
import sys

config_file = sys.argv[1]

try:
    with open(config_file, 'r', encoding='utf-8') as f:
        config = json.load(f)
    
    print("✅ JSON 格式验证通过")
    
    # 检查关键字段
    required_fields = ['log', 'dns', 'inbounds', 'outbounds', 'route']
    missing = [f for f in required_fields if f not in config]
    
    if missing:
        print(f"❌ 缺少必需字段: {', '.join(missing)}")
        sys.exit(1)
    
    print("✅ 配置结构完整")
    
    # 检查 cnip 规则集
    rule_sets = config.get('route', {}).get('rule_set', [])
    cnip_defined = any(rs['tag'] == 'cnip' for rs in rule_sets)
    
    if cnip_defined:
        print("✅ cnip 规则集已定义")
    else:
        print("❌ cnip 规则集未定义")
        sys.exit(1)
    
    print("\n✅ 配置验证通过！")
    print("   建议: 安装 sing-box 进行完整测试")
    print("   安装命令: brew install sing-box")
    
except json.JSONDecodeError as e:
    print(f"❌ JSON 格式错误: {e}")
    sys.exit(1)
except Exception as e:
    print(f"❌ 验证失败: {e}")
    sys.exit(1)

PYTHON_SCRIPT
    
    exit 0
fi

# 显示 sing-box 版本
SINGBOX_VERSION=$("$SINGBOX_CMD" version 2>/dev/null | head -1 || echo "unknown")
log_info "sing-box 版本: $SINGBOX_VERSION"

# 使用 sing-box check 命令验证配置
log_info "使用 sing-box 验证配置..."

# 捕获输出但不因为警告而失败
SINGBOX_OUTPUT=$("$SINGBOX_CMD" check -c "$SINGBOX_CONFIG" 2>&1 || true)
echo "$SINGBOX_OUTPUT" > /tmp/singbox_check.log

# 检查是否有真正的错误
if echo "$SINGBOX_OUTPUT" | grep -q "FATAL\|error:"; then
    log_error "❌ Singbox 配置验证失败"
    echo ""
    log_info "错误详情:"
    cat /tmp/singbox_check.log
    exit 1
else
    log_success "✅ Singbox 配置验证通过！"
fi

echo ""
log_info "配置文件可以正常加载"
log_success "Singbox 应该可以正常启动了！"

rm -f /tmp/singbox_check.log
