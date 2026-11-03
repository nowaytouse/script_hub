# Changes

[English](CHANGELOG.md) · [简体中文](CHANGELOG.zh-CN.md)

## 0.14.1 · 2026-11-03

- Separate local export from Gist format checks; reject broken node dependencies before publishing.
- Show scheduled sync progress and retry reasons. Preserve server waits and keep offline previews available.
- Guide empty collections to the first subscription. Keep drafts recoverable and offer an offline demo.
- Export to Surge, Mihomo, or sing-box. Conversion failures never upload a partial result.
- Protect unsaved settings and keep diagnostics local. Verify bundled licenses and source references.
- Keep X25519 archive compatibility and remove an affected, unused crypto dependency.
- Refresh remote subscriptions and collections before scheduled publication; Gist authorization stays session-only.

## 0.14.0 · 2026-11-03

- Replace the single-session wizard with a searchable subscription and collection workspace, adaptive list/detail navigation, and quick operation shortcuts.
- Save the library locally with password encryption; guard unsaved edits, deletion, locking, and closing. Keep Gist tokens session-only.
- Build ordered collections from cached nodes or refresh their remote members as one transaction. Protect node-index edits when input changes.
- Migrate compatible Sub-Store subscriptions, node files, and complete collections in one pass without restoring settings, tokens, or legacy processors.

## 0.13.0 · 2026-11-03

- Present subscriptions as an independent manager; no Surge module or local server is required.
- Preview Sub-Store JSON/Base64 backups and selectively import recognized local nodes or public HTTPS sources, without restoring tokens, settings or old processors.
- Add classic age X25519 backup unlocking and encrypted Hub backup export, with bounded authenticated reads and private-key clearing.
- Add Rust-hosted Rhai node transformations to the ordered editor, with a restricted node view and execution budgets; no JavaScript or arbitrary native-code execution.
- Reject duplicate backup keys and preserve public HTTPS source metadata through Hub backup round-trips.

## 0.12.0 · 2026-11-03

- Import node text from public HTTPS subscriptions, paste or local files, with bounded parsing and pinned public-address resolution.
- Add editable native enhancement presets and session-only scheduled download/process/Gist synchronization; honor server cooldowns and stop on ambiguous writes.
- Keep scheduled results, previews and copying in sync; preserve source binding, publication consent and cancellation boundaries.
- Separate link, text and file entry points, retain masked private inputs and keep all interface text in translation catalogs.
- Reject executable node directives, pin public probe endpoints, and distinguish addresses that cannot be verified safely.
- Accept equivalent CRLF provider files without hiding content changes; preserve Windows worker diagnostics in CI.

## 0.11.0 · 2026-11-03

- Add native, policy-neutral Mihomo providers with explicit subset counts, deterministic publication and source/output consistency checks.
- Expand advertising, domestic, international and AI sources; retain specialized category ownership and reject ICANN public-suffix catch-alls.
- Reclassify known international speed-test endpoints without losing fallback coverage, and preserve local `home.arpa` names.
- Protect Apple's .apple brand zone, including Safe Browsing proxy/token endpoints, from advertising and threat-feed false positives.
- Protect documented hCaptcha dependencies and the exact GeeTest OneLogin endpoint without exempting unrelated GeeTest hosts.
- Replace widened regex conversions with provably equivalent matches and explicit omissions; validate module policies and Surge update intervals.
- Preserve alternative matcher semantics and quoted values in sing-box exports; emit current-version SRS and keep unsupported conversions explicit.
- Mark option-adapted SRS files `_supported` as well; retire the misleading unmarked filenames without removing their supported base matchers.
- Regenerate client exports without network refresh using `--exports-only`.
- Correct wildcard entries mislabeled as exact domains in DNS mapping sources and include those lists in publication validation.
- Document supported clients outside the README: Shadowrocket, current Surge, Mihomo and current sing-box.

## 0.10.0 · 2026-11-03

- Give system icons a balanced transparent safe area, render each size from the original SVG, and embed multiresolution Windows EXE icons without changing the in-app mark.
- Fix a colliding SVG gradient identifier so native renderers preserve the original circular accent.
- Add ordered batch renaming, range selection, dependency-aware name deduplication and offline IPv4/IPv6 CIDR filtering.
- Expand naming templates with effective server/port, stable source indices and padded indices; validate intermediate names and preserve dependency-linked nodes.
- Verify generated icon consistency and Windows resource round-trips during desktop packaging and CI.
- Replace the yanked async-executor lockfile release while keeping strict dependency-audit gates enabled.

## 0.9.0 · 2026-11-02

- Expand explicit node-domain resolution with standard DoH POST/GET, JSON compatibility, Cloudflare/Google providers, scoped filters and reproducible address selection.
- Honor DNS/HTTP cache lifetimes, isolate provider caches and rate-limit cooldowns, validate wire replies with Hickory, and offer strict resolver AD assertions without TLS or DNSSEC bypass.
- Add a direct DNS operation entry, bounded controls, per-step DNS reports and opt-in processed endpoint previews; retain unresolved nodes and fresh network-consent requirements.

## 0.8.0 · 2026-11-02

- Add combined conditions, name normalization, collision-safe unique names, region labels and reproducible shuffling.
- Add natural-name, effective-server and numeric-port sorting; safely edit server addresses and ports in supported dialects.
- Import and export reusable operation plans independently of subscriptions, with explicit plaintext consent, private files and fresh-preview requirements.
- Keep operation controls usable after importing large step identifiers, and reject unfinished enabled conditions before filtering.

## 0.7.0 · 2026-11-02

- Expand native operations with host, port, region and IP-family filters, limits, reverse ordering and typed node properties.
- Add explicitly authorized DoH processing with bounded concurrency, session caching, cancellation and shared rate-limit cooldowns; preserve unsupported nodes and TLS identities.
- Group operation controls and show modification/skip counts; back up plans without restoring network consent.
- Keep the in-app file browser polling after path navigation, without requiring another input event.

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
