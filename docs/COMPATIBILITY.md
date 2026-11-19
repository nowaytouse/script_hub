# Client compatibility

[English](COMPATIBILITY.md) · [简体中文](COMPATIBILITY.zh-CN.md)

The intended consumption targets are Shadowrocket, the latest Surge, FlClash
RS for Surge modules, Mihomo for routing providers, and the latest sing-box.
Other clients are not publication or compatibility targets. Client versions,
platforms, settings and individual upstream resources can affect behavior;
an artifact passing repository checks is not a device runtime test.
`Sources/` contains build inputs and upstream references, not client-ready configurations.

| Target | Artifact and boundary |
| --- | --- |
| Shadowrocket | Use the Shadowrocket-specific modules and compatible rule lists. Module conversion is not a guarantee that every upstream script, rewrite or MITM behavior is equivalent. |
| Surge / FlClash RS | Use the same Surge `.sgmodule` links from the Script Hub module catalog. PROMAX references shared external rulesets. FlClash RS exposes the effective and ignored entries after import. See the [shared module guide](FLCLASH_RS.md). |
| Mihomo / FlClash routing | Generated providers use `rulesets/Mihomo/NAME.yaml` or `rulesets/AdBlock/Mihomo/NAME.yaml`; configure `behavior: classical` and `format: yaml`. `_supported.yaml` has omissions or adaptations, counted in its header. Use the provider, not a Surge or Shadowrocket module. |
| sing-box | Use the generated `.srs` rule sets, not a Surge or Shadowrocket module. Support targets the latest version. Files marked `_supported` contain only the supported subset, not an equivalent complete ruleset. |

Domain, IP and extra families are separate. Select an appropriate policy in
the client and account for rule order and DNS resolution; an IP family can
contain ASN rules, so it is not necessarily an IP-CIDR-only set. A partial
export may omit important matches. Neither routing rules nor binary rule sets
reproduce module scripts, HTTP rewrites, response modifications or MITM.
For Mihomo, an adapted Surge `extended-matching` rule retains only its basic
domain matcher; matching TLS SNI or HTTP Host depends on target sniffing
configuration and is not guaranteed equivalent to Surge.
SRS conversion does not carry Surge's per-rule `no-resolve` or `extended-matching`
options. Such adapted exports also use `_supported`; select current filenames,
as old unmarked SRS names are not retained. Resolution depends
on the referencing sing-box route and DNS configuration, not that option.

References: [Mihomo rule providers](https://wiki.metacubex.one/en/config/rule-providers/),
[provider contents](https://wiki.metacubex.one/en/config/rule-providers/content/),
[sing-box rule sets](https://sing-box.sagernet.org/configuration/rule-set/),
[Surge scripting](https://manual.nssurge.com/scripting/http-request.html),
and the [Shadowrocket App Store listing](https://apps.apple.com/us/app/shadowrocket/id932747118).

## Complete 217heidai input

The local exporter checks the declared counts of the official complete DNS
release and projects one shared set of protected domain suffixes. It applies
upstream exceptions before deduplication and checks the repository's service
allowlist, Direct routing, public resolvers and the shared Surge PROMAX MITM hosts.
The manifest records the source version, SHA-256 and matcher counts.

~~~sh
cargo run --locked --package rust_processor --bin promax -- --root . --export-heidai-bundle /path/outside/repository/new-heidai-bundle --singbox /path/to/sing-box
~~~

Choose a new output directory each time. Omit `--singbox` to export the
source-format JSON only for that client.

| Client | Generated file and use |
| --- | --- |
| Surge | `.Surge.list` via `RULE-SET,<path>,REJECT`. Resolve the path relative to the profile directory or use an absolute path. |
| Shadowrocket | Import `.Shadowrocket.module` as a routing module. |
| Mihomo / FlClash routing | Load `.Mihomo.yaml` as a local rule provider with `type: file`, `behavior: classical` and `format: yaml`, then select a reject policy for the rule provider. Put it under the core's home directory. |
| sing-box | Register `.SingBox.srs` as a local binary rule set or `.SingBox.json` as a local source rule set and select a reject action in the route or DNS rules that reference it. |
| AdGuard Home | Add `.AdGuardHome.txt` as a DNS filter from a location allowed by the server. |

Use the output for your client and keep the original raw 217heidai provider
disabled to preserve the reviewed exclusions. These files are local exports
because the complete feed aggregates sources with different redistribution
terms. The source-specific exceptions apply to that feed.

The shared compiler keeps app/Local, Advertising, Privacy, Security and
ThreatIntel ownership separate. Within a category and action, a plain
domain suffix owns the narrower plain matches it covers. Explicit matcher
options and independent subscriptions retain their coverage. The complete
217heidai feed stays identifiable as a mixed-source export.
