#!/usr/bin/env bash
# =============================================================================
# Sync MetaCubeX Rules to Local and Convert to Surge Format
# Use sing-box decompile to convert .srs to JSON, then to .list
# Optimization: Parallel download + Local sing-box priority
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

# MetaCubeX rule list (tag:url)
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

echo -e "${BLUE}=== Sync MetaCubeX Rules (Optimized) ===${NC}"
echo "Target directory: $METACUBEX_DIR"

# Create directories
mkdir -p "$METACUBEX_DIR"
mkdir -p "$TMP_DIR"

# Select sing-box: prefer local preview version
if [ -x "$LOCAL_SINGBOX" ]; then
    SINGBOX="$LOCAL_SINGBOX"
    echo -e "${GREEN}Using local sing-box: $("$SINGBOX" version | head -1)${NC}"
elif command -v sing-box &> /dev/null; then
    SINGBOX="sing-box"
    echo -e "${YELLOW}Using system sing-box: $(sing-box version | head -1)${NC}"
else
    echo -e "${RED}sing-box not installed${NC}"
    exit 1
fi

# Convert JSON to Surge format (optimized)
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
    f.write(f'# MetaCubeX geosite-$name\n# Rules: {len(rules)}\n\n')
    f.write('\n'.join(rules) + '\n')
print(f'{len(rules)}')" 2>/dev/null
}

# Process single rule (for parallel)
process_rule() {
    local entry="$1"
    local name="${entry%%:*}"
    local url="${entry#*:}"
    
    local srs_file="${TMP_DIR}/${name}.srs"
    local json_file="${TMP_DIR}/${name}.json"
    local list_file="${METACUBEX_DIR}/MetaCubeX_${name}.list"
    
    # Download + Decompile + Convert
    if curl -sL --connect-timeout 10 "$url" -o "$srs_file" 2>/dev/null && \
       "$SINGBOX" rule-set decompile "$srs_file" -o "$json_file" 2>/dev/null; then
        local count=$(json_to_surge "$json_file" "$list_file" "$name")
        echo -e "${GREEN}${name}: ${count} rules${NC}"
        return 0
    else
        echo -e "${RED}${name} failed${NC}"
        return 1
    fi
}

export -f json_to_surge process_rule
export TMP_DIR METACUBEX_DIR SINGBOX GREEN RED NC

SUCCESS=0
FAILED=0

echo ""
# Parallel processing (max 4 concurrent)
for entry in "${METACUBEX_RULES[@]}"; do
    if process_rule "$entry"; then
        ((SUCCESS++))
    else
        ((FAILED++))
    fi
done

# Cleanup
rm -rf "$TMP_DIR"

echo ""
echo -e "${BLUE}=== Complete ===${NC}"
echo -e "Success: ${GREEN}${SUCCESS}${NC} | Failed: ${RED}${FAILED}${NC}"
echo "Next: ./update_sources_metacubex.sh && ./incremental_merge_all.sh"
