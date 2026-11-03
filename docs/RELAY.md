# Relay

[English](RELAY.md) · [简体中文](RELAY.zh-CN.md)

1. Choose **Link**, **Paste text** or **Local file**. HTTPS subscriptions and Gist sources are supported; only recognized node data is accepted.
2. Edit parameters and ordered operations. Native presets append editable steps; they do not execute the Entrance JavaScript or reproduce every feature of it.
3. Preview. The preparation action is Rust-only; optional protocol health checks use the separately detected core.
4. Enter a Gist-capable token and publish. Leave the output target empty to create a secret Gist, or select an existing distinct target.
5. For recurring updates, confirm publication, choose an interval and enable scheduled sync. Repeatable operations apply to each fresh download; edits bound to old node positions cannot be scheduled for remote sources.

Schedules run only while the app is open, one job at a time. Default: 60 minutes; range: 1 minute–7 days. Missed runs are not replayed. HTTP cooldowns are honored; ambiguous writes stop the plan. Unchanged content is not uploaded. The newest successful result becomes the copy/export result.

Credentials remain in the session, not settings or history. Quitting or Clear stops the plan; a stopped plan must be re-enabled explicitly. A secret Gist is not encrypted. Cancellation cannot undo an accepted write.

HTTPS imports are limited to 4 MiB, strict TLS and public destinations, without redirects or GitHub credentials. Third-party aliases cannot be proven distinct from an output Gist; rechecks are not an atomic transaction with the source server or GitHub. No input script is executed.

Executable, plugin and file-loading node directives are rejected. Optional probing pins validated public addresses; a node stays unknown when its transport or DNS behavior cannot be preserved safely. Unknown does not mean healthy or safe.

Reference only: [Sub-Store development source](https://github.com/sub-store-org/Sub-Store/tree/2aa3cbd9d24c775e14f379015a536559d4c76019). No JavaScript runtime or upstream server is embedded.
