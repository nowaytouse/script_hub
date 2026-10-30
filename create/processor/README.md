# 桌面与处理器

GUI：Windows / macOS。Linux：仅命令行。仓库根目录启动：

```sh
cargo run --release --locked --manifest-path create/processor/Cargo.toml --target-dir target --features gui --bin script-hub-gui -- --root .
```

设置与任务历史留在本地；联网、写入及 Git 操作需主动发起。
macOS 开发包未受信任签名或公证；原生辅助功能仍有验收缺口。

[入口](../../README.md) · [维护](../../docs/UPDATE_GUIDE.md) · [隐私](../../PRIVACY.md)
