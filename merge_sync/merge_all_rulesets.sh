#!/bin/bash
# =============================================================================
# 完整规则集合并脚本 - 合并所有第三方规则到自有规则集
# =============================================================================
# 目标: 将 MetaCubeX/SagerNet/Sukka/Chocolate4U 等规则合并到自有规则集
# 确保 Singbox 配置只使用自有规则集
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULESET_DIR="${SCRIPT_DIR}/../../ruleset/Surge(Shadowkroket)"
TEMP_DIR=$(mktemp -d)

cleanup() { rm -rf "$TEMP_DIR"; }
trap cleanup EXIT

echo -e "${BLUE}=== 规则集完整合并脚本 ===${NC}"
echo ""

# 下载函数
download_rules() {
    local url="$1" output="$2"
    curl -sL --connect-timeout 15 --max-time 60 "$url" -o "$output" 2>/dev/null
}

# 提取有效规则
extract_rules() {
    local input="$1"
    grep -E '^(DOMAIN-SUFFIX|DOMAIN-KEYWORD|DOMAIN|IP-CIDR|IP-CIDR6|PROCESS-NAME),' "$input" 2>/dev/null | \
        sed 's/[[:space:]]*$//' || true
}

# 合并规则到目标文件
merge_to_target() {
    local target="$1"
    shift
    local sources=("$@")
    local temp_merged="$TEMP_DIR/merged_$RANDOM.txt"
    
    echo -e "${YELLOW}合并到: $(basename $target)${NC}"
    
    # 创建新的临时文件
    touch "$temp_merged"
    
    # 保留原有规则
    if [ -f "$target" ]; then
        extract_rules "$target" >> "$temp_merged"
    fi
    
    # 下载并合并每个源
    for url in "${sources[@]}"; do
        local temp_download="$TEMP_DIR/download_$RANDOM.txt"
        echo "  ← $url"
        if download_rules "$url" "$temp_download"; then
            extract_rules "$temp_download" >> "$temp_merged"
        else
            echo -e "    ${RED}下载失败${NC}"
        fi
    done
    
    # 去重并排序
    local count_before=$(wc -l < "$temp_merged" | tr -d ' ')
    sort -u "$temp_merged" -o "$temp_merged"
    local count_after=$(wc -l < "$temp_merged" | tr -d ' ')
    
    # 生成带头部的输出
    local update_date=$(date "+%Y-%m-%d")
    cat > "$target" << EOF
# ═══════════════════════════════════════════════════════════════
# Ruleset: $(basename "$target" .list)
# Updated: ${update_date}
# Total Rules: ${count_after}
# Generator: merge_all_rulesets.sh
# ═══════════════════════════════════════════════════════════════

EOF
    cat "$temp_merged" >> "$target"
    
    echo -e "  ${GREEN}✓ ${count_after} 条规则 (去重前: ${count_before})${NC}"
}

# ============================================
# 1. GlobalMedia - 合并流媒体规则
# ============================================
echo ""
echo -e "${BLUE}[1/8] GlobalMedia - 流媒体服务${NC}"
GLOBALMEDIA_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Netflix/Netflix.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Disney/Disney.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/HBO/HBO.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Hulu/Hulu.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/YouTube/YouTube.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Spotify/Spotify.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Twitch/Twitch.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/AmazonPrimeVideo/AmazonPrimeVideo.list"
)
merge_to_target "$RULESET_DIR/GlobalMedia.list" "${GLOBALMEDIA_SOURCES[@]}"

# ============================================
# 2. AI - 合并AI服务规则
# ============================================
echo ""
echo -e "${BLUE}[2/8] AI - AI服务${NC}"
AI_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/OpenAI/OpenAI.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Claude/Claude.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Gemini/Gemini.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Copilot/Copilot.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/BardAI/BardAI.list"
)
merge_to_target "$RULESET_DIR/AI.list" "${AI_SOURCES[@]}"

# ============================================
# 3. Gaming - 合并游戏规则
# ============================================
echo ""
echo -e "${BLUE}[3/8] Gaming - 游戏平台${NC}"
GAMING_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Steam/Steam.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Epic/Epic.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/PlayStation/PlayStation.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Xbox/Xbox.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Nintendo/Nintendo.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Blizzard/Blizzard.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/EA/EA.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Ubisoft/Ubisoft.list"
)
merge_to_target "$RULESET_DIR/Gaming.list" "${GAMING_SOURCES[@]}"

# ============================================
# 4. GlobalProxy - 合并代理规则 (GFW + geolocation-!cn)
# ============================================
echo ""
echo -e "${BLUE}[4/8] GlobalProxy - 代理规则${NC}"
GLOBALPROXY_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Global/Global.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Proxy/Proxy.list"
)
merge_to_target "$RULESET_DIR/GlobalProxy.list" "${GLOBALPROXY_SOURCES[@]}"

# ============================================
# 5. LAN - 合并私有网络规则
# ============================================
echo ""
echo -e "${BLUE}[5/8] LAN - 私有网络${NC}"
LAN_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Lan/Lan.list"
)
merge_to_target "$RULESET_DIR/LAN.list" "${LAN_SOURCES[@]}"

# ============================================
# 6. Microsoft - 合并微软规则 (含Bing)
# ============================================
echo ""
echo -e "${BLUE}[6/8] Microsoft - 微软服务${NC}"
MICROSOFT_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Microsoft/Microsoft.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Bing/Bing.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/OneDrive/OneDrive.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Teams/Teams.list"
)
merge_to_target "$RULESET_DIR/Microsoft.list" "${MICROSOFT_SOURCES[@]}"

# ============================================
# 7. AdBlock - 合并广告规则
# ============================================
echo ""
echo -e "${BLUE}[7/8] AdBlock - 广告拦截${NC}"
ADBLOCK_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Advertising/Advertising.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Privacy/Privacy.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Hijacking/Hijacking.list"
)
merge_to_target "$RULESET_DIR/AdBlock.list" "${ADBLOCK_SOURCES[@]}"

# ============================================
# 8. 新增: SocialMedia - 社交媒体
# ============================================
echo ""
echo -e "${BLUE}[8/8] SocialMedia - 社交媒体 (新增)${NC}"
SOCIALMEDIA_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Instagram/Instagram.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Twitter/Twitter.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Facebook/Facebook.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Whatsapp/Whatsapp.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Reddit/Reddit.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/LinkedIn/LinkedIn.list"
)
merge_to_target "$RULESET_DIR/SocialMedia.list" "${SOCIALMEDIA_SOURCES[@]}"

# ============================================
# 9. 新增: PayPal
# ============================================
echo ""
echo -e "${BLUE}[9/9] PayPal (新增)${NC}"
PAYPAL_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/PayPal/PayPal.list"
)
merge_to_target "$RULESET_DIR/PayPal.list" "${PAYPAL_SOURCES[@]}"

# ============================================
# 10. 新增: GitHub
# ============================================
echo ""
echo -e "${BLUE}[10/10] GitHub (新增)${NC}"
GITHUB_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/GitHub/GitHub.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/GitLab/GitLab.list"
)
merge_to_target "$RULESET_DIR/GitHub.list" "${GITHUB_SOURCES[@]}"

echo ""
echo -e "${GREEN}=== 规则合并完成 ===${NC}"
echo ""
echo "下一步: 运行 batch_convert_to_singbox.sh 生成 .srs 文件"
