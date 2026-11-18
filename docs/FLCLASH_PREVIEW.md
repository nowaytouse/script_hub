# FlClash Preview native modules

[English](FLCLASH_PREVIEW.md) · [简体中文](FLCLASH_PREVIEW.zh-CN.md)

These modules target **FlClash testing/preview builds with the native MITM module page**, including the development device's `0.9.4+2026111517`. They are not ordinary Mihomo rule providers or profile-extension scripts. A public build without the feature cannot use this delivery path.

## Install from the repository

PROMAX has one installation entry. The module is about 31 MB and combines ad, privacy and phishing domain rules with firewall ports and selected app-specific HTTPS handlers.

- [ScriptHub.AdBlock.Preview.sgmodule](https://raw.githubusercontent.com/nowaytouse/script_hub/master/modules/flclash/head_expanse/github/ScriptHub.AdBlock.Preview.sgmodule) — direct GitHub URL for first installation and timely refresh.
- [Other Preview modules and compatibility report](../modules/flclash/) — select additional application bundles separately.

Update an existing PROMAX entry from the GitHub Raw link above. The module exceeds the default 20 MB single-file limit for GitHub files on jsDelivr. Domain and port rules work independently of HTTPS decryption. To apply URL Rewrite, Map Local and Body Rewrite to HTTPS requests, enable MITM in FlClash and follow its CA trust workflow. PROMAX embeds its response data and requires no script permission. [jsDelivr limits](https://www.jsdelivr.com/documentation#restrictions)

In the active profile's advanced configuration, open **MITM → Modules → Add**, then import the module URL (or download the file and use file import). Use the native module page, not the subscription/profile importer. Keep the existing profile for rollback. `FlClash-Surge.yaml` inherits Surge routing without adding extra advertising providers.

A script-bearing module is initially disabled until the application's script permission is accepted. Review the imported effective entries, ignored entries, downloaded resources and core validation issues before enabling it. A successful import alone is insufficient: the parser also considers a module with only skipped entries non-empty. Changed script declarations or content can invalidate the previous permission and disable the module again.

If you installed the old `ScriptHub.PROMAX.*.sgmodule` routing shards, disable or remove them in FlClash, then update the main PROMAX module above. Keep one PROMAX installation to avoid duplicate handling. Publishing this change does not remove, replace or enable modules already installed on a device.

HTTPS response changes additionally require the preview application's MITM feature and its own CA trust setup. This repository neither contains a CA/private key nor installs a certificate or grants script permission. Enable only the modules and host scopes you intend to use. Do not combine the aggregate advertising module with equivalent standalone ad handlers.

Source parameter definitions, descriptions and declared placeholders are retained for the native Preview parameter editor, including default-disabled jobs. Check the selected parameter values before enabling a module; undefined placeholders are reported instead of being silently emitted.

## What is compiled

### Complete 217heidai source for local use

Export a single local PROMAX module from the [complete official DNS release](https://github.com/217heidai/adblockfilters/releases/download/rule/adblockdns.txt). The exporter reads the complete release and verifies its declared counts. It applies source exceptions to the added rules, preserves the existing PROMAX, and filters additions against the shared service protections and Direct routing list. The export uses the existing load budget.

~~~sh
cargo run --locked --package rust_processor --bin promax -- --root . --export-heidai-full /path/outside/repository/ScriptHub.AdBlock.Heidai.Full.sgmodule
~~~

Import the result from the native module page in place of the standard PROMAX. Re-run the export and reimport to refresh. The [source catalog](../rulesets/Sources/sgmodule/AdBlock_sources_manual.list) lists original complete releases for other clients. The upstream native exports contain raw block matches, including Apple service names.

The generator reads the current public Surge module outputs, including PROMAX, and produces a separate, reproducible Preview namespace. It retains supported URL/header/body rewrites, Map Local responses, script declarations and MITM host scopes. Supported inline routing is normalized; incompatible sections, options and policy targets are counted in the compatibility report.

PROMAX embeds eligible exact domains and dotted suffixes from Local, Anti-AD, Privacy, all Advertising and HaGeZi TIF Medium shards, and the supplemental threat list. It applies the shared protected-service checks and retains the merged firewall ports. Reviewed URL rewrites, inline Map Local responses and native Body rewrites are scoped to exact MITM hosts. Publication is capped at 36 MiB and 850,000 active entries. The [compatibility catalog](../modules/flclash/catalog.json) records sources and omissions.

Other Preview modules continue to project supported HTTPS handlers. `[IP Rewrite]`, `[General]`, arbitrary proxy-group policies, other remote rulesets, and unsupported jq/structured-body or conditional operations remain reported omissions.

The module does not replace the profile, its subscription, system DNS, TUN settings or global TLS/security settings. Routing policies in native modules are limited to the parser's DIRECT/REJECT family. A module's supported routing rules still affect matching traffic.

## Updates and sources

Normal Rust updates regenerate the Preview outputs after the source modules are finalized. To regenerate only this adapter without refreshing upstream feeds:

```sh
cargo run --locked --manifest-path crates/processor/Cargo.toml --bin update -- --root /path/to/script_hub --execute --flclash-only
cargo run --locked --manifest-path crates/processor/Cargo.toml --bin validate -- --root /path/to/script_hub
```

Use the native module **Update** action to fetch a new release. Follow the application's permission review for any other module that uses scripts.

The adapter pins the FlClash dev revision [`5e73f3fb673a0012cc7c7312a52da33b76b78295`](https://github.com/chen08209/FlClash/tree/5e73f3fb673a0012cc7c7312a52da33b76b78295) and records the [module parser](https://github.com/chen08209/FlClash/blob/5e73f3fb673a0012cc7c7312a52da33b76b78295/lib/common/mitm_module.dart) and [compatibility test](https://github.com/chen08209/FlClash/blob/5e73f3fb673a0012cc7c7312a52da33b76b78295/test/common/mitm_module_test.dart) blobs. Verify actual HTTPS handling in the preview client, which uses a private core.

## Device acceptance

1. Confirm a testing/preview build exposes the native MITM/module pages. Record the application and core version.
2. Import one module, inspect effective/ignored entries and resources, and resolve core validation issues. Do not treat an old resource-cache fallback as a successful fresh download.
3. Apply the application's own script permission and CA trust workflow where needed. Do not broaden host exclusions or certificate verification settings just to remove an error.
4. Test representative synthetic request/response cases for each enabled rewrite/script family, then normal application behavior and module refresh. Compare actual responses; no blanket claim of removing all advertisements.
5. To stop using it, disable that module in the application and preserve the previous profile. App-global CA/trust and DNS/TUN choices are separate operations.

See [client compatibility](COMPATIBILITY.md) for the separate routing-provider and sing-box artifacts.
