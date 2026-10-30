# PROMAX Rust Rebuild Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the unusable mixed Go/Rust PROMAX pipeline with one Rust-owned downloader/compiler, remove PROMAX Lite, reject malformed and high-risk media rules, and make scheduled GitHub updates publish only validated artifacts.

**Architecture:** `create/processor/src/promax/` becomes the deep module behind one FFI entrypoint. It parses manifests, downloads remote sources with bounded synchronous HTTP, compiles typed Surge rules, applies safety policy, generates one full PROMAX product, and validates staged artifacts. Go remains only the CLI caller for the repository-wide updater and no longer owns Promax network or parsing behavior.

**Tech Stack:** Rust 2024, `ureq` 3.4 with rustls, serde/serde_json, regex, Go/cgo command wrapper, Surge modules and external RULE-SET files, GitHub Actions.

---

## File map

- Create `create/processor/src/promax/mod.rs`: orchestration and public report types.
- Create `create/processor/src/promax/rule.rs`: quote-aware typed Surge rule parsing and serialization.
- Create `create/processor/src/promax/source.rs`: manifest parsing, provenance, Rust HTTP downloads, retry and size limits.
- Create `create/processor/src/promax/safety.rs`: typed whitelist intersection, protected media/CDN rules, quarantine records.
- Create `create/processor/src/promax/functional.rs`: section-aware functional extraction and deduplication.
- Create `create/processor/src/promax/artifact.rs`: catalog, ruleset, module, Loon, and report generation.
- Create `create/processor/src/promax/validation.rs`: external ruleset and complete module validation.
- Modify then delete `create/processor/src/adblock_manager.rs`: migrate its behavior into the focused internal Promax modules; no compatibility facade remains.
- Modify `create/processor/src/lib.rs`: expose the Rust-only FFI call and string report accessor.
- Modify `create/processor/Cargo.toml`: add `ureq = "3.4.0"`.
- Modify `create/pipeline/adblock_manager.go`: remove all downloads/JSON and call Rust only.
- Modify Promax catalog/converter/QA files under `create/hub`, `create/qa`, and `create/tools`: remove Lite constants and expectations.
- Delete tracked Lite artifacts under `modules/surge`, `modules/loon`, and `modules/shadowrocket`.
- Modify `.github/workflows/guard_generated_tree.yml` and `.github/workflows/update_rulesets.yml`: add single-job Rust tests and Promax validation gates.

### Task 1: Reproduce and fix typed Surge rule corruption

**Files:**
- Create: `create/processor/src/promax/mod.rs`
- Create: `create/processor/src/promax/rule.rs`
- Modify: `create/processor/src/lib.rs`

- [ ] **Step 1: Write failing parser tests**

Add tests that require one canonical representation and no policy leakage:

```rust
#[test]
fn quoted_url_regex_keeps_commas_and_peels_policy() {
    let parsed = SurgeRule::parse(
        r#"URL-REGEX,"^https?://host/(a{1,3}|b)",REJECT"#,
    ).unwrap();
    assert_eq!(parsed.payload, r#"^https?://host/(a{1,3}|b)"#);
    assert_eq!(parsed.policy.as_deref(), Some("REJECT"));
    assert_eq!(parsed.render_module("REJECT"), r#"URL-REGEX,"^https?://host/(a{1,3}|b)",REJECT"#);
    assert_eq!(parsed.render_external(), r#"URL-REGEX,"^https?://host/(a{1,3}|b)""#);
}

#[test]
fn rejects_nested_quote_policy_corruption() {
    assert!(SurgeRule::parse(
        r#"URL-REGEX,""^http://host/d",REJECT",REJECT"#,
    ).is_err());
}

#[test]
fn domain_wildcard_is_supported() {
    let parsed = SurgeRule::parse("DOMAIN-WILDCARD,api-*.example.com").unwrap();
    assert_eq!(parsed.render_external(), "DOMAIN-WILDCARD,api-*.example.com");
}
```

- [ ] **Step 2: Run the tests and verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test --manifest-path create/processor/Cargo.toml promax::rule::tests -- --test-threads=1`

Expected: compilation fails because `promax::rule::SurgeRule` does not exist.

- [ ] **Step 3: Implement the minimal typed parser**

Define:

```rust
pub struct SurgeRule {
    pub kind: RuleKind,
    pub payload: String,
    pub options: Vec<String>,
    pub policy: Option<String>,
}

impl SurgeRule {
    pub fn parse(line: &str) -> Result<Self, RuleError>;
    pub fn render_external(&self) -> String;
    pub fn render_module(&self, default_policy: &str) -> String;
}
```

The parser scans characters once, respects balanced double quotes, peels only known policy/option tokens from the right, rejoins unquoted regex comma fragments, compiles regex payloads, and rejects nested/unbalanced quotes.

- [ ] **Step 4: Run the focused tests and verify GREEN**

Run the Step 2 command again.

Expected: all `promax::rule::tests` pass with one test thread.

- [ ] **Step 5: Commit and push**

```bash
git add create/processor/src/promax create/processor/src/lib.rs
git commit -m "fix(promax): parse Surge rules without policy corruption"
git push
```

### Task 2: Add typed false-positive safety and quarantine

**Files:**
- Create: `create/processor/src/promax/safety.rs`
- Modify: `create/processor/src/promax/mod.rs`
- Modify: `create/processor/src/adblock_manager.rs`

- [ ] **Step 1: Write failing safety tests**

```rust
#[test]
fn keyword_rule_intersecting_protected_media_is_quarantined() {
    let safety = SafetyPolicy::from_lines(["DOMAIN-SUFFIX,googlevideo.com"]);
    let rule = SurgeRule::parse("DOMAIN-KEYWORD,googlevideo").unwrap();
    assert_eq!(safety.decision(&rule), SafetyDecision::Quarantine("protected-domain-intersection"));
}

#[test]
fn broad_shared_cdn_image_rewrite_is_quarantined() {
    let decision = classify_functional_url(
        r#"^https?://cdn.example.com/.+\\.(png|jpe?g|webp)$ _ reject"#,
    );
    assert!(matches!(decision, SafetyDecision::Quarantine("broad-media-match")));
}

#[test]
fn explicit_ad_asset_path_is_kept() {
    let decision = classify_functional_url(
        r#"^https?://cdn.example.com/advert/splash.webp$ _ reject"#,
    );
    assert_eq!(decision, SafetyDecision::Keep);
}
```

- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test --manifest-path create/processor/Cargo.toml promax::safety::tests -- --test-threads=1`

Expected: compilation fails because the safety types do not exist.

- [ ] **Step 3: Implement SafetyPolicy and QuarantineRecord**

```rust
#[derive(Debug, Clone, Serialize)]
pub struct QuarantineRecord {
    pub source: String,
    pub line: usize,
    pub candidate: String,
    pub reason: String,
    pub risk: RiskClass,
}

pub enum SafetyDecision {
    Keep,
    DropDuplicate,
    Quarantine(&'static str),
}
```

Match exact/suffix/IP rules structurally. Test keyword, wildcard, and regex rules against the finite protected-domain set. Require explicit ad-intent tokens for broad media/CDN functional matches.

- [ ] **Step 4: Route all AdBlock rule insertion through the safety decision**

Replace raw `is_whitelisted(rule_payload)` and hard-coded keyword arrays in `add_rule_line` with `SafetyPolicy::decision(&SurgeRule)`. Record quarantine with source path and line rather than silently dropping it.

- [ ] **Step 5: Verify GREEN and commit**

Run the Step 2 command, then:

```bash
git add create/processor/src/promax create/processor/src/adblock_manager.rs
git commit -m "fix(promax): quarantine media and protected-domain false positives"
git push
```

### Task 3: Move Promax networking completely into Rust

**Files:**
- Create: `create/processor/src/promax/source.rs`
- Modify: `create/processor/Cargo.toml`
- Modify: `create/processor/src/adblock_manager.rs`
- Modify: `create/processor/src/lib.rs`
- Modify: `create/pipeline/adblock_manager.go`

- [ ] **Step 1: Add `ureq` and write local-server download tests**

Use `TcpListener::bind("127.0.0.1:0")` in tests. Assert successful content, retry exhaustion on 500 responses, and rejection above `MAX_SOURCE_BYTES` without contacting the internet.

```rust
#[test]
fn downloader_reads_bounded_http_source() {
    let (url, server) = serve_once("200 OK", "DOMAIN,ads.example.com");
    let result = Downloader::for_tests().get(&url).unwrap();
    server.join().unwrap();
    assert_eq!(result, "DOMAIN,ads.example.com");
}
```

- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test --manifest-path create/processor/Cargo.toml promax::source::tests -- --test-threads=1`

Expected: compilation fails because `Downloader` does not exist.

- [ ] **Step 3: Implement the synchronous bounded downloader**

Use `ureq::Agent::config_builder()` with a global timeout, maximum redirects, a ScriptHub user-agent, at most three attempts, capped backoff, and `body_mut().with_config().limit(MAX_SOURCE_BYTES).read_to_string()`.

- [ ] **Step 4: Change the Rust entrypoint**

Change:

```rust
run_adblock_manager(root_dir, remote_contents_json, execute)
```

to:

```rust
run_adblock_manager(root_dir, execute) -> BuildReport
```

Rust loads both manifests, deduplicates remote URLs, downloads them, and passes the resulting snapshots to the existing compiler internals.

- [ ] **Step 5: Reduce Go to the FFI adapter**

`create/pipeline/adblock_manager.go` must contain no `network.SafeDownload`, JSON map, sleep, manifest read, or URL discovery. It converts the root string, invokes `run_adblock_manager_ffi(root, execute)`, and reports success.

- [ ] **Step 6: Verify focused Rust tests and Rust compile**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo test --manifest-path create/processor/Cargo.toml promax::source::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 cargo check --manifest-path create/processor/Cargo.toml
```

Expected: tests pass and `cargo check` exits 0.

- [ ] **Step 7: Commit and push**

```bash
git add create/processor/Cargo.toml create/processor/Cargo.lock create/processor/src create/pipeline/adblock_manager.go
git commit -m "refactor(promax): move source downloads into Rust"
git push
```

### Task 4: Remove PROMAX Lite from code and tracked products

**Files:**
- Modify: `create/processor/src/adblock_manager.rs`
- Modify: `create/hub/project_paths.go`
- Modify: `create/qa/audit_bundle_completeness.go`
- Modify: `create/tools/convert_surge_to_shadowrocket.go`
- Delete: tracked `*PROMAX Lite*` files under `modules/surge`, `modules/loon`, `modules/shadowrocket`
- Regenerate: `modules/helper/modules_data.json`, `modules/helper/shadowrocket_modules_data.json`, `modules/helper/surge_module_helper.html`

- [ ] **Step 1: Write a failing product-set test**

Add a Rust test that obtains the artifact plan and asserts exactly one Promax Surge product plus its GitHub variant, with no file path or catalog key containing `Lite`.

- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test --manifest-path create/processor/Cargo.toml promax::tests::product_set_has_no_lite -- --test-threads=1`

Expected: the existing Lite targets make the assertion fail.

- [ ] **Step 3: Remove all Lite branches and constants**

Delete `lite_only` parameters, `LITE_CATEGORIES`, Lite catalog fields, Lite generation calls, Lite converter cases, and completeness expectations. Keep a single `generate_module`, `generate_loon_plugin`, and conversion path.

- [ ] **Step 4: Delete tracked Lite products and regenerate helper indexes**

Use explicit tracked paths only. Do not touch `create/scripts/config-manager-auto-update/`.

- [ ] **Step 5: Verify no live Lite references remain**

Run: `rg -n 'PROMAX Lite|promax_lite|lite_only|promax-lite' create modules README.md .github --glob '!create/scripts/**'`

Expected: no matches outside historical design/plan documents.

- [ ] **Step 6: Commit and push**

```bash
git add create modules README.md
git commit -m "refactor(promax): remove Lite product variants"
git push
```

### Task 5: Validate the complete module and publish quarantine reports

**Files:**
- Create: `create/processor/src/promax/functional.rs`
- Create: `create/processor/src/promax/artifact.rs`
- Create: `create/processor/src/promax/validation.rs`
- Modify: `create/processor/src/promax/mod.rs`
- Delete: `create/processor/src/adblock_manager.rs`
- Modify: `create/qa/validate_surge_rulesets.go`
- Generate: `rulesets/AdBlock/quarantine.json`

- [ ] **Step 1: Write failing complete-module tests**

Use a small module fixture containing a valid quoted `URL-REGEX`, a nested-quote corrupt rule, an unsupported policy, and malformed MITM hostname syntax. Assert exact validation errors and source line numbers.

- [ ] **Step 2: Verify RED**

Run: `CARGO_BUILD_JOBS=1 cargo test --manifest-path create/processor/Cargo.toml promax::validation::tests -- --test-threads=1`

- [ ] **Step 3: Implement section-aware validation**

Validate metadata, section names, rule policy placement, external list policy absence, quote balance, regex compilation, script fields, rewrite actions, and MITM hostname entries. Reject the build if the full generated module has any structural issue.

- [ ] **Step 4: Finish the internal module split**

Move section inference/extraction into `functional.rs`, move ruleset/catalog/module/Loon/report rendering into `artifact.rs`, and keep only orchestration plus `BuildReport` in `promax/mod.rs`. Delete `adblock_manager.rs` and update `lib.rs` to import `promax` directly. Preserve behavior through the parser, safety, product-set, downloader, and complete-module tests created in Tasks 1-5.

- [ ] **Step 5: Stage, validate, then promote**

Write generated Promax files beneath `.cache/promax-staging/`, validate them, and only then replace tracked outputs with existing safe-write helpers. Always write a deterministic `quarantine.json` sorted by source, line, and candidate.

- [ ] **Step 6: Verify focused tests and commit**

```bash
CARGO_BUILD_JOBS=1 cargo test --manifest-path create/processor/Cargo.toml promax::validation::tests -- --test-threads=1
git add create/processor/src create/qa rulesets/AdBlock/quarantine.json
git commit -m "feat(promax): validate staged modules and report quarantined rules"
git push
```

### Task 6: Repair GitHub CI and scheduled publication gates

**Files:**
- Modify: `.github/workflows/guard_generated_tree.yml`
- Modify: `.github/workflows/update_rulesets.yml`
- Modify: `create/cmd/main_update/main.go`

- [ ] **Step 1: Add low-concurrency offline checks to PR CI**

Before the generated-tree guard, run:

```yaml
- name: Test Rust PROMAX compiler
  env:
    CARGO_BUILD_JOBS: '1'
  run: cargo test --manifest-path create/processor/Cargo.toml -- --test-threads=1
```

Build release only after tests pass.

- [ ] **Step 2: Gate scheduled updates on Promax validation**

The updater must exit non-zero when Rust reports required download failures, coverage regression, staging validation failure, or malformed artifacts. The workflow must not reach the Git commit/push section in those cases.

- [ ] **Step 3: Remove misleading parallel-processing claims**

Replace the workflow summary's `Parallel processing enabled` line with `PROMAX Rust validation passed` and add counts from `catalog.json` and `quarantine.json` when present.

- [ ] **Step 4: Run lightweight syntax and compile checks**

Run:

```bash
CARGO_BUILD_JOBS=1 cargo check --manifest-path create/processor/Cargo.toml
cd create && GOMAXPROCS=2 go test -p 1 ./qa ./hub
```

Expected: both commands exit 0.

- [ ] **Step 5: Commit and push**

```bash
git add .github/workflows create/cmd/main_update/main.go
git commit -m "ci: gate automated updates on PROMAX validation"
git push
```

### Task 7: Final artifact refresh and remote verification

**Files:**
- Modify: generated Promax modules, rulesets, catalogs, conversions, and helper indexes only.

- [ ] **Step 1: Run the offline deterministic fixture locally**

Run Rust tests and `cargo check` with one build job. Do not run the full remote refresh locally.

- [ ] **Step 2: Trigger the branch GitHub workflow or push the final code commit**

Use GitHub Actions for the release build and full scheduled-equivalent refresh so the user's device does not perform the heavy workload.

- [ ] **Step 3: Inspect workflow logs and generated diff**

Require zero nested-quote rules, zero Lite products, zero structural validation errors, and a non-empty quarantine report when risky upstream entries exist. Confirm the one PROMAX module references every generated ruleset shard.

- [ ] **Step 4: Commit refreshed artifacts if CI does not publish branch artifacts automatically**

Stage only the reviewed generated paths, commit `chore(promax): regenerate validated artifacts`, and push.

- [ ] **Step 5: Final verification**

Run targeted validators against the checked-in artifacts and confirm the GitHub branch is up to date. Report any upstream fetch failures or quarantined coverage explicitly rather than calling the rebuild complete silently.
