#!/usr/bin/env bash
# Fix Reddit policy references in Surge config
# This script ensures all Reddit references are updated to 🌐 社交媒体 📱

SURGE_CONFIG=~/Library/Mobile\ Documents/iCloud~com~nssurge~inc/Documents/NyaMiiKo\ Pro\ Max\ plus👑_fixed.conf

echo "🔍 Checking for Reddit policy references..."

# Check if file exists
if [ ! -f "$SURGE_CONFIG" ]; then
    echo "❌ Surge config not found: $SURGE_CONFIG"
    exit 1
fi

# Count Reddit references
reddit_count=$(grep -c ",Reddit$\|,Reddit," "$SURGE_CONFIG" 2>/dev/null || echo "0")

if [ "$reddit_count" -eq 0 ]; then
    echo "✅ No Reddit policy references found"
    exit 0
fi

echo "⚠️  Found $reddit_count Reddit policy reference(s)"
echo ""
echo "📝 Fixing references..."

# Backup
backup_file="$SURGE_CONFIG.backup_reddit_fix_$(date +%Y%m%d_%H%M%S)"
cp "$SURGE_CONFIG" "$backup_file"
echo "   Backup created: $(basename "$backup_file")"

# Fix references
sed -i '' 's/,Reddit$/,🌐 社交媒体 📱/g' "$SURGE_CONFIG"
sed -i '' 's/,Reddit,/,🌐 社交媒体 📱,/g' "$SURGE_CONFIG"

# Verify fix
new_count=$(grep -c ",Reddit$\|,Reddit," "$SURGE_CONFIG" 2>/dev/null || echo "0")

if [ "$new_count" -eq 0 ]; then
    echo "✅ All Reddit references fixed!"
    echo "   Updated: $reddit_count reference(s)"
else
    echo "⚠️  Warning: $new_count reference(s) still remain"
    echo "   Please check manually"
fi

echo ""
echo "🔍 Showing updated lines:"
grep -n "🌐 社交媒体 📱" "$SURGE_CONFIG" | grep -i "reddit" || echo "   (none found)"

