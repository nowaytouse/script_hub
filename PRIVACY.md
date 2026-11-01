# Privacy

[English](PRIVACY.md) · [简体中文](PRIVACY.zh-CN.md)

Workspaces, settings and SQLite task history are not automatically uploaded. Diagnostic logging is opt-in; check for sensitive information before sharing.

Workspaces are not scanned by default. Updates and Git run through explicit actions or CI schedules; builds, external scripts and clients may access the network.

System and diagnostic endpoint exemptions do not disable collection. Use app or system settings to opt out; exemptions do not guarantee anonymity.

Local data is user-managed; secure erasure is not promised. Report concerns through the [security channel](SECURITY.md).

Template libraries stay encrypted at rest. Passwords are not saved; unlocked views, explicit copies and exports expose plaintext to the OS. Public ciphertext remains guessable offline and survives in Git history.

Relay is explicit: Gist content stays in session memory; tokens are not saved. Formatting contacts GitHub only; optional connectivity checks also contact selected node servers and HTTPS test services, with optional Cloudflare DoH. A newly created secret Gist is unlisted, not encrypted. Copies expose plaintext to the OS; publishing makes credentials available to anyone with output access.
