# Client compatibility

[English](COMPATIBILITY.md) · [简体中文](COMPATIBILITY.zh-CN.md)

The intended consumption targets are Shadowrocket, the latest Surge, the
Mihomo core (including FlClash using that core), and the latest sing-box.
Other clients are not publication or compatibility targets. Client versions,
platforms, settings and individual upstream resources can affect behavior;
an artifact passing repository checks is not a device runtime test.
`Sources/` contains build inputs and upstream references, not client-ready configurations.

| Target | Artifact and boundary |
| --- | --- |
| Shadowrocket | Use the Shadowrocket-specific modules and compatible rule lists. Module conversion is not a guarantee that every upstream script, rewrite or MITM behavior is equivalent. |
| Surge | Use Surge modules and rule lists. Support targets the latest version, not historical releases. |
| Mihomo / FlClash | Generated providers use `rulesets/Mihomo/NAME.yaml` or `rulesets/AdBlock/Mihomo/NAME.yaml`; configure `behavior: classical` and `format: yaml`. `_supported.yaml` has omissions or adaptations, counted in its header. Use the provider, not a Surge or Shadowrocket module. |
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
