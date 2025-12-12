#!/opt/homebrew/bin/bash
# =============================================================================
# 同步Surge模块到Shadowrocket
# 功能: 转换Surge模块为Shadowrocket兼容格式并同步
# 改进: 自动清理重复的旧模块
# =============================================================================

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SURGE_MODULE_DIR="${SCRIPT_DIR}/../../module/surge(main)"
SHADOWROCKET_MODULE_DIR="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~liguangming~Shadowrocket/Documents/Modules"

echo -e "${BLUE}=== Surge → Shadowrocket 模块同步 ===${NC}"
echo "源目录: $SURGE_MODULE_DIR"
echo "目标目录: $SHADOWROCKET_MODULE_DIR"
echo ""

# 检查目录
if [ ! -d "$SURGE_MODULE_DIR" ]; then
    echo -e "${RED}❌ Surge模块目录不存在${NC}"
    exit 1
fi

if [ ! -d "$SHADOWROCKET_MODULE_DIR" ]; then
    echo -e "${RED}❌ Shadowrocket模块目录不存在${NC}"
    exit 1
fi

# 转换函数: Surge → Shadowrocket兼容
convert_to_shadowrocket() {
    local input_file="$1"
    local output_file="$2"
    
    cat "$input_file" | \
    sed 's/,extended-matching//g' | \
    sed 's/,pre-matching//g' | \
    sed 's/,"update-interval=[0-9]*"//g' | \
    sed 's/REJECT-DROP/REJECT/g' | \
    sed 's/REJECT-NO-DROP/REJECT/g' | \
    sed 's/%APPEND% //g' \
    > "$output_file"
}

# 清理旧模块函数
cleanup_old_modules() {
    local module_name="$1"
    local safe_name=$(echo "$module_name" | sed 's/[^a-zA-Z0-9._-]/_/g')
    
    echo -e "${CYAN}  清理旧模块...${NC}"
    
    # 删除可能的旧文件
    for pattern in "__${module_name}" "__${safe_name}" "${safe_name}"; do
        local old_file="${SHADOWROCKET_MODULE_DIR}/${pattern}"
        if [ -f "$old_file" ]; then
            echo -e "${YELLOW}    删除: ${pattern}${NC}"
            rm -f "$old_file" || true
        fi
    done
}

# 需要同步的模块列表
MODULES=(
    "🚫 Universal Ad-Blocking Rules Dependency Component LITE (Kali-style).sgmodule"
    "🚀💪General Enhanced⬆️⬆️ plus.sgmodule"
    "🔥 Firewall Port Blocker 🛡️🚫.sgmodule"
    "Encrypted DNS Module 🔒🛡️DNS.sgmodule"
    "URL Rewrite Module 🔄🌐.sgmodule"
)

SUCCESS=0
FAILED=0

for module in "${MODULES[@]}"; do
    src_file="${SURGE_MODULE_DIR}/${module}"
    
    if [ -f "$src_file" ]; then
        echo -e "${YELLOW}同步: ${module}${NC}"
        
        # 清理旧模块
        cleanup_old_modules "$module"
        
        # 新文件名: 使用双下划线前缀 + 原始名称
        dst_file="${SHADOWROCKET_MODULE_DIR}/__${module}"
        
        if convert_to_shadowrocket "$src_file" "$dst_file"; then
            echo -e "${GREEN}  ✅ 完成 → __${module}${NC}"
            SUCCESS=$((SUCCESS + 1))
        else
            echo -e "${RED}  ❌ 失败${NC}"
            FAILED=$((FAILED + 1))
        fi
    else
        echo -e "${RED}跳过: ${module} (不存在)${NC}"
        FAILED=$((FAILED + 1))
    fi
    echo ""
done

echo ""
echo -e "${BLUE}=== 同步完成 ===${NC}"
echo -e "成功: ${GREEN}${SUCCESS}${NC} | 失败: ${RED}${FAILED}${NC}"
echo -e "${CYAN}提示: 同步的模块使用 '__' 前缀以区分手动添加的模块${NC}"
