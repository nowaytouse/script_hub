#!/bin/bash

# Sing-box Manager 启用脚本
# 用途：编译、配置和启动工具

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 打印带颜色的消息
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

# 获取脚本所在目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

print_info "Sing-box Manager 启用脚本"
print_info "=========================="

# 检查 Rust 工具链
print_info "检查 Rust 工具链..."
if ! command -v cargo &> /dev/null; then
    print_error "未找到 cargo，请先安装 Rust"
    echo "访问 https://rustup.rs/ 安装 Rust"
    exit 1
fi
print_success "Rust 工具链已安装"

# 检查配置文件
print_info "检查配置文件..."
if [ ! -f "cm_config.json" ]; then
    print_warning "未找到 cm_config.json，使用默认配置"
    cp cm_config.json.example cm_config.json 2>/dev/null || print_warning "请手动创建 cm_config.json"
else
    print_success "配置文件已存在"
fi

# 编译项目
print_info "编译 Rust 项目..."
print_info "这可能需要几分钟..."

if cargo build --release; then
    print_success "编译成功"
else
    print_error "编译失败"
    exit 1
fi

# 检查二进制文件
BINARY="target/release/singbox-manager"
if [ -f "$BINARY" ]; then
    print_success "二进制文件已生成: $BINARY"
    chmod +x "$BINARY"
else
    print_error "二进制文件生成失败"
    exit 1
fi

# 显示使用方法
print_info "启用完成！"
print_info "=========================="
print_info "使用方法："
echo ""
echo "  交互式模式（推荐）："
echo "    $BINARY --interactive"
echo ""
echo "  命令行模式："
echo "    $BINARY core              # 更新所有内核"
echo "    $BINARY core singbox      # 只更新 sing-box"
echo "    $BINARY geo               # 更新 geofiles"
echo "    $BINARY conf              # 更新配置文件"
echo "    $BINARY all               # 全部更新"
echo "    $BINARY status            # 查看状态"
echo ""
echo "  一键更新："
echo "    $BINARY --once all"
echo ""

# 提示配置
print_warning "请确保 cm_config.json 中的路径正确"
print_warning "特别是 v2rayn_bin_path 和 conf_save_path"

# 询问是否立即运行
read -p "是否立即运行交互式菜单？(y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    print_info "启动交互式菜单..."
    "$BINARY" --interactive
else
    print_info "启用完成，稍后可手动运行工具"
fi
