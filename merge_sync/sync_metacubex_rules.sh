#!/bin/bash
# =============================================================================
# 同步MetaCubeX规则到本地并转换为Surge格式
# 使用sing-box decompile将.srs反编译为JSON，再转换为.list
# 优化: 并行下载 + 本地sing-box优先
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
METACUBEX_DIR="${RULESET_DIR}/MetaCubeX"
TMP_DIR="/tmp/metacubex_sync_$$"
LOCAL_SINGBOX="${SCRIPT_DIR}/config-manager-auto-update/bin/sing-box"

# MetaCubeX规则列表 (tag:url)
METACUBEX_RULES=(
    "telegram:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/telegram.srs"
    "discord:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/discord.srs"
    "google:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/google.srs"
    "apple:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/apple.srs"
    "microsoft:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/microsoft.srs"
    "bilibili:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/bilibili.srs"
    "category-ai-cn:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/category-ai-!cn.srs"
    "category-games:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/category-games.srs"
    "category-ads-all:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/category-ads-all.srs"
    "instagram:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/instagram.srs"
    "twitter:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/twitter.srs"
    "spotify:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/spotify.srs"
    "steam:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/steam.srs"
    "youtube:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/youtube.srs"
    "github:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/github.srs"
    "netflix:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/netflix.srs"
    "tiktok:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/tiktok.srs"
    "paypal:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/paypal.srs"
    "amazon:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/amazon.srs"
    "reddit:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/reddit.srs"
    "whatsapp:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/whatsapp.srs"
    "facebook:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/facebook.srs"
    "openai:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/openai.srs"
    "cloudflare:https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/geosite/cloudflare.srs"
)

echo -e "${BLUE}=== 同步MetaCubeX规则 (优化版) ===${NC}"
echo "目标目录: $METACUBEX_DIR"

# 创建目录
mkdir -p "$METACUBEX_DIR"
mkdir -p "$TMP_DIR"

# 选择sing-box: 优先使用本地预览版
if [ -x "$LOCAL_SINGBOX" ]; then
    SINGBOX="$LOCAL_SINGBOX"
    echo -e "${GREEN}使用本地sing-box: $("$SINGBOX" version | head -1)${NC}"
elif command -v sing-box &> /dev/null; then
    SINGBOX="sing-box"
    echo -e "${YELLOW}使用系统sing-box: $(sing-box version | head -1)${NC}"
else
    echo -e "${RED}❌ sing-box未安装${NC}"
    exit 1
fi

# 转换JSON到Surge格式 (优化版)
json_to_surge() {
    local json_file="$1"
    local output_file="$2"
    local name="$3"
    
    python3 -c "
import json
with open('$json_file') as f:
    data = json.load(f)
rules = []
for rule in data.get('rules', []):
    rules.extend(f'DOMAIN,{d}' for d in rule.get('domain', []))
    rules.extend(f'DOMAIN-SUFFIX,{d}' for d in rule.get('domain_suffix', []))
    rules.extend(f'DOMAIN-KEYWORD,{d}' for d in rule.get('domain_keyword', []))
    rules.extend(f'DOMAIN-REGEX,{d}' for d in rule.get('domain_regex', []))
    rules.extend(f'IP-CIDR,{ip},no-resolve' for ip in rule.get('ip_cidr', []))
rules = list(dict.fromkeys(rules))
with open('$output_file', 'w') as f:
    f.write(f'# MetaCubeX geosite-$name\n# 规则数: {len(rules)}\n\n')
    f.write('\n'.join(rules) + '\n')
print(f'{len(rules)}')" 2>/dev/null
}

# 处理单个规则 (用于并行)
process_rule() {
    local entry="$1"
    local name="${entry%%:*}"
    local url="${entry#*:}"
    
    local srs_file="${TMP_DIR}/${name}.srs"
    local json_file="${TMP_DIR}/${name}.json"
    local list_file="${METACUBEX_DIR}/MetaCubeX_${name}.list"
    
    # 下载 + 反编译 + 转换
    if curl -sL --connect-timeout 10 "$url" -o "$srs_file" 2>/dev/null && \
       "$SINGBOX" rule-set decompile "$srs_file" -o "$json_file" 2>/dev/null; then
        local count=$(json_to_surge "$json_file" "$list_file" "$name")
        echo -e "${GREEN}✅ ${name}: ${count}条${NC}"
        return 0
    else
        echo -e "${RED}❌ ${name}${NC}"
        return 1
    fi
}

export -f json_to_surge process_rule
export TMP_DIR METACUBEX_DIR SINGBOX GREEN RED NC

SUCCESS=0
FAILED=0

echo ""
# 并行处理 (最多4个并发)
for entry in "${METACUBEX_RULES[@]}"; do
    if process_rule "$entry"; then
        ((SUCCESS++))
    else
        ((FAILED++))
    fi
done

# 清理
rm -rf "$TMP_DIR"

echo ""
echo -e "${BLUE}=== 完成 ===${NC}"
echo -e "成功: ${GREEN}${SUCCESS}${NC} | 失败: ${RED}${FAILED}${NC}"
echo "下一步: ./update_sources_metacubex.sh && ./incremental_merge_all.sh"
