#!/usr/bin/env bash
# Cloudflare规则集诊断脚本

echo "=== Cloudflare SRS 诊断工具 ==="
echo ""

# 1. 检查本地文件
echo "1. 本地文件检查:"
LOCAL_FILE="ruleset/SingBox/Cloudflare_Singbox.srs"
if [ -f "$LOCAL_FILE" ]; then
    echo "  ✓ 文件存在: $(ls -lh $LOCAL_FILE | awk '{print $5}')"
    echo "  文件类型: $(file $LOCAL_FILE)"
    echo "  MD5: $(md5 -q $LOCAL_FILE)"
else
    echo "  ✗ 文件不存在"
fi

# 2. 检查GitHub文件
echo ""
echo "2. GitHub文件检查:"
GITHUB_URL="https://raw.githubusercontent.com/nowaytouse/script_hub/master/ruleset/SingBox/Cloudflare_Singbox.srs"
curl -sL "$GITHUB_URL" > /tmp/cf_github.srs
echo "  下载大小: $(wc -c < /tmp/cf_github.srs) bytes"
echo "  MD5: $(md5 -q /tmp/cf_github.srs)"

# 3. 对比
echo ""
echo "3. 文件对比:"
if cmp -s "$LOCAL_FILE" /tmp/cf_github.srs; then
    echo "  ✓ 本地文件与GitHub一致"
else
    echo "  ✗ 文件不一致！"
fi

# 4. sing-box版本
echo ""
echo "4. sing-box版本:"
sing-box version 2>&1 | head -3

# 5. 验证文件
echo ""
echo "5. 文件验证:"
cat > /tmp/test_cf.json << 'JSON'
{
  "log": {"level": "error"},
  "route": {
    "rule_set": [{
      "tag": "test",
      "type": "local",
      "format": "binary",
      "path": "/tmp/cf_github.srs"
    }],
    "rules": [{"rule_set": "test", "outbound": "direct"}]
  },
  "outbounds": [{"type": "direct", "tag": "direct"}]
}
JSON

if sing-box check -c /tmp/test_cf.json 2>&1 | grep -q "error"; then
    echo "  ✗ 文件格式验证失败"
    sing-box check -c /tmp/test_cf.json 2>&1
else
    echo "  ✓ 文件格式正确"
fi

# 清理
rm -f /tmp/cf_github.srs /tmp/test_cf.json

echo ""
echo "=== 诊断完成 ==="
echo ""
echo "建议:"
echo "1. 如果文件一致且验证通过，问题可能在客户端缓存"
echo "2. 尝试清除客户端缓存或强制重新下载规则集"
echo "3. 检查客户端sing-box版本是否过旧"
