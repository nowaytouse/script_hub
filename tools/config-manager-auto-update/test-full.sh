#!/bin/bash
set -e
echo "=== Mihomo 完整测试 ==="
echo ""
echo "阶段 1: 清理"
rm -f "/Users/nyamiiko/Library/Application Support/v2rayN/bin/mihomo/mihomo"
rm -f "/Users/nyamiiko/Library/Application Support/v2rayN/bin/mihomo/mihomo.bak"
sudo rm -f /usr/local/bin/mihomo /usr/local/bin/mihomo.bak
echo "✅ 清理完成"
echo ""
echo "阶段 2: 下载旧版本 v1.18.10"
curl -sL "https://github.com/MetaCubeX/mihomo/releases/download/v1.18.10/mihomo-darwin-arm64-v1.18.10.gz" | gunzip > /tmp/mihomo-old
chmod +x /tmp/mihomo-old
echo "✅ 下载完成"
echo ""
echo "阶段 3: 安装旧版本"
cp /tmp/mihomo-old "/Users/nyamiiko/Library/Application Support/v2rayN/bin/mihomo/mihomo"
sudo cp /tmp/mihomo-old /usr/local/bin/mihomo
echo "✅ 安装完成"
echo ""
echo "阶段 4: 运行更新"
sudo ./target/release/singbox-manager --once
echo ""
echo "阶段 5: 验证"
echo "v2rayN:" && "/Users/nyamiiko/Library/Application Support/v2rayN/bin/mihomo/mihomo" -v | head -1
echo "系统:" && /usr/local/bin/mihomo -v | head -1
echo "备份:" && "/Users/nyamiiko/Library/Application Support/v2rayN/bin/mihomo/mihomo.bak" -v | head -1
echo ""
echo "✅ 测试完成"
