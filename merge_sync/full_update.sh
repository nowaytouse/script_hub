#!/usr/bin/env bash
# =============================================================================
# Full Update Script v3.2
# Function: Git Pull + Sync MetaCubeX + Update Sources + Incremental Merge + 
#           AdBlock Merge + Module Sync + Generate SRS + Git Push
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
PRIVATE_DIR="$PROJECT_ROOT/隐私🔏/merge_sync_private"

# Logging functions
log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Show help
show_help() {
    echo "Usage: $0 [options]"
    echo ""
    echo "Options:"
    echo "  --with-core       Update Sing-box & Mihomo cores (recommended for local)"
    echo "  --with-git        Enable Git operations (pull/push)"
    echo "  --skip-git        Skip Git operations"
    echo "  --skip-sync       Skip MetaCubeX sync"
    echo "  --skip-merge      Skip incremental merge"
    echo "  --skip-adblock    Skip AdBlock module merge"
    echo "  --skip-module     Skip module sync to iCloud"
    echo "  --skip-profile    Skip Surge profile sync"
    echo "  --skip-srs        Skip SRS generation"
    echo "  --parallel        Enable parallel processing (faster)"
    echo "  --verbose         Show detailed output"
    echo "  --quiet           Quiet mode (minimal output)"
    echo "  --quick           Quick mode (skip sync, module and Git)"
    echo "  --turbo           Turbo mode (quick + parallel)"
    echo "  --full            Full mode (include Git operations)"
    echo "  --unattended      Unattended mode (CI/CD, with Git, skip iCloud)"
    echo "  --ci              CI mode (same as --unattended)"
    echo "  --cron            Cron mode (same as --unattended)"
    echo "  -y, --yes         Auto-confirm all operations"
    echo "  -h, --help        Show help"
    echo ""
    echo "Examples:"
    echo "  $0                    # Standard update (no Git, no core)"
    echo "  $0 --full             # Full update (with Git pull/push)"
    echo "  $0 --with-core        # Local full update (with core+Surge profile)"
    echo "  $0 --full --with-core # Most complete update (Git+core+profile)"
    echo "  $0 --unattended       # Unattended mode (CI/CD, skip core and profile)"
    echo "  $0 --quick            # Quick update (merge+SRS only)"
    echo "  $0 --cron             # Cron mode"
    echo ""
    exit 0
}

# Parse arguments
WITH_CORE=false
WITH_GIT=true    # Default: enable Git operations
SKIP_GIT=false
SKIP_SYNC=false
SKIP_MERGE=false
SKIP_ADBLOCK=false
SKIP_MODULE=false
SKIP_PROFILE=false
SKIP_SRS=false
VERBOSE=false
QUIET=false
AUTO_YES=false
UNATTENDED=false
PARALLEL=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --with-core) WITH_CORE=true; shift ;;
        --with-git) WITH_GIT=true; shift ;;
        --skip-git) SKIP_GIT=true; shift ;;
        --skip-sync) SKIP_SYNC=true; shift ;;
        --skip-merge) SKIP_MERGE=true; shift ;;
        --skip-adblock) SKIP_ADBLOCK=true; shift ;;
        --skip-module) SKIP_MODULE=true; shift ;;
        --skip-profile) SKIP_PROFILE=true; shift ;;
        --skip-srs) SKIP_SRS=true; shift ;;
        --parallel|-p) PARALLEL=true; shift ;;
        --verbose) VERBOSE=true; shift ;;
        --quiet) QUIET=true; shift ;;
        -y|--yes) AUTO_YES=true; shift ;;
        --turbo) SKIP_SYNC=true; PARALLEL=true; WITH_GIT=true; shift ;;
        --quick) SKIP_SYNC=true; shift ;;
        --full) WITH_GIT=true; shift ;;
        --unattended|--ci|--cron)
            # Unattended mode: enable Git, skip iCloud module sync, skip core update, quiet output, auto-confirm
            UNATTENDED=true
            WITH_GIT=true
            WITH_CORE=false   # CI env no core update
            SKIP_MODULE=true  # CI env no iCloud
            SKIP_PROFILE=true # CI env skip Surge profile sync
            QUIET=true
            AUTO_YES=true
            shift ;;
        -h|--help) show_help ;;
        *) log_error "Unknown option: $1"; exit 1 ;;
    esac
done

# Redefine log functions in quiet mode
if [ "$QUIET" = true ]; then
    log_info() { :; }  # Silent
    log_warning() { echo -e "${YELLOW}[WARN]${NC} $1"; }  # Warnings still show
    log_error() { echo -e "${RED}[ERROR]${NC} $1"; }  # Errors still show
fi

# Show banner (non-quiet mode)
if [ "$QUIET" = false ]; then
    echo -e "${BLUE}+--------------------------------------------------------------+${NC}"
    echo -e "${BLUE}|       Singbox Rules Full Update Tool v3.2                    |${NC}"
    echo -e "${BLUE}|       Surge + MetaCubeX + SingBox + Module Full Sync         |${NC}"
    echo -e "${BLUE}+--------------------------------------------------------------+${NC}"
    echo ""
fi

# Unattended mode notice
if [ "$UNATTENDED" = true ] && [ "$QUIET" = false ]; then
    log_info "Unattended mode enabled (Git: ON, Core: OFF, iCloud: OFF, Profile: OFF, Auto-confirm: ON)"
fi

# Step counter
STEP=0
TOTAL_STEPS=11

# ═══════════════════════════════════════════════════════════════
# Step 1: Git Pull (fetch remote updates)
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$WITH_GIT" = true ] && [ "$SKIP_GIT" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Git Pull (fetch remote updates)...${NC}"
    cd "$PROJECT_ROOT"
    if git rev-parse --git-dir > /dev/null 2>&1; then
        if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
            log_warning "Detected uncommitted local changes"
            [ "$VERBOSE" = true ] && git status --short
            log_info "Stashing local changes..."
            git stash push -m "auto-stash before full_update $(date +%Y%m%d_%H%M%S)" 2>/dev/null || true
        fi
        CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "main")
        if [ "$VERBOSE" = true ]; then
            git pull --rebase origin "$CURRENT_BRANCH" || git pull origin "$CURRENT_BRANCH" || log_warning "Git pull failed, continuing"
        else
            git pull --rebase origin "$CURRENT_BRANCH" 2>&1 | grep -E "^(Already|Updating|Fast-forward|error:|fatal:)" || log_warning "Git pull failed"
        fi
        if git stash list | grep -q "auto-stash before full_update"; then
            log_info "Restoring local changes..."
            git stash pop 2>/dev/null || log_warning "Stash pop failed, please handle manually"
        fi
        log_success "Git Pull complete"
    else
        log_warning "Not a Git repository, skipping"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Skip Git Pull (use --with-git or --full to enable)${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# Step 2: Update Sing-box & Mihomo cores (optional)
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$WITH_CORE" = true ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Update Sing-box & Mihomo cores...${NC}"
    if [ -f "${SCRIPT_DIR}/update_cores.sh" ]; then
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/update_cores.sh"
        else
            "${SCRIPT_DIR}/update_cores.sh" 2>&1 | grep -E "^\[OK\]|\[INFO\]|\[WARN\]|version|download|install|complete" || true
        fi
        log_success "Core update complete"
    else
        log_warning "Skip: update_cores.sh not found"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Skip core update (use --with-core to enable)${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# Step 3: Sync MetaCubeX rules
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_SYNC" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Sync MetaCubeX rules...${NC}"
    if [ -f "${SCRIPT_DIR}/sync_metacubex_rules.sh" ]; then
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/sync_metacubex_rules.sh"
        else
            "${SCRIPT_DIR}/sync_metacubex_rules.sh" 2>&1 | grep -E "^(✅|❌|===|download|update)" || true
        fi
        log_success "MetaCubeX rules sync complete"
    else
        log_warning "Skip: sync_metacubex_rules.sh not found"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Skip MetaCubeX sync${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# Step 4: Update Sources files
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Update Sources files...${NC}"
if [ -f "${SCRIPT_DIR}/update_sources_metacubex.sh" ]; then
    if [ "$VERBOSE" = true ]; then
        "${SCRIPT_DIR}/update_sources_metacubex.sh"
    else
        "${SCRIPT_DIR}/update_sources_metacubex.sh" 2>&1 | grep -E "^(Update|Skip|===)" || true
    fi
    log_success "Sources files update complete"
else
    log_warning "Skip: update_sources_metacubex.sh not found"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# Step 5: Incremental merge rules
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_MERGE" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Incremental merge rules...${NC}"
    if [ -f "${SCRIPT_DIR}/incremental_merge_all.sh" ]; then
        MERGE_ARGS=""
        [ "$PARALLEL" = true ] && MERGE_ARGS="--parallel"
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/incremental_merge_all.sh" $MERGE_ARGS
        else
            "${SCRIPT_DIR}/incremental_merge_all.sh" $MERGE_ARGS 2>&1 | grep -E "^\[OK\]|^Merge:|^===|Before:|After:|Added:|Skip|✓|↻" || true
        fi
        log_success "Rules incremental merge complete"
    else
        log_warning "Skip: incremental_merge_all.sh not found"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Skip incremental merge${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# Step 5.5: Empty ruleset check + Smart dedup
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Empty ruleset check + Smart dedup...${NC}"

if [ -f "${SCRIPT_DIR}/check_empty_rulesets.sh" ]; then
    if [ "$VERBOSE" = true ]; then
        "${SCRIPT_DIR}/check_empty_rulesets.sh"
    else
        "${SCRIPT_DIR}/check_empty_rulesets.sh" 2>&1 | grep -E "^(Total|Empty)" || true
    fi
fi

if [ -f "${SCRIPT_DIR}/smart_cleanup.py" ]; then
    log_info "Running smart dedup (priority: AdBlock > Specific > Fallback)..."
    if [ "$VERBOSE" = true ]; then
        python3 "${SCRIPT_DIR}/smart_cleanup.py"
    else
        python3 "${SCRIPT_DIR}/smart_cleanup.py" 2>&1 | grep -E "^(Removed|Starting|Complete)" || true
    fi
    log_success "Smart dedup complete"
fi

if [ -f "${SCRIPT_DIR}/consolidate_rulesets.py" ]; then
    log_info "Consolidating related rulesets (Tencent, StreamUS, StreamTW)..."
    if [ "$VERBOSE" = true ]; then
        python3 "${SCRIPT_DIR}/consolidate_rulesets.py"
    else
        python3 "${SCRIPT_DIR}/consolidate_rulesets.py" 2>&1 | grep -E "^(Target|Total|Done)" || true
    fi
    log_success "Ruleset consolidation complete"
fi

if [ -f "${SCRIPT_DIR}/update_ruleset_headers.sh" ]; then
    log_info "Updating ruleset headers (adding policy suggestions)..."
    if [ "$VERBOSE" = true ]; then
        "${SCRIPT_DIR}/update_ruleset_headers.sh"
    else
        "${SCRIPT_DIR}/update_ruleset_headers.sh" 2>&1 | grep -E "^(Update|Skip)" || true
    fi
    log_success "Ruleset header update complete"
else
    log_warning "Skip: update_ruleset_headers.sh not found"
fi

if [ -f "${SCRIPT_DIR}/cleanup_empty_rulesets.sh" ]; then
    log_info "Cleaning up empty/deprecated rulesets..."
    if [ "$VERBOSE" = true ]; then
        "${SCRIPT_DIR}/cleanup_empty_rulesets.sh"
    else
        "${SCRIPT_DIR}/cleanup_empty_rulesets.sh" 2>&1 | grep -E "^\[DELETE\]|^\[ERROR\]|Kept:|Deleted:|Errors:" || true
    fi
    log_success "Empty ruleset cleanup complete"
else
    log_warning "Skip: cleanup_empty_rulesets.sh not found"
fi

if [ -f "${SCRIPT_DIR}/sync_ports_to_firewall_module.sh" ]; then
    log_info "Syncing port rules to firewall module..."
    if [ "$VERBOSE" = true ]; then
        "${SCRIPT_DIR}/sync_ports_to_firewall_module.sh" --execute --no-backup
    else
        "${SCRIPT_DIR}/sync_ports_to_firewall_module.sh" --execute --no-backup 2>&1 | grep -E "^\[INFO\]|^\[OK\]|^\[WARN\]|New Rules Added:|Duplicates Skipped:" || true
    fi
    log_success "Port rules sync complete"
else
    log_warning "Skip: sync_ports_to_firewall_module.sh not found"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# Step 6: Download AdBlock modules + Merge
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_ADBLOCK" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Download AdBlock modules + Merge...${NC}"
    
    # Download modules from URLs
    if [ -f "${SCRIPT_DIR}/download_adblock_modules.sh" ]; then
        log_info "Downloading AdBlock modules from URLs..."
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/download_adblock_modules.sh"
        else
            "${SCRIPT_DIR}/download_adblock_modules.sh" 2>&1 | grep -E "^\[✓\]|^\[✗\]|^\[INFO\] (Download Summary|Extracted|Merging|updated)" || true
        fi
    fi
    
    # Merge local modules
    if [ -f "${SCRIPT_DIR}/merge_adblock_modules.sh" ]; then
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/merge_adblock_modules.sh"
        else
            "${SCRIPT_DIR}/merge_adblock_modules.sh" 2>&1 | grep -E "^\[OK\]|^\[INFO\]|Processing:" || true
        fi
        log_success "AdBlock module merge complete"
    else
        log_warning "Skip: merge_adblock_modules.sh not found"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Skip AdBlock module merge${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# Step 7: Module sync to iCloud
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_MODULE" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Module sync to iCloud (Surge + Shadowrocket)...${NC}"
    # Check private script first, then public
    SYNC_MODULE_SCRIPT="${PRIVATE_DIR}/sync_modules_to_icloud.sh"
    [ ! -f "$SYNC_MODULE_SCRIPT" ] && SYNC_MODULE_SCRIPT="${SCRIPT_DIR}/sync_modules_to_icloud.sh"
    if [ -f "$SYNC_MODULE_SCRIPT" ]; then
        if [ "$VERBOSE" = true ]; then
            "$SYNC_MODULE_SCRIPT" --all
        else
            "$SYNC_MODULE_SCRIPT" --all 2>&1 | grep -E "^\[OK\]|^\[INFO\]|^\[WARN\]|Surge:|Shadowrocket:|modules" || true
        fi
        log_success "Module sync complete"
    else
        log_warning "Skip: sync_modules_to_icloud.sh not found"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Skip module sync${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# Step 8: Sync Surge profile (ingest user rules + update rulesets)
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_PROFILE" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Sync Surge profile (comment keyword classification)...${NC}"
    # Check private script first, then public
    SYNC_PROFILE_SCRIPT="${PRIVATE_DIR}/sync_profile_to_template.sh"
    [ ! -f "$SYNC_PROFILE_SCRIPT" ] && SYNC_PROFILE_SCRIPT="${SCRIPT_DIR}/sync_profile_to_template.sh"
    if [ -f "$SYNC_PROFILE_SCRIPT" ]; then
        if [ "$VERBOSE" = true ]; then
            "$SYNC_PROFILE_SCRIPT"
        else
            "$SYNC_PROFILE_SCRIPT" 2>&1 | grep -E "^\[OK\]|\[INFO\]|\[WARN\]|RULE-SET|user rules|sync complete" || true
        fi
        log_success "Surge profile sync complete"
    else
        log_warning "Skip: sync_profile_to_template.sh not found"
    fi
    
    # Sync Shadowrocket config (from Surge rules)
    log_info "Syncing Shadowrocket config (RULE-SET from Surge)..."
    SYNC_SR_SCRIPT="${PRIVATE_DIR}/sync_shadowrocket_config.py"
    if [ -f "$SYNC_SR_SCRIPT" ]; then
        if [ "$VERBOSE" = true ]; then
            python3 "$SYNC_SR_SCRIPT" --verbose
        else
            python3 "$SYNC_SR_SCRIPT" 2>&1 | grep -E "^\[|^=|Rules to|Added:|Updated:|Deleted:|Sync completed|No changes" || true
        fi
        log_success "Shadowrocket config sync complete"
    else
        log_warning "Skip: sync_shadowrocket_config.py not found"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Skip Surge profile sync${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# Step 9: Generate SRS files
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$SKIP_SRS" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Generate SRS files (Singbox binary rules)...${NC}"
    if [ -f "${SCRIPT_DIR}/batch_convert_to_singbox.sh" ]; then
        SRS_ARGS=""
        [ "$PARALLEL" = true ] && SRS_ARGS="--parallel"
        if [ "$VERBOSE" = true ]; then
            "${SCRIPT_DIR}/batch_convert_to_singbox.sh" $SRS_ARGS
        else
            "${SCRIPT_DIR}/batch_convert_to_singbox.sh" $SRS_ARGS 2>&1 | grep -E "^(✓|✗|===|Success:|Failed:|Processing:|Found|Converted:|Skipped:)" || true
        fi
        log_success "SRS file generation complete"
    else
        log_warning "Skip: batch_convert_to_singbox.sh not found"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Skip SRS generation${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# Step 10: Git Commit & Push
# ═══════════════════════════════════════════════════════════════
STEP=$((STEP + 1))
if [ "$WITH_GIT" = true ] && [ "$SKIP_GIT" = false ]; then
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Git Commit & Push...${NC}"
    cd "$PROJECT_ROOT"
    if git rev-parse --git-dir > /dev/null 2>&1; then
        # Check for changes
        if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null || [ -n "$(git ls-files --others --exclude-standard)" ]; then
            # Add all changes
            git add -A
            
            # Generate commit message
            COMMIT_MSG="chore(ruleset): auto-update $(date '+%Y-%m-%d %H:%M')"
            SURGE_COUNT=$(ls "${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)/"*.list 2>/dev/null | wc -l | tr -d ' ')
            SRS_COUNT=$(ls "${PROJECT_ROOT}/ruleset/SingBox/"*.srs 2>/dev/null | wc -l | tr -d ' ')
            COMMIT_MSG="$COMMIT_MSG - Surge:$SURGE_COUNT SRS:$SRS_COUNT"
            
            # Commit
            if [ "$VERBOSE" = true ]; then
                git commit -m "$COMMIT_MSG"
            else
                git commit -m "$COMMIT_MSG" 2>&1 | grep -E "^\[|files? changed|insertions|deletions" || true
            fi
            
            # Get current branch name
            CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "main")
            
            # Push
            if [ "$VERBOSE" = true ]; then
                git push origin "$CURRENT_BRANCH" || git push || log_warning "Git push failed"
            else
                git push origin "$CURRENT_BRANCH" 2>&1 | grep -E "^To|->|Everything up-to-date|error:|fatal:" || git push 2>&1 | grep -E "^To|->|Everything up-to-date|error:|fatal:" || log_warning "Git push failed"
            fi
            
            log_success "Git Commit & Push complete"
        else
            log_info "No changes to commit"
        fi
    else
        log_warning "Not a Git repository, skipping"
    fi
else
    echo -e "${YELLOW}[$STEP/$TOTAL_STEPS] Skip Git Push (use --with-git or --full to enable)${NC}"
fi
echo ""

# ═══════════════════════════════════════════════════════════════
# Statistics
# ═══════════════════════════════════════════════════════════════
echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                    Update Complete Statistics                ║${NC}"
echo -e "${BLUE}╠══════════════════════════════════════════════════════════════╣${NC}"

METACUBEX_COUNT=$(ls "${PROJECT_ROOT}/ruleset/MetaCubeX/"*.list 2>/dev/null | wc -l | tr -d ' ')
SURGE_COUNT=$(ls "${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)/"*.list 2>/dev/null | wc -l | tr -d ' ')
SRS_COUNT=$(ls "${PROJECT_ROOT}/ruleset/SingBox/"*.srs 2>/dev/null | wc -l | tr -d ' ')
SOURCES_COUNT=$(ls "${PROJECT_ROOT}/ruleset/Sources/Links/"*_sources.txt 2>/dev/null | wc -l | tr -d ' ')
MODULE_COUNT=$(ls "${PROJECT_ROOT}/module/surge(main)/"*.sgmodule 2>/dev/null | wc -l | tr -d ' ')

printf "${BLUE}║  ${CYAN}MetaCubeX Rules:${NC} ${GREEN}%-5s${NC}                                    ${BLUE}║${NC}\n" "$METACUBEX_COUNT"
printf "${BLUE}║  ${CYAN}Surge Rules:${NC}    ${GREEN}%-5s${NC}                                    ${BLUE}║${NC}\n" "$SURGE_COUNT"
printf "${BLUE}║  ${CYAN}SingBox SRS:${NC}    ${GREEN}%-5s${NC}                                    ${BLUE}║${NC}\n" "$SRS_COUNT"
printf "${BLUE}║  ${CYAN}Sources Files:${NC}  ${GREEN}%-5s${NC}                                    ${BLUE}║${NC}\n" "$SOURCES_COUNT"
printf "${BLUE}║  ${CYAN}Surge Modules:${NC}  ${GREEN}%-5s${NC}                                    ${BLUE}║${NC}\n" "$MODULE_COUNT"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"

# Show sing-box version
LOCAL_SINGBOX="${SCRIPT_DIR}/config-manager-auto-update/bin/sing-box"
if [ -x "$LOCAL_SINGBOX" ]; then
    echo ""
    echo -e "${GREEN}Local sing-box: $("$LOCAL_SINGBOX" version | head -1)${NC}"
fi

# Show missing SRS files
echo ""
echo -e "${CYAN}=== SRS Coverage Check ===${NC}"
MISSING_SRS=0
for list_file in "${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)/"*.list; do
    [ ! -f "$list_file" ] && continue
    base_name=$(basename "$list_file" .list)
    [[ "$base_name" == *.backup* ]] && continue
    srs_file="${PROJECT_ROOT}/ruleset/SingBox/${base_name}_Singbox.srs"
    if [ ! -f "$srs_file" ]; then
        echo -e "${YELLOW}  Missing: ${base_name}.list -> ${base_name}_Singbox.srs${NC}"
        MISSING_SRS=$((MISSING_SRS + 1))
    fi
done

if [ $MISSING_SRS -eq 0 ]; then
    echo -e "${GREEN}  All Surge rules have corresponding SRS files${NC}"
else
    echo -e "${YELLOW}  Missing $MISSING_SRS SRS files${NC}"
fi

# Show AdBlock rule count
if [ -f "${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)/AdBlock.list" ]; then
    ADBLOCK_COUNT=$(grep -v "^#" "${PROJECT_ROOT}/ruleset/Surge(Shadowkroket)/AdBlock.list" | grep -v "^$" | wc -l | tr -d ' ')
    echo ""
    echo -e "${CYAN}=== AdBlock Rules ===${NC}"
    echo -e "${GREEN}  AdBlock: $ADBLOCK_COUNT rules${NC}"
fi

echo ""
log_success "All done!"
echo ""
echo -e "${CYAN}Tips:${NC}"
echo "  - Use --quick for fast update (skip sync and modules)"
echo "  - Use --turbo for fastest update (quick + parallel)"
echo "  - Use --parallel for parallel processing"
echo "  - Use --verbose for detailed output"
echo "  - Use --help to see all options"
