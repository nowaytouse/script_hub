#!/opt/homebrew/bin/bash
# ============================================
# Script: Ingest Rules from Surge Profile
# Description:
#   Extracts "New" rules from Surge profile (above specific marker),
#   classifies them by policy, appends to local rulesets,
#   and removes them from the profile.
#
# 🔥 IMPORTANT: Firewall Port Rules Exclusion
#   - Rules with IN-PORT/DEST-PORT/SRC-PORT are AUTOMATICALLY SKIPPED
#   - These rules should stay in the Firewall module (🔥 Firewall Port Blocker 🛡️🚫.sgmodule)
#   - Firewall rules are system-level security policies, NOT content filtering
#   - They should NEVER be merged into AdBlock or other rulesets
# ============================================

set -e

# Configuration
PROFILE_PATH="/Users/nyamiiko/Library/Mobile Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo Pro Max plus👑_fixed.conf"
MARKER="# ============ 以上为新增 ============"
BACKUP_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/backup"
timestamp=$(date "+%Y%m%d_%H%M%S")

# Local Target Files
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DIR_RULESET="$PROJECT_ROOT/ruleset"
DIR_CONF="$DIR_RULESET/Sources/conf"
DIR_CUSTOM="$DIR_RULESET/Sources/custom"

# Ensure dirs exist
mkdir -p "$DIR_CONF" "$DIR_CUSTOM"

# Categories / Files (Targeting 'conf' separate source files)
# These files serve as INPUTS for the merger, preventing overwrite issues.
# Naming convention: SurgeConf_[Category].list
FILE_ADBLOCK="$DIR_CONF/SurgeConf_AdBlock.list"
FILE_DIRECT="$DIR_CONF/SurgeConf_Manual.list"
FILE_PROXY="$DIR_CONF/SurgeConf_GlobalProxy.list"
FILE_NSFW="$DIR_CONF/SurgeConf_NSFW.list"
FILE_MEDIA="$DIR_CONF/SurgeConf_Media.list"
FILE_PROCESS_DIRECT="$DIR_CONF/SurgeConf_DirectProcess.list"
FILE_FIREWALL_PORTS="$DIR_CONF/SurgeConf_DirectPorts.list" # New Direct Port Ruleset

# Specifics (Optional: map to detailed confs or generic manual)
FILE_NETFLIX="$DIR_CONF/SurgeConf_Netflix.list"
FILE_SPOTIFY="$DIR_CONF/SurgeConf_Spotify.list"
FILE_YOUTUBE="$DIR_CONF/SurgeConf_YouTube.list"
FILE_TELEGRAM="$DIR_CONF/SurgeConf_Telegram.list"
FILE_GOOGLE="$DIR_CONF/SurgeConf_Google.list"
FILE_APPLE="$DIR_CONF/SurgeConf_Apple.list"

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

# Classification Logic
get_target_file() {
    local rule="$1"
    local policy="$2"
    # Convert to lowercase for matching
    local lower_rule=$(echo "$rule" | tr '[:upper:]' '[:lower:]')
    
    # Debug
    # echo "DEBUG: Analyzing '$rule' | Policy: '$policy'" >&2
    
    # 🔥 CRITICAL: Exclude Firewall Port Rules (Should stay in module)
    # These rules should NEVER be ingested into rulesets
    if [[ "$rule" =~ ^(IN-PORT|DEST-PORT|SRC-PORT) ]]; then
        echo "SKIP_FIREWALL_RULE"
        return
    fi
    
    # 1. Check Policy
    if [[ "$policy" == "REJECT" || "$policy" == "REJECT-DROP" ]]; then
        echo "$FILE_ADBLOCK"
        return
    fi

    # 2. Check Comments (e.g., // NSFW or # NSFW)
    # 支持的注释关键词: NSFW, R18, Emby, Media, Netflix, Spotify, YouTube, Telegram, Google, AI, Gaming, Direct, Proxy, AdBlock
    
    # NSFW / 成人内容
    if [[ "$rule" =~ (//|#)[[:space:]]*(NSFW|R18|Adult|Porn|成人) ]]; then
        echo "$FILE_NSFW"
        return
    fi
    
    # 流媒体
    if [[ "$rule" =~ (//|#)[[:space:]]*(Emby|Media|流媒体) ]]; then
        echo "$FILE_MEDIA"
        return
    fi
    if [[ "$rule" =~ (//|#)[[:space:]]*(Netflix|奈飞) ]]; then
        echo "$FILE_NETFLIX"
        return
    fi
    if [[ "$rule" =~ (//|#)[[:space:]]*(Spotify|声田) ]]; then
        echo "$FILE_SPOTIFY"
        return
    fi
    if [[ "$rule" =~ (//|#)[[:space:]]*(YouTube|油管) ]]; then
        echo "$FILE_YOUTUBE"
        return
    fi
    
    # 社交/通讯
    if [[ "$rule" =~ (//|#)[[:space:]]*(Telegram|电报|TG) ]]; then
        echo "$FILE_TELEGRAM"
        return
    fi
    
    # 科技公司
    if [[ "$rule" =~ (//|#)[[:space:]]*(Google|谷歌) ]]; then
        echo "$FILE_GOOGLE"
        return
    fi
    
    # Apple 服务
    if [[ "$rule" =~ (//|#)[[:space:]]*(Apple|苹果|iCloud) ]]; then
        echo "$FILE_APPLE"
        return
    fi
    
    # AI 服务
    if [[ "$rule" =~ (//|#)[[:space:]]*(AI|OpenAI|Claude|ChatGPT) ]]; then
        echo "$DIR_CONF/SurgeConf_AI.list"
        return
    fi
    
    # 游戏
    if [[ "$rule" =~ (//|#)[[:space:]]*(Gaming|游戏|Steam|Epic) ]]; then
        echo "$DIR_CONF/SurgeConf_Gaming.list"
        return
    fi
    
    # 直连
    if [[ "$rule" =~ (//|#)[[:space:]]*(Direct|直连|国内) ]]; then
        echo "$FILE_DIRECT"
        return
    fi
    
    # 代理
    if [[ "$rule" =~ (//|#)[[:space:]]*(Proxy|代理|海外) ]]; then
        echo "$FILE_PROXY"
        return
    fi
    
    # 广告拦截
    if [[ "$rule" =~ (//|#)[[:space:]]*(AdBlock|广告|Block|拦截) ]]; then
        echo "$FILE_ADBLOCK"
        return
    fi

    # 3. Check Keyword Content (Smart Classification)
    # NSFW
    if [[ "$lower_rule" =~ (kemono|xvideos|pornhub|jable|missav|18\+|sex|porn) ]]; then
        echo "$FILE_NSFW"
        return
    fi
    
    # Streaming / Apps
    if [[ "$lower_rule" =~ (netflix|nflx) ]]; then echo "$FILE_NETFLIX"; return; fi
    if [[ "$lower_rule" =~ (spotify) ]]; then echo "$FILE_SPOTIFY"; return; fi
    if [[ "$lower_rule" =~ (youtube|googlevideo|youtu\.be) ]]; then echo "$FILE_YOUTUBE"; return; fi
    if [[ "$lower_rule" =~ (telegram|t\.me) ]]; then echo "$FILE_TELEGRAM"; return; fi
    if [[ "$lower_rule" =~ (google|gstatic|googleapis) ]]; then echo "$FILE_GOOGLE"; return; fi
    # Apple 服务 (域名/进程匹配)
    if [[ "$lower_rule" =~ (apple|icloud|itunes|testflight|music\.app) ]]; then echo "$FILE_APPLE"; return; fi
    
    # 4. Fallback based on Policy
    # 🍎 Apple 策略检测 (基于策略名称中的 Apple 关键词)
    if [[ "$policy" == *"Apple"* || "$policy" == *"apple"* || "$policy" == *"🍎"* ]]; then
        echo "$FILE_APPLE"
        return
    fi
    if [[ "$policy" == "DIRECT" ]]; then
        # Check rule type for specific list
        if [[ "$rule" =~ ^(PROCESS-NAME) ]]; then
            echo "$FILE_PROCESS_DIRECT"
            return
        fi
        # 🔥 Port rules already excluded at top of function
        echo "$FILE_DIRECT"
    elif [[ "$policy" == "Proxy" || "$policy" == *"专线"* || "$policy" == *"节点"* ]]; then
        echo "$FILE_PROXY"
    else
        # Default fallback
        echo "$FILE_DIRECT"
    fi
}

# 1. Validation
if [ ! -f "$PROFILE_PATH" ]; then
    echo -e "${RED}Profile not found: $PROFILE_PATH${NC}"
    exit 1
fi

mkdir -p "$BACKUP_DIR"

# 2. Locate Marker
# We use grep to find the line number of the marker
line_num=$(grep -n "$MARKER" "$PROFILE_PATH" | cut -d: -f1 | head -n 1)

if [ -z "$line_num" ]; then
    echo -e "${YELLOW}Marker not found in profile. Nothing to ingest.${NC}"
    exit 0
fi

print_info "Marker found at line $line_num"

# 3. Identify Range
# We need to define the "Start" of the new rules.
# Heuristic: Scan backwards from line_num until we hit a line that is NOT a rule (e.g. empty, or comment, or section header)
# ACTUALLY, simpler: User pastes rules immediately above.
# We will extract ALL non-empty, non-comment lines *immediately preceding* the marker.
# We stop when we hit a line that starts with `#` or `[` matching a standard header, or is blank?
# But user might have multiple blocks.
# Let's extract everything from the previous Section Header `[Rule]` or `#` line?
# "Safe" approach: Read 100 lines above, look for rule patterns.
# Better: Just extract lines `1` to `line_num-1` and filter for rules? No, that's the whole file.
# The user usually puts these at the BOTTOM of the [Rule] section (which is usually near the end or middle).
# Let's assume the "New" area is bounded by the line above the marker up to the first blank line or comment line.

# Extract lines above marker to a temp file
temp_above_marker=$(mktemp)
head -n "$((line_num - 1))" "$PROFILE_PATH" | tail -n 50 > "$temp_above_marker"
# We only take the last 50 lines above marker to avoid grabbing the whole config if it's huge. 
# Assumption: User processes these frequently.

# Now filter this chunk for valid rules.
# A valid rule usually looks like: TYPE,VALUE,POLICY
# or DOMAIN,xxx,...
# We read slightly backwards.
lines_to_ingest=()
lines_to_delete_count=0

# Read file into array (Bash 3.2 compatible)
buffer=()
while IFS= read -r line; do
    buffer+=("$line")
done < "$temp_above_marker"

# Reverse iterate
found_rules_chunk=false
start_delete_line=0

for (( idx=${#buffer[@]}-1 ; idx>=0 ; idx-- )) ; do
    line="${buffer[idx]}"
    clean_line=$(echo "$line" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
    
    # Empty line? Stop if we were finding rules (block ended). Skip if we haven't found any yet.
    if [ -z "$clean_line" ]; then
        if [ "$found_rules_chunk" = true ]; then
            break # End of block
        else
            continue # Skip trailing empty lines
        fi
    fi

    # Comment? Stop.
    if [[ "$clean_line" =~ ^# ]]; then
        break
    fi
    
    # Section Header? Stop.
    if [[ "$clean_line" =~ ^\[ ]]; then
        break
    fi

    # Valid Rule Check (Basic)
    if [[ "$clean_line" =~ ^(DOMAIN|IP-CIDR|PROCESS-NAME|USER-AGENT|URL-REGEX|DEST-PORT|IN-PORT|RULE-SET) ]]; then
        lines_to_ingest+=("$line")
        found_rules_chunk=true
        lines_to_delete_count=$((lines_to_delete_count + 1))
    else
        # Unknown format? If we found rules, this breaks the block.
        if [ "$found_rules_chunk" = true ]; then
            break
        fi
    fi
done

if [ ${#lines_to_ingest[@]} -eq 0 ]; then
    print_info "No new rules found above marker."
    exit 0
fi

print_info "Found ${#lines_to_ingest[@]} rules to ingest."

# Reverse the array back to normal order
reversed_buffer=()
for (( idx=${#lines_to_ingest[@]}-1 ; idx>=0 ; idx-- )) ; do
    reversed_buffer+=("${lines_to_ingest[idx]}")
done


# 4. Preview and Classify
echo ""
echo "---------- PREVIEW: RULES TO INGEST ----------"
printf "%-60s | %-20s | %-20s\n" "RULE" "POLICY" "TARGET LIST"
echo "----------------------------------------------------------------------------------------------------------------"

skipped_firewall_count=0
for rule in "${reversed_buffer[@]}"; do
    policy=$(echo "$rule" | awk -F, '{print $3}' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
    # If awk fails or format differs (e.g. no commas), defaults empty
    [ -z "$policy" ] && policy="UNKNOWN"
    
    target_file=$(get_target_file "$rule" "$policy")
    
    # 🔥 Skip firewall rules
    if [[ "$target_file" == "SKIP_FIREWALL_RULE" ]]; then
        display_rule=$(echo "$rule" | cut -c1-60)
        printf "%-60s | %-20s | ${RED}%-20s${NC}\n" "$display_rule" "$policy" "⚠️ SKIPPED (Firewall)"
        skipped_firewall_count=$((skipped_firewall_count + 1))
        continue
    fi
    
    target_name=$(basename "$target_file")
    
    # Simplify rule display
    display_rule=$(echo "$rule" | cut -c1-60)
    
    printf "%-60s | %-20s | ${CYAN}%-20s${NC}\n" "$display_rule" "$policy" "$target_name"
done
echo "----------------------------------------------------------------------------------------------------------------"
if [ $skipped_firewall_count -gt 0 ]; then
    print_warn "Skipped $skipped_firewall_count firewall port rules (should stay in module)"
fi
echo ""

# Ask for confirmation if running interactively, or just proceed if trusted?
# User said "Test first... confirm error-free then real ingestion".

EXECUTE=false
SKIP_BACKUP=false

# Parse Args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --execute)
            EXECUTE=true
            ;;
        --no-backup)
            SKIP_BACKUP=true
            ;;
        *)
            # unknown arg
            ;;
    esac
    shift
done

# Check CI environment
if [[ "$CI" == "true" ]]; then
    SKIP_BACKUP=true
fi

if [[ "$EXECUTE" == "false" ]]; then
    print_warn "Dry Run Mode. Use --execute to apply changes."
fi

if [ "$EXECUTE" = true ]; then
    # Backup
    if [ "$SKIP_BACKUP" = false ]; then
        cp "$PROFILE_PATH" "$BACKUP_DIR/$(basename "$PROFILE_PATH").$timestamp.bak"
        print_success "Backed up profile to $BACKUP_DIR"
        
        # Rotation: Keep last 3 backups
        cd "$BACKUP_DIR" || true
        ls -t *.bak 2>/dev/null | tail -n +4 | xargs -I {} rm "{}" 2>/dev/null || true
        cd - >/dev/null || true
    else
        print_info "Skipping backup (--no-backup or CI detected)."
    fi

    # Ingest
    ingested_count=0
    skipped_count=0
    for rule in "${reversed_buffer[@]}"; do
        # Detect Policy
        policy=$(echo "$rule" | awk -F, '{print $3}' | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')
        
        # 🔥 Skip firewall rules
        target_file=$(get_target_file "$rule" "$policy")
        if [[ "$target_file" == "SKIP_FIREWALL_RULE" ]]; then
            print_warn "Skipped firewall rule: $rule"
            skipped_count=$((skipped_count + 1))
            continue
        fi
        
        # Strip Policy from Content (User Request)
        # Format: TYPE,VALUE,POLICY,OPTIONS...
        # We need to remove the POLICY field.
        # But wait, Surge rule format is `TYPE,VALUE,POLICY,OPTIONS`.
        # If we remove POLICY, where do OPTIONS go? Usually OPTIONS are specific to policy?
        # User said "Don't keep policy".
        # Assuming simple rules `TYPE,VALUE,POLICY`.
        # We will strip the POLICY part.
        
        # Careful: "DOMAIN-SUFFIX,google.com,Proxy" -> "DOMAIN-SUFFIX,google.com"
        # "PROCESS-NAME,...DIRECT" -> "PROCESS-NAME,..."
        
        # Strip Policy (Safe Method)
        # We know $policy matches exactly what we extracted.
        # But it might contains slashes (comments).
        # Safe way: Use bash substring removal if it matches?
        
        # Or better: Just split by comma and re-assemble without the 3rd column?
        # But comments make it tricky "...,POLICY // comment".
        
        # Let's rely on the comma structure.
        # TYPE,VALUE,POLICY  -> TYPE,VALUE
        # TYPE,VALUE,POLICY,OPTIONS -> TYPE,VALUE,OPTIONS
        
        # Simpler: Just remove "POLICY" literal string from line?
        # `clean_content=${rule/,$policy/}` (Replace FIRST occurrence of ",$policy")
        # This is safe in bash (no regex involved in simple substitution usually, but depends on version).
        # Actually bash pattern matching uses wildcards.
        
        # Let's use python or perl for safety? No, keep it bash/sed.
        # Use awk to print fields except the policy column?
        # `awk -F, ...`
        
        # DEBUG
        echo "[DEBUG] Rule: $rule" >&2
        echo "[DEBUG] Policy: $policy" >&2
        echo "[DEBUG] Target: $target_file" >&2
        
        # Strip Policy (Safe Method)
        # Escape special chars for sed
        escaped_policy=$(printf '%s\n' "$policy" | sed 's:[][\/.^$*|]:\\&:g')
        # Use # as delimiter to avoid slash issues
        clean_content=$(echo "$rule" | sed "s#,$escaped_policy##")

        echo "$clean_content" >> "$target_file"
        print_info "Appended '$clean_content' to $(basename "$target_file")"
        ingested_count=$((ingested_count + 1))
    done
    
    # Summary
    echo ""
    print_success "Ingestion complete: $ingested_count rules processed"
    if [ $skipped_count -gt 0 ]; then
        print_warn "Skipped $skipped_count firewall port rules (kept in module)"
    fi

    # Remove lines from Profile
    # We extracted lines from the bottom up.
    # The count is $lines_to_delete_count.
    # The end line is $line_num - 1 (or - empty lines).
    # Actually, using `sed` to match the exact lines is risky if duplicates exist.
    # Safer: Delete N lines ending at X.
    
    # Re-calculate correct range.
    # We scanned backwards. `found_rules_chunk` logic stopped at specific line.
    # The lines to delete are strictly those we identified.
    # But filtering logic (skipping empty lines) makes line numbers tricky.
    
    # Alternative: Read exact content into memory, rewrite file.
    # Or, simpler: Just delete the identified rules by content? No, duplicates.
    
    # Let's count EXACT physical lines we scanned (including empty/comments we skipped? No we stopped at comment).
    # We only skipped TRAILING empty lines.
    # So we delete from (LineNum - TotalScanned) to (LineNum - 1).
    
    # Let's refine the counting logic.
    # We need to know the start line number in the file.
    # grep -n again?
    
    # We can match the EXTRACTED BLOCK in the file.
    # But sticking to "sed delete range" is best.
    # Total lines removed = The number of lines in `reversed_buffer` + any interleaved empty lines we accepted?
    # My logic: `if [ -z "$clean_line" ]... continue`. I skipped them.
    # If I skipped them, I shouldn't delete them? Or should I?
    # Usually clean to delete the empty space too.
    
    # Revised Logic:
    # Delete from `StartLine` to `EndLine`.
    # EndLine = line_num - 1.
    # StartLine = EndLine - (lines_processed) + 1.
    
    # Let's assume we delete the whole continuous block of non-header text above marker.
    # Re-scanning to count lines including gaps:
    count=0
    # Re-read to count
    while IFS= read -r line; do
        buffer+=("$line")
    done < "$temp_above_marker"
    
    # Iterate array reversed (re-using buffer or just count from file logic?)
    # Simpler: Just count backwards from file end using the array we just loaded?
    # buffer is appended? Clear it first.
    buffer=()
    while IFS= read -r line; do
        buffer+=("$line")
    done < "$temp_above_marker"
    for (( idx=${#buffer[@]}-1 ; idx>=0 ; idx-- )) ; do
        line="${buffer[idx]}"
        if [[ "$line" =~ ^# ]]; then break; fi
        if [[ "$line" =~ ^\[ ]]; then break; fi
        count=$((count+1))
    done
    
    # So we remove `count` lines ending at `line_num - 1`.
    start_del=$((line_num - count))
    end_del=$((line_num - 1))
    
    if [ $start_del -le $end_del ] && [ $count -gt 0 ]; then
        # Use a temp file for sed to avoid issues
        sed "${start_del},${end_del}d" "$PROFILE_PATH" > "${PROFILE_PATH}.tmp" && mv "${PROFILE_PATH}.tmp" "$PROFILE_PATH"
        print_success "Removed ingested lines from profile."
    else
        print_warn "Line calculation check failed. Start: $start_del, End: $end_del. Manual check recommended."
    fi
    
    # Run Sync
    echo ""
    print_info "Triggering Rule Synchronization (Surge -> Sing-box)..."
    # Ensure sync scripts exist
    # DIR_RULESET is .../ruleset.
    # We want .../sync/sync_all_rulesets.sh
    SYNC_SCRIPT="$DIR_RULESET/../sync/sync_all_rulesets.sh"
    if [ -f "$SYNC_SCRIPT" ]; then
        # Run it
        if bash "$SYNC_SCRIPT"; then
            print_success "Synchronization Complete."
        else
            print_warn "Synchronization finished with errors."
        fi
    else
        # Try finding it
        print_warn "Sync script not found at expected path. Skipping auto-sync."
    fi
fi
