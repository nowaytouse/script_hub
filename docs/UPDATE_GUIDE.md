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

After a dependency, desktop feature, or target change, regenerate the notice using cargo-about 0.9.3 from the repository root. Write to a scratch location first and review the diff before replacing the tracked file:

```sh
cargo fetch --locked --manifest-path crates/processor/Cargo.toml
cargo about generate --offline --locked --fail --features gui \
  --manifest-path crates/processor/Cargo.toml \
  --config crates/processor/about.toml \
  --output-file /path/to/scratch/THIRD_PARTY_LICENSES.txt crates/processor/about.hbs
```

The dependency audit compares a fresh notice on relevant pushes, pull requests, and manual runs; scheduled vulnerability checks do not regenerate it. Normalize only trailing spaces, tabs and CRLF in the generated text; retain all notices and license wording. The offline check uses the fetched locked crates and does not substitute for review of additional upstream license material. Separately check bundled assets and upstream material against their terms. Before distributing an archive, verify that the corresponding source is available, that `SOURCE.txt` names the build commit, and that the package includes the full license texts and notices. Do not treat a dirty build's base-commit link as its source.

[Home](../README.md) · [Workflow](../.github/workflows/update_rulesets.yml)
