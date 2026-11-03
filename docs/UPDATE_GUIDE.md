# Maintenance

[English](UPDATE_GUIDE.md) · [简体中文](UPDATE_GUIDE.zh-CN.md)

Run from the repository root; replace the sing-box path.

```sh
cargo run --release --locked --bin update -- --root . --execute --singbox /path/to/sing-box
cargo run --locked --bin validate -- --root .
```

`--quick` still accesses the network. Updates do not automatically commit or push. On failure, inspect logs and the worktree; preserve recovery backups.

`--exports-only --execute` regenerates client formats from the current lists without fetching upstreams; pass `--singbox` to include SRS.
`--classify-only --execute` also reapplies local category boundaries before exporting.

## Desktop notices

After dependency, desktop feature, or target changes, regenerate with cargo-about 0.9.3 and review the diff:

```sh
cargo fetch --locked --manifest-path crates/processor/Cargo.toml
cargo about generate --offline --locked --fail --features gui \
  --manifest-path crates/processor/Cargo.toml \
  --config crates/processor/about.toml \
  --output-file /path/to/scratch/THIRD_PARTY_LICENSES.txt crates/processor/about.hbs
```

Keep the full notices and license texts. Before release, verify bundled and upstream terms, package notices, and the exact source referenced by `SOURCE.txt`.

[Home](../README.md) · [Workflow](../.github/workflows/update_rulesets.yml)
