#!/usr/bin/env bash
# 一键编译、推送并清理 CDN 缓存的部署脚本 (Go/Rust 版本)

set -e

echo "🚀 [1/2] 正在编译 Rust 处理器静态库..."
cd create/processor
cargo build --release
cd ../..

echo "⚙️ [2/2] 开始执行 Go/Rust 编译流水线..."
cd create
go run ./cmd/main_update --execute

echo "✅ 部署完成！更新已即时生效，您可以立即在 Surge/Shadowrocket 中拉取最新版本！"
