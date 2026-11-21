# Privacy

[English](PRIVACY.md) · [简体中文](PRIVACY.zh-CN.md)

Project files, settings, and SQLite task history stay local by default. Diagnostic logging is opt-in. Review diagnostic files for sensitive information before sharing.

The desktop app reads the local files it needs to display each page. Updates and Git operations run through explicit actions or CI schedules. Builds, external scripts, and clients may access the network.

System and diagnostic endpoint exemptions do not disable collection. Use app or system settings to opt out; exemptions do not guarantee anonymity.

Local data is user-managed; secure erasure is not promised. Report concerns through the [security channel](SECURITY.md).

Sample templates have a [public password](docs/TEMPLATES.md): a manual gate, not confidentiality or bot protection. Private libraries need independent secret passwords. Passwords are not saved; unlocking, copying and exporting expose plaintext to the OS.

Relay is explicit: content stays in session memory; tokens are not saved. Local edits stay offline; Gist import and publication contact GitHub. Authorized domain resolution sends node endpoint names to selected Cloudflare or Google DoH providers; it does not change system DNS. Optional connectivity checks contact selected nodes and HTTPS test services. Network consent is not restored from backups. A secret Gist is unlisted, not encrypted. Copies expose plaintext to the OS; publishing exposes credentials to anyone with output access.
