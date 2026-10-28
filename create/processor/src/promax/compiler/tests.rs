use super::AdBlockManager;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn reviewed_privacy_sources_survive_safety_filter_and_keep_suffixes_protected() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let source_path = repo_root.join("rulesets/Sources/Privacy_Reviewed.list");
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", source_path.display()));

    let mut manager = AdBlockManager::new(repo_root);
    manager.load_whitelist().unwrap();
    manager.extract_from_text(
        &source,
        "REJECT",
        "Privacy",
        false,
        false,
        "rulesets/Sources/Privacy_Reviewed.list",
    );
    manager.extract_from_text(
        "DOMAIN-SUFFIX,events.redditmedia.com\nDOMAIN,other.redditmedia.com\n",
        "REJECT",
        "Privacy",
        false,
        false,
        "reviewed-privacy-sibling-fixture",
    );

    let privacy_rules = manager.rules["REJECT"]["Privacy"].clone();
    let filtered = manager.filter_rules(&privacy_rules).unwrap();
    let rendered_hosts: HashSet<String> = filtered
        .iter()
        .filter_map(|line| crate::promax::rule::SurgeRule::parse(line).ok())
        .filter(|rule| rule.kind == crate::promax::rule::RuleKind::Domain)
        .map(|rule| rule.payload.to_ascii_lowercase())
        .collect();
    for host in [
        "firebaselogging-pa.googleapis.com",
        "static.ads-twitter.com",
        "ads-api.twitter.com",
        "events.redditmedia.com",
        "analytics-sg.tiktok.com",
        "ads.tiktok.com",
        "api-adservices.apple.com",
        "iadsdk.apple.com",
    ] {
        assert!(
            rendered_hosts.contains(host),
            "reviewed exact host missing: {host}"
        );
    }

    for (candidate, reason) in [
        (
            "DOMAIN-SUFFIX,events.redditmedia.com",
            "protected-domain-intersection",
        ),
        (
            "DOMAIN,other.redditmedia.com",
            "protected-domain-intersection",
        ),
    ] {
        assert!(
            manager
                .quarantine
                .iter()
                .any(|record| { record.candidate == candidate && record.reason == reason }),
            "expected {candidate} to remain quarantined"
        );
    }
}

#[test]
fn manager_normalizes_an_existing_relative_root() {
    assert!(AdBlockManager::new(".".into()).root_dir.is_absolute());
}

#[test]
fn promax_inline_ip_rules_never_trigger_dns_resolution() {
    let manager = AdBlockManager::new(std::env::temp_dir());
    for rule in [
        "IP-CIDR,180.76.76.200/32",
        "IP-CIDR6,2001:db8::/32",
        "IP-ASN,13335",
        "GEOIP,US",
        "IP-CIDR,192.0.2.0/24,NO-RESOLVE,no-resolve",
    ] {
        for policy in ["REJECT", "REJECT-DROP", "REJECT-NO-DROP"] {
            let rendered = manager.attach_policy(rule, policy).unwrap();
            assert!(rendered.ends_with(",no-resolve"), "{rendered}");
            assert_eq!(rendered.matches(",no-resolve").count(), 1);
            assert!(rendered.contains(&format!(",{policy},")));
        }
    }
    assert_eq!(
        manager
            .attach_policy("DOMAIN,ads.example.test", "REJECT-DROP")
            .unwrap(),
        "DOMAIN,ads.example.test,REJECT-DROP"
    );
}

#[test]
fn generated_modules_cannot_bypass_whitelist_with_live_upstream_rulesets() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    manager.safety = crate::promax::safety::SafetyPolicy::from_lines([
        "DOMAIN-SUFFIX,bilivideo.cn",
        "DOMAIN-SUFFIX,bilivideo.com",
        "DOMAIN-SUFFIX,apple.com",
    ]);
    for line in [
        "DOMAIN-SUFFIX,mcdn.bilivideo.cn",
        "DOMAIN,upos-sz-302kodo.bilivideo.com",
        "DOMAIN,audio-ssl.itunes.apple.com",
        "DOMAIN,pcdn.fixture.test",
    ] {
        manager.add_rule_line(line, "REJECT-NO-DROP", "Security");
    }
    let module = manager.generate_module(&[], "cdn").content;
    assert!(!module.contains("RULE-SET,https://ruleset.skk.moe/"));
    assert!(!module.contains("mcdn.bilivideo.cn"));
    assert!(!module.contains("upos-sz-302kodo.bilivideo.com"));
    assert!(!module.contains("audio-ssl.itunes.apple.com"));
    assert!(module.contains("DOMAIN,pcdn.fixture.test,REJECT-NO-DROP"));
}

#[test]
fn licensed_remote_app_modules_keep_templates_and_ruleset_boundaries() {
    let root = std::env::temp_dir().join(format!(
        "promax-licensed-apps-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("rulesets/Sources/Links")).unwrap();
    let url = "https://raw.githubusercontent.com/app2smile/rules/master/module/tieba.sgmodule";
    fs::write(
        root.join("rulesets/Sources/Links/AdBlock_functional_sources.list"),
        format!("{url}\n../legacy-ad.sgmodule\n"),
    )
    .unwrap();
    fs::write(
        root.join("rulesets/Sources/legacy-ad.sgmodule"),
        "[Script]\nlegacy_ad = type=http-response,pattern=^https://tiebac\\.baidu\\.com/legacy/ads,requires-body=true,script-path=https://raw.githubusercontent.com/app2smile/rules/master/js/tieba-proto.js,argument=per_filter_video_thread=1\nunrelated_ad = type=http-response,pattern=^https://ads\\.example/ad,requires-body=true,script-path=https://scripts.example/remove-ads.js\n",
    ).unwrap();
    let fixture = r"#!name=贴吧去广告
[Rule]
RULE-SET,https://raw.githubusercontent.com/app2smile/rules/master/rule/tieba-ad.list,REJECT-DROP
[Script]
贴吧 = type=http-response,pattern=^https://tiebac\.baidu\.com/c/f/ad/getFeedAd,requires-body=true,binary-body-mode=1,script-path=https://raw.githubusercontent.com/app2smile/rules/master/js/tieba-proto.js,argument=per_filter_video_thread={{{per_filter_video}}}
[MITM]
hostname = %APPEND% tiebac.baidu.com
";
    let mut manager = AdBlockManager::new(root.clone());
    manager.integrate_functional_sections(&HashMap::from([(url.to_string(), fixture.to_string())]));
    let output = manager.generate_module(&[], "cdn").content;
    assert!(output.contains("argument=per_filter_video_thread=0"));
    assert!(output.contains("binary-body-mode=1"));
    assert!(!output.contains("{{{per_filter_video}}}"));
    assert!(!output.contains("RULE-SET,https://raw.githubusercontent.com/app2smile/"));
    assert!(output.contains("tiebac.baidu.com"));
    assert_eq!(output.matches("script-path=https://raw.githubusercontent.com/app2smile/rules/master/js/tieba-proto.js").count(), 1);
    assert!(!output.contains("per_filter_video_thread=1"));
    assert!(output.contains("script-path=https://scripts.example/remove-ads.js"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn obsolete_embedded_scripts_follow_verified_maintained_resources() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    manager.add_section_line("Script", "mlxx = type=http-response,pattern=^https://api\\.bevol\\.com/ads,requires-body=true,script-path=https://cdn.jsdelivr.net/gh/Yu9191/Rewrite@main/mlxx.js");
    manager.add_section_line("Script", "fish = type=http-response,pattern=^https://acs\\.m\\.goofish\\.com/ads,requires-body=true,script-path=https://klraw.pages.dev/kelv1n1n/script/refs/heads/main/js/goofish.js?token=fixture");
    let module = manager.generate_module(&[], "cdn").content;
    assert!(!module.contains("Yu9191/Rewrite"));
    assert!(!module.contains("klraw.pages.dev"));
    assert!(module.contains(
        "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Scripts/mlxx/mlxx.js"
    ));
    assert!(
        module.contains("https://raw.githubusercontent.com/ishowshu/qx/main/script/goofish.js")
    );
    assert!(module.contains("requires-body=true"));
}

#[test]
fn mobile_runtime_keeps_features_but_avoids_unneeded_mitm_and_exclusion_reordering() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    let script = r"remove_ads = type=http-response,pattern=^https://api\.example\.test/ads,requires-body=true,script-path=https://scripts.fixture.test/remove.js";
    manager.add_section_line("Script", script);
    for host in [
        "*.example.test",
        "telemetry.example.test",
        "-login.example.test",
        "*.complex.test",
    ] {
        manager.add_mitm_host(host);
    }
    manager.add_section_line(
        "Map Local",
        r#"^https://(api|feed)\.complex\.test/ads data="https://example.test/empty.json""#,
    );
    manager.retain_referenced_mitm_hosts();
    assert_eq!(manager.sections["Script"], [script]);
    assert!(manager.mitm_hosts.contains("api.example.test"));
    assert!(!manager.mitm_hosts.contains("*.example.test"));
    assert!(!manager.mitm_hosts.contains("telemetry.example.test"));
    assert!(manager.mitm_hosts.contains("*.complex.test"));
    assert!(manager.mitm_hosts.contains("-login.example.test"));
    let module = manager.generate_module(&[], "github");
    let hosts = module
        .content
        .lines()
        .find(|line| line.starts_with("hostname ="))
        .unwrap();
    let hosts = hosts
        .split_once("%APPEND%")
        .unwrap()
        .1
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    let first_inclusion = hosts
        .iter()
        .position(|host| !host.starts_with('-'))
        .unwrap();
    assert!(
        hosts[first_inclusion..]
            .iter()
            .all(|host| !host.starts_with('-'))
    );
}

#[test]
fn runtime_optimization_preserves_first_script_priority_without_changing_admission() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    manager.add_section_line("Script", r"first = type=http-response,pattern=^https://.*\.example\.test/ads,requires-body=true,script-path=https://scripts.test/first.js");
    manager.add_section_line("Script", r"shadowed = type=http-response,pattern=^https://[^\x2f\x3f\x23]*\.example\.test/ads,requires-body=true,script-path=https://scripts.test/shadowed.js");
    assert_eq!(manager.sections["Script"].len(), 2);
    manager.optimize_runtime_sections();
    assert_eq!(manager.sections["Script"].len(), 1);
    assert!(manager.sections["Script"][0].contains("first.js"));
    assert!(manager.sections["Script"][0].contains("requires-body=true"));
    assert!(manager.sections["Script"][0].contains(r"[^\x2f\x3f\x23]*"));
}

#[test]
fn runtime_narrowing_does_not_remove_mitm_required_by_opaque_original_hosts() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    manager.add_section_line("Script", r"flyert = type=http-response,pattern=^https://.*flyert.*/ads,requires-body=true,script-path=https://scripts.test/flyert.js");
    manager.add_mitm_host("*.flyert.*");
    manager.retain_referenced_mitm_hosts();
    manager.optimize_runtime_sections();
    assert_eq!(manager.sections["Script"].len(), 1);
    assert!(manager.mitm_hosts.contains("*.flyert.*"));
}

#[test]
fn published_current_shards_prune_only_catalogued_stale_and_legacy_rulesets() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-retired-shards-{unique}"));
    fs::create_dir_all(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let catalog = root.join("rulesets/AdBlock/catalog.json");
    fs::create_dir_all(catalog.parent().unwrap()).unwrap();
    fs::write(
        &catalog,
        r#"{"shards":[{"file":"rulesets/AdBlock/AdBlock_Advertising.list"},{"file":"rulesets/AdBlock/AdBlock_ThreatIntel_TIF_07.list"},{"file":"rulesets/AdBlock/AdBlock_Other.list"},{"file":"rulesets/AdBlock/AdBlock_Advertising_ip_01.list"},{"file":"rulesets/AdBlock/AdBlock_Security_other_extra.list"}]}"#,
    )
    .unwrap();
    let manager = AdBlockManager::new(root.clone());
    let generated = vec!["rulesets/AdBlock/AdBlock_Advertising.list".to_string()];
    let stale = manager.stale_ruleset_shard_paths(&generated).unwrap();
    assert_eq!(stale.len(), 4);
    let stale_tif = root.join("rulesets/AdBlock/AdBlock_ThreatIntel_TIF_07.list");
    let stale_other = root.join("rulesets/AdBlock/AdBlock_Other.list");
    let stale_ip = root.join("rulesets/AdBlock/AdBlock_Advertising_ip_01.list");
    let stale_extra = root.join("rulesets/AdBlock/AdBlock_Security_other_extra.list");
    fs::write(&stale_tif, "old stale rules\n").unwrap();
    fs::write(&stale_other, "old stale rules\n").unwrap();
    fs::write(&stale_ip, "old stale IP rules\n").unwrap();
    fs::write(&stale_extra, "old stale extra rules\n").unwrap();
    let unknown = root.join("rulesets/AdBlock/AdBlock_Unknown.list");
    fs::write(&unknown, "keep unknown file\n").unwrap();
    let legacy = root.join("rulesets/RULE-SET/AdBlock.list");
    let legacy_mirror = root.join("rulesets/RULE-SET/github/AdBlock.list");
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    fs::create_dir_all(legacy_mirror.parent().unwrap()).unwrap();
    fs::write(&legacy, "legacy union\n").unwrap();
    fs::write(&legacy_mirror, "legacy mirror\n").unwrap();

    let live = super::super::artifact::GeneratedArtifact {
        target: root.join(&generated[0]),
        content: "DOMAIN,live.example\n".to_string(),
        kind: super::super::artifact::ArtifactKind::ExternalRuleset,
    };
    super::super::artifact::publish_artifacts(&root, std::slice::from_ref(&live), &[]).unwrap();
    manager.prune_legacy_and_stale_rulesets(&stale).unwrap();
    assert_eq!(fs::read_to_string(&live.target).unwrap(), live.content);
    assert!(!stale_tif.exists());
    assert!(!stale_other.exists());
    assert!(!stale_ip.exists());
    assert!(!stale_extra.exists());
    assert!(!legacy.exists());
    assert!(!legacy_mirror.exists());
    assert_eq!(fs::read_to_string(unknown).unwrap(), "keep unknown file\n");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn retired_shard_catalog_rejects_traversal_paths() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-retired-shard-path-{unique}"));
    let catalog = root.join("rulesets/AdBlock/catalog.json");
    fs::create_dir_all(catalog.parent().unwrap()).unwrap();
    let manager = AdBlockManager::new(root.clone());
    for file in [
        "rulesets/AdBlock/../outside.list",
        "rulesets/AdBlock/AdBlock_Unknown.list",
        "rulesets/AdBlock/AdBlock_Advertising_unknown.list",
        "rulesets/AdBlock/AdBlock_Advertising_ip_nope.list",
    ] {
        fs::write(&catalog, format!(r#"{{"shards":[{{"file":"{file}"}}]}}"#)).unwrap();
        assert!(manager.stale_ruleset_shard_paths(&[]).is_err());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn generated_shards_separate_domain_ip_and_other_rules_without_loss() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-mixed-shards-{unique}"));
    let mut manager = AdBlockManager::new(root.clone());
    let source_rules = [
        "DOMAIN-SUFFIX,ads.example".to_string(),
        "DOMAIN-KEYWORD,tracking".to_string(),
        "IP-CIDR,192.0.2.0/24,no-resolve".to_string(),
        "GEOIP,CN".to_string(),
        "PROCESS-NAME,ad-app".to_string(),
    ];
    manager
        .rules
        .get_mut("REJECT")
        .unwrap()
        .get_mut("Advertising")
        .unwrap()
        .extend(source_rules.iter().cloned());

    let (_, artifacts) = manager.generate_ruleset().unwrap();
    let domain = artifacts
        .iter()
        .find(|artifact| artifact.target.ends_with("AdBlock_Advertising.list"))
        .unwrap();
    let ip = artifacts
        .iter()
        .find(|artifact| artifact.target.ends_with("AdBlock_Advertising_ip.list"))
        .unwrap();
    let extra = artifacts
        .iter()
        .find(|artifact| {
            artifact
                .target
                .ends_with("AdBlock_Advertising_other_extra.list")
        })
        .unwrap();
    let rules_in = |content: &str| {
        content
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
            .map(str::to_string)
            .collect::<HashSet<_>>()
    };
    let domain_rules = rules_in(&domain.content);
    let ip_rules = rules_in(&ip.content);
    let extra_rules = rules_in(&extra.content);
    let mut emitted = domain_rules.clone();
    emitted.extend(ip_rules.iter().cloned());
    emitted.extend(extra_rules.iter().cloned());

    assert_eq!(emitted, source_rules.into_iter().collect());
    assert!(domain_rules.iter().all(|rule| rule.starts_with("DOMAIN")));
    assert_eq!(
        ip_rules,
        HashSet::from([
            "IP-CIDR,192.0.2.0/24,no-resolve".to_string(),
            "GEOIP,CN".to_string(),
        ])
    );
    assert_eq!(
        extra_rules,
        HashSet::from(["PROCESS-NAME,ad-app".to_string()])
    );
    for artifact in [domain, ip, extra] {
        assert!(artifact.content.contains("# Category-Key: Advertising\n"));
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn offline_layout_migration_preserves_catalog_metadata_and_module_policy() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-offline-layout-{unique}"));
    let old_file = "rulesets/AdBlock/AdBlock_Advertising.list";
    let shard_content = "# Ruleset: fixture\n# Category-Key: Advertising\n# Total Rules: 3\nDOMAIN-SUFFIX,ads.example\nIP-CIDR,192.0.2.0/24\nPROCESS-NAME,ad-app\n";
    let catalog_path = root.join("rulesets/AdBlock/catalog.json");
    fs::create_dir_all(catalog_path.parent().unwrap()).unwrap();
    fs::write(root.join(old_file), shard_content).unwrap();
    let rule_set = "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/AdBlock_Advertising.list";
    let module_content = format!(
        "#!name=PROMAX migration fixture\n[Rule]\nRULE-SET,{rule_set},REJECT,extended-matching,pre-matching,update-interval=86400,no-resolve\n"
    );
    let module_paths = (0..6)
        .map(|index| format!("modules/surge/head_expanse/PROMAX-fixture-{index}.sgmodule"))
        .collect::<Vec<_>>();
    for path in &module_paths {
        let target = root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, &module_content).unwrap();
    }
    let modules = module_paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            (
                format!("fixture_{index}"),
                serde_json::json!({
                    "path": path,
                    "install_url": format!("https://example.test/{index}"),
                    "url_source": "fixture",
                    "note": "preserve me",
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let original_catalog = serde_json::json!({
        "updated": "preserve timestamp",
        "manifest": "preserve manifest",
        "verified_rolling_feeds": {"ThreatIntel_TIF": {"source": "preserve source", "version": "preserve version"}},
        "modules": modules,
        "shards": [{
            "file": old_file,
            "category": "Advertising",
            "label_zh": "fixture label",
            "purpose": "fixture purpose",
            "shard_index": 1,
            "rule_count": 3,
            "cdn_url": "old cdn",
            "github_url": "old github",
        }],
    });
    fs::write(
        &catalog_path,
        serde_json::to_vec_pretty(&original_catalog).unwrap(),
    )
    .unwrap();

    super::migrate_published_rule_layout(&root).unwrap();

    let migrated_catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(catalog_path).unwrap()).unwrap();
    assert_eq!(migrated_catalog["updated"], original_catalog["updated"]);
    assert_eq!(migrated_catalog["manifest"], original_catalog["manifest"]);
    assert_eq!(
        migrated_catalog["verified_rolling_feeds"],
        original_catalog["verified_rolling_feeds"]
    );
    assert_eq!(migrated_catalog["modules"], original_catalog["modules"]);
    assert_eq!(migrated_catalog["shards"].as_array().unwrap().len(), 3);
    let new_files = migrated_catalog["shards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|shard| shard["file"].as_str().unwrap().to_string())
        .collect::<HashSet<_>>();
    assert!(new_files.contains("rulesets/AdBlock/AdBlock_Advertising.list"));
    assert!(new_files.contains("rulesets/AdBlock/AdBlock_Advertising_ip.list"));
    assert!(new_files.contains("rulesets/AdBlock/AdBlock_Advertising_other_extra.list"));
    for path in module_paths {
        let module = fs::read_to_string(root.join(path)).unwrap();
        assert_eq!(module.matches("RULE-SET,").count(), 3);
        assert!(
            module.contains(
                ",REJECT,extended-matching,pre-matching,update-interval=86400,no-resolve"
            )
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn retired_artifact_prune_propagates_remove_failures() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-retired-shard-remove-{unique}"));
    let manager = AdBlockManager::new(root.clone());
    let stale = root.join("rulesets/AdBlock/AdBlock_Advertising_01.list");
    fs::create_dir_all(&stale).unwrap();

    assert!(
        manager
            .prune_legacy_and_stale_rulesets(std::slice::from_ref(&stale))
            .is_err()
    );
    assert!(stale.is_dir());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn adguard_domain_exception_removes_a_merged_block_rule() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    manager.extract_from_text(
        "||ads.example.test^\n||media.ads.example.test^\n@@||ads.example.test^|\n",
        "REJECT",
        "Advertising",
        false,
        false,
        "fixture",
    );
    let rules = &manager.rules["REJECT"]["Advertising"];
    assert!(manager.filter_rules(rules).unwrap().is_empty());
}

#[test]
fn adguard_wildcard_exceptions_preserve_domain_boundaries_and_case() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    manager.extract_from_text(
        "DOMAIN,Safe1.ads.example.test\nDOMAIN,media.safe2.ads.example.test\nDOMAIN,unsafe1.ads.example.test\nDOMAIN,ads.example.test\n@@||safe*.ads.example.test^\n",
        "REJECT", "Advertising", false, false, "fixture",
    );
    assert_eq!(
        manager
            .filter_rules(&manager.rules["REJECT"]["Advertising"])
            .unwrap(),
        vec![
            "DOMAIN,ads.example.test".to_string(),
            "DOMAIN,unsafe1.ads.example.test".to_string(),
        ]
    );
}

#[test]
fn rolling_source_is_verified_before_ingestion_and_recorded_in_catalog() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-rolling-source-{unique}"));
    let mut manager = AdBlockManager::new(root.clone());
    let source =
        "https://raw.githubusercontent.com/hagezi/dns-blocklists/main/adblock/tif.medium.txt";
    let entry = super::SourceEntry {
        manifest_path: manager
            .root_dir
            .join("rulesets/Sources/Links/AdBlock_sources.list"),
        source: source.to_string(),
        policy: "REJECT".to_string(),
        kind: "auto".to_string(),
        category: Some("ThreatIntel_TIF".to_string()),
    };
    let text = "[Adblock Plus]\n! Version: fixture\n! Last modified: 20 Oct 2026 14:31 UTC\n! Number of entries: 2\n||one.fixture.test^\n||two.fixture.test^\n";
    let contents = HashMap::from([(source.to_string(), text.to_string())]);
    assert!(
        manager
            .process_source_entries(std::slice::from_ref(&entry), &contents)
            .is_empty()
    );
    assert_eq!(manager.rules["REJECT"]["ThreatIntel_TIF"].len(), 2);
    let (paths, artifacts) = manager.generate_ruleset().unwrap();
    assert!(
        !artifacts
            .iter()
            .any(|artifact| artifact.target.ends_with("rulesets/RULE-SET/AdBlock.list"))
    );
    let metadata = manager.generate_catalog_artifacts(&paths, &artifacts);
    let json = metadata
        .iter()
        .find(|artifact| artifact.target.ends_with("catalog.json"))
        .unwrap();
    let catalog: serde_json::Value = serde_json::from_str(&json.content).unwrap();
    assert_eq!(
        catalog["verified_rolling_feeds"]["ThreatIntel_TIF"]["declared_entries"],
        2
    );

    let mut manager = AdBlockManager::new(root.clone());
    let truncated = HashMap::from([(
        source.to_string(),
        text.replace("||two.fixture.test^\n", ""),
    )]);
    assert_eq!(
        manager.process_source_entries(&[entry], &truncated).len(),
        1
    );
    assert!(manager.rules["REJECT"]["ThreatIntel_TIF"].is_empty());
    assert!(manager.verified_rolling_feeds.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn suffix_subsumption_is_case_insensitive_policy_scoped_and_label_bounded() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    for line in [
        "DOMAIN-SUFFIX,Metrics.Example.test,REJECT",
        "DOMAIN,Ads.METRICS.EXAMPLE.test,REJECT",
        "DOMAIN,Metrics.Example.test,REJECT",
        "DOMAIN,Ads.METRICS.EXAMPLE.test,REJECT-DROP",
        "DOMAIN,ads.othermetrics.example.test,REJECT",
        "DOMAIN,ads.othermetrics.example.test,REJECT",
    ] {
        manager.add_rule_line(line, "REJECT", "Advertising");
    }

    assert_eq!(
        manager.rules["REJECT"]["Advertising"],
        HashSet::from([
            "DOMAIN-SUFFIX,Metrics.Example.test".to_string(),
            "DOMAIN,ads.othermetrics.example.test".to_string(),
        ])
    );
    assert_eq!(
        manager.rules["REJECT-DROP"]["Advertising"],
        HashSet::from(["DOMAIN,Ads.METRICS.EXAMPLE.test".to_string()])
    );
}

#[test]
fn script_classifier_keeps_ascii_case_and_unicode_fallback_behavior() {
    let manager = AdBlockManager::new(std::env::temp_dir());
    for (line, expected) in [
        ("DOMAIN,ads.example.test,REJECT", false),
        ("# script-path=https://example.test/a.js", false),
        ("fixture = TYPE=HTTP-RESPONSE", true),
        ("^https://ads.example.test/ _ REJECT", true),
        ("^https://ads.example.test/ https://example.test 302", true),
        ("DOMAIN-KEYWORD,example 302", false),
        ("DOMAIN-KEYWORD,example reject", false),
        ("fixture = status-code=200", true),
        ("脚本 = SCRIPT-PATH=https://example.test/a.js", true),
        ("USER-AGENT,普通客户端", false),
    ] {
        assert_eq!(manager.is_script_or_rewrite_line(line), expected, "{line}");
    }
}

#[test]
fn suffix_insertion_keeps_preexisting_domains_and_single_label_behavior() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    for line in [
        "DOMAIN,ads.metrics.example.test",
        "DOMAIN-SUFFIX,metrics.example.test",
        "DOMAIN-SUFFIX,localhost",
        "DOMAIN,localhost",
        "DOMAIN-SUFFIX,test",
        "DOMAIN,metrics.test",
    ] {
        manager.add_rule_line(line, "reject", "Advertising");
    }

    assert_eq!(manager.rules["REJECT"]["Advertising"].len(), 6);
}

#[test]
fn rule_ingestion_preserves_quarantine_provenance_and_tinygif_normalization() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    manager.safety =
        crate::promax::safety::SafetyPolicy::from_lines(["DOMAIN-SUFFIX,protected.example.test"]);
    manager.current_source = "fixture".to_string();
    manager.current_line = 42;
    for line in [
        "DOMAIN,invalid/path,REJECT",
        "DOMAIN,ads.example.test,PROXY",
        "DOMAIN,auth.protected.example.test,REJECT",
        "DOMAIN,ads.example.test,REJECT-TINYGIF # comment",
    ] {
        manager.add_rule_line(line, "REJECT", "Advertising");
    }

    assert_eq!(
        manager.rules["REJECT"]["Advertising"],
        HashSet::from(["DOMAIN,ads.example.test".to_string()])
    );
    assert_eq!(manager.quarantine.len(), 3);
    assert!(
        manager
            .quarantine
            .iter()
            .all(|record| record.source == "fixture" && record.line == 42)
    );
    assert_eq!(manager.quarantine[1].reason, "unsupported-proxy-policy");
    assert_eq!(
        manager.quarantine[2].risk,
        crate::promax::safety::RiskClass::ProtectedService
    );
}

#[test]
#[ignore = "offline release diagnostic; run with --release --ignored --nocapture"]
fn benchmark_rule_ingestion() {
    use std::hint::black_box;
    use std::time::Instant;

    let mut body = String::new();
    for index in 0..250 {
        body.push_str(&format!("DOMAIN-SUFFIX,metrics{index}.example.test\n"));
    }
    for index in 0..6_000 {
        let line = format!(
            "DOMAIN,ads{index}.metrics{}.example.test,REJECT\n",
            index % 500
        );
        body.push_str(&line);
        if index % 4 == 0 {
            body.push_str(&line);
        }
    }
    for with_section in [false, true] {
        let text = if with_section {
            format!("[Rule]\n{body}")
        } else {
            body.clone()
        };
        let mut elapsed = Vec::with_capacity(7);
        for _ in 0..7 {
            let mut manager = AdBlockManager::new(std::env::temp_dir());
            let start = Instant::now();
            manager.extract_from_text(
                black_box(&text),
                "REJECT",
                "Advertising",
                false,
                false,
                "benchmark",
            );
            elapsed.push(start.elapsed());
            assert_eq!(manager.rules["REJECT"]["Advertising"].len(), 3_250);
            assert!(manager.quarantine.is_empty());
            black_box(manager);
        }
        elapsed.sort_unstable();
        eprintln!(
            "rule ingestion: section={with_section}, lines=7750, rounds=7, median={:?}",
            elapsed[elapsed.len() / 2]
        );
    }
}

#[test]
fn local_auto_discovery_does_not_force_unclassified_enhancement_modules() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-local-classifier-{unique}"));
    let local = root.join("modules/source/local");
    fs::create_dir_all(&local).unwrap();
    let module = local.join("url-redirect.sgmodule");
    fs::write(
        &module,
        "#!name=Redirect enhancement\n[URL Rewrite]\n^https://mirror\\.example/(.*) https://origin.example/$1 302\n",
    )
    .unwrap();

    assert_eq!(
        AdBlockManager::new(root.clone()).resolve_module_ingest_mode(&module, false),
        "skip"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn offline_fixture_publishes_complete_promax_product_set() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-offline-{unique}"));
    let links = root.join("rulesets/Sources/Links");
    fs::create_dir_all(&links).unwrap();
    let split_dir = root.join("modules/source/build/promax_splits");
    fs::create_dir_all(&split_dir).unwrap();
    let adblock_dir = root.join("rulesets/AdBlock");
    fs::create_dir_all(&adblock_dir).unwrap();
    fs::write(
        adblock_dir.join("AdBlock_Stale.list"),
        "DOMAIN,stale.test\n",
    )
    .unwrap();
    fs::write(adblock_dir.join("AdBlock_Stale_Singbox.srs"), b"stale").unwrap();
    fs::write(
        split_dir.join("stale-enhancement.ad-extract.sgmodule"),
        "#!name=stale\n[URL Rewrite]\n^https://premium\\.example/ https://origin.example/ 302\n",
    )
    .unwrap();

    let mut whitelist = String::new();
    for index in 0..300 {
        whitelist.push_str(&format!("DOMAIN-SUFFIX,protected{index}.example\n"));
    }
    for index in 0..200 {
        whitelist.push_str(&format!(
            "IP-CIDR,10.{}.{}.0/24\n",
            index / 256,
            index % 256
        ));
    }
    fs::write(
        root.join("rulesets/Sources/adblock_whitelist.list"),
        whitelist,
    )
    .unwrap();
    fs::create_dir_all(root.join("rulesets/RULE-SET")).unwrap();
    fs::write(
        root.join("rulesets/RULE-SET/Apple.list"),
        "DOMAIN,account.apple.com\nDOMAIN-SUFFIX,icloud.com\nPROCESS-NAME,apsd\n",
    )
    .unwrap();

    fs::write(
        links.join("AdBlock_sources.list"),
        "../fixture.list | REJECT | auto\n../drop-fixture.list|REJECT-DROP|auto|Privacy\n../no-drop-fixture.list|REJECT-NO-DROP|auto|Security\n",
    )
    .unwrap();
    fs::write(
        root.join("rulesets/Sources/drop-fixture.list"),
        "DOMAIN,telemetry.fixture.test\nDOMAIN,account.apple.com\nIP-CIDR,180.76.76.200/32\n",
    )
    .unwrap();
    fs::write(
        root.join("rulesets/Sources/no-drop-fixture.list"),
        "DOMAIN,pcdn.fixture.test\nDOMAIN,protected0.example\n",
    )
    .unwrap();
    fs::write(
        root.join("rulesets/Sources/fixture.list"),
        "DOMAIN-SUFFIX,ads.fixture.test\nDOMAIN,tracker.fixture.test\nDOMAIN,extended.fixture.test,REJECT,extended-matching\nIP-CIDR,192.0.2.0/24\n",
    )
    .unwrap();
    fs::write(
        root.join("rulesets/Sources/DirectPorts.list"),
        "DEST-PORT,4444,REJECT\nDEST-PORT,4444,REJECT\n",
    )
    .unwrap();
    fs::write(
        links.join("AdBlock_functional_sources.list"),
        "../fixture.sgmodule\n../mixed-adblock.sgmodule\n",
    )
    .unwrap();
    fs::write(
        root.join("rulesets/Sources/fixture.sgmodule"),
        r#"#!name=Fixture AdBlock
[Rule]
DOMAIN-SUFFIX,functional-ad.fixture.test,REJECT
[URL Rewrite]
^https://ads\.fixture\.test/splash _ reject
[Map Local]
^https://ads\.fixture\.test/splash data-type=text data="{}" status-code=200
[Script]
fixture = type=http-response,pattern=^https://ads\.fixture\.test/feed,requires-body=1,script-path=https://scripts.fixture.test/remove.js
[Body Rewrite]
http-response-jq ^https:\/\/ads\.fixture\.test\/feed 'del(.data.ad)'
[MITM]
hostname = ads.fixture.test, creditcardapp.bankcomm.com
"#,
    )
    .unwrap();
    fs::write(
        root.join("rulesets/Sources/mixed-adblock.sgmodule"),
        r#"#!name=Mixed fixture
[Rule]
DOMAIN-SUFFIX,split-ad.fixture.test,REJECT
IP-CIDR,10.0.0.0/24,REJECT
[URL Rewrite]
^https://split-ad\.fixture\.test/splash _ reject
^https://premium\.fixture\.test/ https://example.test/unlocked 302
[Script]
remove_ads = type=http-response,pattern=^https://split-ad\.fixture\.test/feed,requires-body=1,script-path=https://scripts.fixture.test/remove-ads.js
remove_ads_duplicate = type=http-response,pattern=^https://split-ad\.fixture\.test/feed,requires-body=1,script-path=https://scripts.fixture.test/remove-ads-duplicate.js
unlock_vip = type=http-response,pattern=^https://premium\.fixture\.test/account,requires-body=1,script-path=https://scripts.fixture.test/unlock.js
[MITM]
hostname = split-ad.fixture.test, protected0.example
"#,
    )
    .unwrap();

    assert!(AdBlockManager::new(root.clone()).merge(true));
    assert!(!root.join("modules/loon").exists());

    let products = [
        "modules/surge/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
        "modules/surge/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule",
        "modules/shadowrocket/head_expanse/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module",
        "modules/shadowrocket/head_expanse/github/🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module",
    ];
    for product in &products {
        let content = fs::read_to_string(root.join(product)).unwrap();
        assert!(content.contains("scripts.fixture.test/remove.js"));
        assert!(!content.contains("creditcardapp.bankcomm.com"));
        if product.contains("surge/") || product.contains("shadowrocket/") {
            assert!(!content.contains("RULE-SET,https://ruleset.skk.moe/"));
            assert!(!content.contains("DOMAIN,account.apple.com,REJECT"));
            assert!(!content.contains("DOMAIN,protected0.example,REJECT"));
            assert!(!content.contains("rulesets/Sources/skk_upstream/"));
            assert!(!content.contains("rulesets/AdBlock/HTTPDNS_Hijack.list"));
            if product.contains("shadowrocket/") {
                assert!(content.contains("DOMAIN,pcdn.fixture.test,REJECT"));
                assert!(content.contains("DOMAIN,telemetry.fixture.test,REJECT"));
            } else {
                assert!(content.contains("DOMAIN,pcdn.fixture.test,REJECT-NO-DROP"));
                assert!(content.contains("DOMAIN,telemetry.fixture.test,REJECT-DROP"));
                assert!(content.contains("IP-CIDR,180.76.76.200/32,REJECT-DROP,no-resolve"));
                assert!(!content.contains(",NO-RESOLVE"));
            }
        }
    }
    assert!(root.join("rulesets/AdBlock/catalog.json").is_file());
    assert!(root.join("rulesets/AdBlock/README.md").is_file());
    assert!(root.join("rulesets/AdBlock/quarantine.json").is_file());
    // Unknown files are not catalog-owned and must never be deleted as cleanup.
    assert_eq!(
        fs::read_to_string(adblock_dir.join("AdBlock_Stale.list")).unwrap(),
        "DOMAIN,stale.test\n"
    );
    assert_eq!(
        fs::read(adblock_dir.join("AdBlock_Stale_Singbox.srs")).unwrap(),
        b"stale"
    );
    let shard = fs::read_to_string(root.join("rulesets/AdBlock/AdBlock_Local.list")).unwrap();
    assert!(shard.contains("functional-ad.fixture.test"));
    let split = fs::read_to_string(
        root.join("modules/source/build/promax_splits/mixed-adblock.ad-extract.sgmodule"),
    )
    .unwrap();
    assert!(split.contains("#!split-source=rulesets/Sources/mixed-adblock.sgmodule"));
    assert!(!split.contains(root.to_str().unwrap()));
    assert!(split.contains("split-ad.fixture.test"));
    assert!(split.contains("remove-ads.js"));
    assert!(!split.contains("remove-ads-duplicate.js"));
    assert!(!split.contains("10.0.0.0/24"));
    assert!(!split.contains("premium.fixture.test"));
    assert!(!split.contains("protected0.example"));
    assert!(
        !split_dir
            .join("stale-enhancement.ad-extract.sgmodule")
            .exists()
    );
    let catalog: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join("rulesets/AdBlock/catalog.json")).unwrap(),
    )
    .unwrap();
    let modules = catalog["modules"].as_object().unwrap();
    assert_eq!(modules.len(), 4);
    assert!(
        modules
            .keys()
            .all(|key| !key.to_ascii_lowercase().contains("loon"))
    );
    assert!(modules.values().all(|module| {
        module["path"]
            .as_str()
            .is_some_and(|path| !path.to_ascii_lowercase().contains("loon"))
    }));
    let quarantine = fs::read_to_string(root.join("rulesets/AdBlock/quarantine.json")).unwrap();
    assert!(quarantine.contains("10.0.0.0/24"));

    assert!(AdBlockManager::new(root.clone()).merge(true));
    assert_eq!(
        fs::read_to_string(adblock_dir.join("AdBlock_Stale.list")).unwrap(),
        "DOMAIN,stale.test\n"
    );
    assert_eq!(
        fs::read(adblock_dir.join("AdBlock_Stale_Singbox.srs")).unwrap(),
        b"stale"
    );
    fs::remove_dir_all(root).unwrap();
}
