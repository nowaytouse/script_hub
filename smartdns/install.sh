#!/bin/bash
# smartdns-rs Installation Script for macOS
# Version: 2026.04.17
# Author: nyamiiko

set -e

INSTALL_DIR="$HOME/.smartdns"
CONFIG_FILE="$INSTALL_DIR/smartdns.conf"

echo "🚀 Installing smartdns-rs (fastest-IP layer beneath mosdns)..."

# Sanity: Homebrew present
if ! command -v brew >/dev/null 2>&1; then
    echo "❌ Homebrew not found. Install Homebrew first: https://brew.sh"
    exit 1
fi

# Install via Homebrew (smartdns-rs formula)
echo "📦 Installing smartdns via Homebrew..."
brew install smartdns

# Resolve binary location (differs between Apple Silicon and Intel)
SMARTDNS_BIN="$(brew --prefix)/sbin/smartdns"
if [ ! -x "$SMARTDNS_BIN" ]; then
    echo "❌ smartdns binary not found at $SMARTDNS_BIN"
    exit 1
fi

# Create install directory
mkdir -p "$INSTALL_DIR"

# Copy config
echo "📝 Copying configuration..."
cp "$(dirname "$0")/smartdns.conf" "$CONFIG_FILE"

# Create launchd plist
echo "🔧 Creating launchd service..."
PLIST="$HOME/Library/LaunchAgents/com.smartdns.plist"
cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.smartdns</string>
    <key>ProgramArguments</key>
    <array>
        <string>$SMARTDNS_BIN</string>
        <string>run</string>
        <string>-c</string>
        <string>$CONFIG_FILE</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/smartdns.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/smartdns.stderr.log</string>
</dict>
</plist>
EOF

# Load service
echo "🚀 Starting smartdns service..."
launchctl unload "$PLIST" 2>/dev/null || true
launchctl load "$PLIST"

# Brief pause so launchd has a chance to bring the binds up before we report.
sleep 1

echo ""
echo "✅ smartdns-rs installed successfully!"
echo ""
echo "📍 Binary:        $SMARTDNS_BIN"
echo "📍 Configuration: $CONFIG_FILE"
echo "📍 LaunchAgent:   $PLIST"
echo ""
echo "🔌 Listen ports:"
echo "   - 127.0.0.1:6353  (CN group — racing Ali/Tencent/DNSPod/360, speed-tested)"
echo "   - 127.0.0.1:6354  (Intl group — racing 6 DoH upstreams via Surge SOCKS5)"
echo ""
echo "🔍 Check status: launchctl list | grep com.smartdns"
echo "📋 Main log:     tail -f /tmp/smartdns.log"
echo "📋 Audit log:    tail -f /tmp/smartdns_audit.log"
echo "🛑 Stop:         launchctl unload $PLIST"
echo "🔄 Restart:      launchctl unload $PLIST && launchctl load $PLIST"
echo ""
echo "⚙️  Next step: (re)load mosdns so its intl/cn sequences chain into this daemon:"
echo "    launchctl unload ~/Library/LaunchAgents/com.mosdns.plist"
echo "    launchctl load   ~/Library/LaunchAgents/com.mosdns.plist"
echo ""
echo "✔  Quick smoke tests:"
echo "    dig @127.0.0.1 -p 6353 baidu.com     # CN group, should return CN IP"
echo "    dig @127.0.0.1 -p 6354 google.com    # Intl group, should return non-CN IP"
