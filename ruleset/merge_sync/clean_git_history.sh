#!/opt/homebrew/bin/bash
# =============================================================================
# Clean Git History - Remove Sensitive Files
# Function: Remove sensitive files from Git history using git-filter-repo
# WARNING: This will rewrite Git history!
# =============================================================================

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

echo -e "${RED}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${RED}║          ⚠️  Git History Cleanup Tool  ⚠️                    ║${NC}"
echo -e "${RED}║                                                              ║${NC}"
echo -e "${RED}║  WARNING: This will REWRITE Git history!                    ║${NC}"
echo -e "${RED}║  All collaborators must re-clone the repository!            ║${NC}"
echo -e "${RED}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check if git-filter-repo is installed
if ! command -v git-filter-repo &> /dev/null; then
    log_error "git-filter-repo is not installed"
    echo ""
    echo "Install with:"
    echo "  brew install git-filter-repo    # macOS"
    echo "  pip3 install git-filter-repo    # Python"
    exit 1
fi

# List of sensitive files to remove from history
SENSITIVE_FILES=(
    "merge_sync/ingest_from_surge.sh"
    "merge_sync/sync_modules_to_icloud.sh"
    "merge_sync/sync_profile_to_template.sh"
    "merge_sync/sync_singbox_policy_groups.py"
    "merge_sync/sync_to_all_proxies.sh"
    "merge_sync/check_surge_config.py"
    "merge_sync/verify_all_configs.sh"
)

# Show files to be removed
echo -e "${CYAN}Files to be removed from Git history:${NC}"
for file in "${SENSITIVE_FILES[@]}"; do
    echo "  - $file"
done
echo ""

# Confirm
read -p "Continue? This will rewrite Git history! (yes/no): " confirm
if [ "$confirm" != "yes" ]; then
    log_info "Aborted"
    exit 0
fi

# Backup current branch
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
log_info "Current branch: $CURRENT_BRANCH"

# Create backup
BACKUP_BRANCH="backup-before-cleanup-$(date +%Y%m%d_%H%M%S)"
git branch "$BACKUP_BRANCH"
log_success "Created backup branch: $BACKUP_BRANCH"

# Remove files from history
log_info "Removing files from Git history..."
for file in "${SENSITIVE_FILES[@]}"; do
    if git log --all --pretty=format: --name-only | grep -q "^$file$"; then
        log_info "Removing: $file"
        git filter-repo --path "$file" --invert-paths --force
    else
        log_info "Not in history: $file"
    fi
done

log_success "Git history cleaned"

echo ""
echo -e "${YELLOW}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${YELLOW}║                    Next Steps                                ║${NC}"
echo -e "${YELLOW}╠══════════════════════════════════════════════════════════════╣${NC}"
echo -e "${YELLOW}║  1. Review changes: git log --oneline                        ║${NC}"
echo -e "${YELLOW}║  2. Force push: git push origin --force --all                ║${NC}"
echo -e "${YELLOW}║  3. Force push tags: git push origin --force --tags          ║${NC}"
echo -e "${YELLOW}║  4. Notify collaborators to re-clone                         ║${NC}"
echo -e "${YELLOW}║                                                              ║${NC}"
echo -e "${YELLOW}║  Backup branch: $BACKUP_BRANCH${NC}"
printf "${YELLOW}║  To restore: git checkout $BACKUP_BRANCH                     ║${NC}\n"
echo -e "${YELLOW}╚══════════════════════════════════════════════════════════════╝${NC}"
