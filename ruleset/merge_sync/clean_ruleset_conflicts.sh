#!/opt/homebrew/bin/bash
# ═══════════════════════════════════════════════════════════════════════════════
# Ruleset Conflict Cleaner v1.0
# 清理规则集中的混入域名，确保规则集纯净
# ═══════════════════════════════════════════════════════════════════════════════
#
# 问题：某些规则集从远程源合并时会混入不相关的域名
# 例如：NSFW.list 混入了 x.com, twitter.com, netflix.com 等
#
# 解决方案：
# 1. 定义每个规则集的排除列表
# 2. 定义受保护规则集（不被其他规则集影响）
# 3. 自动清理混入的域名
# ═══════════════════════════════════════════════════════════════════════════════

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
RULESET_DIR="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)"

# 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${CYAN}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║         Ruleset Conflict Cleaner v1.0                        ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# ═══════════════════════════════════════════════════════════════════════════════
# 受保护规则集定义
# 这些规则集的域名不应出现在其他规则集中
# ═══════════════════════════════════════════════════════════════════════════════

# 🔒 DownloadDirect - 下载直连域名（最高优先级）
PROTECTED_DOWNLOADDIRECT=(
    "steamcontent.com"
    "steamusercontent.com"
    "steamserver.net"
    "epicgames-download1.akamaized.net"
    "download.epicgames.com"
    "cdn.gog.com"
    "blzddist1-a.akamaihd.net"
    "origin-a.akamaihd.net"
    "dl.delivery.mp.microsoft.com"
    "gs2.ww.prod.dl.playstation.net"
)

# 🔒 SocialMedia - 社交媒体域名
PROTECTED_SOCIALMEDIA=(
    "x.com"
    "twitter.com"
    "facebook.com"
    "instagram.com"
    "threads.net"
    "reddit.com"
    "linkedin.com"
    "whatsapp.com"
    "whatsapp.net"
    "discord.com"
    "discord.gg"
    "discordapp.com"
    "discordapp.net"
)

# 🔒 Streaming - 流媒体域名
PROTECTED_STREAMING=(
    "netflix.com"
    "hbomax.com"
    "hbo.com"
    "disneyplus.com"
    "disney.com"
    "hulu.com"
    "primevideo.com"
    "amazon.com"
    "youtube.com"
    "youtu.be"
    "googlevideo.com"
    "ytimg.com"
    "twitch.tv"
    "twitchcdn.net"
    "spotify.com"
    "scdn.co"
)

# 🔒 Gaming - 游戏平台域名（网站部分，非下载）
PROTECTED_GAMING=(
    "steampowered.com"
    "steamcommunity.com"
    "epicgames.com"
    "gog.com"
    "battle.net"
    "blizzard.com"
    "ea.com"
    "origin.com"
    "ubisoft.com"
    "uplay.com"
    "rockstargames.com"
    "xbox.com"
    "playstation.com"
    "nintendo.com"
    "itch.io"
)

# 🔒 AI - AI 平台域名
PROTECTED_AI=(
    "openai.com"
    "chatgpt.com"
    "claude.ai"
    "anthropic.com"
    "gemini.google.com"
    "bard.google.com"
    "copilot.microsoft.com"
    "perplexity.ai"
)

# ═══════════════════════════════════════════════════════════════════════════════
# NSFW 规则集排除列表（精确匹配）
# 这些域名不应该出现在 NSFW.list 中
# 使用 ^EXACT$ 格式表示精确匹配
# ═══════════════════════════════════════════════════════════════════════════════
NSFW_EXCLUDE_EXACT=(
    # 社交媒体（精确匹配）
    "x.com"
    "twitter.com"
    "facebook.com"
    "instagram.com"
    "reddit.com"
    "discord.com"
    "discordapp.com"
    "discordapp.net"
    "media.discordapp.net"
    "cdn.discordapp.com"
    
    # 流媒体（精确匹配）
    "netflix.com"
    "hbomax.com"
    "hbo.com"
    "youtube.com"
    "youtu.be"
    "twitch.tv"
    "spotify.com"
    
    # 游戏平台（精确匹配）
    "itch.io"
    
    # 图片托管（精确匹配）
    "images.pexels.com"
    "imgur.com"
    
    # 其他误判（精确匹配）
    "happymag.tv"
    "wortfm.org"
)

# ═══════════════════════════════════════════════════════════════════════════════
# 清理函数
# ═══════════════════════════════════════════════════════════════════════════════

# 精确匹配清理函数
# 只移除完全匹配的域名，不会误删包含该字符串的其他域名
clean_ruleset_exact() {
    local ruleset_file="$1"
    local ruleset_name=$(basename "$ruleset_file" .list)
    shift
    local exclude_domains=("$@")
    
    if [[ ! -f "$ruleset_file" ]]; then
        log_warning "规则集不存在: $ruleset_file"
        return
    fi
    
    local before_count=$(grep -cv "^#\|^$" "$ruleset_file" 2>/dev/null || echo "0")
    local temp_file=$(mktemp)
    local removed_count=0
    
    # 复制头部注释
    grep "^#" "$ruleset_file" > "$temp_file" 2>/dev/null || true
    echo "" >> "$temp_file"
    
    # 过滤规则（精确匹配）
    while IFS= read -r line; do
        [[ -z "$line" || "$line" =~ ^# ]] && continue
        
        local should_exclude=false
        
        # 提取域名部分（格式: DOMAIN-SUFFIX,domain.com 或 DOMAIN,domain.com）
        local domain=""
        if [[ "$line" == DOMAIN-SUFFIX,* ]]; then
            domain="${line#DOMAIN-SUFFIX,}"
        elif [[ "$line" == DOMAIN,* ]]; then
            domain="${line#DOMAIN,}"
        elif [[ "$line" == DOMAIN-KEYWORD,* ]]; then
            domain="${line#DOMAIN-KEYWORD,}"
        fi
        
        # 移除可能的策略后缀（如 ,REJECT）
        domain="${domain%%,*}"
        
        # 精确匹配检查
        for exclude_domain in "${exclude_domains[@]}"; do
            if [[ "$domain" == "$exclude_domain" ]]; then
                should_exclude=true
                log_warning "  移除: $line (精确匹配: $exclude_domain)"
                removed_count=$((removed_count + 1))
                break
            fi
        done
        
        if [[ "$should_exclude" == "false" ]]; then
            echo "$line" >> "$temp_file"
        fi
    done < "$ruleset_file"
    
    if [[ $removed_count -gt 0 ]]; then
        mv "$temp_file" "$ruleset_file"
        local after_count=$(grep -cv "^#\|^$" "$ruleset_file" 2>/dev/null || echo "0")
        log_success "$ruleset_name: 移除 $removed_count 条混入规则 ($before_count → $after_count)"
    else
        rm -f "$temp_file"
        log_info "$ruleset_name: 无混入规则"
    fi
}

# ═══════════════════════════════════════════════════════════════════════════════
# 通用排除列表（适用于大多数规则集）
# ═══════════════════════════════════════════════════════════════════════════════
COMMON_EXCLUDE_EXACT=(
    # 社交媒体
    "x.com"
    "twitter.com"
    "facebook.com"
    "instagram.com"
    "reddit.com"
    "discord.com"
    "discordapp.com"
    "discordapp.net"
    "media.discordapp.net"
    "cdn.discordapp.com"
    
    # 流媒体
    "netflix.com"
    "hbomax.com"
    "hbo.com"
    "youtube.com"
    "youtu.be"
    "twitch.tv"
    "spotify.com"
    
    # 游戏平台
    "itch.io"
    "steampowered.com"
    "epicgames.com"
    
    # 图片托管
    "images.pexels.com"
    "imgur.com"
)

# ═══════════════════════════════════════════════════════════════════════════════
# 跳过清理的规则集列表
# 这些规则集本身就应该包含特定域名，不需要清理
# ═══════════════════════════════════════════════════════════════════════════════
SKIP_RULESETS=(
    # 社交媒体类（应该包含社交媒体域名）
    "SocialMedia"
    "Telegram"
    
    # 游戏类（应该包含游戏平台域名）
    "Gaming"
    "Steam"
    "Epic"
    
    # 流媒体类（应该包含流媒体域名）
    "GlobalMedia"
    "YouTube"
    "Spotify"
    "Twitch"
    "Netflix"
    "TikTok"
    "StreamUS"
    "StreamJP"
    "StreamKR"
    "StreamEU"
    "StreamHK"
    "StreamTW"
)

# 🔒 受保护规则集（手动维护，永不修改）
PROTECTED_RULESETS=(
    "DownloadDirect"
    "Manual"
    "Manual_JP"
    "Manual_US"
    "Manual_West"
    "Manual_Global"
)

# 检查是否应该跳过
should_skip() {
    local name="$1"
    for skip in "${SKIP_RULESETS[@]}"; do
        [[ "$name" == "$skip" ]] && return 0
    done
    return 1
}

# 检查是否为受保护规则集
is_protected() {
    local name="$1"
    for protected in "${PROTECTED_RULESETS[@]}"; do
        [[ "$name" == "$protected" ]] && return 0
    done
    return 1
}

# ═══════════════════════════════════════════════════════════════════════════════
# 主流程
# ═══════════════════════════════════════════════════════════════════════════════

log_info "开始清理规则集冲突..."
echo ""

total_cleaned=0
total_checked=0
total_skipped=0
total_protected=0

# 遍历所有规则集
for ruleset_file in "$RULESET_DIR"/*.list; do
    [[ ! -f "$ruleset_file" ]] && continue
    
    ruleset_name=$(basename "$ruleset_file" .list)
    total_checked=$((total_checked + 1))
    
    # 跳过受保护的规则集（手动维护）
    if is_protected "$ruleset_name"; then
        echo -e "${GREEN}[🔒 PROTECTED]${NC} $ruleset_name (手动维护)"
        total_protected=$((total_protected + 1))
        continue
    fi
    
    # 跳过应该包含特定域名的规则集
    if should_skip "$ruleset_name"; then
        echo -e "${CYAN}[SKIP]${NC} $ruleset_name (应包含特定域名)"
        total_skipped=$((total_skipped + 1))
        continue
    fi
    
    # 根据规则集类型选择排除列表
    case "$ruleset_name" in
        NSFW)
            echo -e "${BLUE}[CLEAN]${NC} $ruleset_name"
            clean_ruleset_exact "$ruleset_file" "${NSFW_EXCLUDE_EXACT[@]}"
            ;;
        CDN|LAN|ChinaDirect|GlobalProxy)
            # 这些规则集有特殊用途，跳过
            echo -e "${CYAN}[SKIP]${NC} $ruleset_name (特殊用途)"
            total_skipped=$((total_skipped + 1))
            ;;
        *)
            # 其他规则集使用通用排除列表
            echo -e "${BLUE}[CLEAN]${NC} $ruleset_name"
            clean_ruleset_exact "$ruleset_file" "${COMMON_EXCLUDE_EXACT[@]}"
            ;;
    esac
done

echo ""
log_success "规则集冲突清理完成！"
echo "  总计: $total_checked 个规则集"
echo "  🔒 受保护: $total_protected 个 (手动维护)"
echo "  ⏭️  跳过: $total_skipped 个 (应包含特定域名)"
echo ""
echo "提示: 运行 batch_convert_to_singbox.sh 重新生成 SRS 文件"
