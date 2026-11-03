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

依赖、桌面功能或目标平台有变化时，在仓库根目录用 cargo-about 0.9.3 刷新许可清单。先输出到临时位置，审阅差异后再替换受 Git 跟踪的文件：

```sh
cargo fetch --locked --manifest-path crates/processor/Cargo.toml
cargo about generate --offline --locked --fail --features gui \
  --manifest-path crates/processor/Cargo.toml \
  --config crates/processor/about.toml \
  --output-file /path/to/scratch/THIRD_PARTY_LICENSES.txt crates/processor/about.hbs
```

依赖审计会在相关推送、拉取请求和手动运行中重新生成并比对清单；定时漏洞审计不会重新生成。生成后仅统一行末空格、制表符和 CRLF，保留全部声明及许可正文。离线检查使用已下载的锁定依赖，不能替代对额外上游许可材料的审阅。字体等捆绑资产和上游材料仍须分别核对。发布归档前，确认对应源码可获取、`SOURCE.txt` 标明构建提交，且包内附带完整的许可正文和声明。未提交改动的本地构建不能以基准提交链接代替对应源码。

[入口](../README.zh-CN.md) · [工作流](../.github/workflows/update_rulesets.yml)
