#!/bin/bash
# mihomo Service Installation Script
# Version: 2026.04.18

set -e

PLIST_NAME="io.mihomo.core.plist"
SOURCE_PLIST="$(dirname "$0")/$PLIST_NAME"
TARGET_PLIST="/Library/LaunchDaemons/$PLIST_NAME"

echo "🚀 Installing mihomo service (System Level)..."

if [ ! -f "$SOURCE_PLIST" ]; then
    echo "❌ Source plist not found: $SOURCE_PLIST"
    exit 1
fi

# Copy plist to System directory
sudo cp "$SOURCE_PLIST" "$TARGET_PLIST"
sudo chown root:wheel "$TARGET_PLIST"
sudo chmod 644 "$TARGET_PLIST"

# Load service
echo "🚀 Loading mihomo service..."
sudo launchctl unload "$TARGET_PLIST" 2>/dev/null || true
sudo launchctl load "$TARGET_PLIST"

echo ""
echo "✅ mihomo service installed and started successfully!"
echo "📋 Logs: tail -f /tmp/mihomo.log"
echo "🔍 Status: sudo launchctl list | grep mihomo"
