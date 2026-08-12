# Script Hub 更新指南

项目的自动更新、网络抓取、本地合并和产物校验统一由 Rust 处理。

```bash
# 在仓库根目录执行：全量更新并发布
cargo run --release --manifest-path create/processor/Cargo.toml --bin update -- \
  --root "$PWD" --execute --singbox /usr/local/bin/sing-box

# 低负载本地验证（不提交、不推送）
cargo run --release --manifest-path create/processor/Cargo.toml --bin update -- \
  --root "$PWD" --quick
```

`--execute` 允许生成产物；CI 只有 `master` 会发布。`--quick` 保留 PROMAX 下载、白名单和规则校验，只跳过可选的外部工具步骤。

自动任务由 `.github/workflows/update_rulesets.yml` 调用 Rust 更新器和 Rust 模块校验器，代理环境变量会由运行环境继承；不要并行启动多个更新任务。
