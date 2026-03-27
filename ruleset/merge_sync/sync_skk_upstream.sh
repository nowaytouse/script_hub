#!/usr/bin/env bash
# =============================================================================
# Sync SKK & BlackMatrix7 Upstream Rulesets
#
# Downloads external ad-blocking rulesets and stores them locally.
# Applies Bilibili whitelist to reject-no-drop.conf for third-party clients.
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SKK_DIR="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)/skk_upstream"
WHITELIST="$SKK_DIR/bilibili_whitelist.list"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[✓]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[⚠]${NC} $1"; }
log_error() { echo -e "${RED}[✗]${NC} $1"; }

mkdir -p "$SKK_DIR"

log_info "═══════════════════════════════════════════════════════════════"
log_info "Sync SKK & BlackMatrix7 Upstream Rulesets"
log_info "═══════════════════════════════════════════════════════════════"

TIMESTAMP="# Synced: $(date '+%Y-%m-%d %H:%M:%S')"
FAILED=0

# Download helper
download_ruleset() {
    local url="$1"
    local output="$2"
    local name="$3"

    log_info "Downloading: $name"
    if curl -fsSL --max-time 30 "$url" -o "$output.tmp"; then
        # Prepend sync timestamp
        { echo "$TIMESTAMP"; echo "# Source: $url"; echo ""; cat "$output.tmp"; } > "$output"
        rm -f "$output.tmp"
        local count
        count=$(grep -cvE '^#|^$' "$output" 2>/dev/null || echo "0")
        log_success "$name: $count rules"
    else
        rm -f "$output.tmp"
        log_error "Failed to download: $name"
        FAILED=$((FAILED + 1))
    fi
}

# Download all 5 upstream rulesets
download_ruleset \
    "https://ruleset.skk.moe/List/non_ip/reject.conf" \
    "$SKK_DIR/reject.conf" \
    "skk.moe reject"

download_ruleset \
    "https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf" \
    "$SKK_DIR/reject-no-drop.conf" \
    "skk.moe reject-no-drop"

download_ruleset \
    "https://ruleset.skk.moe/List/non_ip/reject-drop.conf" \
    "$SKK_DIR/reject-drop.conf" \
    "skk.moe reject-drop"

download_ruleset \
    "https://ruleset.skk.moe/List/domainset/reject.conf" \
    "$SKK_DIR/domainset-reject.conf" \
    "skk.moe domainset-reject"

download_ruleset \
    "https://raw.githubusercontent.com/blackmatrix7/ios_rule_script/master/rule/Surge/BlockHttpDNS/BlockHttpDNS.list" \
    "$SKK_DIR/BlockHttpDNS.list" \
    "blackmatrix7 BlockHttpDNS"

# Apply Bilibili whitelist to reject-no-drop.conf
if [ -f "$SKK_DIR/reject-no-drop.conf" ] && [ -f "$WHITELIST" ]; then
    log_info "Applying Bilibili whitelist to reject-no-drop.conf..."
    BEFORE=$(grep -cvE '^#|^$' "$SKK_DIR/reject-no-drop.conf" 2>/dev/null || echo "0")

    # Build grep pattern from whitelist
    PATTERN=$(grep -v '^#' "$WHITELIST" | grep -v '^$' | paste -sd'|' -)
    if [ -n "$PATTERN" ]; then
        grep -vE "$PATTERN" "$SKK_DIR/reject-no-drop.conf" > "$SKK_DIR/reject-no-drop.conf.tmp"
        mv "$SKK_DIR/reject-no-drop.conf.tmp" "$SKK_DIR/reject-no-drop.conf"
        AFTER=$(grep -cvE '^#|^$' "$SKK_DIR/reject-no-drop.conf" 2>/dev/null || echo "0")
        REMOVED=$((BEFORE - AFTER))
        log_success "Bilibili whitelist applied: removed $REMOVED rules ($BEFORE → $AFTER)"
    fi
else
    log_warning "Whitelist or reject-no-drop.conf not found, skipping filter"
fi

log_info "═══════════════════════════════════════════════════════════════"
if [ $FAILED -eq 0 ]; then
    log_success "All upstream rulesets synced successfully"
else
    log_error "$FAILED download(s) failed"
    exit 1
fi
