# Changes

[简体中文](CHANGELOG.zh-CN.md)

## Unreleased

- Keep GitHub API cooldowns from stopping independent module and ruleset downloads, and deduplicate scripts by their complete request patterns.
- Follow Sub-Store's official rolling script releases in the utilities bundle and keep reviewed, quiet video and utility defaults across refreshes.
- Follow rolling Rust nightly for local builds and CI.
- Keep working PROMAX inputs when upstream returns an empty module, and reject incomplete functional builds instead of silently dropping declared sources.
- Prefer official app2smile handlers over duplicate CDN aliases, use actual request patterns to retain MITM hosts, and limit Mlxx to its two reviewed ad endpoints.
- Follow fmz200’s Shadowrocket X Web Ads module in PROMAX, with a bounded local response filter for the reviewed timeline and post-detail endpoints.

- Default to the current encrypted template library, retire legacy template selection and GitHub ruleset aliases, and derive FlClash and sing-box profiles from the Surge routing baseline.
- Enable direct encrypted DNS and IPv6 in the Shadowrocket template; use Microsoft NCSI for direct checks and GrapheneOS HTTPS for proxy checks.
- Include the published native AdBlock providers in the FlClash and sing-box templates.

- Import and export supported Surge VLESS policies in Subscription Relay. Show the minimum Surge versions before export and stop rather than drop nodes when conversion cannot preserve their settings.
- Reject directory-style export paths for FlClash on Windows, including paths with a trailing separator under a verbatim prefix.
- Reject ambiguous Windows filenames for plaintext exports before they can resolve to another file or an alternate data stream.
- Clarify that copying or exporting a FlClash configuration produces unencrypted YAML containing any entered subscription URLs. The bundled template library stays encrypted.
- Refresh the SQLite, SVG rendering, regex and public-suffix dependencies while retaining the age security hold.

- Split Microsoft into exact sign-in and service rulesets, include GitHub in the latter, and update public sample exports to use the two current subscriptions.

- Refresh reviewed network-safe Brave, uBO Privacy, uBO Badware and Greek filter subsets without flattening browser-only rules or ambiguous exceptions.
- Restore the native Bilibili ADBlock preference-pane endpoint and resolve its script defaults without duplicating ad handlers.
- Restore GitHub service ownership, add exact Microsoft sign-in and Microsoft 365 coverage, and render the same split for Surge, Shadowrocket, FlClash, Mihomo and sing-box.
- Document how to unlock, customize, and export the bundled public configuration templates.

- Add opt-in Windows TUN strict DNS settings to FlClash template export, including TCP and UDP interception, HTTPS-only resolvers, and the local-file China DNS policy.
- Add FlClash/Mihomo YAML templates with structured subscription sanitization and export.

- Follow V2Fly's maintained private-device feed with exact, reviewed Host mappings and last-good recovery; keep routing and global DNS settings unchanged.
- Scope authenticated GitHub feed reads to the official API without redirects; keep updater builds and external SRS compilation separate from those credentials.
- Clarify backup import counts and identify deletion targets and collection dependencies before confirmation.
- Migrate Sub-Store's explicit first-name duplicate deletion into a native, editable operation without silently changing linked-node identity.
- Resolve the withdrawn serde-saphyr release using the latest non-yanked registry version, and refresh compatible psl, svgtypes, and winnow patches while retaining the age security hold.
- Expand DNS Helpers with IANA's locally-served IPv4 and IPv6 reverse zones, independently validated and refreshed alongside Sukka's LAN feed without changing routing or global DNS providers.
- Reject self-referential Host aliases during upstream merge and publication checks; keep DNS Helpers free of unreviewed public-domain overrides.
- Temporarily pin age exactly to 0.11.5 while retaining locked dependency audits.
- Import deterministic Sub-Store name sorting into the native pipeline, with UTF-16 order, stable equal-name ties, and collection member-name support.
- Keep saved subscription and collection previews faithful to their processing plans without implicit name decoration or ordering.

- Keep DNS Helpers free of firewall rules; compile the current validated port denylist into PROMAX instead of reading a missing legacy path.
- Add Steven Black's MIT-licensed original hosts through the existing safety pipeline; preserve exact aliases and ignore real-IP mappings.
- Protect categorized content-service roots from advertising lists without exempting their ad subdomains or threat-intelligence indicators.
- Preserve empty Sub-Store filter, rename, and delete operation semantics during native backup migration.
- Raise the serde-saphyr minimum to the already-locked 2.1 line without introducing age's affected P-256 dependency graph.

- Exercise backup migration, collection previews, and simulated scheduler cycles in the offline demo; prevent duplicate sample imports and draft loss.
- Verify scheduled collection publication, readback, and unchanged-output skipping against a mock Gist.
- Validate the live Helper refresh path and reject upstream grammar changes without replacing the last good Host snapshot.

- Keep existing DNS providers in the AdBlock Helper and offer AliDNS as a separate, opt-in module.
- Keep the last relay preview counts after a failed retry without exposing or reusing its output; hide preview addresses when the window loses focus.
- Reject collection edits that could target the wrong node; reset individual node edits without losing processing steps.

- Separate relay operations, input nodes, and output preview; keep private output details hidden until explicitly revealed.
- Protect exact encrypted resolver endpoints from older Helper configurations without selecting them or adding DNS mappings.

- Preserve Surge's Bonjour resolution and reject incomplete upstream Host snapshots before refreshing the DNS helper.
- Migrate Sub-Store collection regex sorting against original member names without changing published names.

- Follow Sukka's local DNS feed for reviewed device names and private reverse zones without importing public DNS overrides.
- Migrate Sub-Store regex sorting and safe node quick settings; control unmatched node order in the native editor.

- Keep Apple enhancement modules free of routing rules, narrow their HTTPS interception, and use native WeatherKit data by default. Shadowrocket uses upstream TV and an optional WeatherKit Rewrite module.
- Migrate numbered captures in portable Sub-Store regex renames.
- Restore the public encrypted template's sanitization invariant.

- Keep the quieter utility and visible YouTube defaults across upstream refreshes; retain the last valid module when the relevant arguments or hooks change.
- Limit the helper's system DNS mappings to local devices and bootstrap its HTTPS resolver over IPv4 and IPv6.
- Translate Sub-Store's whole-match and literal-dollar replacements in native subscription renames.

- Update encrypted sample module links and make scheduled utilities opt-in while keeping YouTube controls visible.
- Migrate subscription-level Sub-Store flag removal as a native rename operation.

- Combine the BiliBili and YouTube feature modules across Surge and Shadowrocket while keeping their PROMAX build inputs separate.
- Migrate portable subscription-level Sub-Store regex rename and delete operations without running JavaScript.

- Combine the panel utilities with Script Hub Tools. Use the existing Tools module link; the standalone panel links are retired.

- Restore compatible collection name filters, tag-selected members, and validated proxy chains from Sub-Store backups.
- Filter subscriptions with bounded Rhai boolean masks without rewriting or reordering retained nodes.
- Migrate compatible Sub-Store filters as native subscription and collection operations.
- Accept formatting-only refreshes without losing saved collection edits.
- Avoid a duplicate source download during scheduled sync; recheck the source before publishing.

## 0.14.8 · 2026-11-04

- Run bounded Rhai scripts over an entire subscription, with format context, endpoint edits, and regex helpers.
- Edit and export validated proxy chains, inspect effective protocol fields, and reuse scripts with per-step arguments and safe source metadata.

## 0.14.7 · 2026-11-03

- Name verified Surge groups and copy their subscription links without publishing again.
- Apply strict TLS to native Surge exports and validate logical rules before publishing modules.

## 0.14.6 · 2026-11-03

- Run an enabled subscription or collection schedule on demand without bypassing retries or cooldowns.
- Clarify collection refresh and keep file drops from replacing an active schedule.
- Count ruleset comments consistently during validation.

## 0.14.5 · 2026-11-03

- Export compatible AnyTLS nodes to Surge and clarify recovery after an empty preview or a failed file save.
- Validate Apple safeguards before publishing modules, reject invalid publication paths, and fix dependency search on Windows.

## 0.14.4 · 2026-11-03

- Keep edits after syncing unchanged local nodes; explain which remote edits block scheduling.

## 0.14.3 · 2026-11-03

- Preview lightweight saved edits offline on open; run scripts and DNS only when requested.

## 0.14.2 · 2026-11-03

- Copy a Surge policy group after verified Gist publication.

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
