# Subscriptions

[English](RELAY.md) · [简体中文](RELAY.zh-CN.md)

1. Add a **subscription** or **collection** in the library. A subscription accepts an HTTPS link, Gist, pasted nodes, or a local node file.
2. Edit parameters and ordered operations. Native presets append editable steps; they do not execute the Entrance JavaScript or reproduce every feature of it.
3. Preview. The preparation action is Rust-only; optional protocol health checks use the separately detected core.
4. Enter a Gist-capable token and publish. Leave the output target empty to create a secret Gist, or select an existing distinct target.
5. For recurring updates, confirm publication, choose an interval and enable scheduled sync. Repeatable operations apply to each fresh download; edits bound to old node positions cannot be scheduled for remote sources.

The manager runs independently of Surge modules or a local server. Client node formats remain adapters, not runtime dependencies.

For Surge, publish in Surge format, then name and copy the verified group into `[Proxy Group]`. Keep the subscription link private.

Save the encrypted library to keep it between launches. The application uses a fixed private location and never saves its password or Gist token. Switching items protects unsaved drafts; removing a referenced subscription requires removing it from its collections first.

Set each item's Gist target, filename, title, and preferred interval in **Output**, then save the encrypted library. Tokens, authorization, and running schedules are not restored. Remote schedules fetch the source again instead of reusing the library cache. A known source Gist cannot also be the item's destination, including a collection member's source.

Collections combine members in selection order as sing-box node JSON. **Refresh and build** downloads remote members; **Build from cache** stays offline. Scheduled collection runs download remote members and reuse local ones. Failed runs do not partially replace cached nodes.

Import a Sub-Store JSON/Base64 backup and choose **Add available items to library**. Compatible subscriptions and node files are added without overwriting existing entries. Collections with incomplete references are skipped and counted. Settings, tokens and old processors are not restored; import does not download remote links.

Classic age X25519 backups can be unlocked with their private key. Encrypted export wraps a Hub backup for an `age1…` recipient; keep the matching private key separately. Keys are not saved. Passphrase, SSH, plugin and post-quantum age formats are unsupported. Input limit: 8 MiB encrypted, 4 MiB decrypted, 8 KiB decoded header.

The Rust age library is pre-1.0 and marked experimental upstream. Keep an independent recoverable copy of important configurations.

**Script transform (Rhai)** adds a Rust-hosted `fn transform(node) { node }` operation. It is not compiled Rust or JavaScript. Only the documented node view is exposed, without credentials, files, network or commands. Scripts have resource budgets, not process-level isolation. Imported Hub plans may contain scripts; inspect them before previewing or publishing.

Schedules run only while the app is open, one job at a time. **Sync now** starts the enabled schedule early without bypassing cooldowns. Default: 60 minutes; range: 1 minute–7 days. Missed runs are not replayed. Ambiguous writes stop the plan; unchanged content is not uploaded. The newest successful result becomes the copy/export result.

Credentials remain in the session, not settings or history. Quitting or Clear stops the plan; a stopped plan must be re-enabled explicitly. A secret Gist is not encrypted. Cancellation cannot undo an accepted write.

HTTPS imports are limited to 4 MiB, strict TLS and public destinations, without redirects or GitHub credentials. Third-party aliases cannot be proven distinct from an output Gist; rechecks are not an atomic transaction with the source server or GitHub. No input script is executed.

Executable, plugin and file-loading node directives are rejected. Optional probing pins validated public addresses; a node stays unknown when its transport or DNS behavior cannot be preserved safely. Unknown does not mean healthy or safe.

Reference only: [Sub-Store source](https://github.com/sub-store-org/Sub-Store/tree/352f01b7091c99f4e219c44e4ec96f3288059843). No JavaScript runtime or upstream server is embedded; this is partial data compatibility, not a drop-in fork.
