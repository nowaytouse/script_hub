#!/usr/bin/env bash
cd "$(dirname "$0")" || exit
echo "🚀 正在运行 Python 语言版一键更新 (全量执行)..."
python3 scripts/pipeline/main_update.py --execute
echo "✅ 更新完成！请按任意键退出..."
read -n 1 -s
