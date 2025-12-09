#!/usr/bin/env bash
# =============================================================================
# Singbox Startup Test Script
# Purpose: Test if Singbox configuration can load properly
# Created: 2025-12-07
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

SINGBOX_CONFIG="$PROJECT_ROOT/substore/Singbox_substore_1.13.0+.json"

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║           Singbox Startup Test Tool                          ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

if [ ! -f "$SINGBOX_CONFIG" ]; then
    log_error "Config file not found: $SINGBOX_CONFIG"
    exit 1
fi

log_info "Testing config file: $SINGBOX_CONFIG"
echo ""

# Prefer local preview sing-box
LOCAL_SINGBOX="$PROJECT_ROOT/tools/config-manager-auto-update/bin/sing-box"
if [ -x "$LOCAL_SINGBOX" ]; then
    SINGBOX_CMD="$LOCAL_SINGBOX"
    log_info "Using local preview sing-box"
elif command -v sing-box &> /dev/null; then
    SINGBOX_CMD="sing-box"
    log_info "Using system sing-box"
else
    SINGBOX_CMD=""
fi

# Check if sing-box is available
if [ -z "$SINGBOX_CMD" ]; then
    log_warning "sing-box not installed, skipping startup test"
    log_info "Performing config validation only..."
    
    # Use Python to validate JSON format
    python3 - "$SINGBOX_CONFIG" <<'PYTHON_SCRIPT'
import json
import sys

config_file = sys.argv[1]

try:
    with open(config_file, 'r', encoding='utf-8') as f:
        config = json.load(f)
    
    print("✅ JSON format validation passed")
    
    # Check required fields
    required_fields = ['log', 'dns', 'inbounds', 'outbounds', 'route']
    missing = [f for f in required_fields if f not in config]
    
    if missing:
        print(f"❌ Missing required fields: {', '.join(missing)}")
        sys.exit(1)
    
    print("✅ Config structure complete")
    
    # Check cnip ruleset
    rule_sets = config.get('route', {}).get('rule_set', [])
    cnip_defined = any(rs['tag'] == 'cnip' for rs in rule_sets)
    
    if cnip_defined:
        print("✅ cnip ruleset defined")
    else:
        print("❌ cnip ruleset not defined")
        sys.exit(1)
    
    print("\n✅ Config validation passed!")
    print("   Suggestion: Install sing-box for full testing")
    print("   Install command: brew install sing-box")
    
except json.JSONDecodeError as e:
    print(f"❌ JSON format error: {e}")
    sys.exit(1)
except Exception as e:
    print(f"❌ Validation failed: {e}")
    sys.exit(1)

PYTHON_SCRIPT
    
    exit 0
fi

# Show sing-box version
SINGBOX_VERSION=$("$SINGBOX_CMD" version 2>/dev/null | head -1 || echo "unknown")
log_info "sing-box version: $SINGBOX_VERSION"

# Use sing-box check command to validate config
log_info "Validating config with sing-box..."

# Capture output but don't fail on warnings
SINGBOX_OUTPUT=$("$SINGBOX_CMD" check -c "$SINGBOX_CONFIG" 2>&1 || true)
echo "$SINGBOX_OUTPUT" > /tmp/singbox_check.log

# Check for real errors
if echo "$SINGBOX_OUTPUT" | grep -q "FATAL\|error:"; then
    log_error "❌ Singbox config validation failed"
    echo ""
    log_info "Error details:"
    cat /tmp/singbox_check.log
    exit 1
else
    log_success "✅ Singbox config validation passed!"
fi

echo ""
log_info "Config file can load properly"
log_success "Singbox should start normally!"

rm -f /tmp/singbox_check.log
