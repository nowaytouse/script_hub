#!/opt/homebrew/bin/bash
# ============================================
# Script: Sync Port Rules to Firewall Module
# Description:
#   Extracts port rules from SurgeConf_DirectPorts.list
#   and syncs them to the Firewall module
# ============================================

set -e

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PORTS_SOURCE="${PROJECT_ROOT}/ruleset/Sources/conf/SurgeConf_DirectPorts.list"
FIREWALL_MODULE="${PROJECT_ROOT}/module/surge(main)/🔥 Firewall Port Blocker 🛡️🚫.sgmodule"
BACKUP_DIR="${SCRIPT_DIR}/backup"
timestamp=$(date "+%Y%m%d_%H%M%S")

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

print_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
print_success() { echo -e "${GREEN}[OK]${NC} $1"; }
print_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
print_error() { echo -e "${RED}[ERROR]${NC} $1"; }

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║     Sync Port Rules to Firewall Module                      ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# 1. Validation
if [ ! -f "$PORTS_SOURCE" ]; then
    print_warn "Port rules source not found: $PORTS_SOURCE"
    print_info "Skipping port rules sync (source file missing)"
    exit 0
fi

if [ ! -f "$FIREWALL_MODULE" ]; then
    print_warn "Firewall module not found: $FIREWALL_MODULE"
    print_info "Skipping port rules sync (module file missing)"
    exit 0
fi

# 2. Extract port rules from source
print_info "Reading port rules from: $(basename "$PORTS_SOURCE")"

port_rules=()
while IFS= read -r line; do
    # Skip empty lines and comments
    [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue
    
    # Check if it's a port rule
    if [[ "$line" =~ ^(IN-PORT|DEST-PORT|SRC-PORT) ]]; then
        port_rules+=("$line")
    fi
done < "$PORTS_SOURCE"

if [ ${#port_rules[@]} -eq 0 ]; then
    print_warn "No port rules found in source file"
    exit 0
fi

print_info "Found ${#port_rules[@]} port rules"

# 3. Preview rules
echo ""
echo "---------- PORT RULES TO SYNC ----------"
for rule in "${port_rules[@]}"; do
    echo "  $rule"
done
echo "----------------------------------------"
echo ""

# 4. Parse arguments
DRY_RUN=true
SKIP_BACKUP=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --execute)
            DRY_RUN=false
            ;;
        --no-backup)
            SKIP_BACKUP=true
            ;;
        *)
            ;;
    esac
    shift
done

# Check CI environment
if [[ "$CI" == "true" ]]; then
    SKIP_BACKUP=true
fi

if [[ "$DRY_RUN" == "true" ]]; then
    print_warn "Dry Run Mode. Use --execute to apply changes."
    exit 0
fi

# 5. Backup
mkdir -p "$BACKUP_DIR"
if [ "$SKIP_BACKUP" = false ]; then
    cp "$FIREWALL_MODULE" "$BACKUP_DIR/$(basename "$FIREWALL_MODULE").$timestamp.bak"
    print_success "Backed up module to $BACKUP_DIR"
    
    # Rotation: Keep last 3 backups
    cd "$BACKUP_DIR" || true
    ls -t *.bak 2>/dev/null | tail -n +4 | xargs -I {} rm "{}" 2>/dev/null || true
    cd - >/dev/null || true
else
    print_info "Skipping backup (--no-backup or CI detected)."
fi

# 6. Check if rules already exist in module
print_info "Checking for duplicate rules in module..."

new_rules=()
duplicate_count=0

for rule in "${port_rules[@]}"; do
    # Extract port number and type
    rule_type=$(echo "$rule" | cut -d, -f1)
    port_num=$(echo "$rule" | cut -d, -f2)
    
    # Check if this port is already in the module
    if grep -q "^${rule_type},${port_num}," "$FIREWALL_MODULE"; then
        print_info "  Skipped (exists): $rule"
        duplicate_count=$((duplicate_count + 1))
    else
        new_rules+=("$rule")
    fi
done

if [ ${#new_rules[@]} -eq 0 ]; then
    print_success "All port rules already exist in module. No changes needed."
    exit 0
fi

print_info "Found ${#new_rules[@]} new rules to add (${duplicate_count} duplicates skipped)"

# 7. Add new rules to module
# Find the [Rule] section and add rules at the end
print_info "Adding new rules to module..."

# Create temp file
temp_module=$(mktemp)

# Read module and add rules before the last comment block
in_rule_section=false
added_rules=false

while IFS= read -r line; do
    # Detect [Rule] section
    if [[ "$line" =~ ^\[Rule\] ]]; then
        in_rule_section=true
        echo "$line" >> "$temp_module"
        continue
    fi
    
    # If we're in rule section and hit the final notes/comments, add new rules
    if [[ "$in_rule_section" == "true" && "$line" =~ ^#.*═.*SAFE\ PORTS && "$added_rules" == "false" ]]; then
        # Add new rules before the "SAFE PORTS" comment section
        echo "" >> "$temp_module"
        echo "# ═══════════════════════════════════════════════════════════════" >> "$temp_module"
        echo "# SECTION 8: Auto-synced from Surge Config ($(date +%Y-%m-%d))" >> "$temp_module"
        echo "# ═══════════════════════════════════════════════════════════════" >> "$temp_module"
        
        for new_rule in "${new_rules[@]}"; do
            # Convert to REJECT-DROP format (firewall rules should block)
            rule_type=$(echo "$new_rule" | cut -d, -f1)
            port_num=$(echo "$new_rule" | cut -d, -f2)
            echo "${rule_type},${port_num},REJECT-DROP" >> "$temp_module"
        done
        
        added_rules=true
        echo "" >> "$temp_module"
    fi
    
    echo "$line" >> "$temp_module"
done < "$FIREWALL_MODULE"

# If we didn't find the SAFE PORTS section, add at the end
if [[ "$added_rules" == "false" ]]; then
    echo "" >> "$temp_module"
    echo "# ═══════════════════════════════════════════════════════════════" >> "$temp_module"
    echo "# SECTION 8: Auto-synced from Surge Config ($(date +%Y-%m-%d))" >> "$temp_module"
    echo "# ═══════════════════════════════════════════════════════════════" >> "$temp_module"
    
    for new_rule in "${new_rules[@]}"; do
        rule_type=$(echo "$new_rule" | cut -d, -f1)
        port_num=$(echo "$new_rule" | cut -d, -f2)
        echo "${rule_type},${port_num},REJECT-DROP" >> "$temp_module"
    done
fi

# Replace original file
mv "$temp_module" "$FIREWALL_MODULE"

print_success "Added ${#new_rules[@]} new port rules to firewall module"

# 8. Summary
echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║                    Sync Complete                             ║"
echo "╠══════════════════════════════════════════════════════════════╣"
printf "║  New Rules Added:     %-35s║\n" "${#new_rules[@]}"
printf "║  Duplicates Skipped:  %-35s║\n" "$duplicate_count"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

print_info "Firewall module updated: $(basename "$FIREWALL_MODULE")"
print_warn "Remember to reload Surge to apply changes!"
