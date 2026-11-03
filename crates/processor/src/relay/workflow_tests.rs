use super::*;
use gist::tests::{Fixture, ID, INPUT_ID, api, created, document, response};
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

#[test]
fn ordered_pipeline_requires_reviewed_source_and_does_not_start_network_checks() {
    use model::pipeline::{Operation, Step};
    let source = "A = http, first.invalid, 80\nB = http, second.invalid, 80\n";
    let fixture = Fixture::new(vec![]);
    let mut cfg = config();
    cfg.local_content = Some(Zeroizing::new(source.into()));
    cfg.test_nodes = false;
    cfg.processing.pipeline.push(Step {
        id: 8,
        enabled: true,
        operation: Operation::NameFilter {
            pattern: "B".into(),
            keep: true,
        },
    });
    let execute = |cfg: &RelayConfig| {
        run_with(
            cfg,
            &fixture.client(),
            RunMode::Preview,
            &AtomicBool::new(false),
            &|_| {},
            &|_, _, _, _| panic!("local pipeline must not start node checks"),
        )
    };
    assert!(matches!(execute(&cfg), Err(RelayError::InvalidInput)));
    cfg.expected_source = Some(Zeroizing::new(source.into()));
    let output = execute(&cfg).unwrap();
    assert_eq!((output.total, output.processed, output.excluded), (2, 1, 1));
    assert_eq!(
        (
            output.stages[0].id,
            output.stages[0].before,
            output.stages[0].after
        ),
        (8, 2, 1)
    );
    assert!(fixture.request_methods().is_empty());
}

#[test]
fn import_ignores_publication_and_probe_settings_and_preserves_original() {
    let fixture = Fixture::new(vec![input(SOURCE)]);
    let mut config = config();
    config.engine.clear();
    config.output = "invalid output".into();
    config.title = "x".repeat(65);
    config.rename = true;
    config.sort = true;
    let result = run_with(
        &config,
        &fixture.client(),
        RunMode::Import,
        &AtomicBool::new(false),
        &|_| {},
        &|_, _, _, _| panic!("import must not check nodes"),
    )
    .unwrap();
    assert_eq!(result.content.as_str(), SOURCE);
    assert!(!result.tested && !result.published);
    assert_eq!(
        (result.total, result.healthy, result.unknown, result.removed),
        (2, 0, 2, 0)
    );
    assert_eq!(fixture.request_methods(), ["GET"]);
}
fn input(content: &str) -> Option<String> {
    let mut doc = document(content);
    doc["id"] = serde_json::json!(INPUT_ID);
    api(doc)
}

#[test]
fn manual_processing_precedes_checks_and_keeps_origin_and_diagnostics_aligned() {
    let fixture = Fixture::new(vec![input(SOURCE)]);
    let mut cfg = config();
    cfg.expected_source = Some(Zeroizing::new(SOURCE.into()));
    cfg.processing.nodes.push(model::editor::NodeEdit {
        index: 0,
        included: false,
        name: None,
    });
    let result = run_with(
        &cfg,
        &fixture.client(),
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
        &|subscription, _, _, _| {
            assert_eq!(subscription.nodes().len(), 1);
            assert_eq!(subscription.nodes()[0].name(), "B");
            Ok(engine::ProbeBatch::from_health(
                vec![Health::Healthy { latency_ms: 3 }],
                2,
            ))
        },
    )
    .unwrap();
    assert_eq!(
        (
            result.total,
            result.processed,
            result.excluded,
            result.healthy,
            result.removed
        ),
        (2, 1, 1, 1, 1)
    );
    assert_eq!(result.details[0].reason, engine::ProbeReason::NotTested);
    assert_eq!(result.details[1].reason, engine::ProbeReason::Verified);
    assert_eq!(result.source.as_ref().unwrap().content(), SOURCE);
    assert_eq!(fixture.request_methods(), ["GET"]);
}

#[test]
fn deduplication_is_not_double_counted_as_manual_exclusion() {
    let source = "A = http, same.invalid, 80\nB = http, same.invalid, 80\n";
    let fixture = Fixture::new(vec![]);
    let mut cfg = config();
    cfg.test_nodes = false;
    cfg.local_content = Some(Zeroizing::new(source.into()));
    cfg.expected_source = Some(Zeroizing::new(source.into()));
    cfg.processing.deduplicate = true;
    let result = run_with(
        &cfg,
        &fixture.client(),
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
        &|_, _, _, _| panic!("local preview must not check nodes"),
    )
    .unwrap();
    assert_eq!(
        (
            result.total,
            result.processed,
            result.excluded,
            result.duplicates,
            result.removed
        ),
        (2, 1, 0, 1, 1)
    );
    assert!(fixture.request_methods().is_empty());
}

#[test]
fn a_healthy_dependent_removed_with_its_failed_parent_cannot_authorize_publication() {
    let source = "A = http, a.invalid, 80\nB = http, b.invalid, 80, underlying-proxy=A\nC = http, c.invalid, 80\n";
    let fixture = Fixture::new(vec![input(source)]);
    let mut cfg = config();
    cfg.output.clear();
    let result = run_with(
        &cfg,
        &fixture.client(),
        RunMode::Publish,
        &AtomicBool::new(false),
        &|_| {},
        &|_, _, _, _| {
            Ok(engine::ProbeBatch::from_health(
                vec![
                    Health::Failed,
                    Health::Healthy { latency_ms: 3 },
                    Health::Unknown,
                ],
                2,
            ))
        },
    );
    assert!(matches!(result, Err(RelayError::NoHealthyNodes)));
    assert_eq!(fixture.request_methods(), ["GET"]);
}

#[test]
fn excluded_healthy_nodes_cannot_authorize_publication_of_unverified_candidates() {
    let fixture = Fixture::new(vec![input(SOURCE)]);
    let mut cfg = config();
    cfg.output.clear();
    cfg.expected_source = Some(Zeroizing::new(SOURCE.into()));
    cfg.processing.nodes.push(model::editor::NodeEdit {
        index: 0,
        included: false,
        name: None,
    });
    let result = run_with(
        &cfg,
        &fixture.client(),
        RunMode::Publish,
        &AtomicBool::new(false),
        &|_| {},
        &|subscription, _, _, _| {
            assert_eq!(subscription.nodes().len(), 1);
            Ok(engine::ProbeBatch::from_health(vec![Health::Unknown], 2))
        },
    );
    assert!(matches!(result, Err(RelayError::NoHealthyNodes)));
    assert_eq!(fixture.request_methods(), ["GET"]);
}

#[test]
fn edited_preview_is_local_and_a_changed_remote_source_is_rejected_before_checks() {
    let fixture = Fixture::new(vec![]);
    let mut cfg = config();
    cfg.test_nodes = false;
    cfg.expected_source = Some(Zeroizing::new(SOURCE.into()));
    cfg.local_content = Some(Zeroizing::new(SOURCE.into()));
    cfg.processing.find = "A".into();
    cfg.processing.replace = "Edited".into();
    let result = run_with(
        &cfg,
        &fixture.client(),
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
        &|_, _, _, _| panic!("local preview cannot check nodes"),
    )
    .unwrap();
    assert!(!result.tested);
    assert_eq!(
        model::Subscription::parse(&result.content).unwrap().nodes()[0].name(),
        "Edited"
    );
    assert!(fixture.request_methods().is_empty());
    cfg.local_content = None;
    cfg.test_nodes = true;
    let changed = SOURCE.replace("a.example.invalid", "changed.example.invalid");
    let fixture = Fixture::new(vec![input(&changed)]);
    let result = run_with(
        &cfg,
        &fixture.client(),
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
        &|_, _, _, _| panic!("stale snapshot cannot be checked"),
    );
    assert!(matches!(result, Err(RelayError::SourceChanged)));
    assert_eq!(fixture.request_methods(), ["GET"]);
}

#[test]
fn all_failed_preview_returns_diagnostics_but_no_exportable_empty_subscription() {
    let fixture = Fixture::new(vec![input(SOURCE)]);
    let result = run_with(
        &config(),
        &fixture.client(),
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
        &|_, _, _, _| Ok(engine::ProbeBatch::from_health(vec![Health::Failed; 2], 2)),
    )
    .unwrap();
    assert_eq!((result.total, result.failed, result.removed), (2, 2, 2));
    assert!(result.content.is_empty());
    assert_eq!(result.details.len(), 2);
    assert_eq!(fixture.request_methods(), ["GET"]);
}

#[test]
fn new_node_formats_complete_raw_import_cleanup_creation_and_backup() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let uri = "trojan://synthetic@a.invalid:443#A\ntrojan://synthetic@b.invalid:443#B\n";
    for source in [
        uri.to_owned(), STANDARD.encode(uri),
        "proxies:\n - {name: A, type: http, server: a.invalid, port: 80}\n - {name: B, type: http, server: b.invalid, port: 80}\n".into(),
        r#"{"outbounds":[{"tag":"A","type":"http","server":"a.invalid","server_port":80},{"tag":"B","type":"http","server":"b.invalid","server_port":80}],"endpoints":[]}"#.into(),
    ] {
        let cleaned = model::Subscription::parse(&source).unwrap()
            .render(&[Health::Healthy { latency_ms: 5 }, Health::Failed],
                &model::EnhanceOptions { rename: false, sort: false }).unwrap();
        let fixture = Fixture::new(vec![
            Some(response(200, "Content-Type: text/plain\r\n", &source)),
            Some(response(200, "Content-Type: text/plain\r\n", &source)),
            created(document(&cleaned)), api(document(&cleaned)),
        ]);
        let mut config = config();
        config.source = format!("https://gist.githubusercontent.com/example/{INPUT_ID}/raw/nodes.txt");
        config.output.clear();
        let result = run_with(&config, &fixture.client(), RunMode::Publish,
            &AtomicBool::new(false), &|_| {}, &check).unwrap();
        assert!(result.published && result.created.is_some());
        assert_eq!((result.total, result.healthy, result.removed), (2, 1, 1));
        assert_eq!(fixture.request_methods(), ["GET", "GET", "POST", "GET"]);
        let restored = backup::decode(&backup::encode(&config, &result.content).unwrap()).unwrap();
        assert_eq!(&*restored.content, &*cleaned);
    }
}
fn check(
    _: &model::Subscription,
    _: &engine::ProbeOptions,
    _: &AtomicBool,
    _: &(dyn Fn(usize, usize) + Sync),
) -> Result<engine::ProbeBatch, engine::EngineError> {
    Ok(engine::ProbeBatch::from_health(
        vec![Health::Healthy { latency_ms: 5 }, Health::Failed],
        2,
    ))
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
        &|_, _, _, _| Ok(engine::ProbeBatch::from_health(vec![Health::Unknown; 2], 2)),
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
            Ok(engine::ProbeBatch::from_health(vec![health; 2], 2))
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
fn raw_source_changed_before_creation_never_posts() {
    let changed = SOURCE.replace("a.example.invalid", "changed.example.invalid");
    let fixture = Fixture::new(vec![
        Some(response(200, "Content-Type: text/plain\r\n", SOURCE)),
        Some(response(200, "Content-Type: text/plain\r\n", &changed)),
    ]);
    let mut config = config();
    config.source = format!("https://gist.githubusercontent.com/example/{INPUT_ID}/raw/nodes.txt");
    config.output.clear();
    let result = run_with(
        &config,
        &fixture.client(),
        RunMode::Publish,
        &AtomicBool::new(false),
        &|_| {},
        &check,
    );
    assert!(matches!(result, Err(RelayError::SourceChanged)));
    assert_eq!(fixture.request_methods(), ["GET", "GET"]);
}

#[test]
fn prepared_raw_result_uploads_without_rechecking_nodes() {
    let cleaned = candidate();
    let fixture = Fixture::new(vec![
        Some(response(200, "Content-Type: text/plain\r\n", SOURCE)),
        Some(response(200, "Content-Type: text/plain\r\n", SOURCE)),
        created(document(&cleaned)),
        api(document(&cleaned)),
    ]);
    let mut config = config();
    config.source = format!("https://gist.githubusercontent.com/example/{INPUT_ID}/raw/nodes.txt");
    config.output.clear();
    let prepared = run_with(
        &config,
        &fixture.client(),
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
        &check,
    )
    .unwrap();
    config.engine.clear();
    let result = upload_with(
        &config,
        &fixture.client(),
        prepared,
        &AtomicBool::new(false),
        &|_| {},
    )
    .unwrap();
    assert_eq!(result.content.as_str(), cleaned.as_str());
    assert_eq!(
        (result.total, result.healthy, result.failed, result.removed),
        (2, 1, 1, 1)
    );
    assert!(result.tested && result.published);
    assert_eq!(fixture.request_methods(), ["GET", "GET", "POST", "GET"]);
}

#[test]
fn prepared_upload_exports_the_selected_format_without_mutating_its_preview() {
    use export::{OutputFormat, export_subscription};

    let source = "A = socks5, a.example.invalid, 1080\n";
    let mut config = config();
    config.source.clear();
    config.output.clear();
    config.local_content = Some(Zeroizing::new(source.into()));
    config.test_nodes = false;
    config.output_format = OutputFormat::Mihomo;
    let exported = export_subscription(source, config.output_format).unwrap();
    assert_ne!(source, exported.as_str());
    let fixture = Fixture::new(vec![created(document(&exported)), api(document(&exported))]);
    let cancel = AtomicBool::new(false);
    let prepared = run_with(
        &config,
        &fixture.client(),
        RunMode::Preview,
        &cancel,
        &|_| {},
        &check,
    )
    .unwrap();
    assert_eq!(prepared.content.as_str(), source);
    let uploaded = upload_with(&config, &fixture.client(), prepared, &cancel, &|_| {}).unwrap();
    assert!(uploaded.published && uploaded.created.is_some());
    assert_eq!(uploaded.content.as_str(), source);
    assert_eq!(fixture.request_methods(), ["POST", "GET"]);
}

#[test]
fn conversion_failure_cannot_begin_a_gist_write() {
    let mut config = config();
    config.source.clear();
    config.output.clear();
    config.local_content = Some(Zeroizing::new(SOURCE.into()));
    config.test_nodes = false;
    config.output_format = export::OutputFormat::Mihomo;
    let fixture = Fixture::new(vec![]);
    let cancel = AtomicBool::new(false);
    let prepared = run_with(
        &config,
        &fixture.client(),
        RunMode::Preview,
        &cancel,
        &|_| {},
        &check,
    )
    .unwrap();
    for attempt in [
        run_with(
            &config,
            &fixture.client(),
            RunMode::Publish,
            &cancel,
            &|_| {},
            &check,
        ),
        upload_with(&config, &fixture.client(), prepared, &cancel, &|_| {}),
    ] {
        assert!(matches!(
            attempt,
            Err(RelayError::ExportNode { index: 1, .. })
        ));
    }
    assert!(fixture.request_methods().is_empty());
}

#[test]
fn original_gist_upload_rejects_missing_dependencies_before_any_request() {
    let mut config = config();
    config.source.clear();
    config.output.clear();
    config.local_content = Some(Zeroizing::new(
        "Alpha = socks5, node.example.invalid, 1080, underlying-proxy=Missing\n".into(),
    ));
    config.test_nodes = false;
    let fixture = Fixture::new(vec![]);
    let cancel = AtomicBool::new(false);
    let imported = run_with(
        &config,
        &fixture.client(),
        RunMode::Import,
        &cancel,
        &|_| {},
        &check,
    )
    .unwrap();
    assert!(matches!(
        upload_with(&config, &fixture.client(), imported, &cancel, &|_| {}),
        Err(RelayError::Model(model::ModelError::InvalidDependency))
    ));
    assert!(fixture.request_methods().is_empty());
}

#[test]
fn an_explicit_format_replaces_an_equivalent_old_dialect_but_is_then_a_no_op() {
    use export::{OutputFormat, export_subscription};

    let source = "A = socks5, a.example.invalid, 1080\n";
    let mut config = config();
    config.source.clear();
    config.local_content = Some(Zeroizing::new(source.into()));
    config.test_nodes = false;
    config.output_format = OutputFormat::Mihomo;
    let exported = export_subscription(source, config.output_format).unwrap();
    let fixture = Fixture::new(vec![
        api(document(source)),
        api(document(source)),
        api(document(&exported)),
        api(document(&exported)),
        api(document(&exported)),
        api(document(&exported)),
    ]);
    let execute = || {
        run_with(
            &config,
            &fixture.client(),
            RunMode::Publish,
            &AtomicBool::new(false),
            &|_| {},
            &check,
        )
        .unwrap()
    };
    assert!(execute().published);
    let repeated = execute();
    assert!(repeated.unchanged && !repeated.published);
    assert_eq!(
        fixture.request_methods(),
        ["GET", "GET", "PATCH", "GET", "GET", "GET"]
    );
}

#[test]
fn prepared_raw_result_refuses_changed_source_before_post() {
    let changed = SOURCE.replace("a.example.invalid", "changed.example.invalid");
    let fixture = Fixture::new(vec![
        Some(response(200, "Content-Type: text/plain\r\n", SOURCE)),
        Some(response(200, "Content-Type: text/plain\r\n", &changed)),
    ]);
    let mut config = config();
    config.source = format!("https://gist.githubusercontent.com/example/{INPUT_ID}/raw/nodes.txt");
    config.output.clear();
    let prepared = run_with(
        &config,
        &fixture.client(),
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
        &check,
    )
    .unwrap();
    assert!(matches!(
        upload_with(
            &config,
            &fixture.client(),
            prepared,
            &AtomicBool::new(false),
            &|_| {}
        ),
        Err(RelayError::SourceChanged)
    ));
    assert_eq!(fixture.request_methods(), ["GET", "GET"]);
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
        Ok(engine::ProbeBatch::from_health(
            vec![Health::Healthy { latency_ms: 5 }, Health::Unknown],
            2,
        ))
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
