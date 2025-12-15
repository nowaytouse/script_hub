#!/usr/bin/env bash
# =============================================================================
# Batch Convert Surge Rules to Singbox Binary (.srs) - Incremental v2.2
# Optimization: 
#   - Hash rule content only (exclude comments)
#   - Parallel processing with job control
#   - Header updates don't trigger re-conversion
# =============================================================================

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
RULESET_DIR="${PROJECT_ROOT}/ruleset"
SURGE_DIR="${RULESET_DIR}/Surge(Shadowkroket)"
SINGBOX_DIR="${RULESET_DIR}/SingBox"
LOCAL_SINGBOX="${PROJECT_ROOT}/tools/config-manager-auto-update/bin/sing-box"
CACHE_FILE="${PROJECT_ROOT}/.cache/srs_hashes.txt"

# Arguments
FORCE_ALL=false
PARALLEL=false
MAX_JOBS=4

while [[ $# -gt 0 ]]; do
    case $1 in
        --force) FORCE_ALL=true; shift ;;
        --parallel|-p) PARALLEL=true; shift ;;
        --jobs|-j) MAX_JOBS="$2"; shift 2 ;;
        *) shift ;;
    esac
done

# Select sing-box binary
if [ -x "$LOCAL_SINGBOX" ]; then
    SINGBOX="$LOCAL_SINGBOX"
else
    SINGBOX="sing-box"
fi

# Ensure directories exist
mkdir -p "$SINGBOX_DIR"
mkdir -p "$(dirname "$CACHE_FILE")"
touch "$CACHE_FILE"

# Scan files
RULESETS=()
for list_file in "${SURGE_DIR}"/*.list; do
    [ -f "$list_file" ] || continue
    base_name=$(basename "$list_file" .list)
    [[ "$base_name" == *.backup* ]] && continue
    RULESETS+=("$base_name")
done

echo -e "${CYAN}=== Singbox SRS Incremental Conversion v2.2 ===${NC}"
echo "Found ${#RULESETS[@]} rulesets"
[ "$PARALLEL" = true ] && echo "Parallel mode: $MAX_JOBS jobs"

# Statistics (use temp files for parallel mode)
STATS_DIR=$(mktemp -d)
echo "0" > "$STATS_DIR/converted"
echo "0" > "$STATS_DIR/skipped"
echo "0" > "$STATS_DIR/failed"

convert_ruleset() {
    local ruleset="$1"
    local INPUT_FILE="${SURGE_DIR}/${ruleset}.list"
    local JSON_FILE="${SINGBOX_DIR}/${ruleset}_singbox.json"
    local OUTPUT_FILE="${SINGBOX_DIR}/${ruleset}_Singbox.srs"
    
    # Python conversion
    python3 - "$INPUT_FILE" "$JSON_FILE" << 'PYTHON_SCRIPT'
import json, sys
input_file, output_file = sys.argv[1], sys.argv[2]
rules = []
with open(input_file, "r", encoding="utf-8") as f:
    for line in f:
        line = line.strip()
        if not line or line.startswith("#"): continue
        parts = [p.strip() for p in line.split(",")]
        if len(parts) < 2: continue
        rule_type, pattern = parts[0], parts[1]
        rule = {}
        if rule_type == "DOMAIN": rule["domain"] = [pattern]
        elif rule_type == "DOMAIN-SUFFIX": rule["domain_suffix"] = [pattern]
        elif rule_type == "DOMAIN-KEYWORD": rule["domain_keyword"] = [pattern]
        elif rule_type in ["IP-CIDR", "IP-CIDR6"]: rule["ip_cidr"] = [pattern]
        elif rule_type == "PROCESS-NAME": rule["process_name"] = [pattern]
        else: continue
        if rule: rules.append(rule)
with open(output_file, "w", encoding="utf-8") as f:
    json.dump({"version": 2, "rules": rules}, f, ensure_ascii=False)
PYTHON_SCRIPT
    
    if [ $? -ne 0 ]; then
        return 1
    fi
    
    # Compile to binary
    "$SINGBOX" rule-set compile --output "${OUTPUT_FILE}" "${JSON_FILE}" 2>/dev/null
    rm -f "${JSON_FILE}"
    
    [ -f "${OUTPUT_FILE}" ] && return 0 || return 1
}

process_ruleset() {
    local ruleset="$1"
    local INPUT_FILE="${SURGE_DIR}/${ruleset}.list"
    local OUTPUT_FILE="${SINGBOX_DIR}/${ruleset}_Singbox.srs"
    
    # Hash rule content only (exclude comments and empty lines)
    local current_hash=$(grep -v '^#' "$INPUT_FILE" 2>/dev/null | grep -v '^$' | sort | md5 -q 2>/dev/null || md5sum "$INPUT_FILE" | cut -d' ' -f1)
    local cached_hash=$(grep "^${ruleset}:" "$CACHE_FILE" 2>/dev/null | cut -d':' -f2)
    
    # Check if conversion needed
    local need_convert=false
    if [ "$FORCE_ALL" = true ]; then
        need_convert=true
    elif [ ! -f "$OUTPUT_FILE" ]; then
        need_convert=true
    elif [ "$current_hash" != "$cached_hash" ]; then
        need_convert=true
    fi
    
    if [ "$need_convert" = true ]; then
        if convert_ruleset "$ruleset"; then
            local SIZE=$(du -h "${OUTPUT_FILE}" | cut -f1)
            echo -e "${GREEN}✓${NC} ${ruleset} → ${SIZE}"
            # Update cache (atomic)
            (
                flock -x 200
                grep -v "^${ruleset}:" "$CACHE_FILE" > "$CACHE_FILE.tmp" 2>/dev/null || true
                echo "${ruleset}:${current_hash}" >> "$CACHE_FILE.tmp"
                mv "$CACHE_FILE.tmp" "$CACHE_FILE"
            ) 200>"$CACHE_FILE.lock"
            echo "1" >> "$STATS_DIR/converted"
        else
            echo -e "${RED}✗${NC} ${ruleset} conversion failed"
            echo "1" >> "$STATS_DIR/failed"
        fi
    else
        echo "1" >> "$STATS_DIR/skipped"
    fi
}

# Process rulesets
if [ "$PARALLEL" = true ]; then
    # Parallel processing
    job_count=0
    for ruleset in "${RULESETS[@]}"; do
        process_ruleset "$ruleset" &
        job_count=$((job_count + 1))
        if [ $job_count -ge $MAX_JOBS ]; then
            wait -n 2>/dev/null || wait
            job_count=$((job_count - 1))
        fi
    done
    wait
else
    # Sequential processing
    for ruleset in "${RULESETS[@]}"; do
        process_ruleset "$ruleset"
    done
fi

# Calculate statistics
converted=$(wc -l < "$STATS_DIR/converted" | tr -d ' ')
skipped=$(wc -l < "$STATS_DIR/skipped" | tr -d ' ')
failed=$(wc -l < "$STATS_DIR/failed" | tr -d ' ')
rm -rf "$STATS_DIR"

echo ""
echo -e "${CYAN}=== Complete ===${NC}"
echo -e "  Converted: ${GREEN}${converted}${NC}"
echo -e "  Skipped: ${skipped} (no changes)"
[ $failed -gt 0 ] && echo -e "  Failed: ${RED}${failed}${NC}"

exit 0
