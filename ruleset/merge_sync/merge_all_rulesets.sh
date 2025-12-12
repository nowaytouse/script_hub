#!/opt/homebrew/bin/bash
# =============================================================================
# Complete Ruleset Merger - Merge all third-party rules into own rulesets
# =============================================================================
# Goal: Merge MetaCubeX/SagerNet/Sukka/Chocolate4U rules into own rulesets
# Ensure Singbox config only uses own rulesets
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

echo -e "${BLUE}=== Complete Ruleset Merger ===${NC}"
echo ""

# Download function
download_rules() {
    local url="$1" output="$2"
    curl -sL --connect-timeout 15 --max-time 60 "$url" -o "$output" 2>/dev/null
}

# Extract valid rules
extract_rules() {
    local input="$1"
    grep -E '^(DOMAIN-SUFFIX|DOMAIN-KEYWORD|DOMAIN|IP-CIDR|IP-CIDR6|PROCESS-NAME|URL-REGEX|USER-AGENT|DOMAIN-REGEX),' "$input" 2>/dev/null | \
        # Filter out invalid DOMAIN-REGEX rules (empty or single character patterns)
        grep -v '^DOMAIN-REGEX,\s*$' | \
        grep -v '^DOMAIN-REGEX,[^,]*$' | \
        # Fix IPv6 rules: IP-CIDR with IPv6 should be IP-CIDR6
        sed 's/^IP-CIDR,\([0-9a-fA-F:]*::[^,]*\)/IP-CIDR6,\1/' | \
        # Remove trailing whitespace
        sed 's/[[:space:]]*$//' || true
}

# Merge rules to target file
merge_to_target() {
    local target="$1"
    shift
    local sources=("$@")
    local temp_merged="$TEMP_DIR/merged_$RANDOM.txt"
    
    echo -e "${YELLOW}Merging to: $(basename $target)${NC}"
    
    # Create new temp file
    touch "$temp_merged"
    
    # Preserve existing rules
    if [ -f "$target" ]; then
        extract_rules "$target" >> "$temp_merged"
    fi
    
    # Download and merge each source
    for url in "${sources[@]}"; do
        local temp_download="$TEMP_DIR/download_$RANDOM.txt"
        echo "  <- $url"
        if download_rules "$url" "$temp_download"; then
            extract_rules "$temp_download" >> "$temp_merged"
        else
            echo -e "    ${RED}Download failed${NC}"
        fi
    done
    
    # Deduplicate and sort
    local count_before=$(wc -l < "$temp_merged" | tr -d ' ')
    sort -u "$temp_merged" -o "$temp_merged"
    local count_after=$(wc -l < "$temp_merged" | tr -d ' ')
    
    # Generate output with header
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
    
    echo -e "  ${GREEN}✓ ${count_after} rules (before dedup: ${count_before})${NC}"
}

# ============================================
# 1. GlobalMedia - Merge streaming rules
# ============================================
echo ""
echo -e "${BLUE}[1/8] GlobalMedia - Streaming Services${NC}"
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
# 2. AI - Merge AI service rules
# ============================================
echo ""
echo -e "${BLUE}[2/8] AI - AI Services${NC}"
AI_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/OpenAI/OpenAI.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Claude/Claude.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Gemini/Gemini.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Copilot/Copilot.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/BardAI/BardAI.list"
)
merge_to_target "$RULESET_DIR/AI.list" "${AI_SOURCES[@]}"

# ============================================
# 3. Gaming - Merge gaming rules
# ============================================
echo ""
echo -e "${BLUE}[3/8] Gaming - Gaming Platforms${NC}"
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
# 4. GlobalProxy - Merge proxy rules (GFW + geolocation-!cn)
# ============================================
echo ""
echo -e "${BLUE}[4/8] GlobalProxy - Proxy Rules${NC}"
GLOBALPROXY_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Global/Global.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Proxy/Proxy.list"
)
merge_to_target "$RULESET_DIR/GlobalProxy.list" "${GLOBALPROXY_SOURCES[@]}"

# ============================================
# 5. LAN - Merge private network rules
# ============================================
echo ""
echo -e "${BLUE}[5/8] LAN - Private Network${NC}"
LAN_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Lan/Lan.list"
)
merge_to_target "$RULESET_DIR/LAN.list" "${LAN_SOURCES[@]}"

# ============================================
# 6. Microsoft - Merge Microsoft rules (incl. Bing)
# ============================================
echo ""
echo -e "${BLUE}[6/8] Microsoft - Microsoft Services${NC}"
MICROSOFT_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Microsoft/Microsoft.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Bing/Bing.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/OneDrive/OneDrive.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Teams/Teams.list"
)
merge_to_target "$RULESET_DIR/Microsoft.list" "${MICROSOFT_SOURCES[@]}"

# ============================================
# 7. AdBlock - Merge ad blocking rules
# ============================================
echo ""
echo -e "${BLUE}[7/8] AdBlock - Ad Blocking${NC}"
ADBLOCK_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Advertising/Advertising.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Privacy/Privacy.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/Hijacking/Hijacking.list"
)
merge_to_target "$RULESET_DIR/AdBlock.list" "${ADBLOCK_SOURCES[@]}"

# ============================================
# 8. SocialMedia - Social Media (new)
# ============================================
echo ""
echo -e "${BLUE}[8/8] SocialMedia - Social Media (new)${NC}"
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
# 9. PayPal (new)
# ============================================
echo ""
echo -e "${BLUE}[9/9] PayPal (new)${NC}"
PAYPAL_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/PayPal/PayPal.list"
)
merge_to_target "$RULESET_DIR/PayPal.list" "${PAYPAL_SOURCES[@]}"

# ============================================
# 10. GitHub (new)
# ============================================
echo ""
echo -e "${BLUE}[10/10] GitHub (new)${NC}"
GITHUB_SOURCES=(
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/GitHub/GitHub.list"
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/GitLab/GitLab.list"
)
merge_to_target "$RULESET_DIR/GitHub.list" "${GITHUB_SOURCES[@]}"

echo ""
echo -e "${GREEN}=== Ruleset merge complete ===${NC}"
echo ""
echo "Next step: Run batch_convert_to_singbox.sh to generate .srs files"
