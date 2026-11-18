# FlClash Preview native modules

[English](FLCLASH_PREVIEW.md) · [简体中文](FLCLASH_PREVIEW.zh-CN.md)

These modules target **FlClash testing/preview builds with the native MITM module page**, including the development device's `0.9.4+2026111517`. They are not ordinary Mihomo rule providers or profile-extension scripts. A public build without the feature cannot use this delivery path.

## Install from the repository

PROMAX has one installation entry. The compatibility module combines local app ads, Anti-AD and privacy domains with firewall port rules and selected app-specific HTTPS handlers.

- [ScriptHub.AdBlock.Preview.sgmodule](https://raw.githubusercontent.com/nowaytouse/script_hub/master/modules/flclash/head_expanse/github/ScriptHub.AdBlock.Preview.sgmodule) — direct GitHub URL for first installation and timely refresh.
- [Other Preview modules and compatibility report](../modules/flclash/) — select additional application bundles separately.

If PROMAX is already installed, update that entry using the direct GitHub link above to avoid CDN cache delays. The module is listed as **FlClash Preview · AdBlock PROMAX (Compatibility)**. Domain and port rules work independently of HTTPS decryption. To activate the URL Rewrite and Map Local entries for HTTPS requests, enable MITM in FlClash and follow its CA trust workflow. PROMAX uses inline response data and requires no script permission.

In the active profile's advanced configuration, open **MITM → Modules → Add**, then import the module URL (or download the file and use file import). Use the native module page, not the subscription/profile importer. Keep the existing profile for rollback. `FlClash-Surge.yaml` inherits Surge routing without adding extra advertising providers.

A script-bearing module is initially disabled until the application's script permission is accepted. Review the imported effective entries, ignored entries, downloaded resources and core validation issues before enabling it. A successful import alone is insufficient: the parser also considers a module with only skipped entries non-empty. Changed script declarations or content can invalidate the previous permission and disable the module again.

If you installed the old `ScriptHub.PROMAX.*.sgmodule` routing shards, disable or remove them in FlClash, then update the main PROMAX module above. Keep one PROMAX installation to avoid duplicate handling. Publishing this change does not remove, replace or enable modules already installed on a device.

HTTPS response changes additionally require the preview application's MITM feature and its own CA trust setup. This repository neither contains a CA/private key nor installs a certificate or grants script permission. Enable only the modules and host scopes you intend to use. Do not combine the aggregate advertising module with equivalent standalone ad handlers.

Source parameter definitions, descriptions and declared placeholders are retained for the native Preview parameter editor, including default-disabled jobs. Check the selected parameter values before enabling a module; undefined placeholders are reported instead of being silently emitted.

## What is compiled

The generator reads the current public Surge module outputs, including PROMAX, and produces a separate, reproducible Preview namespace. It retains supported URL/header/body rewrites, Map Local responses, script declarations and MITM host scopes. Supported inline routing is normalized; incompatible sections, options and policy targets are counted in the compatibility report.

PROMAX embeds exact domains and dotted suffixes from the existing Local, Anti-AD and Privacy lists alongside the merged firewall ports. Selected URL Rewrite and inline Map Local entries are scoped to exact MITM hosts, with no remote script or response-file downloads. Publication is capped at 640 KiB and 14,000 active entries. The [compatibility catalog](../modules/flclash/catalog.json) records sources and omissions.

Other Preview modules continue to project supported HTTPS handlers. `[IP Rewrite]`, `[General]`, arbitrary proxy-group policies, other remote rulesets, and unsupported jq/structured-body or conditional operations remain reported omissions.

The module does not replace the profile, its subscription, system DNS, TUN settings or global TLS/security settings. Routing policies in native modules are limited to the parser's DIRECT/REJECT family. A module's supported routing rules still affect matching traffic.

## Updates and sources

Normal Rust updates regenerate the Preview outputs after the source modules are finalized. To regenerate only this adapter without refreshing upstream feeds:

```sh
cargo run --locked --manifest-path crates/processor/Cargo.toml --bin update -- --root /path/to/script_hub --execute --flclash-only
cargo run --locked --manifest-path crates/processor/Cargo.toml --bin validate -- --root /path/to/script_hub
```

The direct GitHub variant avoids relying on a CDN's branch cache for immediate delivery. Existing clients may still need their native module **Update** action and, if the application requires it, a fresh script permission review. Publishing a new module does not silently enable it on an existing device.

Adapter contract: FlClash dev [`5e73f3fb673a0012cc7c7312a52da33b76b78295`](https://github.com/chen08209/FlClash/tree/5e73f3fb673a0012cc7c7312a52da33b76b78295), its [module parser](https://github.com/chen08209/FlClash/blob/5e73f3fb673a0012cc7c7312a52da33b76b78295/lib/common/mitm_module.dart), [compatibility tests](https://github.com/chen08209/FlClash/blob/5e73f3fb673a0012cc7c7312a52da33b76b78295/test/common/mitm_module_test.dart), and [native editor template](https://github.com/chen08209/FlClash/blob/5e73f3fb673a0012cc7c7312a52da33b76b78295/lib/features/mitm/module_editor.dart). The parser/test blobs are recorded by the adapter. The bundled private preview core can have additional behavior; source-level adapter tests do not prove that every upstream script runs successfully on that core.

## Device acceptance

1. Confirm a testing/preview build exposes the native MITM/module pages. Record the application and core version.
2. Import one module, inspect effective/ignored entries and resources, and resolve core validation issues. Do not treat an old resource-cache fallback as a successful fresh download.
3. Apply the application's own script permission and CA trust workflow where needed. Do not broaden host exclusions or certificate verification settings just to remove an error.
4. Test representative synthetic request/response cases for each enabled rewrite/script family, then normal application behavior and module refresh. Compare actual responses; no blanket claim of removing all advertisements.
5. To stop using it, disable that module in the application and preserve the previous profile. App-global CA/trust and DNS/TUN choices are separate operations.

See [client compatibility](COMPATIBILITY.md) for the separate routing-provider and sing-box artifacts.
