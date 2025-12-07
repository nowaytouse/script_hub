#!/bin/bash
# =============================================================================
# Sing-box & Mihomo 核心更新脚本
# 功能: 自动下载和更新最新版本的 sing-box 和 mihomo 核心
# 更新: 2025-12-07
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

# 日志函数
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# 配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# 核心安装路径
SINGBOX_PATH="/usr/local/bin/sing-box"
MIHOMO_PATH="/usr/local/bin/mihomo"

# 备份目录
BACKUP_DIR="$HOME/.local/share/proxy-cores/backup"
mkdir -p "$BACKUP_DIR"

# 检测系统架构
detect_arch() {
    local arch=$(uname -m)
    case "$arch" in
        x86_64|amd64) echo "amd64" ;;
        aarch64|arm64) echo "arm64" ;;
        armv7l) echo "armv7" ;;
        *) log_error "不支持的架构: $arch"; exit 1 ;;
    esac
}

# 检测操作系统
detect_os() {
    case "$(uname -s)" in
        Darwin) echo "darwin" ;;
        Linux) echo "linux" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *) log_error "不支持的操作系统"; exit 1 ;;
    esac
}

# 获取最新版本号
get_latest_version() {
    local repo="$1"
    local version=$(curl -s "https://api.github.com/repos/$repo/releases/latest" | grep '"tag_name"' | sed -E 's/.*"v?([^"]+)".*/\1/')
    if [ -z "$version" ]; then
        log_error "无法获取 $repo 的最新版本"
        return 1
    fi
    echo "$version"
}

# 备份现有核心
backup_core() {
    local core_path="$1"
    local core_name=$(basename "$core_path")
    
    if [ -f "$core_path" ]; then
        local version=$("$core_path" version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || echo "unknown")
        local backup_file="$BACKUP_DIR/${core_name}_${version}_$(date +%Y%m%d_%H%M%S)"
        
        log_info "备份 $core_name v$version..."
        cp "$core_path" "$backup_file"
        log_success "已备份到: $backup_file"
        
        # 保留最近3个备份
        ls -t "$BACKUP_DIR/${core_name}_"* 2>/dev/null | tail -n +4 | xargs rm -f 2>/dev/null || true
    fi
}

# 更新 Sing-box
update_singbox() {
    log_info "检查 Sing-box 更新..."
    
    local os=$(detect_os)
    local arch=$(detect_arch)
    local latest_version=$(get_latest_version "SagerNet/sing-box")
    
    if [ -z "$latest_version" ]; then
        log_error "无法获取 Sing-box 最新版本"
        return 1
    fi
    
    # 检查当前版本
    if [ -f "$SINGBOX_PATH" ]; then
        local current_version=$("$SINGBOX_PATH" version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || echo "0.0.0")
        log_info "当前版本: v$current_version"
        log_info "最新版本: v$latest_version"
        
        if [ "$current_version" = "$latest_version" ]; then
            log_success "Sing-box 已是最新版本"
            return 0
        fi
        
        # 备份旧版本
        backup_core "$SINGBOX_PATH"
    else
        log_info "首次安装 Sing-box"
    fi
    
    # 下载新版本
    log_info "下载 Sing-box v$latest_version..."
    local download_url="https://github.com/SagerNet/sing-box/releases/download/v${latest_version}/sing-box-${latest_version}-${os}-${arch}.tar.gz"
    local temp_dir=$(mktemp -d)
    
    if curl -L -o "$temp_dir/sing-box.tar.gz" "$download_url"; then
        log_success "下载完成"
        
        # 解压
        tar -xzf "$temp_dir/sing-box.tar.gz" -C "$temp_dir"
        
        # 安装
        local binary=$(find "$temp_dir" -name "sing-box" -type f | head -1)
        if [ -n "$binary" ]; then
            sudo mv "$binary" "$SINGBOX_PATH"
            sudo chmod +x "$SINGBOX_PATH"
            log_success "Sing-box v$latest_version 安装完成"
            
            # 验证
            "$SINGBOX_PATH" version
        else
            log_error "未找到 sing-box 二进制文件"
            rm -rf "$temp_dir"
            return 1
        fi
    else
        log_error "下载失败"
        rm -rf "$temp_dir"
        return 1
    fi
    
    rm -rf "$temp_dir"
}

# 更新 Mihomo
update_mihomo() {
    log_info "检查 Mihomo 更新..."
    
    local os=$(detect_os)
    local arch=$(detect_arch)
    local latest_version=$(get_latest_version "MetaCubeX/mihomo")
    
    if [ -z "$latest_version" ]; then
        log_error "无法获取 Mihomo 最新版本"
        return 1
    fi
    
    # 检查当前版本
    if [ -f "$MIHOMO_PATH" ]; then
        local current_version=$("$MIHOMO_PATH" -v 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1 || echo "0.0.0")
        log_info "当前版本: v$current_version"
        log_info "最新版本: v$latest_version"
        
        if [ "$current_version" = "$latest_version" ]; then
            log_success "Mihomo 已是最新版本"
            return 0
        fi
        
        # 备份旧版本
        backup_core "$MIHOMO_PATH"
    else
        log_info "首次安装 Mihomo"
    fi
    
    # 下载新版本
    log_info "下载 Mihomo v$latest_version..."
    local download_url="https://github.com/MetaCubeX/mihomo/releases/download/v${latest_version}/mihomo-${os}-${arch}-v${latest_version}.gz"
    local temp_dir=$(mktemp -d)
    
    if curl -L -o "$temp_dir/mihomo.gz" "$download_url"; then
        log_success "下载完成"
        
        # 解压
        gunzip "$temp_dir/mihomo.gz"
        
        # 安装
        if [ -f "$temp_dir/mihomo" ]; then
            sudo mv "$temp_dir/mihomo" "$MIHOMO_PATH"
            sudo chmod +x "$MIHOMO_PATH"
            log_success "Mihomo v$latest_version 安装完成"
            
            # 验证
            "$MIHOMO_PATH" -v
        else
            log_error "未找到 mihomo 二进制文件"
            rm -rf "$temp_dir"
            return 1
        fi
    else
        log_error "下载失败"
        rm -rf "$temp_dir"
        return 1
    fi
    
    rm -rf "$temp_dir"
}

# 显示帮助
show_help() {
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  --singbox-only    仅更新 Sing-box"
    echo "  --mihomo-only     仅更新 Mihomo"
    echo "  --check-only      仅检查版本，不更新"
    echo "  -h, --help        显示帮助"
    echo ""
    echo "示例:"
    echo "  $0                    # 更新所有核心"
    echo "  $0 --singbox-only     # 仅更新 Sing-box"
    echo "  $0 --check-only       # 仅检查版本"
    exit 0
}

# 解析参数
SINGBOX_ONLY=false
MIHOMO_ONLY=false
CHECK_ONLY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --singbox-only) SINGBOX_ONLY=true; shift ;;
        --mihomo-only) MIHOMO_ONLY=true; shift ;;
        --check-only) CHECK_ONLY=true; shift ;;
        -h|--help) show_help ;;
        *) log_error "未知选项: $1"; exit 1 ;;
    esac
done

# 显示 banner
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       Sing-box & Mihomo 核心更新工具                         ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# 检查权限
if [ "$CHECK_ONLY" = false ]; then
    if ! sudo -n true 2>/dev/null; then
        log_warning "需要 sudo 权限来安装核心"
        sudo -v
    fi
fi

# 执行更新
if [ "$CHECK_ONLY" = true ]; then
    log_info "检查模式 (不会更新)"
    echo ""
fi

if [ "$MIHOMO_ONLY" = false ]; then
    update_singbox || log_warning "Sing-box 更新失败"
    echo ""
fi

if [ "$SINGBOX_ONLY" = false ]; then
    update_mihomo || log_warning "Mihomo 更新失败"
    echo ""
fi

log_success "核心更新完成"
