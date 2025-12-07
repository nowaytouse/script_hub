#!/bin/bash
# =============================================================================
# Singbox 配置验证脚本
# 功能: 验证 Singbox 配置文件的完整性和正确性
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
echo -e "${BLUE}║           Singbox 配置验证工具                               ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

if [ ! -f "$SINGBOX_CONFIG" ]; then
    log_error "配置文件不存在: $SINGBOX_CONFIG"
    exit 1
fi

log_info "验证配置文件: $SINGBOX_CONFIG"
echo ""

# 使用 Python 验证 JSON 格式和规则集引用
python3 - "$SINGBOX_CONFIG" <<'PYTHON_SCRIPT'
import json
import sys
from pathlib import Path

config_file = sys.argv[1]

try:
    # 读取配置
    with open(config_file, 'r', encoding='utf-8') as f:
        config = json.load(f)
    
    print("✅ JSON 格式验证通过")
    
    # 提取所有规则集定义
    rule_sets = config.get("route", {}).get("rule_set", [])
    rule_set_tags = {rs["tag"] for rs in rule_sets}
    
    print(f"✅ 找到 {len(rule_set_tags)} 个规则集定义")
    
    # 提取所有规则集引用
    referenced_tags = set()
    
    # 检查路由规则中的引用
    rules = config.get("route", {}).get("rules", [])
    for rule in rules:
        if "rule_set" in rule:
            if isinstance(rule["rule_set"], list):
                referenced_tags.update(rule["rule_set"])
            else:
                referenced_tags.add(rule["rule_set"])
    
    # 检查 DNS 规则中的引用
    dns_rules = config.get("dns", {}).get("rules", [])
    for rule in dns_rules:
        if "rule_set" in rule:
            if isinstance(rule["rule_set"], list):
                referenced_tags.update(rule["rule_set"])
            else:
                referenced_tags.add(rule["rule_set"])
    
    # 检查 inbound 中的引用
    inbounds = config.get("inbounds", [])
    for inbound in inbounds:
        if "route_exclude_address_set" in inbound:
            referenced_tags.add(inbound["route_exclude_address_set"])
    
    print(f"✅ 找到 {len(referenced_tags)} 个规则集引用")
    
    # 检查缺失的规则集
    missing = referenced_tags - rule_set_tags
    if missing:
        print(f"\n❌ 发现 {len(missing)} 个缺失的规则集定义:")
        for tag in sorted(missing):
            print(f"   - {tag}")
        sys.exit(1)
    else:
        print("✅ 所有引用的规则集都已定义")
    
    # 检查未使用的规则集
    unused = rule_set_tags - referenced_tags
    if unused:
        print(f"\n⚠️  发现 {len(unused)} 个未使用的规则集:")
        for tag in sorted(unused):
            print(f"   - {tag}")
    
    # 统计信息
    print("\n" + "="*60)
    print("配置统计:")
    print(f"  规则集定义: {len(rule_set_tags)}")
    print(f"  规则集引用: {len(referenced_tags)}")
    print(f"  路由规则数: {len(rules)}")
    print(f"  DNS规则数: {len(dns_rules)}")
    print(f"  入站数: {len(inbounds)}")
    print(f"  出站数: {len(config.get('outbounds', []))}")
    print("="*60)
    
    print("\n✅ 配置验证通过！")
    
except json.JSONDecodeError as e:
    print(f"❌ JSON 格式错误: {e}")
    sys.exit(1)
except Exception as e:
    print(f"❌ 验证失败: {e}")
    import traceback
    traceback.print_exc()
    sys.exit(1)

PYTHON_SCRIPT

echo ""
log_success "Singbox 配置验证完成"
