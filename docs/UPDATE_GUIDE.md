# 维护

仓库根目录执行；替换 sing-box 路径。

```sh
cargo run --release --locked --manifest-path crates/processor/Cargo.toml --target-dir target --bin update -- --root . --execute --singbox /path/to/sing-box
cargo run --locked --manifest-path crates/processor/Cargo.toml --target-dir target --bin validate -- --root .
```

`--quick` 仍联网；更新不自动提交或推送。失败后检查日志与工作区，不删除恢复备份。

[入口](../README.md) · [工作流](../.github/workflows/update_rulesets.yml)
