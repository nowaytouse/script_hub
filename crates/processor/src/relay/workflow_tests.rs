use super::*;
use gist::tests::{Fixture, ID, INPUT_ID, api, created, document};
use model::Health;

const SOURCE: &str = "A = anytls, a.example.invalid, 443, password=synthetic-a\nB = anytls, b.example.invalid, 443, password=synthetic-b\n";

fn config() -> RelayConfig {
    RelayConfig {
        source: INPUT_ID.to_owned(),
        source_file: "nodes.txt".to_owned(),
        output: ID.to_owned(),
        output_file: "nodes.txt".to_owned(),
        title: "relay".to_owned(),
        rename: false,
        sort: false,
        test_nodes: true,
        ..RelayConfig::default()
    }
}
fn input(content: &str) -> Option<String> {
    let mut doc = document(content);
    doc["id"] = serde_json::json!(INPUT_ID);
    api(doc)
}
fn check(
    _: &model::Subscription,
    _: &engine::ProbeOptions,
    _: &AtomicBool,
    _: &(dyn Fn(usize, usize) + Sync),
) -> Result<engine::ProbeBatch, engine::EngineError> {
    Ok(engine::ProbeBatch {
        health: vec![Health::Healthy { latency_ms: 5 }, Health::Failed],
        baseline_valid: 2,
    })
}
fn candidate() -> Zeroizing<String> {
    model::Subscription::parse(SOURCE)
        .unwrap()
        .render(
            &[Health::Healthy { latency_ms: 5 }, Health::Failed],
            &model::EnhanceOptions {
                rename: false,
                sort: false,
            },
        )
        .unwrap()
}

#[test]
fn local_backup_cleans_creates_gist_and_roundtrips_without_source_requests() {
    let mut config = config();
    config.source.clear();
    config.output.clear();
    config.local_content = Some(Zeroizing::new(SOURCE.into()));
    let cleaned = candidate();
    let fixture = Fixture::new(vec![created(document(&cleaned)), api(document(&cleaned))]);
    let result = run_with(
        &config,
        &fixture.client(),
        RunMode::Publish,
        &AtomicBool::new(false),
        &|_| {},
        &check,
    )
    .unwrap();
    assert!(result.published && result.created.is_some());
    assert_eq!((result.total, result.removed), (2, 1));
    assert_eq!(fixture.request_methods(), ["POST", "GET"]);
    let bytes = backup::encode(&config, &result.content).unwrap();
    let restored = backup::decode(&bytes).unwrap();
    assert!(model::semantic_equal(&restored.content, &cleaned));
    assert_eq!(
        model::Subscription::parse(&restored.content)
            .unwrap()
            .nodes()
            .len(),
        1
    );
}

#[test]
fn local_source_cannot_overwrite_its_recorded_origin_and_invalid_data_never_writes() {
    let mut config = config();
    config.local_content = Some(Zeroizing::new(SOURCE.into()));
    config.output = config.source.clone();
    assert!(config.validate(RunMode::Publish).is_err());
    config.output.clear();
    config.source.clear();
    config.local_content = Some(Zeroizing::new("not nodes".into()));
    let fixture = Fixture::new(vec![]);
    assert!(matches!(
        run_with(
            &config,
            &fixture.client(),
            RunMode::Publish,
            &AtomicBool::new(false),
            &|_| {},
            &check
        ),
        Err(RelayError::Model(_))
    ));
    assert!(fixture.request_methods().is_empty());
}

#[test]
fn default_formatting_never_invokes_a_node_checker_or_claims_health() {
    let fixture = Fixture::new(vec![input(SOURCE)]);
    let mut config = config();
    config.engine.clear();
    config.test_nodes = false;
    let result = run_with(
        &config,
        &fixture.client(),
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
        &|_, _, _, _| panic!("format-only mode must not invoke sing-box or node checks"),
    )
    .unwrap();
    assert_eq!((result.healthy, result.unknown, result.removed), (0, 2, 0));
    assert!(model::semantic_equal(SOURCE, &result.content));
    assert_eq!(fixture.request_methods(), ["GET"]);
}

#[test]
fn explicit_publish_accepts_a_new_independent_secret_destination() {
    let mut config = config();
    config.output.clear();
    assert!(config.validate(RunMode::Publish).is_ok());
}

#[test]
fn format_only_publish_creates_a_secret_gist_after_source_revalidation() {
    let fixture = Fixture::new(vec![
        input(SOURCE),
        input(SOURCE),
        created(document(SOURCE)),
        api(document(SOURCE)),
    ]);
    let mut config = config();
    config.test_nodes = false;
    config.engine.clear();
    config.output.clear();
    let result = run_with(
        &config,
        &fixture.client(),
        RunMode::Publish,
        &AtomicBool::new(false),
        &|_| {},
        &|_, _, _, _| panic!("format-only publication must not test nodes"),
    )
    .unwrap();
    assert!(!result.tested && result.published);
    assert_eq!((result.healthy, result.unknown, result.removed), (0, 2, 0));
    assert_eq!(result.created.unwrap().id, ID);
    assert_eq!(fixture.request_methods(), ["GET", "GET", "POST", "GET"]);
}

#[test]
fn real_testing_does_not_create_when_every_node_is_unverified() {
    let fixture = Fixture::new(vec![input(SOURCE)]);
    let mut config = config();
    config.output.clear();
    let result = run_with(
        &config,
        &fixture.client(),
        RunMode::Publish,
        &AtomicBool::new(false),
        &|_| {},
        &|_, _, _, _| {
            Ok(engine::ProbeBatch {
                health: vec![Health::Unknown; 2],
                baseline_valid: 2,
            })
        },
    );
    assert!(matches!(result, Err(RelayError::NoHealthyNodes)));
    assert_eq!(fixture.request_methods(), ["GET"]);
}

#[test]
fn format_only_preserves_unknown_protocol_and_insecure_tls_fields() {
    let source = "A = future-protocol, a.example.invalid, 443, future=value\nB = trojan, b.example.invalid, 443, password=synthetic, skip-cert-verify=true\n";
    let fixture = Fixture::new(vec![input(source)]);
    let mut config = config();
    config.test_nodes = false;
    let result = run_with(
        &config,
        &fixture.client(),
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
        &|_, _, _, _| panic!("unverified nodes must only be preserved"),
    )
    .unwrap();
    assert_eq!(result.content.as_str(), source);
    assert_eq!(
        (
            result.healthy,
            result.failed,
            result.unknown,
            result.removed
        ),
        (0, 0, 2, 0)
    );
}

#[test]
fn format_only_reuse_does_not_upload_semantically_unchanged_nodes() {
    let old = format!("# existing formatting\n{SOURCE}");
    let fixture = Fixture::new(vec![
        input(SOURCE),
        api(document(&old)),
        input(SOURCE),
        api(document(&old)),
    ]);
    let mut config = config();
    config.test_nodes = false;
    let result = run_with(
        &config,
        &fixture.client(),
        RunMode::Publish,
        &AtomicBool::new(false),
        &|_| {},
        &|_, _, _, _| panic!("format-only reuse must not test nodes"),
    )
    .unwrap();
    assert!(result.unchanged && !result.published && result.created.is_none());
    assert_eq!(fixture.request_methods(), ["GET", "GET", "GET", "GET"]);
}

#[test]
fn preview_reads_only_the_source_and_never_publishes() {
    let fixture = Fixture::new(vec![input(SOURCE)]);
    let result = run_with(
        &config(),
        &fixture.client(),
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
        &check,
    )
    .unwrap();
    assert_eq!((result.total, result.healthy, result.removed), (2, 1, 1));
    assert!(!result.published);
    assert_eq!(fixture.request_methods(), ["GET"]);
}

#[test]
fn all_red_or_inconclusive_never_replaces_output() {
    for health in [Health::Failed, Health::Unknown, Health::SecurityBlocked] {
        let fixture = Fixture::new(vec![input(SOURCE), api(document("existing"))]);
        let checker = |_: &model::Subscription,
                       _: &engine::ProbeOptions,
                       _: &AtomicBool,
                       _: &(dyn Fn(usize, usize) + Sync)| {
            Ok(engine::ProbeBatch {
                health: vec![health; 2],
                baseline_valid: 2,
            })
        };
        let result = run_with(
            &config(),
            &fixture.client(),
            RunMode::Publish,
            &AtomicBool::new(false),
            &|_| {},
            &checker,
        );
        assert!(matches!(result, Err(RelayError::NoHealthyNodes)));
        assert_eq!(fixture.request_methods(), ["GET", "GET"]);
    }
}

#[test]
fn source_changed_during_checking_prevents_publication() {
    let changed = SOURCE.replace("synthetic-a", "synthetic-changed");
    let fixture = Fixture::new(vec![
        input(SOURCE),
        api(document("existing")),
        input(&changed),
    ]);
    let result = run_with(
        &config(),
        &fixture.client(),
        RunMode::Publish,
        &AtomicBool::new(false),
        &|_| {},
        &check,
    );
    assert!(matches!(result, Err(RelayError::SourceChanged)));
    assert_eq!(fixture.request_methods(), ["GET", "GET", "GET"]);
}

#[test]
fn semantic_no_op_keeps_existing_bytes_without_patch() {
    let old = format!("# older formatting\n{}", candidate().as_str());
    let fixture = Fixture::new(vec![
        input(SOURCE),
        api(document(&old)),
        input(SOURCE),
        api(document(&old)),
    ]);
    let result = run_with(
        &config(),
        &fixture.client(),
        RunMode::Publish,
        &AtomicBool::new(false),
        &|_| {},
        &check,
    )
    .unwrap();
    assert!(result.unchanged && !result.published);
    assert_eq!(fixture.request_methods(), ["GET", "GET", "GET", "GET"]);
}

#[test]
fn changed_output_is_written_once_and_verified() {
    let candidate = candidate();
    let fixture = Fixture::new(vec![
        input(SOURCE),
        api(document("existing")),
        input(SOURCE),
        api(document("existing")),
        api(document(&candidate)),
        api(document(&candidate)),
    ]);
    let result = run_with(
        &config(),
        &fixture.client(),
        RunMode::Publish,
        &AtomicBool::new(false),
        &|_| {},
        &check,
    )
    .unwrap();
    assert!(result.published && !result.unchanged);
    assert_eq!(
        fixture.request_methods(),
        ["GET", "GET", "GET", "GET", "PATCH", "GET"]
    );
}

#[test]
fn cancellation_after_checking_cannot_start_publication() {
    let fixture = Fixture::new(vec![input(SOURCE), api(document("existing"))]);
    let cancel = AtomicBool::new(false);
    let checker = |s: &model::Subscription,
                   o: &engine::ProbeOptions,
                   c: &AtomicBool,
                   p: &(dyn Fn(usize, usize) + Sync)| {
        let result = check(s, o, c, p);
        c.store(true, std::sync::atomic::Ordering::Release);
        result
    };
    let result = run_with(
        &config(),
        &fixture.client(),
        RunMode::Publish,
        &cancel,
        &|_| {},
        &checker,
    );
    assert!(matches!(result, Err(RelayError::Cancelled)));
    assert_eq!(fixture.request_methods(), ["GET", "GET"]);
}

#[test]
fn one_healthy_node_does_not_make_an_inconclusive_peer_removable() {
    let fixture = Fixture::new(vec![
        input(SOURCE),
        api(document(SOURCE)),
        input(SOURCE),
        api(document(SOURCE)),
    ]);
    let checker = |_: &model::Subscription,
                   _: &engine::ProbeOptions,
                   _: &AtomicBool,
                   _: &(dyn Fn(usize, usize) + Sync)| {
        Ok(engine::ProbeBatch {
            health: vec![Health::Healthy { latency_ms: 5 }, Health::Unknown],
            baseline_valid: 2,
        })
    };
    let result = run_with(
        &config(),
        &fixture.client(),
        RunMode::Publish,
        &AtomicBool::new(false),
        &|_| {},
        &checker,
    )
    .unwrap();
    assert_eq!((result.healthy, result.unknown, result.removed), (1, 1, 0));
    assert_eq!(
        model::Subscription::parse(&result.content)
            .unwrap()
            .nodes()
            .len(),
        2
    );
    assert!(result.unchanged);
    assert_eq!(fixture.request_methods(), ["GET", "GET", "GET", "GET"]);
}
