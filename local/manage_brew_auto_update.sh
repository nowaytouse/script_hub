#!/opt/homebrew/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
# Homebrew Auto Update 管理脚本
# 用于安装、启动、停止、查看状态
# ═══════════════════════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLIST_FILE="$SCRIPT_DIR/com.homebrew.auto-update.plist"
PLIST_DEST="$HOME/Library/LaunchAgents/com.homebrew.auto-update.plist"
LOG_DIR="$HOME/.brew_auto_update"
LOG_FILE="$LOG_DIR/brew_update.log"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 打印函数
print_header() {
    echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║  🍺 Homebrew Auto Update 管理工具${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

# 安装服务
install_service() {
    print_header
    echo ""
    print_info "正在安装 Homebrew 自动更新服务..."
    
    # 创建 LaunchAgents 目录
    mkdir -p "$HOME/Library/LaunchAgents"
    
    # 复制 plist 文件
    cp "$PLIST_FILE" "$PLIST_DEST"
    print_success "已复制 plist 文件到 $PLIST_DEST"
    
    # 加载服务
    launchctl load "$PLIST_DEST"
    print_success "服务已加载"
    
    echo ""
    print_info "服务已安装并启动"
    print_info "日志文件: $LOG_FILE"
    echo ""
}

# 启动服务
start_service() {
    print_header
    echo ""
    
    if [ ! -f "$PLIST_DEST" ]; then
        print_error "服务未安装，请先运行: $0 install"
        exit 1
    fi
    
    print_info "正在启动服务..."
    launchctl start com.homebrew.auto-update
    print_success "服务已启动"
    echo ""
}

# 停止服务
stop_service() {
    print_header
    echo ""
    
    if [ ! -f "$PLIST_DEST" ]; then
        print_error "服务未安装"
        exit 1
    fi
    
    print_info "正在停止服务..."
    launchctl stop com.homebrew.auto-update
    print_success "服务已停止"
    echo ""
}

# 卸载服务
uninstall_service() {
    print_header
    echo ""
    
    if [ ! -f "$PLIST_DEST" ]; then
        print_error "服务未安装"
        exit 1
    fi
    
    print_info "正在卸载服务..."
    launchctl unload "$PLIST_DEST"
    rm -f "$PLIST_DEST"
    print_success "服务已卸载"
    echo ""
}

# 查看状态
show_status() {
    print_header
    echo ""
    
    if [ ! -f "$PLIST_DEST" ]; then
        print_error "服务未安装"
        echo ""
        return 1
    fi
    
    # 检查服务是否运行
    if launchctl list | grep -q "com.homebrew.auto-update"; then
        print_success "服务状态: 运行中"
    else
        print_error "服务状态: 已停止"
    fi
    
    # 显示日志信息
    echo ""
    print_info "最近的日志:"
    if [ -f "$LOG_FILE" ]; then
        tail -10 "$LOG_FILE"
    else
        print_info "暂无日志"
    fi
    echo ""
}

# 查看日志
show_logs() {
    print_header
    echo ""
    
    if [ ! -f "$LOG_FILE" ]; then
        print_error "日志文件不存在"
        echo ""
        return 1
    fi
    
    print_info "日志文件: $LOG_FILE"
    echo ""
    tail -50 "$LOG_FILE"
    echo ""
}

# 重启服务
restart_service() {
    print_header
    echo ""
    
    print_info "正在重启服务..."
    stop_service
    sleep 2
    start_service
    print_success "服务已重启"
    echo ""
}

# 显示帮助
show_help() {
    print_header
    echo ""
    echo "用法: $0 <命令>"
    echo ""
    echo "命令:"
    echo "  install    - 安装并启动服务"
    echo "  start      - 启动服务"
    echo "  stop       - 停止服务"
    echo "  restart    - 重启服务"
    echo "  uninstall  - 卸载服务"
    echo "  status     - 查看服务状态"
    echo "  logs       - 查看完整日志"
    echo "  help       - 显示此帮助信息"
    echo ""
    echo "示例:"
    echo "  $0 install   # 首次安装"
    echo "  $0 status    # 查看状态"
    echo "  $0 logs      # 查看日志"
    echo ""
}

# 主函数
main() {
    case "${1:-help}" in
        install)
            install_service
            ;;
        start)
            start_service
            ;;
        stop)
            stop_service
            ;;
        restart)
            restart_service
            ;;
        uninstall)
            uninstall_service
            ;;
        status)
            show_status
            ;;
        logs)
            show_logs
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            print_error "未知命令: $1"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

main "$@"
