#!/usr/bin/env bash
cd "$(dirname "$0")/go_scripts" || exit
echo "🚀 正在运行 Go 语言版一键更新 (全量执行)..."
go run ./cmd/main_update --execute
echo "✅ 更新完成！请按任意键退出..."
read -n 1 -s
