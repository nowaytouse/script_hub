#!/bin/bash
# Test: Verify firewall rules are excluded from ingestion

echo "🧪 Testing Firewall Rule Exclusion..."
echo ""

# Source the get_target_file function
source "$(dirname "$0")/ingest_from_surge.sh" 2>/dev/null || true

# Test cases
test_cases=(
    "DEST-PORT,445,REJECT-DROP // SMB"
    "DEST-PORT,3389,REJECT-DROP // RDP"
    "IN-PORT,8080,DIRECT // Local proxy"
    "SRC-PORT,53,DIRECT // DNS"
    "DOMAIN-SUFFIX,google.com,Proxy // Normal rule"
    "IP-CIDR,192.168.0.0/16,DIRECT // Normal rule"
)

echo "Testing rule classification:"
echo "================================================"

for rule in "${test_cases[@]}"; do
    policy=$(echo "$rule" | awk -F, '{print $3}' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//' | awk '{print $1}')
    result=$(get_target_file "$rule" "$policy")
    
    if [[ "$result" == "SKIP_FIREWALL_RULE" ]]; then
        echo "✅ SKIPPED (Correct): $rule"
    else
        if [[ "$rule" =~ ^(IN-PORT|DEST-PORT|SRC-PORT) ]]; then
            echo "❌ NOT SKIPPED (ERROR): $rule -> $(basename "$result")"
        else
            echo "✅ PROCESSED (Correct): $rule -> $(basename "$result")"
        fi
    fi
done

echo "================================================"
echo ""
echo "✅ Test complete!"
