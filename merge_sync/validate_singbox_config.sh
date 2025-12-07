#!/usr/bin/env bash
# =============================================================================
# Singbox Configuration Validator
# Purpose: Validate Singbox configuration file integrity and correctness
# Created: 2025-12-07
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

SINGBOX_CONFIG="$PROJECT_ROOT/substore/Singbox_substore_1.13.0+.json"

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           Singbox Configuration Validator                    ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

if [ ! -f "$SINGBOX_CONFIG" ]; then
    log_error "Config file not found: $SINGBOX_CONFIG"
    exit 1
fi

log_info "Validating config file: $SINGBOX_CONFIG"
echo ""

# Use Python to validate JSON format and ruleset references
python3 - "$SINGBOX_CONFIG" <<'PYTHON_SCRIPT'
import json
import sys
from pathlib import Path

config_file = sys.argv[1]

try:
    # Read config
    with open(config_file, 'r', encoding='utf-8') as f:
        config = json.load(f)
    
    print("✅ JSON format validation passed")
    
    # Extract all ruleset definitions
    rule_sets = config.get("route", {}).get("rule_set", [])
    rule_set_tags = {rs["tag"] for rs in rule_sets}
    
    print(f"✅ Found {len(rule_set_tags)} ruleset definitions")
    
    # Extract all ruleset references
    referenced_tags = set()
    
    # Check references in route rules
    rules = config.get("route", {}).get("rules", [])
    for rule in rules:
        if "rule_set" in rule:
            if isinstance(rule["rule_set"], list):
                referenced_tags.update(rule["rule_set"])
            else:
                referenced_tags.add(rule["rule_set"])
    
    # Check references in DNS rules
    dns_rules = config.get("dns", {}).get("rules", [])
    for rule in dns_rules:
        if "rule_set" in rule:
            if isinstance(rule["rule_set"], list):
                referenced_tags.update(rule["rule_set"])
            else:
                referenced_tags.add(rule["rule_set"])
    
    # Check references in inbounds
    inbounds = config.get("inbounds", [])
    for inbound in inbounds:
        if "route_exclude_address_set" in inbound:
            referenced_tags.add(inbound["route_exclude_address_set"])
    
    print(f"✅ Found {len(referenced_tags)} ruleset references")
    
    # Check for missing rulesets
    missing = referenced_tags - rule_set_tags
    if missing:
        print(f"\n❌ Found {len(missing)} missing ruleset definitions:")
        for tag in sorted(missing):
            print(f"   - {tag}")
        sys.exit(1)
    else:
        print("✅ All referenced rulesets are defined")
    
    # Check for unused rulesets
    unused = rule_set_tags - referenced_tags
    if unused:
        print(f"\n⚠️  Found {len(unused)} unused rulesets:")
        for tag in sorted(unused):
            print(f"   - {tag}")
    
    # Statistics
    print("\n" + "="*60)
    print("Configuration Statistics:")
    print(f"  Ruleset definitions: {len(rule_set_tags)}")
    print(f"  Ruleset references: {len(referenced_tags)}")
    print(f"  Route rules: {len(rules)}")
    print(f"  DNS rules: {len(dns_rules)}")
    print(f"  Inbounds: {len(inbounds)}")
    print(f"  Outbounds: {len(config.get('outbounds', []))}")
    print("="*60)
    
    print("\n✅ Configuration validation passed!")
    
except json.JSONDecodeError as e:
    print(f"❌ JSON format error: {e}")
    sys.exit(1)
except Exception as e:
    print(f"❌ Validation failed: {e}")
    import traceback
    traceback.print_exc()
    sys.exit(1)

PYTHON_SCRIPT

echo ""
log_success "Singbox configuration validation complete"
