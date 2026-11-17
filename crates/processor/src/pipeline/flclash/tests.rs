use super::{fields, render, run_generator, validate_generated};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn body_rewrites_share_serialization_and_independent_publication_checks() {
    let raw =
        r#"http-response ^https://example\.test/cms offset_ms":\d+ offset_ms":99999999999999"#;
    assert!(
        super::validate_native_rewrites(&fixture(&format!("[Body Rewrite]\n{raw}\n"))).is_err()
    );
    let result = render(&fixture(&format!("[Body Rewrite]\n{raw}\n"))).unwrap();
    super::validate_native_rewrites(&result.content).unwrap();
    let row = result
        .content
        .lines()
        .find(|line| line.starts_with("http-response "))
        .unwrap();
    assert_eq!(
        crate::rewrite_syntax::BodyRewrite::parse_source(raw).unwrap(),
        crate::rewrite_syntax::BodyRewrite::parse_strict(row).unwrap()
    );
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.disposition != "omitted")
    );
}

#[test]
fn malformed_body_rewrite_cannot_be_silently_published() {
    for row in [
        "http-response ^https://example.test search",
        "http-response ^https://example.test [bad value",
    ] {
        assert!(render(&fixture(&format!("[Body Rewrite]\n{row}\n"))).is_err());
    }
}

#[test]
fn map_local_json_remains_valid_after_native_field_decoding() {
    let row = r#"^https://example\.test/json data-type=text data="{"status":"SUCCESS"}" header="Content-Type:application/json""#;
    assert!(super::validate_native_rewrites(&fixture(&format!("[Map Local]\n{row}\n"))).is_err());
    let result = render(&fixture(&format!("[Map Local]\n{row}\n"))).unwrap();
    super::validate_native_rewrites(&result.content).unwrap();
    let row = result
        .content
        .lines()
        .find(|line| line.starts_with("^https:"))
        .unwrap();
    let fields = crate::rewrite_syntax::parse_strict(row).unwrap();
    let data = fields
        .iter()
        .find_map(|field| field.strip_prefix("data="))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(data).unwrap(),
        serde_json::json!({"status":"SUCCESS"})
    );
}

#[test]
fn multiple_body_replacement_pairs_remain_in_one_entry() {
    let row = r#"http-response ^https://example\.test "ad":true "ad":false offset_ms":\d+ offset_ms":99999999999999"#;
    let result = render(&fixture(&format!("[Body Rewrite]\n{row}\n"))).unwrap();
    super::validate_native_rewrites(&result.content).unwrap();
    assert_eq!(result.sections["Body Rewrite"], 1);
    let target = result
        .content
        .lines()
        .find(|line| line.starts_with("http-response "))
        .unwrap();
    assert_eq!(
        crate::rewrite_syntax::BodyRewrite::parse_source(row).unwrap(),
        crate::rewrite_syntax::BodyRewrite::parse_strict(target).unwrap()
    );
}
fn fixture(body: &str) -> String {
    format!("#!name=Synthetic fixture\n{body}")
}
fn temporary() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "flclash-projector-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(path.join("modules/surge/head_expanse")).unwrap();
    path
}
#[test]
fn comma_parser_preserves_quoted_regex_and_nested_logical_groups() {
    assert_eq!(
        fields(r#"URL-REGEX,"^https://example\.test/a{1,3}",REJECT"#).unwrap()[1],
        r"^https://example\.test/a{1,3}"
    );
    assert_eq!(
        fields("AND,((DOMAIN,example.test),(PROTOCOL,TCP)),REJECT")
            .unwrap()
            .len(),
        3
    );
    assert!(fields("AND,((DOMAIN,a),REJECT").is_none());
}
#[test]
fn projects_supported_rules_and_reports_lost_options() {
    let source = fixture(
        "[Rule]\nDOMAIN,ads.example.test,REJECT,extended-matching\nIP-CIDR,192.0.2.0/24,REJECT,no-resolve,pre-matching\nGEOIP,US,REJECT,no-resolve\nURL-REGEX,\"^https://example\\.test/a{1,3}\",REJECT\n",
    );
    let result = render(&source).unwrap();
    assert!(result.content.contains("DOMAIN,ads.example.test,REJECT\n"));
    assert!(
        result
            .content
            .contains("IP-CIDR,192.0.2.0/24,REJECT,no-resolve")
    );
    assert!(!result.content.contains("GEOIP,"));
    assert!(
        result
            .content
            .contains("^https://example\\.test/a{1,3} reject")
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.reason.contains("extended-matching"))
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.reason.contains("GEOIP"))
    );
}
#[test]
fn local_inline_fallbacks_are_expanded_and_external_lists_remain_reported() {
    let source = fixture(
        "[Rule]\nRULE-SET,Local group,REJECT,no-resolve\nRULE-SET,https://example.test/provider.list,REJECT\n[Ruleset Local group]\nIP-CIDR,192.0.2.0/24,no-resolve\nDEST-PORT,443\n",
    );
    let result = render(&source).unwrap();
    assert!(
        result
            .content
            .contains("IP-CIDR,192.0.2.0/24,REJECT,no-resolve")
    );
    assert!(result.content.contains("DEST-PORT,443,REJECT"));
    assert!(!result.content.contains("RULE-SET,"));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.reason.contains("external RULE-SET"))
    );
}
#[test]
fn inline_rules_inherit_parent_no_resolve_only_for_ip_matchers() {
    let source = fixture(
        "[Rule]\nRULE-SET,Local group,REJECT,no-resolve\n[Ruleset Local group]\nIP-CIDR,192.0.2.0/24\nIP-CIDR6,2001:db8::/32\nIP-CIDR,198.51.100.0/24,no-resolve\nDOMAIN,ads.example.test\nDEST-PORT,443\n",
    );
    let result = render(&source).unwrap();
    assert!(
        result
            .content
            .contains("IP-CIDR,192.0.2.0/24,REJECT,no-resolve\n")
    );
    assert!(
        result
            .content
            .contains("IP-CIDR6,2001:db8::/32,REJECT,no-resolve\n")
    );
    assert!(
        result
            .content
            .contains("IP-CIDR,198.51.100.0/24,REJECT,no-resolve\n")
    );
    assert!(!result.content.contains("no-resolve,no-resolve"));
    assert!(result.content.contains("DOMAIN,ads.example.test,REJECT\n"));
    assert!(result.content.contains("DEST-PORT,443,REJECT\n"));
}

#[test]
fn unsupported_inline_parent_options_are_reported_once_per_reference() {
    let source = fixture(
        "[Rule]\nRULE-SET,Local group,REJECT,no-resolve,pre-matching\n[Ruleset Local group]\nIP-CIDR,192.0.2.0/24\nIP-CIDR,198.51.100.0/24\n",
    );
    let result = render(&source).unwrap();
    let diagnostics = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.reason.starts_with("parent RULE-SET options"))
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].section, "Rule");
    assert_eq!(diagnostics[0].disposition, "adapted");
    assert_eq!(
        diagnostics[0].reason,
        "parent RULE-SET options are not preserved: pre-matching"
    );
    assert!(!result.content.contains("pre-matching"));
    assert!(
        result
            .content
            .contains("IP-CIDR,192.0.2.0/24,REJECT,no-resolve\n")
    );
    assert!(
        result
            .content
            .contains("IP-CIDR,198.51.100.0/24,REJECT,no-resolve\n")
    );
}

#[test]
fn scripts_keep_native_flags_but_do_not_silently_forward_unknown_fields() {
    let source = fixture(
        "[Script]\nBody = type=http-response, pattern=^https://example\\.test, script-path=https://example.test/a.js, requires-body=true, max-size=1024, timeout=10, argument=mode=ad, engine=webview\nGeneric = type=generic, script-path=https://example.test/b.js\nOff = type=http-request, pattern=^https://off\\.test, script-path=https://example.test/off.js, enable=false\n",
    );
    let result = render(&source).unwrap();
    assert!(
        result
            .content
            .contains("requires-body=true, max-size=1024, timeout=10, argument=mode=ad")
    );
    assert!(result.content.contains("Generic = type=generic"));
    assert!(!result.content.contains("Off ="));
    assert!(!result.content.contains("engine=webview"));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.reason.contains("engine"))
    );
}
#[test]
fn maplocal_translates_tinygif_and_rejects_unattached_local_resources() {
    let source = fixture(
        "[Map Local]\n^https://example\\.test/tiny data-type=tiny-gif\n^https://example\\.test/json data-type=text data=\"{}\" status-code=200\n^https://example\\.test/local data-type=file data=\"missing.json\"\n",
    );
    let result = render(&source).unwrap();
    assert!(result.content.contains("data-type=base64 data=\"R0lGODlh"));
    assert!(result.content.contains("data-type=text data=\"{}\""));
    assert!(!result.content.contains("missing.json"));
    assert_eq!(result.sections["Map Local"], 2);
}
#[test]
fn global_settings_and_jq_are_explicitly_omitted() {
    let source = fixture(
        "[General]\nforce-http-engine-hosts = %APPEND% example.test\n[Body Rewrite]\nhttp-response-jq ^https://example\\.test .ad=false\nhttp-response ^https://example\\.test \"ad\":true \"ad\":false\n[MITM]\nhostname = %APPEND% example.test\n",
    );
    let result = render(&source).unwrap();
    assert!(!result.content.contains("[General]"));
    assert!(!result.content.contains("http-response-jq"));
    assert!(result.content.contains("http-response ^https://example"));
    assert!(result.diagnostics.iter().any(|d| d.reason.contains("jq")));
}
#[test]
fn output_and_catalog_are_deterministic_and_tampering_is_detected() {
    let root = temporary();
    fs::write(root.join("modules/surge/head_expanse/Test.sgmodule"),fixture("[URL Rewrite]\n^https://example\\.test reject\n[MITM]\nhostname = %APPEND% example.test\n")).unwrap();
    assert_eq!(run_generator(&root).unwrap(), 2);
    assert_eq!(validate_generated(&root).unwrap(), 3);
    let catalog = root.join("modules/flclash/catalog.json");
    let first = fs::read(&catalog).unwrap();
    run_generator(&root).unwrap();
    assert_eq!(first, fs::read(&catalog).unwrap());
    let value: serde_json::Value = serde_json::from_slice(&first).unwrap();
    assert_eq!(value["modules"][0]["projection"]["active_lines"], 1);
    fs::write(
        root.join("modules/flclash/head_expanse/Test.sgmodule"),
        "tampered",
    )
    .unwrap();
    assert!(validate_generated(&root).is_err());
    assert!(run_generator(&root).is_err());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn host_only_sources_are_reported_without_misleading_empty_modules() {
    let root = temporary();
    fs::write(
        root.join("modules/surge/head_expanse/Host.sgmodule"),
        fixture("[Host]\nexample.test = 192.0.2.1\n[MITM]\nhostname = %APPEND% example.test\n"),
    )
    .unwrap();
    assert_eq!(run_generator(&root).unwrap(), 0);
    let value: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("modules/flclash/catalog.json")).unwrap(),
    )
    .unwrap();
    assert!(value["modules"][0]["path"].is_null());
    assert_eq!(value["modules"][0]["projection"]["active_lines"], 0);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn native_argument_definitions_and_quoted_commas_are_preserved() {
    let source = "#!name=Defaults fixture\n#!arguments=Types:\"Official,Translate\",Job:\"# Disabled job\",cron:0 0 * * *,Switch:false\n#!arguments-desc=Native parameters\n[Script]\nBody = type=http-response, pattern=^https://example\\.test, script-path=https://example.test/a.js, argument=\"Types={{{Types}}}\", enable={{{Switch}}}\n{{{Job}}} = type=cron, cronexp=\"{{{cron}}}\", script-path=https://example.test/b.js\nUnknown = type=http-request, pattern=^https://example\\.test/c, script-path=https://example.test/c.js, argument={{{Missing}}}\n";
    let result = render(source).unwrap();
    assert!(
        result
            .content
            .contains("#!arguments=Types:\"Official,Translate\"")
    );
    assert!(
        result
            .content
            .contains("#!arguments-desc=Native parameters")
    );
    assert!(
        result
            .content
            .contains("argument=\"Types={{{Types}}}\", enable={{{Switch}}}")
    );
    assert!(result.content.contains("{{{Job}}} = type=cron"));
    assert!(!result.content.contains("Unknown ="));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.reason.contains("no public source default"))
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.disposition == "parameterized")
    );
}
#[test]
fn regex_character_class_parenthesis_and_literal_apostrophe_remain_intact() {
    assert_eq!(
        fields("URL-REGEX,^https://example.test/[)]/ad,REJECT").unwrap()[1],
        "^https://example.test/[)]/ad"
    );
    assert_eq!(
        fields("URL-REGEX,^https://example.test/'ads',REJECT").unwrap()[1],
        "^https://example.test/'ads'"
    );
    let parts = fields("AND,((DOMAIN,\"a,b\"),(PROTOCOL,TCP)),REJECT").unwrap();
    assert!(parts[1].contains("\"a,b\""));
}

#[test]
fn duplicate_script_keys_are_rejected_instead_of_changing_last_wins_semantics() {
    let source = fixture(
        "[Script]\nAmbiguous = type=http-request, pattern=^https://example\\.test, script-path=https://example.test/a.js, enable=true, enable=false\n",
    );
    let result = render(&source).unwrap();
    assert!(!result.content.contains("Ambiguous ="));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.reason.contains("duplicate script parameter"))
    );
}

#[test]
fn quoted_tinygif_type_is_normalized_and_existing_data_is_not_overwritten() {
    let source = fixture(
        "[Map Local]\n^https://example\\.test/tiny data-type=\"tiny-gif\" header=\"X-Existing:yes\"\n^https://example\\.test/data data-type=tiny-gif data=\"original\"\n",
    );
    let result = render(&source).unwrap();
    assert!(result.content.contains("data-type=base64"));
    let row = result
        .content
        .lines()
        .find(|line| line.starts_with("^https:"))
        .unwrap();
    assert!(
        crate::rewrite_syntax::parse_strict(row)
            .unwrap()
            .iter()
            .any(|field| field == "header=X-Existing:yes")
    );
    assert!(!result.content.contains("data=\"original\""));
    assert_eq!(result.sections["Map Local"], 1);
}

#[test]
fn dropping_the_only_http_handler_does_not_leave_its_mitm_hostname() {
    let source = fixture(
        "[Rule]\nDOMAIN,ads.example.test,REJECT\n[Body Rewrite]\nhttp-response-jq ^https://unused\\.example\\.test .ad=false\n[URL Rewrite]\n^https://used\\.example\\.test/ad reject\n[MITM]\nhostname = %INSERT% -excluded.example.test, unused.example.test, used.example.test\n",
    );
    let result = render(&source).unwrap();
    assert!(
        result
            .content
            .contains("hostname = %INSERT% -excluded.example.test, used.example.test")
    );
    assert!(!result.content.contains(", unused.example.test"));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.reason.contains("unreferenced hostname"))
    );
}

#[test]
fn current_public_sources_project_to_native_modules_without_unsupported_sections() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let planned = super::plan(&root).unwrap();
    let main = root.join("modules/flclash/head_expanse/ScriptHub.AdBlock.Preview.sgmodule");
    let text = &planned[&main];
    assert!(text.contains("[Script]"));
    assert!(text.contains("[Map Local]"));
    assert!(text.contains("[URL Rewrite]"));
    assert!(!text.contains("[IP Rewrite]"));
    assert!(!text.contains("RULE-SET,"));
    for (path, text) in &planned {
        if path.extension().is_some_and(|e| e == "sgmodule") {
            for (section, _) in super::parse_sections(text) {
                assert!(super::SECTIONS.contains(&section.as_str()), "{section}");
            }
            let defaults = super::argument_defaults(text).unwrap();
            for line in text.lines().filter(|line| !line.starts_with('#')) {
                assert!(
                    super::resolve_defaults(line, &defaults).is_some(),
                    "unresolved argument in {}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn tinygif_normalization_preserves_quoted_pattern_boundaries() {
    let output = super::map_local(r#""^https://example.test/[ ]" data-type="tiny-gif""#).unwrap();
    assert!(output.starts_with(r#""^https://example.test/[ ]" data-type=base64"#));
}

#[test]
fn promax_routing_sources_allow_only_safe_public_repository_lists() {
    for prefix in [
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/",
        "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/",
    ] {
        assert_eq!(
            super::public_adblock_path(&format!("{prefix}AdBlock_Local.list")).unwrap(),
            PathBuf::from("rulesets/AdBlock/AdBlock_Local.list")
        );
    }
    for url in [
        "https://example.test/AdBlock_Local.list",
        "https://secret@raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Local.list",
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/../AdBlock_Local.list",
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_../private.list",
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_%2e%2e.list",
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_Local.list?token=synthetic",
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_Local.list#fragment",
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_\\private.list",
    ] {
        assert!(super::public_adblock_path(url).is_err());
    }
}

#[test]
fn promax_routing_preserves_supported_rules_and_reports_source_options_once() {
    let projection = super::render_routing(
        "DOMAIN,ads.example.test\nIP-CIDR,192.0.2.0/24\nDEST-PORT,1234\nURL-REGEX,\"^https://example\\.test/a{1,3}\"\nUSER-AGENT,SyntheticAgent\n",
        "AdBlock_Synthetic",
        "REJECT",
        &["no-resolve".into(), "pre-matching".into(), "extended-matching".into()],
    ).unwrap();
    assert_eq!(projection.input_lines, 5);
    assert_eq!(projection.sections["Rule"], 3);
    assert_eq!(projection.sections["URL Rewrite"], 1);
    assert!(
        projection
            .content
            .contains("DOMAIN,ads.example.test,REJECT\n")
    );
    assert!(
        projection
            .content
            .contains("IP-CIDR,192.0.2.0/24,REJECT,no-resolve\n")
    );
    assert!(projection.content.contains("DEST-PORT,1234,REJECT\n"));
    assert!(
        projection
            .content
            .contains("^https://example\\.test/a{1,3} reject\n")
    );
    assert!(!projection.content.contains("[MITM]"));
    assert!(!projection.content.contains("[Script]"));
    assert_eq!(
        projection
            .diagnostics
            .iter()
            .filter(|d| d.reason.starts_with("parent RULE-SET"))
            .count(),
        1
    );
    assert!(
        projection
            .diagnostics
            .iter()
            .any(|d| d.reason.contains("USER-AGENT"))
    );
    let direct =
        super::render_routing("DOMAIN,allowed.example.test\n", "Synthetic", "DIRECT", &[]).unwrap();
    assert!(
        direct
            .content
            .contains("DOMAIN,allowed.example.test,DIRECT")
    );
    assert!(
        super::render_routing(
            "DOMAIN,ads.example.test\n",
            "Synthetic",
            "PrivateGroup",
            &[]
        )
        .is_err()
    );
}

#[test]
fn promax_routing_retains_registered_notices_without_private_profile_material() {
    let root = temporary();
    fs::create_dir_all(root.join("rulesets/AdBlock")).unwrap();
    fs::write(
        root.join("rulesets/AdBlock/AdBlock_Synthetic.list"),
        "DOMAIN,ads.example.test\n",
    )
    .unwrap();
    let source = fixture(
        "[Rule]\nRULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Synthetic.list,REJECT\n",
    );
    let routing = super::plan_promax_routing(&root, &source).unwrap();
    let projection = super::render_embedded(&source, &routing).unwrap();
    for (owner, notice) in super::REGISTERED_MIT_NOTICES {
        assert!(
            projection
                .content
                .contains(&format!("# Registered MIT notice: {owner}\n"))
        );
        assert_eq!(
            projection
                .content
                .matches(&format!("# Registered MIT notice: {owner}\n"))
                .count(),
            1
        );
        for line in notice.lines().filter(|line| !line.is_empty()) {
            assert!(projection.content.contains(&format!("# {line}\n")));
        }
    }
    assert!(
        projection
            .content
            .contains("Copyright 2021 Adguard Software Ltd")
    );
    assert!(projection.content.contains("Copyright (c) 2023 app2smile"));
    assert!(
        projection
            .content
            .contains("permission notice shall be included")
    );
    assert!(projection.content.contains("catalog.json (promax_routing)"));
    assert!(projection.content.contains("# Modified:"));
    assert!(
        projection
            .content
            .contains("not every source or license applies to every source rule")
    );
    for material in [
        "ss://",
        "vmess://",
        "vless://",
        "trojan://",
        "proxies:",
        "proxy-providers:",
        "{{SUBSCRIPTION_",
    ] {
        assert!(!projection.content.contains(material));
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn promax_routing_plan_keeps_declaration_order_and_reports_empty_shards() {
    let root = temporary();
    fs::create_dir_all(root.join("rulesets/AdBlock")).unwrap();
    fs::write(
        root.join("rulesets/AdBlock/AdBlock_Z.list"),
        "DOMAIN,z.example.test\n",
    )
    .unwrap();
    fs::write(
        root.join("rulesets/AdBlock/AdBlock_Empty.list"),
        "USER-AGENT,Synthetic\n",
    )
    .unwrap();
    fs::write(
        root.join("rulesets/AdBlock/AdBlock_A.list"),
        "DOMAIN,a.example.test\n",
    )
    .unwrap();
    let source = fixture(
        "[Rule]\nRULE-SET,Local inline,REJECT\nRULE-SET,https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_Z.list,REJECT,pre-matching\nRULE-SET,https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_Empty.list,REJECT\nRULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_A.list,DIRECT\n[Ruleset Local inline]\nDOMAIN,inline.example.test\n",
    );
    let reports = super::plan_promax_routing(&root, &source).unwrap();
    assert_eq!(reports.len(), 3);
    assert!(reports[0].source.ends_with("AdBlock_Z.list"));
    assert!(reports[1].source.ends_with("AdBlock_Empty.list"));
    assert!(reports[2].source.ends_with("AdBlock_A.list"));
    assert!(reports[1].embedded_in.is_none());
    assert_eq!(reports[1].projection.active_lines, 0);
    assert!(reports[1].source_bytes > 0);
    assert_eq!(reports[1].projected_bytes, 0);
    assert_eq!(reports[0].source_bytes, "DOMAIN,z.example.test\n".len());
    assert_eq!(
        reports[0].projected_bytes,
        reports[0].projection.content.len()
    );
    assert_eq!(reports[0].embedded_in.as_deref(), Some(super::MAIN_MODULE));
    assert_eq!(reports[2].embedded_in.as_deref(), Some(super::MAIN_MODULE));
    let before = reports[0].projection.content.clone();
    fs::write(
        root.join("rulesets/AdBlock/AdBlock_Z.list"),
        "DOMAIN,changed.example.test\n",
    )
    .unwrap();
    let refreshed = super::plan_promax_routing(&root, &source).unwrap();
    assert_ne!(refreshed[0].projection.content, before);
    assert!(
        super::plan_promax_routing(
            &root,
            &fixture("[Rule]\nRULE-SET,https://example.test/untrusted.list,REJECT\n")
        )
        .is_err()
    );
    assert!(
        super::read_public_bounded(&root, &root.join("rulesets/AdBlock/AdBlock_Z.list"), 8)
            .is_err()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn full_public_plan_is_identical_for_lf_and_crlf_worktrees() {
    let root = temporary();
    let source_path = root.join("modules/surge/head_expanse").join(super::PROMAX);
    let ruleset_path = root.join("rulesets/AdBlock/AdBlock_Synthetic.list");
    fs::create_dir_all(ruleset_path.parent().unwrap()).unwrap();
    let source = fixture(
        "[Rule]\nDOMAIN,inline.example.test,REJECT\nRULE-SET,https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_Synthetic.list,REJECT,no-resolve\n",
    );
    let ruleset = "# Synthetic public list\nDOMAIN,ads.example.test\nIP-CIDR,192.0.2.0/24\n";
    fs::write(&source_path, &source).unwrap();
    fs::write(&ruleset_path, ruleset).unwrap();
    let lf = super::plan(&root).unwrap();

    fs::write(&source_path, source.replace('\n', "\r\n")).unwrap();
    fs::write(&ruleset_path, ruleset.replace('\n', "\r\n")).unwrap();
    let crlf = super::plan(&root).unwrap();
    // Includes both module variants, routing content and the complete catalog
    // with source fingerprints and source/output byte counts.
    assert_eq!(lf, crlf);
    let catalog: serde_json::Value =
        serde_json::from_str(&crlf[&root.join("modules/flclash/catalog.json")]).unwrap();
    assert_eq!(catalog["promax_routing"][0]["source_bytes"], ruleset.len());
    assert_eq!(
        catalog["promax_routing"][0]["source_md5_change_fingerprint"],
        super::fingerprint(ruleset)
    );
    // Normalization must not bypass the raw on-disk byte limit.
    assert!(super::read_public_bounded(&root, &ruleset_path, ruleset.len()).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn merged_promax_keeps_source_positions_and_deduplicates_only_exact_rows() {
    let root = temporary();
    fs::create_dir_all(root.join("rulesets/AdBlock")).unwrap();
    fs::write(
        root.join("rulesets/AdBlock/AdBlock_One.list"),
        "DOMAIN,first.example.test\nDOMAIN,shared.example.test\nIP-CIDR,192.0.2.0/24\n",
    )
    .unwrap();
    fs::write(
        root.join("rulesets/AdBlock/AdBlock_Two.list"),
        "DOMAIN,shared.example.test\nDOMAIN,last.example.test\nIP-CIDR,192.0.2.0/24\n",
    )
    .unwrap();
    let source = fixture(
        "[Rule]\nDOMAIN,before.example.test,DIRECT\nRULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_One.list,REJECT,no-resolve\nDOMAIN,middle.example.test,DIRECT\nRULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Two.list,REJECT\nDOMAIN,shared.example.test,DIRECT\nDOMAIN,after.example.test,DIRECT\n",
    );
    let routing = super::plan_promax_routing(&root, &source).unwrap();
    let merged = super::render_embedded(&source, &routing).unwrap();
    let rows = super::parse_sections(&merged.content)
        .into_iter()
        .find(|(s, _)| s == "Rule")
        .unwrap()
        .1
        .into_iter()
        .map(|(_, s)| s)
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        [
            "DOMAIN,before.example.test,DIRECT",
            "DOMAIN,first.example.test,REJECT",
            "DOMAIN,shared.example.test,REJECT",
            "IP-CIDR,192.0.2.0/24,REJECT,no-resolve",
            "DOMAIN,middle.example.test,DIRECT",
            "DOMAIN,last.example.test,REJECT",
            "IP-CIDR,192.0.2.0/24,REJECT",
            "DOMAIN,shared.example.test,DIRECT",
            "DOMAIN,after.example.test,DIRECT"
        ]
    );
    assert!(super::validate_merged_size(super::MAX_ROUTING_TOTAL_BYTES).is_ok());
    assert!(super::validate_merged_size(super::MAX_ROUTING_TOTAL_BYTES + 1).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn generator_retires_only_owned_routing_after_preflight() {
    let root = temporary();
    fs::write(
        root.join("modules/surge/head_expanse/Test.sgmodule"),
        fixture("[Rule]\nDOMAIN,ads.example.test,REJECT\n"),
    )
    .unwrap();
    run_generator(&root).unwrap();
    let old = root.join("modules/flclash/head_expanse/ScriptHub.PROMAX.AdBlock_Old.sgmodule");
    fs::write(
        &old,
        format!("{}\n[Rule]\nDOMAIN,old.example.test,REJECT\n", super::OWNER),
    )
    .unwrap();
    let unknown = root.join("modules/flclash/head_expanse/Personal.sgmodule");
    fs::write(&unknown, "#!name=Preserve user file\n").unwrap();
    assert!(run_generator(&root).is_err());
    assert!(old.exists() && unknown.exists());
    fs::remove_file(&unknown).unwrap();
    run_generator(&root).unwrap();
    assert!(!old.exists());
    validate_generated(&root).unwrap();
    fs::write(&old, "#!name=Manually authored matching filename\n").unwrap();
    assert!(run_generator(&root).is_err());
    assert!(old.exists());
    fs::remove_dir_all(root).unwrap();
}
