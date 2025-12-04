#!/bin/bash

# Sing-box Manager 一键更新脚本
# 用途：快速更新所有内容（内核 + geofiles + 配置）

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

BINARY="target/release/singbox-manager"

# 检查二进制文件是否存在
if [ ! -f "$BINARY" ]; then
    echo "❌ 未找到二进制文件，请先运行 enable.sh"
    exit 1
fi

echo "🚀 开始一键更新..."
echo "===================="

# 执行更新
"$BINARY" --once all

echo ""
echo "✅ 更新完成！"
echo "===================="
