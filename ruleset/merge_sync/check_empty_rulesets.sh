#!/usr/bin/env bash

# ═══════════════════════════════════════════════════════════════
# Empty Ruleset Checker
# Check and report empty ruleset status
# ═══════════════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
RULESET_DIR="$PROJECT_ROOT/ruleset/Surge(Shadowkroket)"
SOURCES_DIR="$PROJECT_ROOT/ruleset/Sources/Links"

echo "╔══════════════════════════════════════════╗"
echo "║     Empty Ruleset Checker v1.0           ║"
echo "╚══════════════════════════════════════════╝"
echo ""

# Statistics
total_rulesets=0
empty_rulesets=0
truly_empty=0
has_sources=0

# Check all .list files
for ruleset_file in "$RULESET_DIR"/*.list; do
    filename=$(basename "$ruleset_file")
    ruleset_name="${filename%.list}"
    total_rulesets=$((total_rulesets + 1))
    
    # Count rules (exclude comments and empty lines)
    rule_count=$(grep -v '^#' "$ruleset_file" | grep -v '^$' | grep -v '^\s*$' | wc -l | tr -d ' ')
    
    if [ "$rule_count" -eq 0 ]; then
        empty_rulesets=$((empty_rulesets + 1))
        
        # Check if corresponding sources file exists
        sources_file="$SOURCES_DIR/${ruleset_name}_sources.txt"
        if [ -f "$sources_file" ]; then
            sources_count=$(grep -v '^#' "$sources_file" | grep -v '^$' | wc -l | tr -d ' ')
            if [ "$sources_count" -gt 0 ]; then
                echo "Empty ruleset with $sources_count sources (needs merge): $filename"
                has_sources=$((has_sources + 1))
            else
                echo "Empty ruleset, sources also empty: $filename"
                truly_empty=$((truly_empty + 1))
            fi
        else
            echo "Empty ruleset, no sources file: $filename"
            truly_empty=$((truly_empty + 1))
        fi
    fi
done

echo ""
echo "╔══════════════════════════════════════════╗"
echo "║            Check Results                 ║"
echo "╠══════════════════════════════════════════╣"
printf "║  Total rulesets:  %-21s ║\n" "$total_rulesets"
printf "║  Empty rulesets:  %-21s ║\n" "$empty_rulesets"
printf "║  Has sources:     %-21s ║\n" "$has_sources"
printf "║  Truly empty:     %-21s ║\n" "$truly_empty"
echo "╚══════════════════════════════════════════╝"

if [ "$truly_empty" -gt 0 ]; then
    echo ""
    echo "Tip: Run cleanup_empty_rulesets.sh to clean truly empty rulesets"
fi

if [ "$has_sources" -gt 0 ]; then
    echo ""
    echo "Tip: Run incremental_merge_all.sh to merge sources into rulesets"
fi
