# 桌面与处理器

[English](README.md) · [简体中文](README.zh-CN.md)

GUI：Windows / macOS。Linux：仅命令行。仓库根目录构建：

```sh
cargo app
```

打开根目录 `ScriptHub.app`，或与 `ScriptHub.resources/` 同放的 `script-hub-gui.exe`。旧产物保留在 `target/app-backups/`。
`src/gui/` 负责界面；`src/runtime/` 负责共享任务；`../dev/` 统一本地与 CI 桌面打包。

设置与任务历史留在本地；联网、写入及 Git 操作需主动发起。
[翻译资源](assets/locales/)在构建时校验。
中继由 Rust 整理纯节点订阅；按需开启的测活使用已安装的 sing-box。先预览，再主动新建或更新独立 Gist；未测活与未知节点保留。
加密库使用忽略的 `.cache/vaults/`。打开模板页会刷新位置记录，并可能复制已知的本地库；原文件保留。失效时清除记录，不删除加密文件。
macOS 开发包未受信任签名或公证；原生辅助功能仍有验收缺口。

[入口](../../README.zh-CN.md) · [维护](../../docs/UPDATE_GUIDE.zh-CN.md) · [隐私](../../PRIVACY.zh-CN.md)
