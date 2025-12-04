#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════════
# 模块同步脚本 - Surge 模块同步到 iCloud (Surge + Shadowrocket)
# ═══════════════════════════════════════════════════════════════════════════════
# 功能：
# 1. 同步 Surge 模块到 Surge iCloud 目录
# 2. 同步并转换模块到 Shadowrocket iCloud 目录（兼容字段）
# 3. 自动排除敏感信息
# 4. 支持选择性同步或全部同步
# ═══════════════════════════════════════════════════════════════════════════════

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# 路径配置
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SOURCE_DIR="$PROJECT_ROOT/module/surge(main)"

# iCloud 目录配置
SURGE_ICLOUD_DIR="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents"
SHADOWROCKET_ICLOUD_DIR="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents"

# 敏感信息关键词（用于排除）
SENSITIVE_KEYWORDS=(
    "敏感"
    "私密"
    "private"
    "secret"
    "password"
    "token"
    "api-key"
    "YOUR_"
)

# 日志函数
log_info() {
    echo -e "${CYAN}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[⚠]${NC} $1"
}

log_error() {
    echo -e "${RED}[✗]${NC} $1"
}

log_section() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
}

# 检查目录是否存在
check_directories() {
    log_section "检查目录"
    
    if [[ ! -d "$SOURCE_DIR" ]]; then
        log_error "源目录不存在: $SOURCE_DIR"
        exit 1
    fi
    log_success "源目录: $SOURCE_DIR"
    
    if [[ ! -d "$SURGE_ICLOUD_DIR" ]]; then
        log_warning "Surge iCloud 目录不存在: $SURGE_ICLOUD_DIR"
        log_info "将跳过 Surge 同步"
        SURGE_AVAILABLE=false
    else
        log_success "Surge iCloud: $SURGE_ICLOUD_DIR"
        SURGE_AVAILABLE=true
    fi
    
    if [[ ! -d "$SHADOWROCKET_ICLOUD_DIR" ]]; then
        log_warning "Shadowrocket iCloud 目录不存在: $SHADOWROCKET_ICLOUD_DIR"
        log_info "将跳过 Shadowrocket 同步"
        SHADOWROCKET_AVAILABLE=false
    else
        log_success "Shadowrocket iCloud: $SHADOWROCKET_ICLOUD_DIR"
        SHADOWROCKET_AVAILABLE=true
    fi
    
    if [[ "$SURGE_AVAILABLE" == false ]] && [[ "$SHADOWROCKET_AVAILABLE" == false ]]; then
        log_error "所有目标目录都不可用，无法同步"
        exit 1
    fi
}

# 检查文件是否包含敏感信息
is_sensitive_file() {
    local filename="$1"
    
    for keyword in "${SENSITIVE_KEYWORDS[@]}"; do
        if [[ "$filename" == *"$keyword"* ]]; then
            return 0  # 是敏感文件
        fi
    done
    
    return 1  # 不是敏感文件
}

# 转换模块为 Shadowrocket 兼容格式
convert_to_shadowrocket() {
    local input_file="$1"
    local output_file="$2"
    
    # 读取文件内容并转换
    sed -e 's/extended-matching,//g' \
        -e 's/,extended-matching//g' \
        -e 's/pre-matching,//g' \
        -e 's/,pre-matching//g' \
        -e 's/update-interval=[0-9]*,//g' \
        -e 's/,update-interval=[0-9]*//g' \
        -e 's/"update-interval=[0-9]*",//g' \
        -e 's/,"update-interval=[0-9]*"//g' \
        -e 's/REJECT-DROP/REJECT/g' \
        -e 's/REJECT-NO-DROP/REJECT/g' \
        -e 's/hostname = %APPEND% /hostname = /g' \
        "$input_file" > "$output_file"
}

# 同步单个模块到 Surge iCloud
sync_to_surge() {
    local module_file="$1"
    local module_name=$(basename "$module_file")
    
    # 检查是否是敏感文件
    if is_sensitive_file "$module_name"; then
        log_warning "跳过敏感文件: $module_name"
        return
    fi
    
    # 复制到 Surge iCloud
    cp "$module_file" "$SURGE_ICLOUD_DIR/$module_name"
    log_success "Surge: $module_name"
}

# 同步单个模块到 Shadowrocket iCloud
sync_to_shadowrocket() {
    local module_file="$1"
    local module_name=$(basename "$module_file")
    
    # 检查是否是敏感文件
    if is_sensitive_file "$module_name"; then
        log_warning "跳过敏感文件: $module_name"
        return
    fi
    
    # 转换并复制到 Shadowrocket iCloud
    local output_file="$SHADOWROCKET_ICLOUD_DIR/$module_name"
    
    # 使用sed进行兼容性转换（一次性处理）
    sed -e 's/REJECT-DROP/REJECT/g' \
        -e 's/REJECT-NO-DROP/REJECT/g' \
        -e 's/hostname = %APPEND% /hostname = /g' \
        "$module_file" > "$output_file"
    
    if [[ $? -eq 0 ]]; then
        log_success "Shadowrocket: $module_name"
    else
        log_warning "Shadowrocket转换失败: $module_name"
    fi
}

# 同步所有模块
sync_all_modules() {
    log_section "同步所有模块"
    
    local surge_count=0
    local shadowrocket_count=0
    local skipped_count=0
    
    for module_file in "$SOURCE_DIR"/*.sgmodule; do
        if [[ ! -f "$module_file" ]]; then
            continue
        fi
        
        local module_name=$(basename "$module_file")
        
        # 检查是否是敏感文件
        if is_sensitive_file "$module_name"; then
            log_warning "跳过敏感文件: $module_name"
            ((skipped_count++))
            continue
        fi
        
        log_info "处理: $module_name"
        
        # 同步到 Surge
        if [[ "$SURGE_AVAILABLE" == true ]]; then
            sync_to_surge "$module_file"
            ((surge_count++))
        fi
        
        # 同步到 Shadowrocket
        if [[ "$SHADOWROCKET_AVAILABLE" == true ]]; then
            sync_to_shadowrocket "$module_file"
            ((shadowrocket_count++))
        fi
        
        echo ""
    done
    
    log_section "同步统计"
    if [[ "$SURGE_AVAILABLE" == true ]]; then
        echo "Surge: $surge_count 个模块"
    fi
    if [[ "$SHADOWROCKET_AVAILABLE" == true ]]; then
        echo "Shadowrocket: $shadowrocket_count 个模块"
    fi
    echo "跳过: $skipped_count 个敏感文件"
}

# 同步指定模块
sync_specific_module() {
    local module_name="$1"
    local module_file="$SOURCE_DIR/$module_name"
    
    if [[ ! -f "$module_file" ]]; then
        log_error "模块文件不存在: $module_name"
        exit 1
    fi
    
    # 检查是否是敏感文件
    if is_sensitive_file "$module_name"; then
        log_error "无法同步敏感文件: $module_name"
        exit 1
    fi
    
    log_section "同步指定模块: $module_name"
    
    # 同步到 Surge
    if [[ "$SURGE_AVAILABLE" == true ]]; then
        sync_to_surge "$module_file"
    fi
    
    # 同步到 Shadowrocket
    if [[ "$SHADOWROCKET_AVAILABLE" == true ]]; then
        sync_to_shadowrocket "$module_file"
    fi
}

# 列出所有可同步的模块
list_modules() {
    log_section "可同步的模块列表"
    
    local count=0
    local sensitive_count=0
    
    for module_file in "$SOURCE_DIR"/*.sgmodule; do
        if [[ ! -f "$module_file" ]]; then
            continue
        fi
        
        local module_name=$(basename "$module_file")
        
        if is_sensitive_file "$module_name"; then
            echo -e "${YELLOW}[敏感]${NC} $module_name"
            ((sensitive_count++))
        else
            echo -e "${GREEN}[可同步]${NC} $module_name"
            ((count++))
        fi
    done
    
    echo ""
    echo "可同步: $count 个模块"
    echo "敏感文件: $sensitive_count 个（将被跳过）"
}

# 清理重复模块
clean_duplicate_modules() {
    log_section "清理重复模块"
    
    local cleaned=0
    
    # 清理 Surge iCloud 中的重复模块
    if [[ "$SURGE_AVAILABLE" == true ]]; then
        log_info "检查 Surge iCloud 重复模块..."
        
        # 已知重复模块列表
        local duplicates=(
            "🔐加密dns.sgmodule"  # 与 "Encrypted DNS Module 🔒🛡️DNS.sgmodule" 重复
        )
        
        for dup in "${duplicates[@]}"; do
            local dup_file="$SURGE_ICLOUD_DIR/$dup"
            if [[ -f "$dup_file" ]]; then
                rm "$dup_file"
                log_success "删除重复: $dup"
                ((cleaned++))
            fi
        done
    fi
    
    # 清理 Shadowrocket 中以 __ 开头的旧文件
    if [[ "$SHADOWROCKET_AVAILABLE" == true ]]; then
        log_info "清理 Shadowrocket 旧同步文件..."
        for old_file in "$SHADOWROCKET_ICLOUD_DIR"/__*.sgmodule; do
            if [[ -f "$old_file" ]]; then
                rm "$old_file"
                log_info "删除旧文件: $(basename "$old_file")"
                ((cleaned++))
            fi
        done
    fi
    
    if [[ $cleaned -eq 0 ]]; then
        log_info "未发现重复或旧文件"
    else
        log_success "总计清理: $cleaned 个文件"
    fi
}

# 显示帮助信息
show_help() {
    cat << EOF
${BLUE}═══════════════════════════════════════════════════════════════${NC}
  模块同步脚本 - Surge 模块同步到 iCloud
${BLUE}═══════════════════════════════════════════════════════════════${NC}

用法:
  $0 [选项] [模块名称]

选项:
  -a, --all       同步所有模块（默认）
  -l, --list      列出所有可同步的模块
  -c, --clean     清理旧的同步文件
  -h, --help      显示此帮助信息

示例:
  $0                                    # 同步所有模块
  $0 --all                              # 同步所有模块
  $0 "URL Rewrite Module 🔄🌐.sgmodule"  # 同步指定模块
  $0 --list                             # 列出所有模块
  $0 --clean                            # 清理旧文件

同步目标:
  - Surge iCloud: $SURGE_ICLOUD_DIR
  - Shadowrocket: $SHADOWROCKET_ICLOUD_DIR

敏感文件排除:
  包含以下关键词的文件将被跳过：
  ${SENSITIVE_KEYWORDS[@]}

EOF
}

# 主函数
main() {
    log_section "模块同步脚本"
    
    # 检查目录
    check_directories
    
    # 解析参数
    case "${1:-}" in
        -h|--help)
            show_help
            exit 0
            ;;
        -l|--list)
            list_modules
            exit 0
            ;;
        -c|--clean)
            clean_duplicate_modules
            exit 0
            ;;
        -a|--all|"")
            clean_duplicate_modules
            sync_all_modules
            ;;
        *)
            # 同步指定模块
            sync_specific_module "$1"
            ;;
    esac
    
    log_section "完成"
    log_success "模块同步完成！"
    echo ""
    echo "下一步："
    echo "1. 打开 Surge 或 Shadowrocket 应用"
    echo "2. 刷新模块列表"
    echo "3. 启用需要的模块"
}

# 执行主函数
main "$@"
