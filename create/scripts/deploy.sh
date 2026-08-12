#!/usr/bin/env bash
# 一键编译、推送并清理 CDN 缓存的部署脚本 (Rust 版本)

set -e

echo "🚀 [1/2] 正在编译 Rust 更新器..."
cd create/processor
cargo build --release
cd ../..

echo "⚙️ [2/2] 开始执行 Rust 更新流水线..."
cd create
cargo run --release --manifest-path processor/Cargo.toml --bin update -- --root "$(pwd)/.." --execute
cd ..
git add modules rulesets dns
if ! git diff --cached --quiet; then
  git commit -m "chore(ruleset): manual Rust update"
  git push origin HEAD:master
fi

echo "✅ 部署完成！更新已即时生效，您可以立即在 Surge/Shadowrocket 中拉取最新版本！"
