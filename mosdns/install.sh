#!/bin/bash
# mosdns Installation Script for macOS
# Version: 2026.04.16
# Author: nyamiiko

set -e

MOSDNS_VERSION="v5.3.1"
INSTALL_DIR="$HOME/.mosdns"
CONFIG_DIR="$HOME/.mosdns/config"
DATA_DIR="$HOME/.mosdns/data"

echo "🚀 Installing mosdns ${MOSDNS_VERSION}..."

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

# Download CN IP list
echo "📥 Downloading CN IP list..."
curl -L -o "$DATA_DIR/cn_ip.txt" "https://raw.githubusercontent.com/17mon/china_ip_list/master/china_ip_list.txt"

# Copy config
echo "📝 Copying configuration..."
cp "$(dirname "$0")/config.yaml" "$CONFIG_DIR/config.yaml"

# Update config paths
sed -i '' "s|./geosite.dat|$DATA_DIR/geosite.dat|g" "$CONFIG_DIR/config.yaml"
sed -i '' "s|./geoip.mmdb|$DATA_DIR/geoip.mmdb|g" "$CONFIG_DIR/config.yaml"
sed -i '' "s|./cn_domain.txt|$DATA_DIR/cn_domain.txt|g" "$CONFIG_DIR/config.yaml"
sed -i '' "s|./cn_ip.txt|$DATA_DIR/cn_ip.txt|g" "$CONFIG_DIR/config.yaml"

# Create launchd plist
echo "🔧 Creating launchd service..."
cat > "$HOME/Library/LaunchAgents/com.mosdns.plist" <<EOF
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
</dict>
</plist>
EOF

# Load service
echo "🚀 Starting mosdns service..."
launchctl unload "$HOME/Library/LaunchAgents/com.mosdns.plist" 2>/dev/null || true
launchctl load "$HOME/Library/LaunchAgents/com.mosdns.plist"

echo ""
echo "✅ mosdns installed successfully!"
echo ""
echo "📍 Installation directory: $INSTALL_DIR"
echo "📍 Configuration: $CONFIG_DIR/config.yaml"
echo "📍 Data directory: $DATA_DIR"
echo ""
echo "🔍 Check status: launchctl list | grep mosdns"
echo "📋 View logs: tail -f /tmp/mosdns.log"
echo "🛑 Stop service: launchctl unload ~/Library/LaunchAgents/com.mosdns.plist"
echo "🔄 Restart service: launchctl unload ~/Library/LaunchAgents/com.mosdns.plist && launchctl load ~/Library/LaunchAgents/com.mosdns.plist"
echo ""
echo "⚙️  Next step: Update Surge configuration to use 127.0.0.1:5335 as DNS server"
