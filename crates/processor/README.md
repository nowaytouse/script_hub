# Desktop & processor

[English](README.md) · [简体中文](README.zh-CN.md)

GUI: Windows / macOS. Linux: CLI only. Build from the repository root:

```sh
cargo app
```

Open `ScriptHub.app` or `script-hub-gui.exe` beside `ScriptHub.resources/`. Previous builds stay under `target/app-backups/`.
`src/gui/` presents; `src/runtime/` runs shared tasks; `../dev/` assembles desktop packages for local use and CI.

Settings and task history stay local; network, write and Git actions are explicit.
[Translations](assets/locales/) are checked at build time.
Relay prepares node-only subscriptions in Rust; optional connectivity tests use your installed sing-box. Preview first, then explicitly create or update a separate Gist. Untested and unknown nodes are retained.
Encrypted libraries use ignored `.cache/vaults/`. Opening Templates refreshes location records and may copy a known local library; source files are kept. Stale records are removed, not encrypted files.
macOS development packages lack trusted signing and notarization; native accessibility acceptance remains incomplete.

[Home](../../README.md) · [Maintenance](../../docs/UPDATE_GUIDE.md) · [Privacy](../../PRIVACY.md)
