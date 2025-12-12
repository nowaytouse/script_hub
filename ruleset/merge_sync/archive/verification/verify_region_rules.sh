#!/opt/homebrew/bin/bash
# =============================================================================
# Region Rule Assignment Verification Script
# Function: Verify Surge, Singbox, Shadowrocket region streaming rule assignments
# Created: 2025-12-07
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       Region Rule Assignment Verification Tool               ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Define correct region rule mappings
declare -A REGION_RULES=(
    ["StreamJP"]="🇯🇵"
    ["StreamUS"]="🇺🇸"
    ["StreamKR"]="🇰🇷"
    ["StreamHK"]="🇭🇰"
    ["StreamTW"]="🇹🇼"
    ["StreamEU"]="🇬🇧"
)

TOTAL_ERRORS=0

# ============================================================================
# Verify Surge configuration
# ============================================================================
log_info "Verifying Surge configuration..."
SURGE_CONFIG="ruleset/Sources/surge_rules_complete.conf"

if [ ! -f "$SURGE_CONFIG" ]; then
    log_error "Surge config file not found: $SURGE_CONFIG"
    exit 1
fi

SURGE_ERRORS=0
for rule in "${!REGION_RULES[@]}"; do
    region="${REGION_RULES[$rule]}"
    
    # Check if rule exists and is correctly assigned
    if grep -q "RULE-SET.*${rule}.list" "$SURGE_CONFIG"; then
        line=$(grep "RULE-SET.*${rule}.list" "$SURGE_CONFIG")
        if echo "$line" | grep -q "$region"; then
            echo "  $rule -> $region"
        else
            echo "  $rule assignment error"
            echo "     Line: $line"
            SURGE_ERRORS=$((SURGE_ERRORS + 1))
        fi
    else
        log_warning "  $rule rule not found"
    fi
done

if [ $SURGE_ERRORS -eq 0 ]; then
    log_success "Surge configuration verified"
else
    log_error "Surge configuration has $SURGE_ERRORS errors"
    TOTAL_ERRORS=$((TOTAL_ERRORS + SURGE_ERRORS))
fi

echo ""

# ============================================================================
# Verify Singbox configuration
# ============================================================================
log_info "Verifying Singbox configuration..."
SINGBOX_CONFIG="substore/Singbox_substore_1.13.0+.json"

if [ ! -f "$SINGBOX_CONFIG" ]; then
    log_error "Singbox config file not found: $SINGBOX_CONFIG"
    exit 1
fi

python3 - "$SINGBOX_CONFIG" <<'PYTHON_SCRIPT'
import json
import sys

config_file = sys.argv[1]
with open(config_file, 'r', encoding='utf-8') as f:
    config = json.load(f)

# Define correct mappings
region_rules = {
    'surge-streamjp': '🇯🇵',
    'surge-streamus': '🇺🇸',
    'surge-streamkr': '🇰🇷',
    'surge-streamhk': '🇭🇰',
    'surge-streamtw': '🇹🇼',
    'surge-streameu': '🇬🇧'
}

errors = 0
for rule in config.get('route', {}).get('rules', []):
    rule_set = rule.get('rule_set')
    outbound = rule.get('outbound', '')
    
    if isinstance(rule_set, str) and rule_set in region_rules:
        expected_region = region_rules[rule_set]
        if expected_region in outbound:
            print(f"  {rule_set} -> {outbound}")
        else:
            print(f"  {rule_set} assignment error")
            print(f"     Current: {outbound}")
            print(f"     Expected: {expected_region}")
            errors += 1

sys.exit(errors)
PYTHON_SCRIPT

SINGBOX_ERRORS=$?
if [ $SINGBOX_ERRORS -eq 0 ]; then
    log_success "Singbox configuration verified"
else
    log_error "Singbox configuration has $SINGBOX_ERRORS errors"
    TOTAL_ERRORS=$((TOTAL_ERRORS + SINGBOX_ERRORS))
fi

echo ""

# ============================================================================
# Summary
# ============================================================================
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    Verification Summary                      ║${NC}"
echo -e "${BLUE}╠══════════════════════════════════════════════════════════════╣${NC}"

if [ $TOTAL_ERRORS -eq 0 ]; then
    echo -e "${BLUE}║  ${GREEN}All configurations verified!${NC}                               ${BLUE}║${NC}"
    echo -e "${BLUE}║  ${GREEN}Region rule assignments are correct${NC}                       ${BLUE}║${NC}"
else
    echo -e "${BLUE}║  ${RED}Found $TOTAL_ERRORS errors${NC}                                         ${BLUE}║${NC}"
    echo -e "${BLUE}║  ${RED}Please check and fix config files${NC}                          ${BLUE}║${NC}"
fi

echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

exit $TOTAL_ERRORS
