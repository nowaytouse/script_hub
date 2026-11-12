# Using configuration templates

[English](TEMPLATES.md) · [简体中文](TEMPLATES.zh-CN.md)

## Unlock the sample

Open the [desktop app](../crates/processor/README.md) and go to **Library → Templates → Open library**. Select **Templates**, not **Encrypted originals**. Paste the full line below into **Password**, then click **Unlock**. No account, token, or subscription is needed to view the sample.

```text
tTUa!Acdb7!vhUpSUD1Rl9g*^bm@S#Xahrj1TK72KrA!TMty$lhh6zl1aIbOvAODzM#N2SxzG53iEgVRu571DNGU9rLWijO!0jZCDGPDQmLu%cmnC&QT9vOfqZ75V7*m
```

The bundled sample is encrypted, but its password is public. It is a manual gate, not confidentiality or bot protection. Do not reuse it for private libraries.

## Make it yours

In the latest desktop app, the bundled Surge and Shadowrocket samples use `Microsoft_direct` for sign-in and `Microsoft_proxy` for other Microsoft services, including GitHub. Export a fresh copy or update previously exported configurations manually; your existing files and private libraries are not rewritten.

1. Select a template for your client and enable **Show full template** to read its original text.
2. Use **Edit template → Apply changes** to adjust policy groups or rule order, or edit a copy in your own text editor. To keep edits in the app, choose **Save encrypted library** with a project available. This writes a local copy, not the public sample. Save before switching apps: automatic locking discards unsaved changes.
3. Fill any subscription fields needed by the client, acknowledge the plaintext warning, then choose **Copy configuration** or **Export configuration** to a path outside the repository. Import the result as a new profile and keep your existing profile until the new one works.

Subscription URLs are inserted only when copying or exporting, and Script Hub does not fetch them. Add a Shadowrocket subscription separately. For a FlClash/Mihomo template, fill its provider slots and import the exported `.yaml` as a local profile.

Install optional modules listed in the comments separately. Keep plaintext exports and subscription URLs out of Git.
