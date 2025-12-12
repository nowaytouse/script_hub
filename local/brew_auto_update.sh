#!/opt/homebrew/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
# Homebrew Auto Update & Upgrade
# 每 30 分钟自动执行 brew update 和 brew upgrade
# 自动启动、后台运行、日志记录
# ═══════════════════════════════════════════════════════════════════════════════

set -e

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_DIR="$HOME/.brew_auto_update"
LOG_FILE="$LOG_DIR/brew_update.log"
PID_FILE="$LOG_DIR/brew_update.pid"
LOCK_FILE="$LOG_DIR/brew_update.lock"
UPDATE_INTERVAL=1800  # 30 分钟（秒）

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 创建日志目录
mkdir -p "$LOG_DIR"

# 日志函数
log_info() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] [INFO] $1" | tee -a "$LOG_FILE"
}

log_success() {
    echo -e "${GREEN}[$(date '+%Y-%m-%d %H:%M:%S')] [✓] $1${NC}" | tee -a "$LOG_FILE"
}

log_error() {
    echo -e "${RED}[$(date '+%Y-%m-%d %H:%M:%S')] [✗] $1${NC}" | tee -a "$LOG_FILE"
}

log_warn() {
    echo -e "${YELLOW}[$(date '+%Y-%m-%d %H:%M:%S')] [⚠] $1${NC}" | tee -a "$LOG_FILE"
}

# 检查是否已在运行
check_running() {
    if [ -f "$PID_FILE" ]; then
        local old_pid=$(cat "$PID_FILE")
        if ps -p "$old_pid" > /dev/null 2>&1; then
            log_warn "进程已在运行 (PID: $old_pid)"
            return 0
        else
            rm -f "$PID_FILE"
        fi
    fi
    return 1
}

# 获取互斥锁
acquire_lock() {
    local timeout=10
    local elapsed=0
    
    while [ -f "$LOCK_FILE" ] && [ $elapsed -lt $timeout ]; do
        sleep 1
        elapsed=$((elapsed + 1))
    done
    
    if [ -f "$LOCK_FILE" ]; then
        log_warn "无法获取锁，跳过本次更新"
        return 1
    fi
    
    echo $$ > "$LOCK_FILE"
    return 0
}

# 释放互斥锁
release_lock() {
    rm -f "$LOCK_FILE"
}

# 执行 brew 更新
do_brew_update() {
    log_info "开始 Homebrew 更新..."
    
    if ! acquire_lock; then
        return 1
    fi
    
    trap release_lock EXIT
    
    # 更新 brew
    if /opt/homebrew/bin/brew update >> "$LOG_FILE" 2>&1; then
        log_success "brew update 完成"
    else
        log_error "brew update 失败"
        return 1
    fi
    
    # 升级包
    if /opt/homebrew/bin/brew upgrade >> "$LOG_FILE" 2>&1; then
        log_success "brew upgrade 完成"
    else
        log_warn "brew upgrade 失败或无可用更新"
    fi
    
    # 清理
    if /opt/homebrew/bin/brew cleanup >> "$LOG_FILE" 2>&1; then
        log_success "brew cleanup 完成"
    fi
    
    return 0
}

# 主循环
main_loop() {
    log_info "🍺 Homebrew 自动更新服务启动"
    log_info "更新间隔: $((UPDATE_INTERVAL / 60)) 分钟"
    log_info "日志文件: $LOG_FILE"
    
    # 保存 PID
    echo $$ > "$PID_FILE"
    
    # 启动时立即执行一次
    do_brew_update
    
    # 定期执行
    while true; do
        sleep "$UPDATE_INTERVAL"
        do_brew_update
    done
}

# 清理函数
cleanup() {
    log_info "收到停止信号，正在关闭..."
    rm -f "$PID_FILE" "$LOCK_FILE"
    log_info "服务已停止"
    exit 0
}

# 信号处理
trap cleanup SIGTERM SIGINT

# 检查是否已运行
if check_running; then
    exit 0
fi

# 启动主循环
main_loop
