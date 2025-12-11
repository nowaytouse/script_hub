#!/usr/bin/env bash
# Update Surge policy groups: merge Reddit/Twitter/Instagram into SocialMedia

SURGE_CONFIG=~/Library/Mobile\ Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo\ Pro\ Max\ plus👑_fixed.conf

echo "📝 Updating Surge policy groups and rules..."

# Backup
cp "$SURGE_CONFIG" "$SURGE_CONFIG.backup_$(date +%Y%m%d_%H%M%S)"

# Create temp file
TEMP_FILE=$(mktemp)

# Read file and process
IN_PROXY_GROUP=0
IN_RULE=0
ADDED_SOCIAL_MEDIA=0

while IFS= read -r line; do
    # Detect [Proxy Group] section
    if [[ "$line" == "[Proxy Group]" ]]; then
        IN_PROXY_GROUP=1
        IN_RULE=0
        echo "$line" >> "$TEMP_FILE"
        continue
    fi
    
    # Detect [Rule] section
    if [[ "$line" == "[Rule]" ]]; then
        IN_RULE=1
        IN_PROXY_GROUP=0
        echo "$line" >> "$TEMP_FILE"
        continue
    fi
    
    # Detect other sections
    if [[ "$line" =~ ^\[.*\]$ ]] && [[ "$line" != "[Proxy Group]" ]] && [[ "$line" != "[Rule]" ]]; then
        IN_PROXY_GROUP=0
        IN_RULE=0
    fi
    
    # Process Proxy Group section
    if [[ $IN_PROXY_GROUP -eq 1 ]]; then
        # Skip old policy groups (Reddit, Twitter, Instagram)
        if [[ "$line" =~ ^Reddit\ = ]] || \
           [[ "$line" =~ ^🐦\ Twitter\ 🔵\ = ]] || \
           [[ "$line" =~ ^📷\ instgram\ ⛰️\ = ]]; then
            echo "  ❌ Removing policy: ${line:0:30}..."
            continue
        fi
        
        # Add new SocialMedia policy group after first policy group
        if [[ $ADDED_SOCIAL_MEDIA -eq 0 ]] && [[ "$line" =~ ^[^#] ]] && [[ -n "$line" ]]; then
            # Add SocialMedia policy group (using Twitter's config as template)
            echo "🌐 社交媒体 📱 = select, 🇯🇵日本专线🧱, 🇺🇸美国专线🧱, 🇰🇷韩国专线🧱, \"🇯🇵 JP 🇯🇵\", \"🇺🇸 美国 🇺🇸\", include-all-proxies=0, hidden=0, icon-url=https://raw.githubusercontent.com/Koolson/Qure/master/IconSet/Social.png" >> "$TEMP_FILE"
            ADDED_SOCIAL_MEDIA=1
            echo "  ✅ Added policy: 🌐 社交媒体 📱"
        fi
    fi
    
    # Process Rule section - update policy references
    if [[ $IN_RULE -eq 1 ]]; then
        # Replace Reddit policy reference (handle both end-of-line and mid-line)
        if [[ "$line" =~ ,Reddit$ ]]; then
            line="${line%,Reddit},🌐 社交媒体 📱"
            echo "  🔄 Updated rule: Reddit → 🌐 社交媒体 📱"
        elif [[ "$line" =~ ,Reddit, ]]; then
            line="${line//,Reddit,/,🌐 社交媒体 📱,}"
            echo "  🔄 Updated rule: Reddit → 🌐 社交媒体 📱"
        fi
        
        # Skip Twitter and Instagram RULE-SET lines (now merged into SocialMedia)
        if [[ "$line" =~ Twitter\.list ]] || [[ "$line" =~ Instagram\.list ]]; then
            echo "  ❌ Removing rule: ${line:0:50}..."
            continue
        fi
        
        # Update SocialMedia RULE-SET policy reference
        if [[ "$line" =~ SocialMedia\.list ]]; then
            # Replace old policy with new one
            line=$(echo "$line" | sed 's/,"🌍 海外通用 🌍"/,"🌐 社交媒体 📱"/')
            echo "  🔄 Updated SocialMedia rule policy"
        fi
    fi
    
    echo "$line" >> "$TEMP_FILE"
done < "$SURGE_CONFIG"

# Replace original file
mv "$TEMP_FILE" "$SURGE_CONFIG"

echo ""
echo "✅ Surge configuration updated!"
echo "   Policy Groups:"
echo "     - Removed: Reddit, 🐦 Twitter 🔵, 📷 instgram ⛰️"
echo "     - Added: 🌐 社交媒体 📱"
echo "   Rules:"
echo "     - Updated Reddit rule references"
echo "     - Removed Twitter.list and Instagram.list RULE-SET"
echo "     - Updated SocialMedia.list policy"
