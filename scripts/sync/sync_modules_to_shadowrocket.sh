#!/bin/bash
# =============================================================================
# 同步Surge模块到Shadowrocket
# 功能: 转换Surge模块为Shadowrocket兼容格式并同步
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
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
    
    # 复制并进行必要的转换
    cat "$input_file" | \
    # 移除Surge特有的extended-matching参数
    sed 's/,extended-matching//g' | \
    # 移除pre-matching参数
    sed 's/,pre-matching//g' | \
    # 移除update-interval参数 (Shadowrocket不支持)
    sed 's/,"update-interval=[0-9]*"//g' | \
    # 转换REJECT-DROP为REJECT (Shadowrocket兼容)
    sed 's/REJECT-DROP/REJECT/g' | \
    # 转换REJECT-NO-DROP为REJECT
    sed 's/REJECT-NO-DROP/REJECT/g' | \
    # 移除%APPEND%前缀 (Shadowrocket不需要)
    sed 's/%APPEND% //g' \
    > "$output_file"
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
        # 生成Shadowrocket兼容的文件名
        dst_name=$(echo "$module" | sed 's/[^a-zA-Z0-9._-]/_/g')
        dst_file="${SHADOWROCKET_MODULE_DIR}/${dst_name}"
        
        echo -e "${YELLOW}同步: ${module}${NC}"
        
        if convert_to_shadowrocket "$src_file" "$dst_file"; then
            echo -e "${GREEN}  ✅ 完成 → ${dst_name}${NC}"
            ((SUCCESS++))
        else
            echo -e "${RED}  ❌ 失败${NC}"
            ((FAILED++))
        fi
    else
        echo -e "${RED}跳过: ${module} (不存在)${NC}"
        ((FAILED++))
    fi
done

echo ""
echo -e "${BLUE}=== 同步完成 ===${NC}"
echo -e "成功: ${GREEN}${SUCCESS}${NC} | 失败: ${RED}${FAILED}${NC}"
