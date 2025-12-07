#!/usr/bin/env bash
# =============================================================================
# Verify All Proxy Configs Sync Status
# Function: Check Surge, Singbox, Shadowrocket configs are in sync
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           Proxy Configs Sync Verification                    ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ═══════════════════════════════════════════════════════════════
# 1. Check Surge config
# ═══════════════════════════════════════════════════════════════
echo -e "${CYAN}=== 1. Surge Config ===${NC}"
SURGE_TEMPLATE="$PROJECT_ROOT/ruleset/Sources/surge_rules_complete.conf"
SURGE_ICLOUD="$HOME/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo Pro Max plus👑_fixed.conf"

if [ -f "$SURGE_TEMPLATE" ]; then
    SURGE_TEMPLATE_RULES=$(grep -c "^RULE-SET," "$SURGE_TEMPLATE" || echo "0")
    echo -e "${GREEN}✅ Template: $SURGE_TEMPLATE_RULES RULE-SET${NC}"
else
    echo -e "${RED}❌ Template not found${NC}"
fi

if [ -f "$SURGE_ICLOUD" ]; then
    SURGE_ICLOUD_RULES=$(grep -c "^RULE-SET," "$SURGE_ICLOUD" || echo "0")
    echo -e "${GREEN}✅ iCloud: $SURGE_ICLOUD_RULES RULE-SET${NC}"
    
    # Check for invalid lines
    python3 "$SCRIPT_DIR/check_surge_config.py" 2>&1 | grep -E "^(✅|❌)" || true
else
    echo -e "${YELLOW}⚠️  iCloud config not found (may not be synced yet)${NC}"
fi

echo ""

# ═══════════════════════════════════════════════════════════════
# 2. Check Singbox config
# ═══════════════════════════════════════════════════════════════
echo -e "${CYAN}=== 2. Singbox Config ===${NC}"
SINGBOX_SUBSTORE="$PROJECT_ROOT/substore/Singbox_substore_1.13.0+.json"
SINGBOX_PRIVATE="$PROJECT_ROOT/隐私🔏/singbox_config_生成后.json"

if [ -f "$SINGBOX_SUBSTORE" ]; then
    SINGBOX_SUBSTORE_RULES=$(python3 -c "import json; print(len(json.load(open('$SINGBOX_SUBSTORE'))['route']['rule_set']))" 2>/dev/null || echo "0")
    echo -e "${GREEN}✅ Substore: $SINGBOX_SUBSTORE_RULES rule_set${NC}"
    
    # Validate config
    if bash "$SCRIPT_DIR/test_singbox_startup.sh" 2>&1 | grep -q "validation passed"; then
        echo -e "${GREEN}✅ Config validation: PASSED${NC}"
    else
        echo -e "${RED}❌ Config validation: FAILED${NC}"
    fi
else
    echo -e "${RED}❌ Substore config not found${NC}"
fi

if [ -f "$SINGBOX_PRIVATE" ]; then
    SINGBOX_PRIVATE_RULES=$(python3 -c "import json; print(len(json.load(open('$SINGBOX_PRIVATE'))['route']['rule_set']))" 2>/dev/null || echo "0")
    echo -e "${GREEN}✅ Private: $SINGBOX_PRIVATE_RULES rule_set${NC}"
else
    echo -e "${YELLOW}⚠️  Private config not found${NC}"
fi

echo ""

# ═══════════════════════════════════════════════════════════════
# 3. Check Shadowrocket config
# ═══════════════════════════════════════════════════════════════
echo -e "${CYAN}=== 3. Shadowrocket Config ===${NC}"
SR_CONFIG="$PROJECT_ROOT/隐私🔏/shadowrocket_config.conf"

if [ -f "$SR_CONFIG" ]; then
    SR_RULES=$(grep -c "^RULE-SET," "$SR_CONFIG" || echo "0")
    echo -e "${GREEN}✅ Config: $SR_RULES RULE-SET${NC}"
else
    echo -e "${YELLOW}⚠️  Config not found${NC}"
fi

echo ""

# ═══════════════════════════════════════════════════════════════
# 4. Sync Status Summary
# ═══════════════════════════════════════════════════════════════
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    Sync Status Summary                       ║${NC}"
echo -e "${BLUE}╠══════════════════════════════════════════════════════════════╣${NC}"

printf "${BLUE}║  ${CYAN}Surge Template:${NC}    ${GREEN}%-5s${NC} RULE-SET                        ${BLUE}║${NC}\n" "${SURGE_TEMPLATE_RULES:-0}"
printf "${BLUE}║  ${CYAN}Surge iCloud:${NC}      ${GREEN}%-5s${NC} RULE-SET                        ${BLUE}║${NC}\n" "${SURGE_ICLOUD_RULES:-0}"
printf "${BLUE}║  ${CYAN}Singbox Substore:${NC} ${GREEN}%-5s${NC} rule_set                        ${BLUE}║${NC}\n" "${SINGBOX_SUBSTORE_RULES:-0}"
printf "${BLUE}║  ${CYAN}Singbox Private:${NC}  ${GREEN}%-5s${NC} rule_set                        ${BLUE}║${NC}\n" "${SINGBOX_PRIVATE_RULES:-0}"
printf "${BLUE}║  ${CYAN}Shadowrocket:${NC}     ${GREEN}%-5s${NC} RULE-SET                        ${BLUE}║${NC}\n" "${SR_RULES:-0}"

echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"

echo ""
echo -e "${CYAN}📝 Notes:${NC}"
echo "  - Surge: 48 RULE-SET (includes LAN + SYSTEM)"
echo "  - Singbox: 47 rule_set (ChinaIP used in route config)"
echo "  - Difference is expected and normal"
echo ""

# Check if all configs are in sync
if [ "${SURGE_TEMPLATE_RULES:-0}" -eq 48 ] && \
   [ "${SINGBOX_SUBSTORE_RULES:-0}" -eq 47 ] && \
   [ "${SINGBOX_PRIVATE_RULES:-0}" -eq 47 ]; then
    echo -e "${GREEN}✅ All configs are in sync!${NC}"
    exit 0
else
    echo -e "${YELLOW}⚠️  Some configs may need syncing${NC}"
    echo -e "${CYAN}💡 Run: bash merge_sync/full_update.sh --quick${NC}"
    exit 1
fi
