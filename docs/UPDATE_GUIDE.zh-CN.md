# 维护

[English](UPDATE_GUIDE.md) · [简体中文](UPDATE_GUIDE.zh-CN.md)

仓库根目录执行；替换 sing-box 路径。

```sh
cargo run --release --locked --bin update -- --root . --execute --singbox /path/to/sing-box
cargo run --locked --bin validate -- --root .
```

`--quick` 仍联网；更新不自动提交或推送。失败后检查日志与工作区，不删除恢复备份。

`--exports-only --execute` 从当前列表重建客户端格式，不拉取上游；搭配 `--singbox` 同时生成 SRS。
`--classify-only --execute` 还会在导出前重新执行本地分类收束。

[入口](../README.zh-CN.md) · [工作流](../.github/workflows/update_rulesets.yml)
