#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SURGE_FILE="$SCRIPT_DIR/../Surge(Shadowkroket)/Cloudflare.list"
SINGBOX_FILE="$SCRIPT_DIR/../SingBox/Cloudflare_Singbox.srs"

echo "=== Cloudflare Ruleset Sync ==="

# 1. 创建Surge/Shadowrocket规则集
cat > "$SURGE_FILE" << 'EOF'
# Cloudflare Anti-Blocking Rules
# Policy: 🤖反屏蔽🤖

DOMAIN-KEYWORD,cloudflare
DOMAIN-SUFFIX,cloudflare.com
DOMAIN-SUFFIX,cloudflare.net
DOMAIN-SUFFIX,cloudflareinsights.com
DOMAIN-SUFFIX,workers.dev
DOMAIN-SUFFIX,pages.dev
IP-CIDR,173.245.48.0/20,no-resolve
IP-CIDR,103.21.244.0/22,no-resolve
IP-CIDR,103.22.200.0/22,no-resolve
IP-CIDR,103.31.4.0/22,no-resolve
IP-CIDR,141.101.64.0/18,no-resolve
IP-CIDR,108.162.192.0/18,no-resolve
IP-CIDR,190.93.240.0/20,no-resolve
IP-CIDR,188.114.96.0/20,no-resolve
IP-CIDR,197.234.240.0/22,no-resolve
IP-CIDR,198.41.128.0/17,no-resolve
IP-CIDR,162.158.0.0/15,no-resolve
IP-CIDR,104.16.0.0/13,no-resolve
IP-CIDR,104.24.0.0/14,no-resolve
IP-CIDR,172.64.0.0/13,no-resolve
IP-CIDR,131.0.72.0/22,no-resolve
IP-CIDR6,2400:cb00::/32,no-resolve
IP-CIDR6,2606:4700::/32,no-resolve
IP-CIDR6,2803:f800::/32,no-resolve
IP-CIDR6,2405:b500::/32,no-resolve
IP-CIDR6,2405:8100::/32,no-resolve
IP-CIDR6,2a06:98c0::/29,no-resolve
IP-CIDR6,2c0f:f248::/32,no-resolve
EOF

echo "✓ Surge/Shadowrocket: $SURGE_FILE"

# 2. 创建Sing-box规则集
cat > "$SINGBOX_FILE" << 'EOF'
{
  "version": 2,
  "rules": [
    {
      "domain_keyword": ["cloudflare"]
    },
    {
      "domain_suffix": [
        "cloudflare.com",
        "cloudflare.net",
        "cloudflareinsights.com",
        "workers.dev",
        "pages.dev"
      ]
    },
    {
      "ip_cidr": [
        "173.245.48.0/20",
        "103.21.244.0/22",
        "103.22.200.0/22",
        "103.31.4.0/22",
        "141.101.64.0/18",
        "108.162.192.0/18",
        "190.93.240.0/20",
        "188.114.96.0/20",
        "197.234.240.0/22",
        "198.41.128.0/17",
        "162.158.0.0/15",
        "104.16.0.0/13",
        "104.24.0.0/14",
        "172.64.0.0/13",
        "131.0.72.0/22",
        "2400:cb00::/32",
        "2606:4700::/32",
        "2803:f800::/32",
        "2405:b500::/32",
        "2405:8100::/32",
        "2a06:98c0::/29",
        "2c0f:f248::/32"
      ]
    }
  ]
}
EOF

echo "✓ Sing-box: $SINGBOX_FILE"

# 3. 验证
if python3 -m json.tool "$SINGBOX_FILE" > /dev/null 2>&1; then
    echo "✓ JSON validation passed"
else
    echo "✗ JSON validation FAILED!" >&2
    exit 1
fi

echo "=== Sync Complete ==="
