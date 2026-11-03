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

## 桌面许可声明

依赖、桌面功能或目标平台变化后，用 cargo-about 0.9.3 生成清单并审阅差异：

```sh
cargo fetch --locked --manifest-path crates/processor/Cargo.toml
cargo about generate --offline --locked --fail --features gui \
  --manifest-path crates/processor/Cargo.toml \
  --config crates/processor/about.toml \
  --output-file /path/to/scratch/THIRD_PARTY_LICENSES.txt crates/processor/about.hbs
```

保留完整许可与声明。发布前核对捆绑资产和上游许可、随包声明，以及 `SOURCE.txt` 所指的确切源码。

[入口](../README.zh-CN.md) · [工作流](../.github/workflows/update_rulesets.yml)
