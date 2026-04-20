#!/bin/bash
# mihomo Service Installation Script
# Version: 2026.04.18

set -e

PLIST_NAME="io.mihomo.core.plist"
SOURCE_PLIST="$(dirname "$0")/$PLIST_NAME"
TARGET_PLIST="/Library/LaunchDaemons/$PLIST_NAME"

# Copy binaries to /usr/local/bin
echo "🚀 Copying binaries to /usr/local/bin/..."
sudo cp "$(dirname "$0")/mihomo" "/usr/local/bin/mihomo-bin"
sudo cp "$(dirname "$0")/sing-box" "/usr/local/bin/sing-box"
sudo cp "$(dirname "$0")/wrapper.sh" "/usr/local/bin/mihomo"

# Set permissions
sudo chown root:wheel /usr/local/bin/mihomo-bin /usr/local/bin/sing-box /usr/local/bin/mihomo
sudo chmod 755 /usr/local/bin/mihomo-bin /usr/local/bin/sing-box /usr/local/bin/mihomo

# Copy plist to System directory
echo "🚀 Installing mihomo service (System Level)..."
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
