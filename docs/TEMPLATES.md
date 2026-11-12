# Using configuration templates

[English](TEMPLATES.md) · [简体中文](TEMPLATES.zh-CN.md)

## Unlock the sample

Open the [desktop app](../crates/processor/README.md) and go to **Library → Templates → Open library**. **Current bundled templates** is selected by default. Paste the full line below into **Password**, then click **Unlock**. No account, token, or subscription is needed to view the sample.

```text
tTUa!Acdb7!vhUpSUD1Rl9g*^bm@S#Xahrj1TK72KrA!TMty$lhh6zl1aIbOvAODzM#N2SxzG53iEgVRu571DNGU9rLWijO!0jZCDGPDQmLu%cmnC&QT9vOfqZ75V7*m
```

The bundled library stays encrypted on disk. Its public password is a manual gate, not confidentiality or bot protection. Do not reuse it for private libraries.

## Make it yours

After unlocking, choose Surge, Shadowrocket, FlClash or sing-box. The variants follow the bundled Surge routing baseline, using only the rule formats each client supports. GitHub belongs to `Microsoft_proxy`; for exported profiles still using retired ruleset links, export a fresh copy. Original encrypted backups are not offered as templates, and private libraries are opened only on request. Existing files are not rewritten.

Shadowrocket enables IPv6 and uses AliDNS and DNSPod DoH for DIRECT DNS. Surge and Shadowrocket use [Microsoft NCSI](https://learn.microsoft.com/en-us/troubleshoot/windows-server/networking/troubleshoot-ncsi-guidance) (HTTP 200) for direct checks and the [GrapheneOS HTTPS endpoint](https://grapheneos.org/faq#default-connections) (204) for proxy and group checks. The direct probe uses plaintext HTTP; either service can see the source address of its probe.

FlClash and sing-box include the current native AdBlock rule providers. They do not execute Surge modules or scripts, intercept TLS, or claim coverage for rules without a native representation. Script Hub checks its own ruleset references against the catalog bundled with the app; this does not check third-party availability.

1. Select a template for your client and enable **Show full template** to read its original text.
2. Use **Edit template → Apply changes** to adjust policy groups or rule order, or edit a copy in your own text editor. To keep edits in the app, choose **Save encrypted library** with a project available. This writes a local copy, not the public sample. Save before switching apps: automatic locking discards unsaved changes.
3. Fill any subscription fields needed by the client, acknowledge the plaintext warning, then choose **Copy configuration** or **Export configuration** to a path outside the repository. Import the result as a new profile and keep your existing profile until the new one works.

Subscription URLs are inserted only when copying or exporting, and Script Hub does not fetch them. Add a Shadowrocket subscription separately. For FlClash/Mihomo, fill the provider slots with Clash-compatible URLs and import the exported `.yaml` as a local profile. For sing-box 1.14 or later, export `.json`; run a local SOCKS proxy at `127.0.0.1:1080` or replace the `script-hub-proxy` outbound with your own while keeping its tag. The sing-box template does not accept a subscription URL.

Install optional modules listed in the comments separately. Keep plaintext exports and subscription URLs out of Git.
