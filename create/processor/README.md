# Rust processor and desktop app

`rust_processor` owns the shared rule/catalog processing and native desktop app. The GUI is supported on Windows and macOS only; Linux remains supported for command-line processing, but the GUI dependency and CI matrix are desktop-only.

The native app brings the former desktop workflows together: workspace overview; searchable module and rule catalogs with subscription-link copying; custom-source management for Wool, Weibo, and Devtools; validation and update tasks; repository status and explicitly requested Git actions; preferences; and help/about information. This describes the currently wired page and job groups, not external distribution acceptance.

## Preferences and task feedback

The settings UI exposes theme, accent color, System / Simplified Chinese / English / Japanese, 80–150% UI scale, compact / standard / wide window size, and comfortable / compact density. System language uses the native locale when available and standard locale environment variables as a fallback. Settings are stored at:

- macOS: `~/Library/Application Support/Script Hub/settings.json`
- Windows: `%APPDATA%\ScriptHub\settings.json`

A saved workspace is a convenience for the GUI. An explicit CLI `--root` always takes precedence. Tasks report their actual named stage, elapsed time, outcome, warnings, and bounded logs. No estimated completion percentage is shown when the worker has no reliable progress measurement.

Default startup does not discover or read a workspace. The saved path is shown without accessing it; open it explicitly, drop the folder, pass `--root`, or opt in to reopening it. One read-only background loader keeps the first frame and close action responsive while filesystem access waits. Concurrent loads are not queued.

macOS protects Desktop, Documents, Downloads, network and removable volumes. Ad-hoc signing binds the designated requirement to a specific build, so replacing a development build can invalidate prior consent. The app cannot bypass that consent. For unattended runs, explicitly select a local workspace outside protected folders and use a stable app location; cross-version consent continuity requires a consistent distribution signing identity. No Full Disk Access, custom permissive signing requirement, or TCC database edits are used.

## Sources and repository actions

Custom source entries are stored in the workspace at `modules/source/custom_sources.json`; Wool, Weibo, and Devtools are supported source categories. Source add/remove actions are explicit and confirmed. Run a full update to rebuild the affected bundle after changing sources; quick update skips functional bundles.

Repository actions are always user-initiated. Pull requires a clean working tree and is fast-forward-only; commits include only explicitly selected changed paths; push targets the same-named branch on `origin` and is non-forced. Do not add automatic Git operations.

The unmaintained compiled React/Tauri distribution and the HTML module helper have been retired in favor of the native app. The Library reads current local modules directly, supports batch URL copying, and creates Surge RULE-SET snippets only after the user supplies a policy. No helper-only JSON snapshots are generated. Loon output and its conversion/validation code are retired; supported module outputs are Surge and Shadowrocket. Historical files remain recoverable from Git history.

## Source organization

```text
src/
├── app/             # update_cli, validate_cli
├── desktop/         # app, app_history, history, catalog, i18n, job, progress,
│                    # repository, settings, theme, worker,
│                    # pages/{overview,catalog,tasks,history,repository,
│                    # settings,sources,support}
├── pipeline/        # custom_sources, firewall_sync, functional_update,
│                    # general_update, merge_bundles, module_catalog, srs_generator
├── transforms/      # mitm_manager, smart_cleanup, url_rewriter
├── promax/          # parsing, safety, sources, compilation and publication;
│   └── compiler/    # compiler regression tests, separate from production code
├── bin/
└── lib.rs           # shared ruleset utilities and compatibility re-exports
```

Crate-root module paths remain available through re-exports; `functional_update` now lives under `pipeline` while callers can continue using its historical root path. Keep public CLI entry points and generated output paths stable during the refactor.

`pipeline/module_catalog` only inspects published module files; `desktop/catalog` adds rule/source models and safe copy formatting. Neither maintains a second generated web catalog. Reviewed local privacy endpoints live in `rulesets/Sources/Privacy_Reviewed.list` and enter the same PROMAX source manifest, validation and publication pipeline as upstream inputs. An exact reviewed advertising host never authorizes blocking its parent domain or bypassing an explicit host whitelist or critical Apple service protection.

## Local commands

Core processor checks and CLI work do not require the GUI feature. Build or test the GUI only on a supported desktop target:

```sh
cargo test --manifest-path create/processor/Cargo.toml
cargo run --manifest-path create/processor/Cargo.toml --features gui --bin script-hub-gui
```

The GUI CI workflow runs on macOS and Windows and names artifacts with the runner OS and actual runner architecture. Release packages include the repository license and compliance notices, plus a source link pinned to the exact build commit. A local build or CI artifact does not imply trusted signing, notarization, or runtime acceptance on every supported device.
