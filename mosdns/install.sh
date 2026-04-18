#!/bin/bash
# mosdns Installation Script for macOS
# Version: 2026.04.17
# Author: nyamiiko

set -e

MOSDNS_VERSION="v5.3.1"
INSTALL_DIR="$HOME/.mosdns"
CONFIG_DIR="$HOME/.mosdns/config"
DATA_DIR="$HOME/.mosdns/data"

echo "🚀 Installing mosdns ${MOSDNS_VERSION}..."

# Prerequisite: smartdns-rs should be installed first — mosdns cn_sequence forwards
# into 127.0.0.1:6353 for CN fastest-IP selection. Warn but don't fail; specialty-steered
# and intl DoH paths still work without smartdns.
if ! launchctl list 2>/dev/null | grep -q com.smartdns; then
    echo "⚠️  smartdns-rs does not appear to be running (com.smartdns launchd job missing)."
    echo "   The CN and intl fallback buckets depend on it."
    echo "   Install it first: cd $(dirname "$(pwd)")/smartdns && ./install.sh"
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
curl -L -o mosdns.zip "$BINARY_URL"
unzip -o mosdns.zip
chmod +x mosdns
rm mosdns.zip

# Download geosite and geoip data
echo "📥 Downloading geosite.dat and geoip.mmdb..."
curl -L -o "$DATA_DIR/geosite.dat" "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/geosite.dat"
curl -L -o "$DATA_DIR/geoip.mmdb" "https://github.com/xream/geoip/releases/latest/download/ipinfo.country.mmdb"

# Download CN domain list
echo "📥 Downloading CN domain list..."
curl -L -o "$DATA_DIR/cn_domain.txt" "https://raw.githubusercontent.com/felixonmars/dnsmasq-china-list/master/accelerated-domains.china.conf" | \
    sed 's/server=\/\(.*\)\/114.114.114.114/\1/' > "$DATA_DIR/cn_domain.txt"

# Download CN IP list (IPv4 + IPv6)
echo "📥 Downloading CN IP lists..."
curl -L -o "$DATA_DIR/cn_ip.txt" "https://raw.githubusercontent.com/17mon/china_ip_list/master/china_ip_list.txt"
curl -L -o "$DATA_DIR/cn_ip_v6.txt" "https://raw.githubusercontent.com/gaoyifan/china-operator-ip/ip-lists/china6.txt"

# Copy config and data files
echo "📝 Copying configuration and local data lists..."
cp "$(dirname "$0")/config.yaml" "$CONFIG_DIR/config.yaml"
cp "$(dirname "$0")"/*.txt "$DATA_DIR/"

# Update config paths
sed -i '' "s|./geosite.dat|$DATA_DIR/geosite.dat|g" "$CONFIG_DIR/config.yaml"
sed -i '' "s|./geoip.mmdb|$DATA_DIR/geoip.mmdb|g" "$CONFIG_DIR/config.yaml"
sed -i '' "s|./cn_domain.txt|$DATA_DIR/cn_domain.txt|g" "$CONFIG_DIR/config.yaml"
sed -i '' "s|./cn_ip.txt|$DATA_DIR/cn_ip.txt|g" "$CONFIG_DIR/config.yaml"
sed -i '' "s|./cn_ip_v6.txt|$DATA_DIR/cn_ip_v6.txt|g" "$CONFIG_DIR/config.yaml"

# Create launchd plist
echo "🔧 Creating launchd service (System Level)..."
sudo cat > "/Library/LaunchDaemons/com.mosdns.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.mosdns</string>
    <key>ProgramArguments</key>
    <array>
        <string>$INSTALL_DIR/mosdns</string>
        <string>start</string>
        <string>-c</string>
        <string>$CONFIG_DIR/config.yaml</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/mosdns.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/mosdns.stderr.log</string>
    <key>SoftResourceLimits</key>
    <dict>
        <key>numberOfFiles</key>
        <integer>65536</integer>
    </dict>
    <key>HardResourceLimits</key>
    <dict>
        <key>numberOfFiles</key>
        <integer>65536</integer>
    </dict>
</dict>
</plist>
EOF

# Load service
echo "🚀 Starting mosdns service..."
sudo launchctl unload "/Library/LaunchDaemons/com.mosdns.plist" 2>/dev/null || true
sudo launchctl load "/Library/LaunchDaemons/com.mosdns.plist"

echo ""
echo "✅ mosdns installed successfully!"
echo ""
echo "📍 Installation directory: $INSTALL_DIR"
echo "📍 Configuration: $CONFIG_DIR/config.yaml"
echo "📍 Data directory: $DATA_DIR"
echo ""
echo "🔍 Check status: sudo launchctl list | grep mosdns"
echo "📋 View logs: tail -f /tmp/mosdns.log"
echo "🛑 Stop service: sudo launchctl unload /Library/LaunchDaemons/com.mosdns.plist"
echo "🔄 Restart service: sudo launchctl unload /Library/LaunchDaemons/com.mosdns.plist && sudo launchctl load /Library/LaunchDaemons/com.mosdns.plist"
echo ""
echo "⚙️  Next step: Update Surge configuration to use 127.0.0.1:53 as DNS server"
echo ""
echo "ℹ️  Architecture reminder: mosdns handles steering; only its CN bucket chains into smartdns-rs:"
echo "   Surge → 127.0.0.1:53 (mosdns) → 127.0.0.1:6353 (smartdns CN group)"
