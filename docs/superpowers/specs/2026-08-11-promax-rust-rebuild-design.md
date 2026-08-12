# PROMAX Rust Rebuild Design

## Goal

Make the PROMAX Surge module usable, deterministic, standards-compliant, and broad in ad-block coverage without blocking ordinary media assets. PROMAX becomes a single Rust-owned pipeline for both network and local work.

## Confirmed constraints

- Rust owns remote downloads, local reads, parsing, normalization, safety filtering, merging, validation, and artifact generation.
- Remove PROMAX Lite and every Lite-specific catalog, conversion, Loon, Shadowrocket, documentation, and validation path.
- Keep one full PROMAX product with domain rules, URL rewrites, scripts, body/header rewrites, map-local entries, and MITM hosts when those entries are valid and ad-related.
- Preserve maximum useful coverage, but quarantine rules that are invalid or likely to block ordinary images, video, avatars, downloads, or shared CDN content.
- Keep local development checks low-load and single-process. Full upstream refresh remains a scheduled CI task.

## Current failure chain

The existing pipeline treats many rule lines as comma-separated strings even when a quoted regular expression contains commas or already has a policy. Normalization and later policy attachment therefore disagree about field boundaries. The final module contains entries such as nested-quote `URL-REGEX` rules with embedded policy text, which Surge cannot interpret reliably.

Local rule behavior is also duplicated across Go and Rust. The whitelist compares mostly untyped payload strings, allowing broad keyword rules to bypass protected suffixes. Functional extraction accepts broad media-path rewrites without a risk gate. Generated module sections are not validated as one complete Surge module, so malformed lines can be published even when external list checks pass.

## Architecture

PROMAX is one deep Rust module with this small external interface:

1. `run(root, execute) -> BuildReport`
2. `validate(root) -> ValidationReport`

`run` hides the complete implementation:

- `source`: typed manifests, local sources, remote requests, bounded retries, and provenance.
- `rule`: quote-aware parsing into typed rules and lossless Surge serialization.
- `safety`: typed whitelist intersection, protected-service rules, media/CDN risk scoring, and quarantine decisions.
- `functional`: section-aware extraction and deduplication for rewrites, scripts, map-local, body/header rewrite, and MITM.
- `artifact`: ruleset shards, the single PROMAX module, platform conversions, catalog, and quarantine report.
- `validation`: external ruleset grammar, complete module grammar, references, counts, and deterministic output checks.

The retired Go command is no longer part of the product. The Rust updater owns source downloads, local processing, validation, and publication directly.

## Rule model and Surge compliance

Rules are parsed once into a typed representation containing rule kind, payload, options, policy, source, and source line. Quoted regular expressions remain a single payload. Policies and options are recognized only in valid positions, then serialized exactly once.

The Surge capability table is centralized in Rust and aligned with the current official manual. External RULE-SET files omit policy components. Module rule lines use only policies supported for modules. `DOMAIN-WILDCARD` is preserved where supported; `DOMAIN-REGEX` is converted only when a lossless supported representation exists, otherwise quarantined.

## False-positive protection

Whitelist matching is type-aware:

- Exact and suffix block rules are rejected when they cover a protected domain.
- Keyword, wildcard, and regex rules are tested for intersection with protected services rather than comparing raw strings only.
- IP rules are rejected when their network overlaps a protected network.

Functional URL rules targeting shared media/CDN paths require explicit ad intent in the hostname or path, such as `ad`, `ads`, `advert`, `splash`, `promotion`, or a reviewed application-specific endpoint. Broad matches based only on file extensions, dimensions, generic image/video directories, or an entire shared CDN are quarantined.

Quarantine is fail-visible rather than silent. Each item records source, line, normalized candidate, reason, and risk class.

## Artifacts

The published product set contains:

- One PROMAX Surge module and its GitHub URL variant.
- PROMAX Loon and Shadowrocket conversions only when complete validation succeeds.
- Purpose-grouped external rulesets and Sing-box SRS files.
- `catalog.json` with input/output hashes and source statistics.
- `quarantine.json` with invalid and high-risk entries.

All PROMAX Lite artifacts and references are removed. Generation writes to staging paths first and promotes outputs only after validation, preventing partially generated updates from being published.

## Failure behavior

- A required manifest or local source failure stops the build.
- Remote downloads use bounded concurrency, timeouts, response-size limits, and retries with backoff.
- Optional upstream failures are reported; publication is refused when required sources fail or coverage falls below the previous successful manifest threshold.
- Invalid individual rules are quarantined. Structural module errors fail the build.
- No source file is rewritten as a cleanup side effect.

## Verification

Implementation is test-first and low-load locally:

- Rust unit tests reproduce nested quotes, policy leakage, wildcard handling, whitelist intersection, media false positives, and section extraction.
- Fixture tests compile a small offline source graph and validate the complete generated module.
- Rust verifies the updater result and generated module tree directly.
- CI runs formatting, Rust tests, module validation, generated-tree guards, and offline deterministic fixtures on pull requests.
- Scheduled CI performs the full network refresh, validates staged artifacts, and publishes only when every gate passes.

Local commands run with one build job. Full generation is deferred to GitHub Actions unless a narrowly scoped local fixture requires it.

## Scope discipline

Adjacent fixes are included only when they are reproduced in the PROMAX pipeline or prevent its GitHub automation from building, validating, or publishing. Unrelated cleanup and the untracked `create/scripts/config-manager-auto-update/` directory remain untouched.
