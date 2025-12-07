#!/bin/bash
# ============================================
# Script: Convert Merged Ruleset to Surge Module Format
# Version: 1.0
# Description: Convert AdBlock.list to module-embeddable format
# ============================================

set -e

INPUT_FILE="AdBlock.list"
OUTPUT_FILE="AdBlock_Module_Rules.txt"

echo "Converting $INPUT_FILE to module format..."

# Extract rules by policy and add policy suffix
{
    echo "# ═══════════════════════════════════════════════════════════════"
    echo "# Universal Ad-Blocking Rules (Merged - 235k+ rules)"
    echo "# Generated: $(date '+%Y-%m-%d %H:%M:%S')"
    echo "# ═══════════════════════════════════════════════════════════════"
    echo ""
    
    # Process REJECT rules
    awk '/^# ========== DOMAIN-SUFFIX \(REJECT\)/,/^# ========== DOMAIN-KEYWORD \(REJECT\)/ {
        if ($0 ~ /^DOMAIN-SUFFIX,/) print $0 ",REJECT,no-resolve"
    }' "$INPUT_FILE" | head -1000
    
    awk '/^# ========== DOMAIN-KEYWORD \(REJECT\)/,/^# ========== DOMAIN \(REJECT\)/ {
        if ($0 ~ /^DOMAIN-KEYWORD,/) print $0 ",REJECT"
    }' "$INPUT_FILE" | head -500
    
    awk '/^# ========== DOMAIN \(REJECT\)/,/^# ========== IP-CIDR \(REJECT\)/ {
        if ($0 ~ /^DOMAIN,/ && $0 !~ /^DOMAIN-SUFFIX/ && $0 !~ /^DOMAIN-KEYWORD/) print $0 ",REJECT,no-resolve"
    }' "$INPUT_FILE" | head -500
    
    awk '/^# ========== IP-CIDR \(REJECT\)/,/^# ========== IP-CIDR6 \(REJECT\)/ {
        if ($0 ~ /^IP-CIDR,/) print $0 ",REJECT,no-resolve"
    }' "$INPUT_FILE" | head -200
    
    echo ""
    echo "# ═══════════════════════════════════════════════════════════════"
    echo "# REJECT-DROP Rules (DNS/Tracking)"
    echo "# ═══════════════════════════════════════════════════════════════"
    
    # Process REJECT-DROP rules
    awk '/^# ========== DOMAIN-SUFFIX \(REJECT-DROP\)/,/^# ========== DOMAIN-KEYWORD \(REJECT-DROP\)/ {
        if ($0 ~ /^DOMAIN-SUFFIX,/) print $0 ",REJECT-DROP,no-resolve"
    }' "$INPUT_FILE"
    
    awk '/^# ========== DOMAIN \(REJECT-DROP\)/,/^# ========== IP-CIDR \(REJECT-DROP\)/ {
        if ($0 ~ /^DOMAIN,/ && $0 !~ /^DOMAIN-SUFFIX/ && $0 !~ /^DOMAIN-KEYWORD/) print $0 ",REJECT-DROP,no-resolve"
    }' "$INPUT_FILE"
    
    awk '/^# ========== IP-CIDR \(REJECT-DROP\)/,/^# ========== IP-CIDR6 \(REJECT-DROP\)/ {
        if ($0 ~ /^IP-CIDR,/) print $0 ",REJECT-DROP,no-resolve"
    }' "$INPUT_FILE"
    
    echo ""
    echo "# ═══════════════════════════════════════════════════════════════"
    echo "# REJECT-NO-DROP Rules (Compatibility)"
    echo "# ═══════════════════════════════════════════════════════════════"
    
    # Process REJECT-NO-DROP rules
    awk '/^# ========== DOMAIN-SUFFIX \(REJECT-NO-DROP\)/,/^# ========== DOMAIN-KEYWORD \(REJECT-NO-DROP\)/ {
        if ($0 ~ /^DOMAIN-SUFFIX,/) print $0 ",REJECT-NO-DROP,no-resolve"
    }' "$INPUT_FILE"
    
    awk '/^# ========== DOMAIN \(REJECT-NO-DROP\)/,/^# ========== IP-CIDR \(REJECT-NO-DROP\)/ {
        if ($0 ~ /^DOMAIN,/ && $0 !~ /^DOMAIN-SUFFIX/ && $0 !~ /^DOMAIN-KEYWORD/) print $0 ",REJECT-NO-DROP,no-resolve"
    }' "$INPUT_FILE"
    
} > "$OUTPUT_FILE"

echo "✅ Converted to: $OUTPUT_FILE"
echo "📊 Statistics:"
wc -l "$OUTPUT_FILE"
