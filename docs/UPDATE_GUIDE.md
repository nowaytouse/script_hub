# Maintenance

[English](UPDATE_GUIDE.md) · [简体中文](UPDATE_GUIDE.zh-CN.md)

Run from the repository root; replace the sing-box path.

```sh
cargo run --release --locked --bin update -- --root . --execute --singbox /path/to/sing-box
cargo run --locked --bin validate -- --root .
```

`--quick` still accesses the network. Updates do not automatically commit or push. On failure, inspect logs and the worktree; preserve recovery backups.

[Home](../README.md) · [Workflow](../.github/workflows/update_rulesets.yml)
