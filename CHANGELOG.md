# Changes

[English](CHANGELOG.md) · [简体中文](CHANGELOG.zh-CN.md)

## Unreleased

- Reuse exception matching and parse rules once when generating ad-block shards; avoid intermediate rule-string copies.

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
