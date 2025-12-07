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
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RULESET_DIR="${PROJECT_ROOT}/ruleset"
SURGE_DIR="${RULESET_DIR}/Surge(Shadowkroket)"
# Sources文件在Links子文件夹中
SOURCES_DIR="${RULESET_DIR}/Sources/Links"
# 自定义规则在custom子文件夹中
CUSTOM_DIR="${RULESET_DIR}/Sources/custom"
# DNS规则在DNS_mapping子文件夹中
DNS_DIR="${RULESET_DIR}/Sources/DNS_mapping"
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
# 核心规则集
# ============================================
echo -e "${GREEN}=== 核心规则集 ===${NC}"

for name in GlobalMedia AI Gaming GlobalProxy Microsoft Discord Fediverse NSFW LAN; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    elif [ -f "${SURGE_DIR}/${name}.list" ]; then
        echo -e "${YELLOW}跳过 ${name}: 已存在，无sources文件${NC}"
    fi
done

# ============================================
# 社交媒体规则集
# ============================================
echo ""
echo -e "${GREEN}=== 社交媒体规则集 ===${NC}"

for name in SocialMedia Telegram TikTok Twitter Instagram Reddit WeChat; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    elif [ -f "${SURGE_DIR}/${name}.list" ]; then
        echo -e "${YELLOW}跳过 ${name}: 已存在，无sources文件${NC}"
    fi
done

# ============================================
# 流媒体规则集
# ============================================
echo ""
echo -e "${GREEN}=== 流媒体规则集 ===${NC}"

for name in YouTube Netflix Disney Spotify Bahamut AppleNews; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    elif [ -f "${SURGE_DIR}/${name}.list" ]; then
        echo -e "${YELLOW}跳过 ${name}: 已存在，无sources文件${NC}"
    fi
done

# ============================================
# 科技公司规则集
# ============================================
echo ""
echo -e "${GREEN}=== 科技公司规则集 ===${NC}"

for name in Google Bing Apple Microsoft GitHub PayPal Tesla Binance; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    elif [ -f "${SURGE_DIR}/${name}.list" ]; then
        echo -e "${YELLOW}跳过 ${name}: 已存在，无sources文件${NC}"
    fi
done

# ============================================
# 游戏规则集
# ============================================
echo ""
echo -e "${GREEN}=== 游戏规则集 ===${NC}"

for name in Gaming Steam Epic; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    elif [ -f "${SURGE_DIR}/${name}.list" ]; then
        echo -e "${YELLOW}跳过 ${name}: 已存在，无sources文件${NC}"
    fi
done

# ============================================
# 中国相关规则集
# ============================================
echo ""
echo -e "${GREEN}=== 中国相关规则集 ===${NC}"

for name in ChinaDirect ChinaIP Bilibili QQ Tencent XiaoHongShu NetEaseMusic GoogleCN; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    elif [ -f "${SURGE_DIR}/${name}.list" ]; then
        echo -e "${YELLOW}跳过 ${name}: 已存在，无sources文件${NC}"
    fi
done

# ============================================
# 网络基础设施规则集
# ============================================
echo ""
echo -e "${GREEN}=== 网络基础设施规则集 ===${NC}"

for name in CDN LAN Speedtest; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    elif [ -f "${SURGE_DIR}/${name}.list" ]; then
        echo -e "${YELLOW}跳过 ${name}: 已存在，无sources文件${NC}"
    fi
done

# ============================================
# 地区流媒体规则集
# ============================================
echo ""
echo -e "${GREEN}=== 地区流媒体规则集 ===${NC}"

for name in StreamJP StreamUS StreamKR StreamHK StreamTW StreamEU; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    elif [ -f "${SURGE_DIR}/${name}.list" ]; then
        echo -e "${YELLOW}跳过 ${name}: 已存在，无需合并${NC}"
    fi
done

# ============================================
# Process 规则集
# ============================================
echo ""
echo -e "${GREEN}=== Process 规则集 ===${NC}"

for name in AIProcess DirectProcess DownloadProcess GamingProcess; do
    if [ -f "${SOURCES_DIR}/${name}_sources.txt" ]; then
        merge_ruleset "$name" && ((success++)) || ((failed++))
        ((total++))
    elif [ -f "${SURGE_DIR}/${name}.list" ]; then
        echo -e "${YELLOW}跳过 ${name}: 已存在，无需合并${NC}"
    fi
done

# ============================================
# 手动规则集
# ============================================
echo ""
echo -e "${GREEN}=== 手动规则集 ===${NC}"

for name in Manual Manual_US Manual_West Manual_JP Manual_Global; do
    if [ -f "${SURGE_DIR}/${name}.list" ]; then
        echo -e "${YELLOW}跳过 ${name}: 手动维护，无需合并${NC}"
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
