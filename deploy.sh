#!/usr/bin/env bash
# 一键编译、推送并清理 CDN 缓存的部署脚本

set -e

echo "🚀 [1/4] 开始执行编译流水线..."
PYTHONPATH=$(pwd)/scripts python3 scripts/pipeline/main_update.py

echo "📦 [2/4] 正在提交更改..."
git add .
git commit -m "chore: auto update modules and rulesets" || echo "No changes to commit."

echo "☁️ [3/4] 正在推送到 GitHub..."
git push origin master

echo "🧹 [4/4] 正在执行 JSDelivr CDN 全球边缘节点缓存清理..."
PYTHONPATH=$(pwd)/scripts python3 scripts/pipeline/purge_cache.py

echo "✅ 部署完成！更新已即时生效，您可以立即在 Surge/Shadowrocket 中拉取最新版本！"
