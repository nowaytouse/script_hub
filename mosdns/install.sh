#!/bin/bash
# mosdns Installation Script for macOS
# Version: 2026.04.19
# Author: nyamiiko

set -euo pipefail

MOSDNS_VERSION="v5.3.1"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="$HOME/.mosdns"
CONFIG_DIR="$HOME/.mosdns/config"
DATA_DIR="$HOME/.mosdns/data"
PLIST_TEMPLATE="$SCRIPT_DIR/com.mosdns.daemon.plist"

echo "🚀 Installing mosdns ${MOSDNS_VERSION}..."

# Prerequisite: smartdns-rs should be installed first — mosdns cn_sequence forwards
# into 127.0.0.1:6353 for CN fastest-IP selection. Warn but don't fail; specialty-steered
# and intl DoH paths still work without smartdns.
if ! launchctl list 2>/dev/null | grep -q com.smartdns; then
    echo "⚠️  smartdns-rs does not appear to be running (com.smartdns launchd job missing)."
    echo "   The CN acceleration bucket depends on it."
    echo "   Install it first: cd $SCRIPT_DIR/../smartdns && ./install.sh"
    echo "   Continuing anyway — specialty-steered paths will still resolve."
fi

# Create directories
mkdir -p "$INSTALL_DIR"
mkdir -p "$CONFIG_DIR"
mkdir -p "$DATA_DIR"

# Download mosdns binary
echo "📦 Downloading mosdns binary..."
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    BINARY_URL="https://github.com/IrineSistiana/mosdns/releases/download/${MOSDNS_VERSION}/mosdns-darwin-arm64.zip"
elif [ "$ARCH" = "x86_64" ]; then
    BINARY_URL="https://github.com/IrineSistiana/mosdns/releases/download/${MOSDNS_VERSION}/mosdns-darwin-amd64.zip"
else
    echo "❌ Unsupported architecture: $ARCH"
    exit 1
fi

cd "$INSTALL_DIR"
curl -fsSL -o mosdns.zip "$BINARY_URL"
unzip -o mosdns.zip
chmod +x mosdns
rm mosdns.zip

# Download geosite and geoip data
echo "📥 Downloading geosite.dat and geoip.mmdb..."
curl -fsSL -o "$DATA_DIR/geosite.dat" "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat"
curl -fsSL -o "$DATA_DIR/geoip.mmdb" "https://github.com/xream/geoip/releases/latest/download/ipinfo.country.mmdb"

# Download CN domain list
echo "📥 Downloading CN domain list..."
curl -fsSL "https://raw.githubusercontent.com/felixonmars/dnsmasq-china-list/master/accelerated-domains.china.conf" | \
    sed 's/server=\/\(.*\)\/114.114.114.114/\1/' > "$DATA_DIR/cn_domain.txt"

# Download CN IP list (IPv4 + IPv6)
echo "📥 Downloading CN IP lists..."
curl -fsSL -o "$DATA_DIR/cn_ip.txt" "https://raw.githubusercontent.com/17mon/china_ip_list/master/china_ip_list.txt"
curl -fsSL -o "$DATA_DIR/cn_ip_v6.txt" "https://raw.githubusercontent.com/gaoyifan/china-operator-ip/ip-lists/china6.txt"

# Copy config and data files
echo "📝 Copying configuration and local data lists..."
cp "$SCRIPT_DIR/config.yaml" "$CONFIG_DIR/config.yaml"
cp "$SCRIPT_DIR"/*.txt "$DATA_DIR/"

# Render config paths
sed -i '' "s|__MOSDNS_DATA__|$DATA_DIR|g" "$CONFIG_DIR/config.yaml"

# Create launchd plist
echo "🔧 Creating launchd service (System Level)..."
PLIST="/Library/LaunchDaemons/com.mosdns.plist"
sudo cp "$PLIST_TEMPLATE" "$PLIST"
sudo sed -i '' "s|__MOSDNS_BIN__|$INSTALL_DIR/mosdns|g" "$PLIST"
sudo sed -i '' "s|__MOSDNS_CONFIG__|$CONFIG_DIR/config.yaml|g" "$PLIST"
sudo sed -i '' "s|__MOSDNS_WORKDIR__|$INSTALL_DIR|g" "$PLIST"
sudo chown root:wheel "$PLIST"
sudo chmod 644 "$PLIST"
plutil -lint "$PLIST" >/dev/null

# Load service
echo "🚀 Starting mosdns service..."
sudo launchctl bootout system "$PLIST" 2>/dev/null || true
sudo launchctl bootstrap system "$PLIST"
sudo launchctl kickstart -k system/com.mosdns

echo ""
echo "✅ mosdns installed successfully!"
echo ""
echo "📍 Installation directory: $INSTALL_DIR"
echo "📍 Configuration: $CONFIG_DIR/config.yaml"
echo "📍 Data directory: $DATA_DIR"
echo ""
echo "🔍 Check status: launchctl print system/com.mosdns"
echo "📋 View logs: tail -f /tmp/mosdns.log"
echo "🛑 Stop service: sudo launchctl bootout system /Library/LaunchDaemons/com.mosdns.plist"
echo "🔄 Restart service: sudo launchctl kickstart -k system/com.mosdns"
echo ""
echo "⚙️  Next step: Update Surge configuration to use 127.0.0.1:53 as DNS server"
echo ""
echo "ℹ️  Architecture reminder: mosdns handles steering; only its CN bucket chains into smartdns-rs:"
echo "   Surge → 127.0.0.1:53 (mosdns) → 127.0.0.1:6353 (smartdns CN group)"
