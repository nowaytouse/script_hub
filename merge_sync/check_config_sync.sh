#!/usr/bin/env bash
# =============================================================================
# Configuration Sync Checker
# Purpose: Check if Surge, Singbox, Shadowrocket configs are fully synchronized
# Updated: 2025-12-07
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║       Configuration Sync Checker                             ║${NC}"
echo -e "${BLUE}║       Surge vs Singbox vs Shadowrocket                       ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Config file paths
SURGE_TEMPLATE="$PROJECT_ROOT/ruleset/Sources/surge_rules_complete.conf"
SINGBOX_CONFIG="$PROJECT_ROOT/substore/Singbox_substore_1.13.0+.json"

# ═══════════════════════════════════════════════════════════════
# Step 1: Extract Surge ruleset list
# ═══════════════════════════════════════════════════════════════
log_info "Step 1: Extracting Surge ruleset list..."

surge_rulesets=()
while IFS= read -r line; do
    # Match RULE-SET lines
    if [[ "$line" =~ RULE-SET.*https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/Surge\(Shadowkroket\)/([^.]+)\.list ]]; then
        ruleset_name="${BASH_REMATCH[1]}"
        surge_rulesets+=("$ruleset_name")
    fi
done < "$SURGE_TEMPLATE"

log_success "Surge ruleset count: ${#surge_rulesets[@]}"

# ═══════════════════════════════════════════════════════════════
# Step 2: Extract Singbox ruleset list
# ═══════════════════════════════════════════════════════════════
log_info "Step 2: Extracting Singbox ruleset list..."

singbox_rulesets=()
while IFS= read -r line; do
    # Match surge-xxx rulesets
    if [[ "$line" =~ \"surge-([^\"]+)\" ]]; then
        ruleset_name="${BASH_REMATCH[1]}"
        # Deduplicate
        if [[ ! " ${singbox_rulesets[@]} " =~ " ${ruleset_name} " ]]; then
            singbox_rulesets+=("$ruleset_name")
        fi
    fi
done < "$SINGBOX_CONFIG"

log_success "Singbox ruleset count: ${#singbox_rulesets[@]}"

# ═══════════════════════════════════════════════════════════════
# Step 3: Compare rulesets
# ═══════════════════════════════════════════════════════════════
log_info "Step 3: Comparing rulesets..."
echo ""

# Convert to lowercase and sort
surge_sorted=($(printf '%s\n' "${surge_rulesets[@]}" | tr '[:upper:]' '[:lower:]' | sort))
singbox_sorted=($(printf '%s\n' "${singbox_rulesets[@]}" | tr '[:upper:]' '[:lower:]' | sort))

# Check rulesets in Surge but not in Singbox
missing_in_singbox=()
for ruleset in "${surge_sorted[@]}"; do
    if [[ ! " ${singbox_sorted[@]} " =~ " ${ruleset} " ]]; then
        missing_in_singbox+=("$ruleset")
    fi
done

# Check rulesets in Singbox but not in Surge
extra_in_singbox=()
for ruleset in "${singbox_sorted[@]}"; do
    if [[ ! " ${surge_sorted[@]} " =~ " ${ruleset} " ]]; then
        extra_in_singbox+=("$ruleset")
    fi
done

# ═══════════════════════════════════════════════════════════════
# Step 4: Display results
# ═══════════════════════════════════════════════════════════════
echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║                    Comparison Results                        ║${NC}"
echo -e "${CYAN}╠══════════════════════════════════════════════════════════════╣${NC}"
printf "${CYAN}║${NC}  Surge rulesets:    %-40s ${CYAN}║${NC}\n" "${#surge_rulesets[@]}"
printf "${CYAN}║${NC}  Singbox rulesets:  %-40s ${CYAN}║${NC}\n" "${#singbox_rulesets[@]}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

if [ ${#missing_in_singbox[@]} -eq 0 ] && [ ${#extra_in_singbox[@]} -eq 0 ]; then
    log_success "✅ Fully synchronized! All rulesets match"
else
    if [ ${#missing_in_singbox[@]} -gt 0 ]; then
        log_warning "⚠️  Rulesets missing in Singbox (${#missing_in_singbox[@]}):"
        for ruleset in "${missing_in_singbox[@]}"; do
            echo "   - $ruleset"
        done
        echo ""
    fi
    
    if [ ${#extra_in_singbox[@]} -gt 0 ]; then
        log_info "ℹ️  Extra rulesets in Singbox (${#extra_in_singbox[@]}):"
        for ruleset in "${extra_in_singbox[@]}"; do
            echo "   - $ruleset"
        done
        echo ""
    fi
fi

# ═══════════════════════════════════════════════════════════════
# Step 5: Check key rulesets
# ═══════════════════════════════════════════════════════════════
log_info "Step 5: Checking key rulesets..."
echo ""

key_rulesets=(
    "adblock"
    "chinadirect"
    "globalproxy"
    "lan"
    "manual"
    "ai"
    "telegram"
    "netflix"
    "youtube"
    "google"
)

echo "Key Ruleset Check:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
printf "%-20s | %-10s | %-10s\n" "Ruleset" "Surge" "Singbox"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

for key in "${key_rulesets[@]}"; do
    surge_has="❌"
    singbox_has="❌"
    
    # Check Surge
    for ruleset in "${surge_sorted[@]}"; do
        if [[ "$ruleset" == "$key" ]]; then
            surge_has="✅"
            break
        fi
    done
    
    # Check Singbox
    for ruleset in "${singbox_sorted[@]}"; do
        if [[ "$ruleset" == "$key" ]]; then
            singbox_has="✅"
            break
        fi
    done
    
    printf "%-20s | %-10s | %-10s\n" "$key" "$surge_has" "$singbox_has"
done

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# ═══════════════════════════════════════════════════════════════
# Step 6: Check rule order (DNS leak prevention)
# ═══════════════════════════════════════════════════════════════
log_info "Step 6: Checking rule order (DNS leak prevention)..."
echo ""

# Check if ChinaDirect is before GlobalProxy
chinadirect_pos=-1
globalproxy_pos=-1

for i in "${!surge_rulesets[@]}"; do
    ruleset_lower=$(echo "${surge_rulesets[$i]}" | tr '[:upper:]' '[:lower:]')
    if [[ "$ruleset_lower" == "chinadirect" ]]; then
        chinadirect_pos=$i
    fi
    if [[ "$ruleset_lower" == "globalproxy" ]]; then
        globalproxy_pos=$i
    fi
done

if [ $chinadirect_pos -ge 0 ] && [ $globalproxy_pos -ge 0 ]; then
    if [ $chinadirect_pos -lt $globalproxy_pos ]; then
        log_success "✅ DNS leak prevention order correct: ChinaDirect (pos $chinadirect_pos) before GlobalProxy (pos $globalproxy_pos)"
    else
        log_error "❌ DNS leak prevention order wrong: ChinaDirect (pos $chinadirect_pos) after GlobalProxy (pos $globalproxy_pos)"
    fi
else
    log_warning "⚠️  Cannot check rule order: ChinaDirect or GlobalProxy not found"
fi

echo ""

# ═══════════════════════════════════════════════════════════════
# Task 4: Validate key ruleset configuration
# ═══════════════════════════════════════════════════════════════
log_info "Task 4: Validating key ruleset configuration..."

# Check cnip ruleset (critical for DNS leak prevention)
python3 - "$SINGBOX_CONFIG" <<'PYTHON_SCRIPT4'
import json
import sys

config_file = sys.argv[1]

with open(config_file, 'r', encoding='utf-8') as f:
    config = json.load(f)

# Check cnip ruleset definition
rule_sets = config.get('route', {}).get('rule_set', [])
cnip_defined = any(rs['tag'] == 'cnip' for rs in rule_sets)

# Check cnip references
inbounds = config.get('inbounds', [])
cnip_in_inbound = any(
    'route_exclude_address_set' in ib and ib['route_exclude_address_set'] == 'cnip'
    for ib in inbounds
)

rules = config.get('route', {}).get('rules', [])
cnip_in_rules = any(
    'rule_set' in rule and rule['rule_set'] == 'cnip'
    for rule in rules
)

print("\nKey Ruleset Check:")
print(f"  cnip defined: {'✅ Yes' if cnip_defined else '❌ No'}")
print(f"  cnip inbound ref: {'✅ Yes' if cnip_in_inbound else '❌ No'}")
print(f"  cnip rules ref: {'✅ Yes' if cnip_in_rules else '❌ No'}")

if not cnip_defined:
    print("\n❌ Error: cnip ruleset not defined!")
    print("   This will cause Singbox startup failure")
    print("   Please run: ./merge_sync/sync_all_configs.sh")
    sys.exit(1)

if cnip_defined and (cnip_in_inbound or cnip_in_rules):
    print("\n✅ cnip ruleset configured correctly")
    print("   Purpose: DNS leak prevention + China IP direct connection")

PYTHON_SCRIPT4

echo ""

# Summary
# ═══════════════════════════════════════════════════════════════
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    Check Complete                            ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"

if [ ${#missing_in_singbox[@]} -eq 0 ] && [ ${#extra_in_singbox[@]} -eq 0 ]; then
    log_success "All configurations fully synchronized!"
    exit 0
else
    log_warning "Configuration differences found, please run sync_all_configs.sh to sync"
    exit 1
fi
