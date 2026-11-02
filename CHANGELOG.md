# Changes

[English](CHANGELOG.md) · [简体中文](CHANGELOG.zh-CN.md)

## 0.7.0 · 2026-11-02

- Expand native operations with host, port, region and IP-family filters, limits, reverse ordering and typed node properties.
- Add explicitly authorized DoH processing with bounded concurrency, session caching, cancellation and shared rate-limit cooldowns; preserve unsupported nodes and TLS identities.
- Group operation controls and show modification/skip counts; back up plans without restoring network consent.

## 0.6.0 · 2026-11-02

- Add eight native Rust node operations with ordered cards, enable/reorder/duplicate controls and per-step preview counts.
- Include the original source and processing plan in relay backups; require a fresh preview after restoration and preserve existing project backups.
- Keep local operations separate from explicit network checks and publication, without a JavaScript runtime or Sub-Store API compatibility layer.

## 0.5.0 · 2026-11-02

- Redesign the in-app file browser with places, history, breadcrumbs, file columns, format filters and keyboard navigation.
- Simplify the node workflow with grouped sources, common actions and expandable advanced options; retain privacy and export safeguards.
- Refine desktop spacing, navigation, empty states and responsive settings, with contrast and layout regression checks.

## 0.4.1 · 2026-11-02

- Replace external file dialogs with an in-app browser for node import, backup, export and checker selection; bound background directory reads and preserve private, no-overwrite exports.
- Keep module artifacts LF-normalized on Windows checkouts so conversion validation stays byte-exact across platforms.

## 0.4.0 · 2026-11-02

- Add a native node-processing panel: preview, regex filters, name editing, conservative deduplication, reset, checks and Gist publication.
- Try source-requested TLS bypass nodes with strict verification first; report safe diagnostic reasons and separate cold HTTPS latency by target.
- Use structured authenticated SOCKS replies, interleaved retries and loopback-only proxy resolution; keep ambiguous failures out of automatic cleanup.
- Match the desktop theme to the purple mark, with light/dark contrast tests and preserved saved preferences.

- Recognize URI/Base64 subscriptions and Shadowrocket/mihomo node YAML/JSON; preserve Sub-Store sing-box export containers, metadata and unsupported nodes through cleanup and backup.
- Use the supplied Script Hub mark for the desktop window, About page and macOS application bundle.
- Isolate the writer-lock regression test from unrelated parallel process launches.
- Read explicit Gist raw links directly; respect rate-limit cooldowns and upload prepared results without repeated node checks.
- Focus relay on one step at a time; discover the connection checker automatically and replace path entry with a native picker.
- Preserve unsupported Surge external definitions without running their programs or discarding the rest of the subscription.
- Guide relay import, cleanup and Gist publication through three steps, with automatic file discovery and Sub-Store Plain JSON recognition.
- Add native node-file and relay-backup import/export without storing GitHub tokens or restoring unsafe permissions.
- Preserve Sub-Store proxy dependencies, unknown nodes and explicit TLS checks during cleanup.
- Reuse exception matching and parse rules once when generating ad-block shards; avoid intermediate rule-string copies.
- Select module-only rules before collecting them, without cloning complete rule categories.
- Keep one IP fallback per equivalent rule in PROMAX and stop loading redundant IP shards; standalone lists remain available. Embedded IP updates follow module refreshes.
- Preserve supported inline pre-matching and no-resolve options across validation and client conversion.
- Protect narrowly identified commerce compatibility and shared-image hosts; quarantine a broad business-response replacement.
- Restore a reviewed Weibo ad host and a bounded Meituan advertising endpoint without widening platform blocks.

## 0.3.2 · 2026-11-01

- Compile accepted IPv4/IPv6 reject and drop networks into Surge's early IP path.
- Preserve IP policy priority, protected networks and no-resolve client fallbacks.
- Skip timestamp-only writes of the generated catalog documentation.

## 0.3.1 · 2026-11-01

- Add four device-confirmed exact IP drops; preserve Shadowrocket DNS-resolution controls.
- Normalize local-source identity across platforms and keep converted module descriptions accurate.
- Reconcile Actions history against fresh inventories; keep ten runs per workflow.
- Add template editing and a documented, manually entered public sample password.
- Keep READMEs minimal; archive earlier master history as a versioned baseline.

## 0.3.0

- Rust subscription relay, optional connectivity checks and independent Gist creation.
- Managed encrypted libraries; reviewed IP-layer drops and narrower advertising matches.
