#!/bin/bash
# =============================================================================
# 增量合并所有规则集 - 使用 ruleset_merger.sh
# 确保: 增量堆积 + 去重唯一化 + 保留原有规则
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULESET_DIR="${SCRIPT_DIR}/../../ruleset"
SURGE_DIR="${RULESET_DIR}/Surge(Shadowkroket)"
SOURCES_DIR="${RULESET_DIR}/Sources"
DNS_DIR="${RULESET_DIR}/DNS_mapping"
MERGER="${SCRIPT_DIR}/ruleset_merger.sh"

echo -e "${BLUE}=== 增量合并所有规则集 ===${NC}"
echo "使用 ruleset_merger.sh 进行增量合并"
echo "规则目录: $RULESET_DIR"
echo ""

# 合并函数 (使用新的文件夹结构)
merge_ruleset() {
    local name="$1"
    local sources_file="${SOURCES_DIR}/${name}_sources.txt"
    local target_file="${SURGE_DIR}/${name}.list"
    
    if [ ! -f "$sources_file" ]; then
        echo -e "${YELLOW}跳过 ${name}: 无 sources 文件${NC}"
        return
    fi
    
    echo -e "${BLUE}合并: ${name}${NC}"
    
    # 如果目标文件不存在，创建空文件
    [ ! -f "$target_file" ] && touch "$target_file"
    
    "$MERGER" -t "$target_file" -l "$sources_file" -o "$target_file" -n "$name" -v
    echo ""
}

# 统计
total=0
success=0
failed=0

# ============================================
# 核心规则集 (已有)
# ============================================
echo -e "${GREEN}=== 核心规则集 ===${NC}"

for name in GlobalMedia AI Gaming GlobalProxy Microsoft Discord Fediverse NSFW LAN AdBlock; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    fi
done

# ============================================
# 新增规则集 (从第三方合并)
# ============================================
echo ""
echo -e "${GREEN}=== 新增规则集 (从第三方合并) ===${NC}"

for name in SocialMedia PayPal GitHub Disney YouTube Spotify Bing Telegram Steam TikTok Twitter Netflix Instagram; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    fi
done

# ============================================
# 中国相关规则集
# ============================================
echo ""
echo -e "${GREEN}=== 中国相关规则集 ===${NC}"

for name in ChinaDirect ChinaIP CDN; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    fi
done

# ============================================
# DNS 规则集
# ============================================
echo ""
echo -e "${GREEN}=== DNS 规则集 ===${NC}"

for name in DNS_China_114 DNS_China_360 DNS_China_AliDNS DNS_China_ByteDance DNS_Global_Cloudflare DNS_Global_Google DNS_Global_Quad9; do
    if [ -f "${DNS_DIR}/${name}_sources.txt" ]; then
        # DNS 规则使用 DNS_mapping 文件夹
        dns_sources_file="${DNS_DIR}/${name}_sources.txt"
        dns_target_file="${DNS_DIR}/${name}.list"
        [ ! -f "$dns_target_file" ] && touch "$dns_target_file"
        "$MERGER" -t "$dns_target_file" -l "$dns_sources_file" -o "$dns_target_file" -n "$name" -v && ((success++)) || ((failed++))
        ((total++))
    fi
done

# ============================================
# 统计结果
# ============================================
echo ""
echo -e "${BLUE}╔══════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║            合并统计                      ║${NC}"
echo -e "${BLUE}╠══════════════════════════════════════════╣${NC}"
echo -e "${BLUE}║  总计: ${total}                                  ║${NC}"
echo -e "${GREEN}║  成功: ${success}                                  ║${NC}"
if [ $failed -gt 0 ]; then
    echo -e "${RED}║  失败: ${failed}                                  ║${NC}"
fi
echo -e "${BLUE}╚══════════════════════════════════════════╝${NC}"
echo ""
echo -e "${GREEN}=== 增量合并完成 ===${NC}"
echo "下一步: 运行 batch_convert_to_singbox.sh 生成 .srs 文件"
