#!/bin/bash

# ═══════════════════════════════════════════════════════════════
# Empty Ruleset Cleanup Script
# Purpose: Remove empty or comment-only rulesets that have no value
# ═══════════════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RULESET_DIR="$SCRIPT_DIR/../ruleset/Surge(Shadowkroket)"
SOURCES_DIR="$SCRIPT_DIR/../ruleset/Sources/Links"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo "╔══════════════════════════════════════════╗"
echo "║     Empty Ruleset Cleanup v1.0           ║"
echo "╚══════════════════════════════════════════╝"
echo ""

# Counters
deleted=0
kept=0
errors=0

# Check each .list file
for ruleset_file in "$RULESET_DIR"/*.list; do
    filename=$(basename "$ruleset_file")
    ruleset_name="${filename%.list}"
    
    # Count actual rules (exclude comments and empty lines)
    rule_count=$(grep -v '^#' "$ruleset_file" | grep -v '^$' | grep -v '^\s*$' | wc -l | tr -d ' ')
    
    if [ "$rule_count" -eq 0 ]; then
        # Check if this is a known deprecated ruleset
        case "$ruleset_name" in
            BlockHttpDNS|ChinaDirect|ChinaIP|FirewallPorts)
                echo -e "${YELLOW}[DELETE]${NC} $filename (0 rules - deprecated/replaced)"
                
                # Delete ruleset file
                rm -f "$ruleset_file"
                
                # Delete corresponding sources file if exists
                sources_file="$SOURCES_DIR/${ruleset_name}_sources.txt"
                if [ -f "$sources_file" ]; then
                    rm -f "$sources_file"
                    echo "         Also deleted: ${ruleset_name}_sources.txt"
                fi
                
                deleted=$((deleted + 1))
                ;;
            *)
                # Unknown empty ruleset - this is an error!
                echo -e "${RED}[ERROR]${NC} $filename is empty but not in deprecated list!"
                echo "        This needs investigation and fixing!"
                errors=$((errors + 1))
                ;;
        esac
    else
        kept=$((kept + 1))
    fi
done

echo ""
echo "╔══════════════════════════════════════════╗"
echo "║            Cleanup Summary               ║"
echo "╠══════════════════════════════════════════╣"
printf "║  Kept:     %-28s ║\n" "$kept rulesets"
printf "║  Deleted:  %-28s ║\n" "$deleted rulesets"
printf "║  Errors:   %-28s ║\n" "$errors rulesets"
echo "╚══════════════════════════════════════════╝"

if [ "$errors" -gt 0 ]; then
    echo ""
    echo -e "${RED}⚠️  WARNING: Found $errors unexpected empty rulesets!${NC}"
    echo "   Please investigate and fix these rulesets."
    exit 1
fi

exit 0
