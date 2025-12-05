#!/bin/bash
# ============================================
# Script: Sync Surge Profile to Template
# Version: 1.0
# Updated: 2025-12-03
# Description:
#   - Sync sensitive profile to template
#   - Auto-desensitize (replace sensitive data)
#   - Preserve template structure
#   - Git commit optional
# Usage:
#   ./sync_profile_to_template.sh [options]
#   Options:
#     -s, --source <file>     Source profile (default: 敏感profile 排除上传git)
#     -t, --target <file>     Target template (default: surge_profile_template.conf)
#     -g, --git-commit        Auto git commit after sync
#     -d, --dry-run           Show changes without writing
#     -v, --verbose           Show verbose output
#     -h, --help              Show help
# ============================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_FILE="$SCRIPT_DIR/敏感profile 排除上传git"
TARGET_FILE="$SCRIPT_DIR/surge_profile_template.conf"
DRY_RUN=false
VERBOSE=false
GIT_COMMIT=false

print_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[OK]${NC} $1"; }
print_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }
print_verbose() { [[ "$VERBOSE" == "true" ]] && echo -e "${CYAN}[DEBUG]${NC} $1"; }

show_help() {
    cat << EOF
Sync Surge Profile to Template v1.0

Usage: $(basename "$0") [options]

Options:
  -s, --source <file>     Source profile (default: 敏感profile 排除上传git)
  -t, --target <file>     Target template (default: surge_profile_template.conf)
  -g, --git-commit        Auto git commit after sync
  -d, --dry-run           Show changes without writing
  -v, --verbose           Show verbose output
  -h, --help              Show this help message

Desensitization Rules:
  - Subscription URLs → YOUR_SUBSCRIPTION_URL
  - API Keys → YOUR_API_KEY
  - WireGuard configs → YOUR_* placeholders
  - CA certificates → YOUR_CA_* placeholders
  - Personal domains → Removed or genericized
  - Personal proxy names → Removed

Examples:
  $(basename "$0")                    # Basic sync
  $(basename "$0") -g                 # Sync and commit
  $(basename "$0") -d -v              # Dry run with verbose

EOF
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -s|--source) SOURCE_FILE="$2"; shift 2 ;;
        -t|--target) TARGET_FILE="$2"; shift 2 ;;
        -g|--git-commit) GIT_COMMIT=true; shift ;;
        -d|--dry-run) DRY_RUN=true; shift ;;
        -v|--verbose) VERBOSE=true; shift ;;
        -h|--help) show_help; exit 0 ;;
        *) print_error "Unknown option: $1"; exit 1 ;;
    esac
done

# Validate files
if [[ ! -f "$SOURCE_FILE" ]]; then
    print_error "Source file not found: $SOURCE_FILE"
    exit 1
fi

if [[ ! -f "$TARGET_FILE" ]]; then
    print_error "Target file not found: $TARGET_FILE"
    exit 1
fi

echo ""
echo "╔══════════════════════════════════════════╗"
echo "║   Surge Profile → Template Sync          ║"
echo "╚══════════════════════════════════════════╝"
echo ""

print_info "Source: $(basename "$SOURCE_FILE")"
print_info "Target: $(basename "$TARGET_FILE")"
echo ""

# Create temp file
TEMP_FILE=$(mktemp)
trap "rm -f $TEMP_FILE" EXIT

# Copy source to temp
cp "$SOURCE_FILE" "$TEMP_FILE"

# ============================================
# Desensitization Rules
# ============================================

print_info "Applying desensitization rules..."

# 1. Subscription URLs
print_verbose "  [1] Replacing subscription URLs"
sed -i '' 's|policy-path=https://[^,]*|policy-path=YOUR_SUBSCRIPTION_URL|g' "$TEMP_FILE"

# 2. API Keys
print_verbose "  [2] Replacing API keys"
sed -i '' 's|http-api = [^@]*@|http-api = YOUR_API_KEY@|g' "$TEMP_FILE"

# 3. CA Passphrase and P12
print_verbose "  [3] Replacing CA certificates"
sed -i '' 's|^ca-passphrase = .*|# ca-passphrase = YOUR_CA_PASSPHRASE|g' "$TEMP_FILE"
sed -i '' 's|^ca-p12 = .*|# ca-p12 = YOUR_CA_P12|g' "$TEMP_FILE"

# 4. WireGuard Section Names
print_verbose "  [4] Replacing WireGuard configs"
sed -i '' 's|\[WireGuard [^]]*\]|# [WireGuard YOUR_WG_SECTION]|g' "$TEMP_FILE"
sed -i '' 's|section-name=[^,]*|section-name=YOUR_WG_SECTION|g' "$TEMP_FILE"
sed -i '' 's|private-key = .*|# private-key = YOUR_PRIVATE_KEY|g' "$TEMP_FILE"
sed -i '' 's|self-ip = .*|# self-ip = YOUR_SELF_IP|g' "$TEMP_FILE"
sed -i '' 's|public-key = [^,]*|public-key = YOUR_PUBLIC_KEY|g' "$TEMP_FILE"
sed -i '' 's|endpoint = .*)|endpoint = YOUR_ENDPOINT)|g' "$TEMP_FILE"

# 5. Personal Domains (remove specific personal domains)
print_verbose "  [5] Removing personal domains"
sed -i '' '/DOMAIN,.*\.sgddns,/d' "$TEMP_FILE"
sed -i '' '/DOMAIN,.*nyamiiko/d' "$TEMP_FILE"
sed -i '' '/DOMAIN,pass-api\.proton\.me,/d' "$TEMP_FILE"
sed -i '' '/DOMAIN,list\.linehk\.top,/d' "$TEMP_FILE"

# 6. Personal Proxy Names (remove specific proxy groups)
print_verbose "  [6] Removing personal proxy groups"
sed -i '' '/^🛜 PonTen 🏠 = /d' "$TEMP_FILE"
sed -i '' '/^Reddit = /d' "$TEMP_FILE"

# 7. Personal Rules
print_verbose "  [7] Removing personal rules"
sed -i '' '/^DOMAIN-KEYWORD,reddit,Reddit/d' "$TEMP_FILE"

# 8. Comment out WireGuard proxy definitions
print_verbose "  [8] Commenting WireGuard proxy definitions"
sed -i '' 's|^🔐 WIREGUARD = wireguard|# 🔐 WIREGUARD = wireguard|g' "$TEMP_FILE"

# 9. Update header comment
print_verbose "  [9] Updating header"
sed -i '' 's|# 作者:.*|# 作者: nowaytouse|g' "$TEMP_FILE"
sed -i '' 's|# 说明:.*|# 说明: 去敏版本，需要替换 YOUR_* 占位符|g' "$TEMP_FILE"

# 10. Remove empty lines created by deletions (optional)
print_verbose "  [10] Cleaning up empty lines"
# Keep this minimal to preserve structure

print_success "Desensitization complete"
echo ""

# Show diff
if [[ "$VERBOSE" == "true" ]] || [[ "$DRY_RUN" == "true" ]]; then
    print_info "Changes preview:"
    diff -u "$TARGET_FILE" "$TEMP_FILE" | head -50 || true
    echo ""
fi

# Write to target
if [[ "$DRY_RUN" == "true" ]]; then
    print_warning "DRY RUN - Not writing to target file"
else
    cp "$TEMP_FILE" "$TARGET_FILE"
    print_success "Template updated: $(basename "$TARGET_FILE")"
fi

# Git commit
if [[ "$GIT_COMMIT" == "true" ]] && [[ "$DRY_RUN" == "false" ]]; then
    print_info "Git commit..."
    cd "$SCRIPT_DIR/../.."
    
    if git diff --quiet "$TARGET_FILE"; then
        print_info "No changes to commit"
    else
        git add "$TARGET_FILE"
        git commit -m "chore(surge): sync profile to template - $(date '+%Y-%m-%d %H:%M')"
        print_success "Committed changes"
        
        read -p "Push to remote? [y/N]: " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            git push
            print_success "Pushed to remote"
        fi
    fi
fi

echo ""
print_success "Sync complete!"
echo ""
