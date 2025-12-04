#!/bin/bash

# 配置验证脚本
# 用途：验证 cm_config.json 配置是否正确

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

CONFIG_FILE="cm_config.json"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# 检查配置文件是否存在
if [ ! -f "$CONFIG_FILE" ]; then
    print_error "配置文件不存在: $CONFIG_FILE"
    exit 1
fi

print_info "验证配置文件: $CONFIG_FILE"
print_info "================================"

# 验证 JSON 格式
if ! jq empty "$CONFIG_FILE" 2>/dev/null; then
    print_error "JSON 格式错误"
    exit 1
fi
print_success "JSON 格式正确"

# 检查必要字段
print_info "检查必要字段..."

# 检查 v2rayn_bin_path
V2RAYN_PATH=$(jq -r '.v2rayn_bin_path' "$CONFIG_FILE")
if [ -z "$V2RAYN_PATH" ] || [ "$V2RAYN_PATH" = "null" ]; then
    print_error "缺少 v2rayn_bin_path"
    exit 1
fi

# 展开路径中的 ~
V2RAYN_PATH="${V2RAYN_PATH/#\~/$HOME}"

if [ -d "$V2RAYN_PATH" ]; then
    print_success "v2rayn_bin_path 存在: $V2RAYN_PATH"
else
    print_warning "v2rayn_bin_path 不存在（可能需要创建）: $V2RAYN_PATH"
fi

# 检查 conf_save_path
CONF_PATH=$(jq -r '.conf_save_path' "$CONFIG_FILE")
if [ -z "$CONF_PATH" ] || [ "$CONF_PATH" = "null" ]; then
    print_error "缺少 conf_save_path"
    exit 1
fi

# 展开路径中的 ~
CONF_PATH="${CONF_PATH/#\~/$HOME}"

if [ -d "$CONF_PATH" ]; then
    print_success "conf_save_path 存在: $CONF_PATH"
else
    print_warning "conf_save_path 不存在（可能需要创建）: $CONF_PATH"
fi

# 检查内核配置
print_info "检查内核配置..."
CORES=$(jq '.cores | keys[]' "$CONFIG_FILE" -r)
for core in $CORES; do
    REPO=$(jq -r ".cores.$core.repo" "$CONFIG_FILE")
    BINARY=$(jq -r ".cores.$core.binary_name" "$CONFIG_FILE")
    SUBDIR=$(jq -r ".cores.$core.subdir" "$CONFIG_FILE")
    
    if [ -z "$REPO" ] || [ "$REPO" = "null" ]; then
        print_error "内核 $core 缺少 repo"
        exit 1
    fi
    
    if [ -z "$BINARY" ] || [ "$BINARY" = "null" ]; then
        print_error "内核 $core 缺少 binary_name"
        exit 1
    fi
    
    if [ -z "$SUBDIR" ] || [ "$SUBDIR" = "null" ]; then
        print_error "内核 $core 缺少 subdir"
        exit 1
    fi
    
    print_success "内核 $core 配置正确 (repo: $REPO)"
done

# 检查 geofiles
print_info "检查 geofiles..."
GEOFILES=$(jq '.geofiles | keys[]' "$CONFIG_FILE" -r)
GEOFILE_COUNT=$(echo "$GEOFILES" | wc -l)
print_success "找到 $GEOFILE_COUNT 个 geofiles"

# 检查配置文件
print_info "检查配置文件..."
CONFIGS=$(jq '.configs | length' "$CONFIG_FILE")
if [ "$CONFIGS" -gt 0 ]; then
    print_success "找到 $CONFIGS 个配置文件"
    jq '.configs[] | "\(.name)"' "$CONFIG_FILE" -r | while read -r name; do
        print_info "  - $name"
    done
else
    print_warning "未配置任何配置文件"
fi

# 总结
print_info "================================"
print_success "配置验证完成！"
print_info "================================"

# 显示配置摘要
echo ""
echo "📋 配置摘要："
echo "  v2rayn_bin_path: $V2RAYN_PATH"
echo "  conf_save_path: $CONF_PATH"
echo "  内核数量: $(echo "$CORES" | wc -l)"
echo "  Geofiles 数量: $GEOFILE_COUNT"
echo "  配置文件数量: $CONFIGS"
echo ""
