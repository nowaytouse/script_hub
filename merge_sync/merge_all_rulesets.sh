#!/bin/bash
# =============================================================================
# 完整规则集合并脚本 - 智能无人值守版
# =============================================================================
# 1. GIT PULL 拉取最新代码
# 2. 调用 merge_adblock_modules.sh 处理广告规则
# 3. 从 ruleset/Sources/Links/*.txt 读取源链接并合并
# 4. 调用 smart_cleanup.py 进行清洗
# 5. GIT PUSH 提交更改
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RULESET_DIR="${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)"
LINKS_DIR="${PROJECT_ROOT}/ruleset/Sources/Links"
TEMP_DIR=$(mktemp -d)

cleanup() { rm -rf "$TEMP_DIR"; }
trap cleanup EXIT

echo -e "${BLUE}=== 规则集智能合并脚本 (无人值守模式) ===${NC}"
echo ""

# 0. Git Pull
echo -e "${YELLOW}>>> 正在同步 Git 仓库...${NC}"
cd "$PROJECT_ROOT"
if git pull; then
    echo -e "${GREEN}✓ Git Pull 成功${NC}"
else
    echo -e "${RED}✗ Git Pull 失败，但继续尝试执行...${NC}"
fi

# 1. 处理广告拦截模块 (AdBlock)
echo ""
echo -e "${YELLOW}>>> (1/4) 处理广告拦截模块...${NC}"
if [ -f "${SCRIPT_DIR}/merge_adblock_modules.sh" ]; then
    bash "${SCRIPT_DIR}/merge_adblock_modules.sh" --auto --no-backup
else
    echo -e "${RED}错误: 找不到 merge_adblock_modules.sh${NC}"
fi

# 下载函数
download_rules() {
    local url="$1" output="$2"
    if [[ "$url" == file://* ]]; then
        local path="${url#file://}"
        if [ -f "$path" ]; then cp "$path" "$output"; return 0; fi
    elif [ -f "$url" ]; then
        cp "$url" "$output"; return 0;
    else
        curl -sL --connect-timeout 15 --max-time 60 "$url" -o "$output" 2>/dev/null
    fi
}

# 提取函数
extract_rules() {
    local input="$1"
    grep -E '^(DOMAIN-SUFFIX|DOMAIN-KEYWORD|DOMAIN|IP-CIDR|IP-CIDR6|PROCESS-NAME|IN-PORT|DEST-PORT|SRC-PORT),' "$input" 2>/dev/null | \
        sed 's/[[:space:]]*$//' | \
        awk -F, '{
            type = $1; gsub(/^[ \t]+|[ \t]+$/, "", $2); split($2, a, " "); val = a[1];
            if(type == "IP-CIDR" && index(val, ":") > 0) type = "IP-CIDR6";
            out = type "," val;
            for(i=3; i<=NF; i++) { gsub(/^[ \t]+|[ \t]+$/, "", $i); if($i == "no-resolve") out = out "," $i; }
            print out;
        }' || true
}

# 合并函数 - 必须使用ruleset_merger.sh
merge_category() {
    local list_name="$1"
    local source_name="$2"
    local target="$RULESET_DIR/$list_name"
    local source_file="$LINKS_DIR/$source_name"
    
    if [ ! -f "$source_file" ]; then 
        echo -e "${YELLOW}Skipping $list_name (Not found: $source_name)${NC}"
        return
    fi
    
    echo -e "${BLUE}正在生成: $list_name ...${NC}"
    
    # 🔥 必须使用ruleset_merger.sh，不存在则响亮报错
    if [ ! -f "${SCRIPT_DIR}/ruleset_merger.sh" ]; then
        echo -e "${RED}❌ FATAL ERROR: ruleset_merger.sh not found!${NC}"
        echo -e "${RED}   Path: ${SCRIPT_DIR}/ruleset_merger.sh${NC}"
        echo -e "${RED}   This script is REQUIRED for ruleset generation.${NC}"
        exit 1
    fi
    
    # 创建临时空文件作为target (ruleset_merger需要已存在的target)
    if [ ! -f "$target" ]; then
        touch "$target"
    fi
    
    # 调用ruleset_merger.sh
    bash "${SCRIPT_DIR}/ruleset_merger.sh" \
        -t "$target" \
        -l "$source_file" \
        -o "$target" \
        -n "$(basename "$target" .list)" \
        2>&1 | grep -E "(INFO|OK|WARN|ERROR)" || true
    
    echo -e "  ${GREEN}✓ 合并完成${NC}"
}

# 2. 执行所有分类合并
echo ""
echo -e "${YELLOW}>>> (2/4) 合并通用规则集...${NC}"

# Core
merge_category "GlobalMedia.list" "GlobalMedia_sources.txt"
merge_category "GlobalProxy.list" "GlobalProxy_sources.txt"
merge_category "ChinaDirect.list" "ChinaDirect_sources.txt"
merge_category "ChinaIP.list" "ChinaIP_sources.txt"
merge_category "LAN.list" "LAN_sources.txt"
merge_category "NSFW.list" "NSFW_sources.txt"

# Process & Ports (Local Conf Absorbed)
merge_category "DirectProcess.list" "DirectProcess_sources.txt"
merge_category "FirewallPorts.list" "FirewallPorts_sources.txt"

# Categories
merge_category "AI.list" "AI_sources.txt"
merge_category "Gaming.list" "Gaming_sources.txt"
merge_category "SocialMedia.list" "SocialMedia_sources.txt"
merge_category "Microsoft.list" "Microsoft_sources.txt"
merge_category "Apple.list" "Apple_sources.txt"
merge_category "PayPal.list" "PayPal_sources.txt"
merge_category "Telegram.list" "Telegram_sources.txt"
merge_category "GitHub.list" "GitHub_sources.txt"
merge_category "CDN.list" "CDN_sources.txt"
merge_category "Fediverse.list" "Fediverse_sources.txt"

# Specifics (for those preferring granular lists)
merge_category "Twitter.list" "Twitter_sources.txt"
merge_category "Instagram.list" "Instagram_sources.txt"
merge_category "TikTok.list" "TikTok_sources.txt"
merge_category "Netflix.list" "Netflix_sources.txt"
merge_category "Spotify.list" "Spotify_sources.txt"
merge_category "YouTube.list" "YouTube_sources.txt"
merge_category "Google.list" "Google_sources.txt"
merge_category "Steam.list" "Steam_sources.txt"
merge_category "Disney.list" "Disney_sources.txt"
merge_category "Reddit.list" "Reddit_sources.txt"
merge_category "Bing.list" "Bing_sources.txt"
merge_category "Bilibili.list" "Bilibili_sources.txt"


# 3. 智能清洗
echo ""
echo -e "${YELLOW}>>> (3/4) 执行智能冲突清洗 (Smart Cleanup)...${NC}"
python3 "${SCRIPT_DIR}/smart_cleanup.py"


# 4. Git Push
echo ""
echo -e "${YELLOW}>>> (4/4) 提交更改到 Git...${NC}"
cd "$PROJECT_ROOT"

# Check if there are changes
if [[ -n $(git status -s) ]]; then
    git add .
    git commit -m "Auto-update: Ruleset synchronization $(date '+%Y-%m-%d %H:%M')"
    
    if git push; then
        echo -e "${GREEN}✓ Git Push 成功${NC}"
    else
        echo -e "${RED}✗ Git Push 失败 (可能需要手动处理)${NC}"
    fi
else
    echo -e "${GREEN}无需提交 (没有变更)${NC}"
fi

echo ""
echo -e "${GREEN}=== 全流程完成 ===${NC}"
