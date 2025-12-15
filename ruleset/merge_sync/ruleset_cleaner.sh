#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# Ruleset Cleaner v2.0 (合并版)
# 功能：
#   1. 清理空规则集（删除已弃用的空规则集）
#   2. 清理混入域名（移除不应该出现的域名）
#   3. 保护手动维护的规则集
# ═══════════════════════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
RULESET_DIR="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)"
SOURCES_DIR="$PROJECT_ROOT/ruleset/Sources/Links"

# 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# 统计
STAT_PROTECTED=0
STAT_DELETED=0
STAT_CLEANED=0
STAT_SKIPPED=0
STAT_ERRORS=0

# ═══════════════════════════════════════════════════════════════════════════════
# 配置
# ═══════════════════════════════════════════════════════════════════════════════

# 🔒 受保护规则集（手动维护，永不修改/删除）
PROTECTED_RULESETS="DownloadDirect Manual Manual_JP Manual_US Manual_West Manual_Global"

# ⏭️ 跳过混入检查的规则集（应该包含特定域名）
SKIP_CONFLICT_CHECK="SocialMedia Gaming Steam Epic GlobalMedia YouTube Spotify TikTok Telegram Twitter Twitch Netflix Facebook Instagram Reddit StreamUS StreamJP StreamKR StreamEU StreamHK StreamTW"

# 🗑️ 已弃用的空规则集（可以删除）
DEPRECATED_RULESETS="BlockHttpDNS FirewallPorts"

# 🚫 混入域名黑名单（精确匹配）
CONFLICT_DOMAINS=(
    # 社交媒体
    "x.com" "twitter.com" "facebook.com" "instagram.com" "reddit.com"
    "discord.com" "discordapp.com" "discordapp.net"
    "media.discordapp.net" "cdn.discordapp.com"
    # 流媒体
    "netflix.com" "hbomax.com" "hbo.com" "youtube.com" "youtu.be"
    "twitch.tv" "spotify.com"
    # 游戏平台
    "itch.io" "steampowered.com" "epicgames.com"
    # 图片托管
    "images.pexels.com" "imgur.com"
    # 其他误判
    "happymag.tv" "wortfm.org"
)

# ═══════════════════════════════════════════════════════════════════════════════
# 工具函数
# ═══════════════════════════════════════════════════════════════════════════════

is_protected() {
    local name="$1"
    [[ " $PROTECTED_RULESETS " == *" $name "* ]]
}

should_skip_conflict_check() {
    local name="$1"
    [[ " $SKIP_CONFLICT_CHECK " == *" $name "* ]]
}

is_deprecated() {
    local name="$1"
    [[ " $DEPRECATED_RULESETS " == *" $name "* ]]
}

get_rule_count() {
    local file="$1"
    grep -cv "^#\|^$\|^\s*$" "$file" 2>/dev/null || echo "0"
}

# ═══════════════════════════════════════════════════════════════════════════════
# 清理混入域名
# ═══════════════════════════════════════════════════════════════════════════════

clean_conflicts() {
    local ruleset_file="$1"
    local ruleset_name=$(basename "$ruleset_file" .list)
    
    local before_count=$(get_rule_count "$ruleset_file")
    local temp_file=$(mktemp)
    local removed_count=0
    
    # 复制头部注释
    grep "^#" "$ruleset_file" > "$temp_file" 2>/dev/null || true
    echo "" >> "$temp_file"
    
    # 过滤规则（精确匹配）
    while IFS= read -r line; do
        [[ -z "$line" || "$line" =~ ^# ]] && continue
        
        local should_exclude=false
        local domain=""
        
        # 提取域名
        if [[ "$line" == DOMAIN-SUFFIX,* ]]; then
            domain="${line#DOMAIN-SUFFIX,}"
        elif [[ "$line" == DOMAIN,* ]]; then
            domain="${line#DOMAIN,}"
        fi
        domain="${domain%%,*}"
        
        # 精确匹配检查
        for conflict_domain in "${CONFLICT_DOMAINS[@]}"; do
            if [[ "$domain" == "$conflict_domain" ]]; then
                should_exclude=true
                echo -e "    ${YELLOW}移除:${NC} $line"
                removed_count=$((removed_count + 1))
                break
            fi
        done
        
        [[ "$should_exclude" == "false" ]] && echo "$line" >> "$temp_file"
    done < "$ruleset_file"
    
    if [[ $removed_count -gt 0 ]]; then
        mv "$temp_file" "$ruleset_file"
        local after_count=$(get_rule_count "$ruleset_file")
        echo -e "  ${GREEN}[CLEANED]${NC} $ruleset_name: 移除 $removed_count 条 ($before_count → $after_count)"
        STAT_CLEANED=$((STAT_CLEANED + 1))
    else
        rm -f "$temp_file"
    fi
    
    # 🔥 Fix: Always return 0 to avoid set -e exit
    return 0
}

# ═══════════════════════════════════════════════════════════════════════════════
# 主流程
# ═══════════════════════════════════════════════════════════════════════════════

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║           Ruleset Cleaner v2.0 (合并版)                      ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

for ruleset_file in "$RULESET_DIR"/*.list; do
    [[ ! -f "$ruleset_file" ]] && continue
    
    ruleset_name=$(basename "$ruleset_file" .list)
    rule_count=$(get_rule_count "$ruleset_file")
    
    # 1. 受保护规则集
    if is_protected "$ruleset_name"; then
        echo -e "${GREEN}[🔒 PROTECTED]${NC} $ruleset_name ($rule_count rules)"
        STAT_PROTECTED=$((STAT_PROTECTED + 1))
        continue
    fi
    
    # 2. 空规则集处理
    if [[ "$rule_count" -eq 0 ]]; then
        if is_deprecated "$ruleset_name"; then
            rm -f "$ruleset_file"
            [[ -f "$SOURCES_DIR/${ruleset_name}_sources.txt" ]] && rm -f "$SOURCES_DIR/${ruleset_name}_sources.txt"
            echo -e "${YELLOW}[🗑️ DELETED]${NC} $ruleset_name (已弃用空规则集)"
            STAT_DELETED=$((STAT_DELETED + 1))
        else
            echo -e "${RED}[❌ ERROR]${NC} $ruleset_name 为空但不在弃用列表!"
            STAT_ERRORS=$((STAT_ERRORS + 1))
        fi
        continue
    fi
    
    # 3. 跳过混入检查的规则集
    if should_skip_conflict_check "$ruleset_name"; then
        echo -e "${CYAN}[⏭️ SKIP]${NC} $ruleset_name ($rule_count rules)"
        STAT_SKIPPED=$((STAT_SKIPPED + 1))
        continue
    fi
    
    # 4. 检查并清理混入域名
    echo -e "${BLUE}[🔍 CHECK]${NC} $ruleset_name ($rule_count rules)"
    clean_conflicts "$ruleset_file"
done

# ═══════════════════════════════════════════════════════════════════════════════
# 统计
# ═══════════════════════════════════════════════════════════════════════════════

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║                      清理完成                                 ║"
echo "╠══════════════════════════════════════════════════════════════╣"
printf "║  🔒 受保护: %-47s ║\n" "$STAT_PROTECTED 个"
printf "║  ⏭️  跳过:   %-47s ║\n" "$STAT_SKIPPED 个"
printf "║  🧹 清理:   %-47s ║\n" "$STAT_CLEANED 个"
printf "║  🗑️  删除:   %-47s ║\n" "$STAT_DELETED 个"
[[ $STAT_ERRORS -gt 0 ]] && printf "║  ❌ 错误:   %-47s ║\n" "$STAT_ERRORS 个"
echo "╚══════════════════════════════════════════════════════════════╝"

[[ $STAT_ERRORS -gt 0 ]] && exit 1
exit 0
