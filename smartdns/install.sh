#!/bin/bash
# smartdns-rs Installation Script for macOS
# Version: 2026.04.19
# Author: nyamiiko

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="$HOME/.smartdns"
CONFIG_FILE="$INSTALL_DIR/smartdns.conf"
PLIST_TEMPLATE="$SCRIPT_DIR/com.smartdns.plist"
LAUNCHD_DOMAIN="gui/$(id -u)"

echo "🚀 Installing smartdns-rs (dual-stack local DNS steering)..."

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
mkdir -p "$HOME/Library/LaunchAgents"

# Copy config
echo "📝 Copying configuration..."
cp "$SCRIPT_DIR/smartdns.conf" "$CONFIG_FILE"

# Create launchd plist
echo "🔧 Creating launchd service..."
PLIST="$HOME/Library/LaunchAgents/com.smartdns.plist"
cp "$PLIST_TEMPLATE" "$PLIST"
sed -i '' "s|__SMARTDNS_BIN__|$SMARTDNS_BIN|g" "$PLIST"
sed -i '' "s|__SMARTDNS_CONFIG__|$CONFIG_FILE|g" "$PLIST"
sed -i '' "s|__SMARTDNS_WORKDIR__|$INSTALL_DIR|g" "$PLIST"
chmod 644 "$PLIST"
plutil -lint "$PLIST" >/dev/null

# Load service
echo "🚀 Starting smartdns service..."
launchctl bootout "$LAUNCHD_DOMAIN" "$PLIST" 2>/dev/null || true
launchctl bootstrap "$LAUNCHD_DOMAIN" "$PLIST"
launchctl kickstart -k "$LAUNCHD_DOMAIN/com.smartdns"

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
echo "   - 127.0.0.1:6053  (UDP/TCP)"
echo "   - [::1]:6053      (UDP/TCP)"
echo ""
echo "🔍 Check status: launchctl print $LAUNCHD_DOMAIN/com.smartdns"
echo "📋 Main log:     tail -f /tmp/smartdns.log"
echo "🛑 Stop:         launchctl bootout $LAUNCHD_DOMAIN $PLIST"
echo "🔄 Restart:      launchctl kickstart -k $LAUNCHD_DOMAIN/com.smartdns"
echo ""
echo "⚙️  Optional PF redirect for system-wide port 53 capture:"
echo "    echo \"rdr pass on lo0 inet proto { udp, tcp } from any to 127.0.0.1 port 53 -> 127.0.0.1 port 6053"
echo "    rdr pass on lo0 inet6 proto { udp, tcp } from any to ::1 port 53 -> ::1 port 6053\" | sudo pfctl -a com.apple/smartdns -f -"
echo "    sudo pfctl -e 2>/dev/null || true"
echo ""
echo "✔  Quick smoke tests:"
echo "    dig @127.0.0.1 -p 6053 baidu.com     # IPv4 loopback"
echo "    dig @::1 -p 6053 baidu.com AAAA      # IPv6 loopback"
