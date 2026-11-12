use super::AdBlockManager;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn reviewed_browser_host_does_not_duplicate_a_network_suffix_in_another_category() {
    let browser = "https://raw.githubusercontent.com/kargig/greek-adblockplus-filter/master/void-gr-filters.txt";
    let network = "https://example.test/compiled.list";
    let entry = |source: &str, kind: &str, category: &str| super::SourceEntry {
        manifest_path: "rulesets/Sources/Links/AdBlock_sources.list".into(),
        source: source.into(),
        policy: "REJECT".into(),
        kind: kind.into(),
        category: Some(category.into()),
    };
    let sources = [
        entry(browser, "browser-domains", "Advertising"),
        entry(network, "auto", "ThreatIntel"),
    ];
    let contents = HashMap::from([
        (browser.into(), "||adman.kathimerini.gr^".into()),
        (network.into(), "DOMAIN-SUFFIX,adman.kathimerini.gr".into()),
    ]);
    let mut manager =
        AdBlockManager::new(std::env::temp_dir().join("script-hub-browser-dedupe-fixture"));
    assert!(
        manager
            .process_source_entries(&sources, &contents)
            .is_empty()
    );
    assert!(manager.rules["REJECT"]["ThreatIntel"].contains("DOMAIN-SUFFIX,adman.kathimerini.gr"));
    assert!(!manager.rules["REJECT"]["Advertising"].contains("DOMAIN,adman.kathimerini.gr"));
}

#[test]
fn content_service_roots_are_quarantined_without_whitelisting_subdomains_or_threats() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "script-hub-content-roots-{}-{stamp}",
        std::process::id()
    ));
    let repository = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for file in [
        "rulesets/Sources/adblock_whitelist.list",
        "rulesets/RULE-SET/Apple.list",
    ] {
        let target = root.join(file);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(repository.join(file), target).unwrap();
    }
    let path = root.join("rulesets/Sources/compiled/NSFW.list");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "DOMAIN-SUFFIX,www.service.fixture.test\nIP-CIDR,192.0.2.0/24\nDOMAIN,invalid..test\nDOMAIN,invalid_name.test\n").unwrap();
    let manual = root.join("rulesets/Sources/custom/NSFW_manual.list");
    fs::create_dir_all(manual.parent().unwrap()).unwrap();
    fs::write(manual, "DOMAIN-SUFFIX,manual.fixture.test\n").unwrap();
    let mut manager = AdBlockManager::new(root.clone());
    manager.load_whitelist().unwrap();
    manager.extract_from_text(
        "DOMAIN,service.fixture.test\nDOMAIN,www.service.fixture.test\nDOMAIN-SUFFIX,service.fixture.test\nDOMAIN,manual.fixture.test\nDOMAIN,ads.service.fixture.test\nDOMAIN,invalid_name.test\n",
        "REJECT", "Advertising", false, false, "fixture",
    );
    let rules = &manager.rules["REJECT"]["Advertising"];
    assert!(!rules.contains("DOMAIN,service.fixture.test"));
    assert!(!rules.contains("DOMAIN,www.service.fixture.test"));
    assert!(!rules.contains("DOMAIN-SUFFIX,service.fixture.test"));
    assert!(!rules.contains("DOMAIN,manual.fixture.test"));
    assert!(rules.contains("DOMAIN,ads.service.fixture.test"));
    assert!(rules.contains("DOMAIN,invalid_name.test"));
    assert_eq!(
        manager
            .quarantine
            .iter()
            .filter(|record| record.reason == "protected-content-service-root")
            .count(),
        4
    );
    manager.extract_from_text(
        "DOMAIN,service.fixture.test\n",
        "REJECT",
        "Security",
        false,
        false,
        "security-fixture",
    );
    assert!(manager.rules["REJECT"]["Security"].contains("DOMAIN,service.fixture.test"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn hosts_sinkholes_keep_all_exact_aliases_and_normal_service_exemptions() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    manager.extract_from_text(
        "0.0.0.0 ad-host.fixture.test tracker-host.fixture.test\n127.0.0.1 localhost\n192.0.2.1 normal-host.fixture.test\n::1 weather-analytics-events.apple.com i.ytimg.com registry.api.cnn.io vkcache.com\n",
        "REJECT", "Advertising", false, false, "hosts-fixture",
    );
    assert_eq!(
        manager.rules["REJECT"]["Advertising"],
        HashSet::from([
            "DOMAIN,ad-host.fixture.test".to_owned(),
            "DOMAIN,tracker-host.fixture.test".to_owned(),
        ])
    );
}

#[test]
fn published_promax_keeps_one_ip_fallback_without_external_ip_shards() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ports = crate::firewall_sync::read_port_rules(
        &root.join("rulesets/Sources/conf/SurgeConf_DirectPorts.list"),
    )
    .unwrap();
    for mirror in ["", "github/"] {
        let path = root.join(format!("modules/surge/head_expanse/{mirror}🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).sgmodule"));
        let content = fs::read_to_string(path).unwrap();
        for port_rule in &ports {
            assert_eq!(
                content.lines().filter(|line| *line == port_rule).count(),
                1,
                "{port_rule}"
            );
        }
        let mut section = "";
        let mut seen = HashSet::new();
        let mut rewrites = 0;
        let mut subscriptions = 0;
        for line in content.lines().map(str::trim) {
            if line.starts_with('[') && line.ends_with(']') {
                section = line;
                continue;
            }
            if section == "[IP Rewrite]" && line.contains('=') {
                rewrites += 1;
            }
            if section != "[Rule]" || line.starts_with('#') || line.is_empty() {
                continue;
            }
            if line.starts_with("RULE-SET,") {
                subscriptions += 1;
                let file = line.split(',').nth(1).unwrap().rsplit('/').next().unwrap();
                assert!(
                    !file.ends_with("_ip.list") && !file.contains("_ip_"),
                    "{line}"
                );
            } else if let Ok(rule) = crate::promax::rule::SurgeRule::parse(line)
                && matches!(
                    rule.kind,
                    crate::promax::rule::RuleKind::IpCidr
                        | crate::promax::rule::RuleKind::IpCidr6
                        | crate::promax::rule::RuleKind::IpAsn
                        | crate::promax::rule::RuleKind::GeoIp
                )
            {
                let target = rule
                    .ip_rewrite_target()
                    .unwrap_or_else(|| rule.payload.clone());
                let mut options = rule.options.clone();
                options.sort_unstable();
                assert!(
                    seen.insert((rule.kind.as_str(), target, rule.policy, options)),
                    "duplicate IP fallback: {line}"
                );
            }
        }
        assert!(rewrites > 0 && subscriptions > 0 && !seen.is_empty());
        let shadowrocket = root.join(format!("modules/shadowrocket/head_expanse/{mirror}🚫 Universal Ad-Blocking Rules Dependency Component PROMAX (Kali-style).module"));
        assert_eq!(
            crate::promax::artifact::convert_surge_to_shadowrocket(&content),
            fs::read_to_string(shadowrocket).unwrap()
        );
        eprintln!(
            "published PROMAX {mirror}: IP rewrites={rewrites}, unique inline IP rules={}, external non-IP shards={subscriptions}",
            seen.len()
        );
    }
}

#[test]
fn reviewed_mainland_module_is_discovered_and_keeps_only_precise_ad_handlers() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root.clone());
    manager.load_whitelist().unwrap();
    let path = root.join("rulesets/Sources/LocalModules/Mainland_Ads_Reviewed.sgmodule");
    assert_eq!(manager.resolve_module_ingest_mode(&path, false), "full");
    let text = fs::read_to_string(path).unwrap();
    manager.extract_from_text(&text, "REJECT", "Local", true, false, "reviewed-mainland");
    assert!(
        manager.sections["Map Local"]
            .iter()
            .any(|line| line.contains("queryWechatAdvertisement(?:\\?|$)")),
        "kept={:?}, quarantined={:?}",
        manager.sections["Map Local"],
        manager.quarantine
    );
    assert!(manager.quarantine.is_empty());
    manager.retain_referenced_mitm_hosts();
    assert_eq!(
        manager
            .mitm_hosts
            .iter()
            .filter(|host| !host.starts_with('-'))
            .cloned()
            .collect::<HashSet<_>>(),
        HashSet::from(["rms.meituan.com".to_string()])
    );
    assert!(manager.mitm_hosts.contains("-account.apple.com"));
}

#[test]
fn staged_ip_shards_are_inlined_without_losing_nonidentical_rules() {
    use crate::promax::artifact::{ArtifactKind, GeneratedArtifact};
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "promax-staged-ip-inline-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let manager = AdBlockManager::new(root.clone());
    let path = "rulesets/AdBlock/AdBlock_Advertising_ip.list".to_string();
    let content = "IP-ASN,13335\nIP-CIDR,198.51.100.1/24\nIP-CIDR,198.51.100.0/24\nIP-CIDR,198.51.100.0/25\nIP-CIDR6,2001:db8::1/128\n";
    let artifacts = vec![GeneratedArtifact {
        target: manager.root_dir.join(&path),
        content: content.to_string(),
        kind: ArtifactKind::ExternalRuleset,
    }];
    for mirror in ["cdn", "github"] {
        let module = manager
            .generate_module_with_artifacts(std::slice::from_ref(&path), &artifacts, mirror)
            .unwrap()
            .content;
        assert!(
            !module
                .lines()
                .any(|line| line.starts_with("RULE-SET,") && line.contains(&path))
        );
        assert!(
            module.contains("IP-ASN,13335,REJECT,no-resolve,pre-matching")
                || module.contains("IP-ASN,13335,REJECT,pre-matching,no-resolve")
        );
        assert_eq!(
            module.matches("198.51.100.1/24").count() + module.matches("198.51.100.0/24").count(),
            1
        );
        assert!(module.contains("198.51.100.0/25"));
        assert!(module.contains("2001:db8::1/128"));
        assert!(crate::promax::validation::validate_surge_module(&module).is_empty());
        let sr = crate::promax::artifact::convert_surge_to_shadowrocket(&module);
        assert!(!sr.contains("pre-matching"));
        assert!(sr.contains("IP-ASN,13335,REJECT,no-resolve"));
        assert!(crate::promax::validation::validate_shadowrocket_module(&sr).is_empty());
    }
    assert_eq!(artifacts[0].content, content);
    assert!(
        manager
            .generate_module_with_artifacts(std::slice::from_ref(&path), &[], "cdn")
            .is_err()
    );
    let invalid = GeneratedArtifact {
        target: manager.root_dir.join(&path),
        content: "DOMAIN,invalid.example\n".to_string(),
        kind: ArtifactKind::ExternalRuleset,
    };
    assert!(
        manager
            .generate_module_with_artifacts(std::slice::from_ref(&path), &[invalid], "cdn")
            .is_err()
    );
    let empty = GeneratedArtifact {
        target: manager.root_dir.join(&path),
        content: "# empty\n".to_string(),
        kind: ArtifactKind::ExternalRuleset,
    };
    assert!(
        manager
            .generate_module_with_artifacts(std::slice::from_ref(&path), &[empty], "cdn")
            .is_err()
    );
    assert!(
        manager
            .generate_module_with_artifacts(
                &[path],
                &[artifacts[0].clone(), artifacts[0].clone()],
                "cdn"
            )
            .is_err()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn inlined_reject_keeps_distinct_drop_fallback() {
    use crate::promax::artifact::{ArtifactKind, GeneratedArtifact};
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "promax-ip-inline-conflict-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let mut manager = AdBlockManager::new(root.clone());
    manager.extract_from_text(
        "IP-CIDR,198.51.100.0/24,REJECT\nIP-CIDR,198.51.100.0/24,REJECT-DROP\n",
        "REJECT",
        "Security",
        false,
        false,
        "fixture",
    );
    let path = "rulesets/AdBlock/AdBlock_Security_ip.list".to_string();
    let artifact = GeneratedArtifact {
        target: manager.root_dir.join(&path),
        content: "IP-CIDR,198.51.100.0/24\n".to_string(),
        kind: ArtifactKind::ExternalRuleset,
    };
    let module = manager
        .generate_module_with_artifacts(&[path], &[artifact], "cdn")
        .unwrap()
        .content;
    assert!(
        module
            .lines()
            .any(|line| line.starts_with("IP-CIDR,198.51.100.0/24,REJECT,")
                && line.contains("pre-matching"))
    );
    assert!(module.contains("IP-CIDR,198.51.100.0/24,REJECT-DROP,no-resolve"));
    assert!(crate::promax::validation::validate_surge_module(&module).is_empty());
    let sr = crate::promax::artifact::convert_surge_to_shadowrocket(&module);
    assert!(crate::promax::validation::validate_shadowrocket_module(&sr).is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn authored_source_identity_is_root_relative_and_portable() {
    let root = std::env::temp_dir().join("promax-source-identity");
    let entry = super::SourceEntry {
        manifest_path: root.join("rulesets/Sources/Links/AdBlock_sources.list"),
        source: "/rulesets/Sources/IP_Drop_Reviewed.list".to_string(),
        policy: "REJECT-DROP".to_string(),
        kind: "auto".to_string(),
        category: Some("Local".to_string()),
    };
    let expected = "rulesets/Sources/IP_Drop_Reviewed.list";
    assert_eq!(entry.resolved_path(&root), root.join(expected));
    assert_eq!(entry.display_name(&root), expected);
    for (address, source) in crate::promax::safety::REVIEWED_IP_DROPS {
        if [
            "101.36.166.17",
            "101.36.166.18",
            "101.36.166.19",
            "47.106.88.76",
        ]
        .contains(address)
        {
            assert_eq!(*source, entry.display_name(&root));
        }
    }
}

#[cfg(unix)]
#[test]
fn local_source_identity_does_not_alias_a_literal_unix_backslash() {
    let root = std::env::temp_dir().join("promax-source-identity");
    let entry = super::SourceEntry {
        manifest_path: root.join("rulesets/Sources/Links/AdBlock_sources.list"),
        source: r"/rulesets\Sources\IP_Drop_Reviewed.list".to_string(),
        policy: "REJECT-DROP".to_string(),
        kind: "auto".to_string(),
        category: Some("Local".to_string()),
    };
    assert_eq!(
        entry.display_name(&root),
        r"rulesets\Sources\IP_Drop_Reviewed.list"
    );
    assert_ne!(
        entry.display_name(&root),
        "rulesets/Sources/IP_Drop_Reviewed.list"
    );
}

#[test]
fn user_confirmed_ip_drops_are_authored_exact_inputs_and_generated_with_fallbacks() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root.clone());
    manager.load_whitelist().unwrap();
    let entries = manager.load_source_entries();
    let entry = entries
        .iter()
        .find(|entry| entry.source == "/rulesets/Sources/IP_Drop_Reviewed.list")
        .expect("explicit user IP drops must have a checked-in authored source");
    assert_eq!(entry.policy, "REJECT-DROP");
    assert_eq!(entry.category.as_deref(), Some("Local"));
    let content = fs::read_to_string(entry.resolved_path(&root)).unwrap();
    let inputs: Vec<_> = content
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
        .map(|line| crate::promax::rule::SurgeRule::parse(line).unwrap())
        .collect();
    let addresses = [
        "101.36.166.17",
        "101.36.166.18",
        "101.36.166.19",
        "47.106.88.76",
    ];
    assert_eq!(inputs.len(), addresses.len());
    assert_eq!(
        inputs
            .iter()
            .map(|rule| rule.payload.clone())
            .collect::<HashSet<_>>(),
        addresses
            .iter()
            .map(|address| format!("{address}/32"))
            .collect()
    );
    assert!(
        inputs
            .iter()
            .all(|rule| rule.kind == crate::promax::rule::RuleKind::IpCidr
                && rule.options.iter().any(|option| option == "NO-RESOLVE"))
    );
    assert!(
        manager
            .process_source_entries(std::slice::from_ref(entry), &HashMap::new())
            .is_empty()
    );
    for mirror in ["cdn", "github"] {
        let module = manager.generate_module(&[], mirror).content;
        let drops: Vec<_> = module
            .split_once("[IP Rewrite]\n")
            .unwrap()
            .1
            .split('[')
            .next()
            .unwrap()
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        assert_eq!(drops, addresses.map(|address| format!("{address} = drop")));
        for address in addresses {
            assert!(module.contains(&format!("IP-CIDR,{address}/32,REJECT-DROP,no-resolve")));
        }
        assert!(crate::promax::validation::validate_surge_module(&module).is_empty());
    }
}

#[test]
fn mainland_diagnostics_from_existing_music_weibo_and_zhihu_sources_are_retained() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    for host in [
        "sentry.music.163.com",
        "zhihu-web-analytics.zhihu.com",
        "datacollection.uve.weibo.com",
    ] {
        let rule = format!("DOMAIN,{host}");
        manager.add_rule_line(&rule, "REJECT", "Privacy");
        assert!(manager.rules["REJECT"]["Privacy"].contains(&rule), "{rule}");
    }
    for host in [
        "music.163.com",
        "api.music.163.com",
        "www.zhihu.com",
        "api.weibo.com",
        "sinaimg.cn",
        "t10.baidu.com",
        "weather-analytics-events.apple.com",
    ] {
        let rule = format!("DOMAIN,{host}");
        manager.add_rule_line(&rule, "REJECT", "Privacy");
        assert!(
            !manager.rules["REJECT"]["Privacy"].contains(&rule),
            "{rule}"
        );
    }
}

#[test]
fn mainland_weibo_log_handlers_are_retained_without_auth_or_thumbnail_targets() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    for pattern in [
        r"^https://api-cloudim\.api\.weibo\.com/v1/log/ - reject",
        r"^https://api\.weibo\.cn/2/logservice/service - reject",
    ] {
        manager.add_section_line("URL Rewrite", pattern);
        assert!(
            manager.sections["URL Rewrite"].contains(&pattern.to_string()),
            "{pattern}"
        );
    }
    for pattern in [
        r"^https://api\.weibo\.com/auth/telemetry - reject",
        r"^https://api\.weibo\.com/login - reject",
        r"^https://api\.weibo\.com/thumb/ads - reject",
        r"^https://sinaimg\.cn/ads - reject",
        r"^https://api\.music\.163\.com/api/song/detail - reject",
    ] {
        manager.add_section_line("URL Rewrite", pattern);
        assert!(
            !manager.sections["URL Rewrite"].contains(&pattern.to_string()),
            "{pattern}"
        );
    }
}

#[test]
fn mainland_override_rejects_top_level_request_alternatives() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    for pattern in [
        r"^https://api\.bilibili\.com/ads/list|^https://api\.bilibili\.com/x/feed - reject",
        r"^https://logs\.amap\.com/collect|^https://api\.bilibili\.com/x/feed - reject",
        r"^https://logs\.amap\.com/(?#[)collect|^https://api\.bilibili\.com/x/feed - reject",
        r"^https://logs\.amap\.com/(?#()collect|^https://api\.bilibili\.com/x/feed - reject",
    ] {
        manager.add_section_line("URL Rewrite", pattern);
        assert!(
            !manager.sections["URL Rewrite"].contains(&pattern.to_string()),
            "{pattern}"
        );
    }
}

#[test]
fn mainland_override_rejects_nullable_separators_and_unterminated_intent_tokens() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    for pattern in [
        r"^https://api\.bilibili\.com/ads/?mission - reject",
        r"^https://api\.bilibili\.com/ads/*mission - reject",
        r"^https://api\.bilibili\.com/ads/{0,1}mission - reject",
        r"^https://api\.bilibili\.com/ads/(?#x)?mission - reject",
        r"^https://api\.bilibili\.com/api/ads - reject",
        r"^https://api\.bilibili\.com/api/log - reject",
        r"^https://api\.bilibili\.com/api/ads\??mission - reject",
    ] {
        manager.add_section_line("URL Rewrite", pattern);
        assert!(
            !manager.sections["URL Rewrite"].contains(&pattern.to_string()),
            "{pattern}"
        );
    }
}

#[test]
fn mainland_override_keeps_literal_separator_terminal_and_query_proofs() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    for pattern in [
        r"^https://api\.bilibili\.com/ads/branch - reject",
        r"^https://api\.bilibili\.com/api/ads$ - reject",
        r"^https://api\.bilibili\.com/api/ads\?key=1 - reject",
        r"^https://app\.bilibili\.com/x/v2/splash/(list|show|event/list2)\? - reject",
        r"^https://logs\.amap\.com/(collect|report) - reject",
        r"^https://logs\.amap\.com/[|]collect - reject",
        r"^https://logs\.amap\.com/collect\|report - reject",
        r"^https://api\.bilibili\.com/x/report/click - reject",
    ] {
        manager.add_section_line("URL Rewrite", pattern);
        assert!(
            manager.sections["URL Rewrite"].contains(&pattern.to_string()),
            "{pattern}"
        );
    }
}

#[test]
fn mainland_override_does_not_borrow_ad_intent_from_alternative_paths() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    let mixed = r#"^https?:\/\/www\.xiaohongshu\.com\/api\/sns\/(v\d+\/ads\/resource|v\d+\/hey\/\w+\/hey_gallery) data="https://example.test/empty.json""#;
    manager.add_section_line("Map Local", mixed);
    assert!(!manager.sections["Map Local"].contains(&mixed.to_string()));
    for pattern in [
        r"^https://api\.bilibili\.com/api/ad(mission|s) - reject",
        r"^https://api\.bilibili\.com/api/ad[ms] - reject",
        r"^https://api\.bilibili\.com/api/ads?(mission|s) - reject",
        r"^https://api\.bilibili\.com/x/report/click\d+ - reject",
    ] {
        manager.add_section_line("URL Rewrite", pattern);
        assert!(
            !manager.sections["URL Rewrite"].contains(&pattern.to_string()),
            "{pattern}"
        );
    }
}

#[test]
fn mainland_override_does_not_enable_promotion_cosmetics() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    let cosmetic = r"badges = type=http-response,pattern=^https:\/\/m5-zb\.amap\.com\/ws\/promotion-web\/resource(\/home)?\?,script-path=https://example.test/fixture.js,requires-body=true";
    manager.add_section_line("Script", cosmetic);
    assert!(!manager.sections["Script"].contains(&cosmetic.to_string()));
    for path in ["promotion", "promote", "commercial"] {
        let pattern = format!(r"^https://m5-zb\.amap\.com/ws/{path}/resource - reject");
        manager.add_section_line("URL Rewrite", &pattern);
        assert!(
            !manager.sections["URL Rewrite"].contains(&pattern),
            "{pattern}"
        );
    }
}

#[test]
fn mainland_override_keeps_common_splash_prefix_and_dedicated_telemetry_host() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    let splash = r#"http-response-jq ^https:\/\/app\.bilibili\.com\/x\/v2\/splash\/(list|show|event\/list2)\? '.data |= with_entries(if .key | IN("show", "event_list") then .value = [] else . end)'"#;
    manager.add_section_line("Body Rewrite", splash);
    assert!(manager.sections["Body Rewrite"].contains(&splash.to_string()));
    for pattern in [
        r"^https://api\.bilibili\.com/x/log/collect - reject",
        r"^https://logs\.amap\.com/v\d+/collect - reject",
    ] {
        manager.add_section_line("URL Rewrite", pattern);
        assert!(
            manager.sections["URL Rewrite"].contains(&pattern.to_string()),
            "{pattern}"
        );
    }
}

#[test]
fn compiled_ip_networks_keep_reject_and_drop_actions_in_vif_and_proxy_paths() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-ip-actions-{unique}"));
    let mut manager = AdBlockManager::new(root.clone());
    manager.extract_from_text(
        "IP-CIDR,198.51.100.0/24,REJECT\nIP-CIDR,203.0.113.64/26,REJECT-DROP\n",
        "REJECT",
        "Security",
        false,
        false,
        "fixture",
    );
    let module = manager.generate_module(&[], "cdn").content;
    let rewrites = module
        .split_once("[IP Rewrite]\n")
        .unwrap()
        .1
        .split('[')
        .next()
        .unwrap();
    let rules: Vec<_> = rewrites
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(
        rules,
        ["198.51.100.0/24 = reject", "203.0.113.64/26 = drop"]
    );
    assert!(module.contains("IP-CIDR,198.51.100.0/24,REJECT,no-resolve"));
    assert!(module.contains("IP-CIDR,203.0.113.64/26,REJECT-DROP,no-resolve"));
    assert!(crate::promax::validation::validate_surge_module(&module).is_empty());
    let converted = crate::promax::artifact::convert_surge_to_shadowrocket(&module);
    assert!(!converted.contains("[IP Rewrite]"));
    assert!(converted.contains("IP-CIDR,198.51.100.0/24,REJECT,no-resolve"));
    assert!(converted.contains("IP-CIDR,203.0.113.64/26,REJECT,no-resolve"));
    assert!(crate::promax::validation::validate_shadowrocket_module(&converted).is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reviewed_exact_ip_drops_keep_priority_with_other_accepted_networks() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-ip-rewrite-{unique}"));
    let mut manager = AdBlockManager::new(root.clone());
    manager.extract_from_text(
        "IP-CIDR,180.76.76.200/32\nIP-CIDR,180.76.76.200/32\nIP-CIDR,180.76.76.0/24\nIP-CIDR,8.8.8.8/32\nIP-CIDR,47.106.88.76/32\n",
        "REJECT-DROP", "Local", false, false,
        "https://raw.githubusercontent.com/app2smile/rules/master/rule/tieba-ad.list",
    );
    let module = manager.generate_module(&[], "cdn").content;
    assert!(module.contains("#!requirement=CORE_VERSION>=6010000"));
    let rewrites = module
        .split_once("[IP Rewrite]\n")
        .unwrap()
        .1
        .split('[')
        .next()
        .unwrap();
    let drops: Vec<_> = rewrites
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    assert_eq!(
        drops,
        [
            "180.76.76.200 = drop",
            "180.76.76.0/24 = drop",
            "47.106.88.76 = drop",
            "8.8.8.8 = drop"
        ]
    );
    assert!(module.contains("IP-CIDR,180.76.76.200/32,REJECT-DROP,no-resolve"));
    assert!(crate::promax::validation::validate_surge_module(&module).is_empty());
    manager.safety = crate::promax::safety::SafetyPolicy::from_lines(["IP-CIDR,180.76.76.0/24"]);
    let protected_module = manager.generate_module(&[], "cdn").content;
    assert!(
        compiled_ip_rewrite_lines(&protected_module)
            .iter()
            .all(|line| !line.starts_with("180.76.76."))
    );
    assert!(protected_module.contains("47.106.88.76 = drop"));
    let mut unreviewed = AdBlockManager::new(root.clone());
    unreviewed.extract_from_text(
        "IP-CIDR,180.76.76.200/32,REJECT",
        "REJECT-DROP",
        "Advertising",
        false,
        false,
        "unknown-source",
    );
    let unreviewed_module = unreviewed.generate_module(&[], "cdn").content;
    assert!(unreviewed_module.contains("180.76.76.200 = reject"));
    assert!(!unreviewed_module.contains("180.76.76.200 = drop"));
    let mut protected = AdBlockManager::new(root.clone());
    protected.safety = crate::promax::safety::SafetyPolicy::from_lines(["IP-CIDR,180.76.76.0/24"]);
    protected.extract_from_text(
        "IP-CIDR,180.76.76.200/32",
        "REJECT-DROP",
        "Local",
        false,
        false,
        "https://raw.githubusercontent.com/app2smile/rules/master/rule/tieba-ad.list",
    );
    assert!(
        !protected
            .generate_module(&[], "cdn")
            .content
            .contains("[IP Rewrite]")
    );
    fs::remove_dir_all(root).unwrap();
}

fn compiled_ip_rewrite_lines(module: &str) -> Vec<&str> {
    module
        .split_once("[IP Rewrite]\n")
        .map(|(_, section)| section.split('[').next().unwrap())
        .unwrap_or("")
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

#[test]
fn compiled_ip_rewrites_normalize_ipv6_and_host_bits_without_widening_prefixes() {
    let root = std::env::temp_dir().join(format!(
        "promax-ip-v6-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut manager = AdBlockManager::new(root.clone());
    manager.extract_from_text(
        "IP-CIDR6,2001:db8::1/128,REJECT\nIP-CIDR6,2001:0db8:0:0:0:0:0:1/128,REJECT\nIP-CIDR6,2001:db8:1::a/48,REJECT-DROP\nIP-CIDR,203.107.1.1/24,REJECT\nIP-CIDR,203.107.1.0/24,REJECT\n",
        "REJECT", "Security", false, false, "fixture",
    );
    let module = manager.generate_module(&[], "cdn").content;
    assert_eq!(
        compiled_ip_rewrite_lines(&module),
        [
            "2001:db8::1 = reject",
            "203.107.1.0/24 = reject",
            "2001:db8:1::/48 = drop"
        ]
    );
    assert!(module.contains("IP-CIDR6,2001:db8:1::a/48,REJECT-DROP,no-resolve"));
    assert!(crate::promax::validation::validate_surge_module(&module).is_empty());
    let converted = crate::promax::artifact::convert_surge_to_shadowrocket(&module);
    assert!(!converted.contains("[IP Rewrite]"));
    assert!(converted.contains("IP-CIDR6,2001:db8:1::a/48,REJECT,no-resolve"));
    assert!(crate::promax::validation::validate_shadowrocket_module(&converted).is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn compiled_ip_rewrites_preserve_policy_priority_and_ignore_source_order() {
    let root = std::env::temp_dir().join(format!(
        "promax-ip-order-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let rows = [
        "IP-CIDR,180.76.76.200/32,REJECT",
        "IP-CIDR,198.51.100.0/24,REJECT-DROP",
        "IP-CIDR,198.51.100.7/32,REJECT",
        "IP-CIDR,203.0.113.0/24,REJECT",
        "IP-CIDR,203.0.113.7/32,REJECT-DROP",
    ];
    let mut outputs = Vec::new();
    for reverse in [false, true] {
        let mut manager = AdBlockManager::new(root.clone());
        manager.extract_from_text(
            "IP-CIDR,180.76.76.200/32",
            "REJECT-DROP",
            "Local",
            false,
            false,
            "https://raw.githubusercontent.com/app2smile/rules/master/rule/tieba-ad.list",
        );
        let mut input = rows.to_vec();
        if reverse {
            input.reverse();
        }
        manager.extract_from_text(
            &input.join("\n"),
            "REJECT",
            "Security",
            false,
            false,
            "fixture",
        );
        let module = manager.generate_module(&[], "cdn").content;
        assert_eq!(
            compiled_ip_rewrite_lines(&module),
            [
                "180.76.76.200 = drop",
                "198.51.100.7 = reject",
                "203.0.113.0/24 = reject",
                "198.51.100.0/24 = drop",
                "203.0.113.7 = drop"
            ]
        );
        assert!(crate::promax::validation::validate_surge_module(&module).is_empty());
        outputs.push(module);
    }
    assert_eq!(outputs[0], outputs[1]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn compiled_ip_rewrites_use_only_unconditional_fallbacks_for_a_shared_network() {
    let root = std::env::temp_dir().join(format!(
        "promax-ip-shared-options-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut outputs = Vec::new();
    for input in [
        "IP-CIDR,198.51.100.0/24,REJECT,extended-matching\nIP-CIDR,198.51.100.7/24,REJECT\n",
        "IP-CIDR,198.51.100.7/24,REJECT\nIP-CIDR,198.51.100.0/24,REJECT,extended-matching\n",
    ] {
        let mut manager = AdBlockManager::new(root.clone());
        manager.extract_from_text(input, "REJECT", "Security", false, false, "fixture");
        let module = manager.generate_module(&[], "cdn").content;
        assert!(module.contains("IP-CIDR,198.51.100.7/24,REJECT,no-resolve\n"));
        assert!(!module.contains("IP-CIDR,198.51.100.0/24,REJECT,extended-matching"));
        assert_eq!(
            compiled_ip_rewrite_lines(&module),
            ["198.51.100.0/24 = reject"]
        );
        outputs.push(module);
    }
    assert_eq!(outputs[0], outputs[1]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn compiled_ip_rewrites_do_not_erase_source_network_requirements() {
    let root = std::env::temp_dir().join(format!(
        "promax-ip-conditions-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut manager = AdBlockManager::new(root.clone());
    manager.extract_from_text(
        "IP-CIDR,198.51.100.0/24,REJECT,force-cellular\nIP-CIDR6,2001:db8::1/128,REJECT\n",
        "REJECT",
        "Security",
        false,
        false,
        "fixture",
    );
    let module = manager.generate_module(&[], "cdn").content;
    assert_eq!(compiled_ip_rewrite_lines(&module), ["2001:db8::1 = reject"]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn compiled_ip_rewrites_respect_protected_direct_and_no_drop_intersections() {
    let root = std::env::temp_dir().join(format!(
        "promax-ip-exclusions-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut manager = AdBlockManager::new(root.clone());
    manager.safety = crate::promax::safety::SafetyPolicy::from_lines(["IP-CIDR,198.51.100.1/32"]);
    manager.extract_from_text(
        "IP-CIDR,198.51.100.0/24,REJECT\nIP-CIDR,203.0.113.0/24,REJECT\nIP-CIDR,203.0.113.7/32,DIRECT\nIP-CIDR,192.0.2.0/24,REJECT-DROP\nIP-CIDR,192.0.2.7/32,REJECT-NO-DROP\nIP-CIDR6,2001:db8::1/128,REJECT\nIP-ASN,64512,REJECT\nGEOIP,CN,REJECT\nDOMAIN,ads.example.test,REJECT\n",
        "REJECT", "Security", false, false, "fixture",
    );
    let module = manager.generate_module(&[], "cdn").content;
    assert_eq!(compiled_ip_rewrite_lines(&module), ["2001:db8::1 = reject"]);
    assert!(module.contains("IP-CIDR,192.0.2.0/24,REJECT-DROP,no-resolve"));
    assert!(module.contains("IP-CIDR,192.0.2.7/32,REJECT-NO-DROP,no-resolve"));
    assert!(crate::promax::validation::validate_surge_module(&module).is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn mainland_telemetry_and_ad_paths_are_not_operational_allow_exceptions() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    for host in [
        "data.mistat.xiaomi.com",
        "data.mistat.intl.xiaomi.com",
        "tracking.miui.com",
        "sentry.d.xiaomi.net",
        "logupdate.avlyun.sec.miui.com",
        "logs.amap.com",
        "extlog.snssdk.com",
        "t-ads.xiaohongshu.com",
    ] {
        let rule = format!("DOMAIN,{host}");
        manager.add_rule_line(&rule, "REJECT", "Privacy");
        assert!(manager.rules["REJECT"]["Privacy"].contains(&rule), "{host}");
    }
    let ads = r"^https://www\.xiaohongshu\.com/api/sns/v1/ads/resource - reject";
    manager.add_section_line("URL Rewrite", ads);
    assert!(manager.sections["URL Rewrite"].contains(&ads.to_string()));
    let telemetry = r"^https://api\.bilibili\.com/x/report/click - reject";
    manager.add_section_line("URL Rewrite", telemetry);
    assert!(manager.sections["URL Rewrite"].contains(&telemetry.to_string()));
    for host in ["www.xiaohongshu.com", "api.bilibili.com"] {
        manager.add_mitm_host(host);
    }
    manager.retain_referenced_mitm_hosts();
    assert!(manager.mitm_hosts.contains("www.xiaohongshu.com"));
    assert!(manager.mitm_hosts.contains("api.bilibili.com"));
    for host in [
        "weather-analytics-events.apple.com",
        "t10.baidu.com",
        "update.avlyun.sec.miui.com",
    ] {
        let rule = format!("DOMAIN,{host}");
        manager.add_rule_line(&rule, "REJECT", "Privacy");
        assert!(
            !manager.rules["REJECT"]["Privacy"].contains(&rule),
            "{host}"
        );
    }
    for line in [
        r"^https://api\.bilibili\.com/x/player/playurl - reject",
        r"^https://www\.xiaohongshu\.com/ - reject",
        r"^https://.*\.xiaohongshu\.com/api/ads - reject",
        r"^https://t10\.baidu\.com/ads - reject",
        r"^https://api\.bilibili\.com/login - reject",
        r"^https://api\.bilibili\.com/logout - reject",
        r"^https://api\.bilibili\.com/catalog - reject",
        r"^https://api\.bilibili\.com/dialog - reject",
        r"^https://api\.bilibili\.com/blog - reject",
        r"^https://api\.bilibili\.com/auth/telemetry - reject",
        r"^https://api\.bilibili\.com/update/log - reject",
        r"^https://api\.bilibili\.com/thumb/ads - reject",
        r"^https://api\.bilibili\.com/x/feed\?ads=1 - reject",
    ] {
        manager.add_section_line("URL Rewrite", line);
        assert!(
            !manager.sections["URL Rewrite"].contains(&line.to_string()),
            "{line}"
        );
    }
}

#[test]
fn published_index_refresh_is_english_and_preserves_local_inputs() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-index-{unique}"));
    let mut manager = AdBlockManager::new(root.clone());
    manager
        .rules
        .get_mut("REJECT")
        .unwrap()
        .get_mut("Advertising")
        .unwrap()
        .insert("DOMAIN,ads.fixture.test".to_string());
    let (paths, artifacts) = manager.generate_ruleset().unwrap();
    let metadata = manager.generate_catalog_artifacts(&paths, &artifacts);
    for artifact in artifacts.iter().chain(&metadata) {
        fs::create_dir_all(artifact.target.parent().unwrap()).unwrap();
        fs::write(&artifact.target, &artifact.content).unwrap();
    }
    let catalog_path = root.join("rulesets/AdBlock/catalog.json");
    let catalog = fs::read(&catalog_path).unwrap();
    let shard = fs::read(&artifacts[0].target).unwrap();
    let readme_path = root.join("rulesets/AdBlock/README.md");
    fs::write(&readme_path, "old index\n").unwrap();

    super::refresh_published_index(&root).unwrap();

    let readme = fs::read_to_string(&readme_path).unwrap();
    assert!(readme.starts_with("# AdBlock\n"));
    assert!(readme.contains("[English](README.md) · [简体中文](README.zh-CN.md)"));
    assert!(readme.contains("| Category | File | Rules | CDN | GitHub |"));
    assert!(readme.contains("| Advertising | `rulesets/AdBlock/AdBlock_Advertising.list` | 1 |"));
    assert_eq!(fs::read(&catalog_path).unwrap(), catalog);
    assert_eq!(fs::read(&artifacts[0].target).unwrap(), shard);
    super::refresh_published_index(&root).unwrap();
    assert_eq!(fs::read_to_string(&readme_path).unwrap(), readme);

    let mut invalid: serde_json::Value = serde_json::from_slice(&catalog).unwrap();
    invalid["shards"][0]["rule_count"] = serde_json::json!(2);
    fs::write(&catalog_path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    assert!(super::refresh_published_index(&root).is_err());
    assert_eq!(fs::read_to_string(&readme_path).unwrap(), readme);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn published_index_migration_keeps_language_and_table_separator() {
    let shards = [serde_json::json!({
        "category": "Advertising",
        "label_zh": "通用广告",
        "file": "rulesets/AdBlock/AdBlock_Advertising.list",
        "rule_count": 2,
        "cdn_url": "https://example.test/cdn",
        "github_url": "https://example.test/github",
    })];
    for (header, next, label) in [
        (
            "| Category | File | Rules | CDN | GitHub |",
            "## Modules",
            "Advertising",
        ),
        (
            "| 用途 | 文件 | 规则数 | CDN | GitHub |",
            "## 应用内脚本层",
            "通用广告",
        ),
    ] {
        let content =
            format!("# Index\n\n{header}\n| old separator |\n| old row |\n\n{next}\nunchanged\n");
        assert_eq!(
            super::update_migration_readme(&content, &shards).unwrap(),
            format!(
                "# Index\n\n{header}\n| --- | --- | --- | --- | --- |\n| {label} | `rulesets/AdBlock/AdBlock_Advertising.list` | 2 | [CDN](https://example.test/cdn) | [GitHub](https://example.test/github) |\n\n{next}\nunchanged\n"
            )
        );
    }
}

#[test]
fn supplemental_hagezi_sources_keep_categories_and_diagnostic_exemptions() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    let entries = manager.load_source_entries();
    assert!(
        !entries
            .iter()
            .any(|entry| entry.source.contains("MIUIPrivacy"))
    );
    for (name, category, host) in [
        ("popupads.txt", "Advertising", "ads.fixture.test"),
        ("fake.txt", "ThreatIntel", "scam.fixture.test"),
    ] {
        let url =
            format!("https://raw.githubusercontent.com/hagezi/dns-blocklists/main/adblock/{name}");
        let entry = entries.iter().find(|entry| entry.source == url).unwrap();
        assert_eq!(entry.category.as_deref(), Some(category));
        let body = format!(
            "[Adblock Plus]\n||{host}^\n||weather-analytics-events.apple.com^\n||o4506746191020032.ingest.sentry.io^\n"
        );
        assert!(
            manager
                .process_source_entries(std::slice::from_ref(entry), &HashMap::from([(url, body)]),)
                .is_empty()
        );
        assert_eq!(
            manager.rules["REJECT"][category],
            HashSet::from([format!("DOMAIN-SUFFIX,{host}")]),
        );
    }
}

#[test]
fn operational_services_are_not_rejected_rewritten_or_intercepted() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    for host in [
        "weather-analytics-events.apple.com",
        "firebaselogging-pa.googleapis.com",
        "crashlyticsreports-pa.googleapis.com",
        "sentry.io",
        "sentry.iobroker.net",
        "o4506746191020032.ingest.sentry.io",
        "o123.ingest.us.sentry.io",
        "o123.ingest.de.sentry.io",
        "notify.bugsnag.com",
        "sessions.bugsnag.com",
        "reports.crashlytics.com",
        "bam.nr-data.net",
        "mobile-crash.newrelic.com",
        "api.rollbar.com",
        "vortex.data.microsoft.com",
        "v10.vortex-win.data.microsoft.com",
        "ceuswatcab01.blob.core.windows.net",
        "v10.events.data.microsoft.com",
        "watson.telemetry.microsoft.com",
        "settings-win.data.microsoft.com",
        "crl.microsoft.com",
        "cp.wd.microsoft.com",
        "ctldl.windowsupdate.com",
        "www.msftconnecttest.com",
        "connectivitycheck.gstatic.com",
        "android.clients.google.com",
        "clients2.google.com",
        "auth.be.sec.miui.com",
        "update.avlyun.sec.miui.com",
    ] {
        for category in ["Advertising", "Privacy", "ThreatIntel_Ultimate"] {
            for kind in ["DOMAIN", "DOMAIN-SUFFIX"] {
                let rule = format!("{kind},{host}");
                manager.add_rule_line(&rule, "REJECT", category);
                assert!(
                    !manager.rules["REJECT"][category].contains(&rule),
                    "diagnostic endpoint rejected: {category}/{rule}"
                );
            }
        }
        let pattern = format!(r"^https://{}/", host.replace('.', r"\."));
        manager.add_section_line("URL Rewrite", &format!("{pattern} - reject"));
        manager.add_section_line(
            "Map Local",
            &format!(r#"{pattern} data="https://example.test/empty.json""#),
        );
        manager.add_mitm_host(host);
        assert!(
            !manager.mitm_hosts.contains(host),
            "diagnostic MITM: {host}"
        );
    }
    assert!(manager.sections["URL Rewrite"].is_empty());
    assert!(manager.sections["Map Local"].is_empty());
    for (rule, category) in [
        ("DOMAIN,ads-api.twitter.com", "Privacy"),
        ("DOMAIN-SUFFIX,doubleclick.net", "Advertising"),
        ("DOMAIN,mobileads.msn.com", "Advertising"),
        ("DOMAIN,ad.xiaomi.com", "Advertising"),
        ("DOMAIN,telemetry.adsninja.ca", "Privacy"),
        ("DOMAIN,malware.fixture.test", "ThreatIntel_Ultimate"),
        ("DOMAIN,sentry.io.attacker.test", "ThreatIntel_Ultimate"),
        (
            "DOMAIN,ingest.sentry.io.attacker.test",
            "ThreatIntel_Ultimate",
        ),
    ] {
        manager.add_rule_line(rule, "REJECT", category);
        assert!(manager.rules["REJECT"][category].contains(rule));
    }
}

#[test]
fn captcha_dependency_hosts_remain_available_without_allowing_geetest_siblings() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(root);
    manager.load_whitelist().unwrap();
    for host in [
        "hcaptcha.com",
        "a.hcaptcha.com",
        "newassets.hcaptcha.com",
        "onepass.geetest.com",
    ] {
        for category in ["Advertising", "ThreatIntel_Ultimate"] {
            for kind in ["DOMAIN", "DOMAIN-SUFFIX"] {
                let rule = format!("{kind},{host}");
                manager.add_rule_line(&rule, "REJECT", category);
                assert!(!manager.rules["REJECT"][category].contains(&rule), "{rule}");
            }
        }
    }
    for rule in [
        "DOMAIN-SUFFIX,doubleclick.net",
        "DOMAIN-SUFFIX,log.geetest.com",
        "DOMAIN,onepass.geetest.com.attacker.test",
    ] {
        manager.add_rule_line(rule, "REJECT", "Advertising");
        assert!(
            manager.rules["REJECT"]["Advertising"].contains(rule),
            "{rule}"
        );
    }
}

#[test]
fn pinterest_pin_widgets_remain_available_without_allowing_tracking_siblings() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(repo_root);
    manager.load_whitelist().unwrap();
    for (rule, category) in [
        (
            "DOMAIN,widgets.pinterest.com,extended-matching",
            "Advertising",
        ),
        ("DOMAIN-SUFFIX,widgets.pinterest.com", "AntiAD"),
    ] {
        manager.add_rule_line(rule, "REJECT", category);
        assert!(
            manager
                .quarantine
                .iter()
                .any(|record| record.candidate == rule),
            "normal Pin widget was rejected: {rule}"
        );
    }
    for host in ["log.pinterest.com", "ct.pinterest.com", "ads.pinterest.com"] {
        let rule = format!("DOMAIN,{host}");
        manager.add_rule_line(&rule, "REJECT", "Privacy");
        assert!(
            manager.rules["REJECT"]["Privacy"].contains(&rule),
            "dedicated tracking host lost: {host}"
        );
    }
    for parent in ["pinterest.com", "pinimg.com"] {
        let rule = format!("DOMAIN-SUFFIX,{parent}");
        manager.add_rule_line(&rule, "REJECT", "Advertising");
        assert!(
            manager
                .quarantine
                .iter()
                .any(|record| record.candidate == rule),
            "parent rejection would still break content: {parent}"
        );
    }
    let sibling = "DOMAIN-SUFFIX,s-passets.pinimg.com";
    manager.add_rule_line(sibling, "REJECT", "ThreatIntel_Ultimate");
    assert!(manager.rules["REJECT"]["ThreatIntel_Ultimate"].contains(sibling));
}

#[test]
fn shared_thumbnail_hosts_and_baidu_image_proxy_are_not_adblock_targets() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut manager = AdBlockManager::new(repo_root);
    manager.load_whitelist().unwrap();
    for host in [
        "t0.baidu.com",
        "t1.baidu.com",
        "t2.baidu.com",
        "t3.baidu.com",
        "t4.baidu.com",
        "t5.baidu.com",
        "t6.baidu.com",
        "t7.baidu.com",
        "t8.baidu.com",
        "t9.baidu.com",
        "t10.baidu.com",
        "t11.baidu.com",
        "t12.baidu.com",
        "dss0.bdstatic.com",
        "dss1.bdstatic.com",
        "dss2.bdstatic.com",
        "dss3.bdstatic.com",
        "ss0.bdstatic.com",
        "ss1.bdstatic.com",
        "ss2.bdstatic.com",
        "ss3.bdstatic.com",
        "i.ytimg.com",
        "i9.ytimg.com",
        "yt3.googleusercontent.com",
        "i.pinimg.com",
        "s.pinimg.com",
    ] {
        for category in ["Advertising", "Local", "ThreatIntel_Ultimate"] {
            let rule = format!("DOMAIN-SUFFIX,{host}");
            manager.add_rule_line(&rule, "REJECT", category);
            assert!(
                manager
                    .quarantine
                    .iter()
                    .any(|record| record.candidate == rule),
                "shared image host was admitted: {host} in {category}"
            );
        }
    }
    let pattern = r"^https?:\/\/t\d{2}\.baidu\.com";
    assert!(
        fancy_regex::Regex::new(pattern)
            .unwrap()
            .is_match("https://t10.baidu.com/it/u=2178613570,2866529354&fm=58")
            .unwrap()
    );
    manager.add_section_line(
        "Map Local",
        &format!(r#"{pattern} data="https://example.test/reject-img.gif""#),
    );
    assert!(manager.sections["Map Local"].is_empty());
    manager.add_rule_line("DOMAIN-SUFFIX,mobads.baidu.com", "REJECT", "Advertising");
    assert!(manager.rules["REJECT"]["Advertising"].contains("DOMAIN-SUFFIX,mobads.baidu.com"));
}

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
    let parts = AdBlockManager::partition_rules(
        &privacy_rules,
        &manager.source_exception_matcher().unwrap(),
    );
    let rendered_hosts: HashSet<String> = parts[0]
        .iter()
        .filter_map(|line| crate::promax::rule::SurgeRule::parse(line).ok())
        .filter(|rule| rule.kind == crate::promax::rule::RuleKind::Domain)
        .map(|rule| rule.payload.to_ascii_lowercase())
        .collect();
    for host in [
        "static.ads-twitter.com",
        "ads-api.twitter.com",
        "events.redditmedia.com",
        "analytics-sg.tiktok.com",
        "ads.tiktok.com",
        "api-adservices.apple.com",
        "iadsdk.apple.com",
        "alb.reddit.com",
        "mads-eu.amazon.com",
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
贴吧 = type=http-response,pattern=^https?:\/\/(tiebac|c\.tieba)\.baidu\.com/c/f/ad/getFeedAd,requires-body=true,binary-body-mode=1,script-path=https://raw.githubusercontent.com/app2smile/rules/master/js/tieba-proto.js,argument=per_filter_video_thread={{{per_filter_video}}}
[MITM]
hostname = %APPEND% tiebac.baidu.com
";
    let mut manager = AdBlockManager::new(root.clone());
    manager.safety = crate::promax::safety::SafetyPolicy::from_lines(["DOMAIN,t10.baidu.com"]);
    manager
        .integrate_functional_sections(&HashMap::from([(url.to_string(), fixture.to_string())]))
        .unwrap();
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
    assert!(
        AdBlockManager::partition_rules(rules, &manager.source_exception_matcher().unwrap())
            .iter()
            .all(Vec::is_empty)
    );
}

#[test]
fn adguard_wildcard_exceptions_preserve_domain_boundaries_and_case() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    manager.extract_from_text(
        "DOMAIN,Safe1.ads.example.test\nDOMAIN,media.safe2.ads.example.test\nDOMAIN,unsafe1.ads.example.test\nDOMAIN,ads.example.test\n@@||safe*.ads.example.test^\n",
        "REJECT", "Advertising", false, false, "fixture",
    );
    assert_eq!(
        AdBlockManager::partition_rules(
            &manager.rules["REJECT"]["Advertising"],
            &manager.source_exception_matcher().unwrap(),
        )[0],
        vec![
            "DOMAIN,ads.example.test".to_string(),
            "DOMAIN,unsafe1.ads.example.test".to_string(),
        ]
    );
}

#[test]
fn rule_partition_preserves_exception_scope_order_and_invalid_fallbacks() {
    let mut manager = AdBlockManager::new(std::env::temp_dir());
    manager
        .source_exceptions
        .insert("safe*.ads.example.test".to_string());
    let rules: HashSet<String> = [
        "DOMAIN,Safe1.ads.example.test",
        "DOMAIN-SUFFIX,media.safe2.ads.example.test",
        "DOMAIN-WILDCARD,*.safe3.ads.example.test",
        "DOMAIN,ads.example.test",
        "DOMAIN-KEYWORD,safe1.ads.example.test",
        r"DOMAIN-REGEX,safe1\.ads\.example\.test",
        "IP-CIDR,192.0.2.0/24,no-resolve",
        "IP-CIDR6,2001:db8::/32,no-resolve",
        "IP-ASN,64496,no-resolve",
        "GEOIP,CN",
        "PROCESS-NAME,ad-app",
        r"URL-REGEX,^https://safe1\.ads\.example\.test/",
        "DOMAIN,safe1.ads.example.test,invalid-option",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();

    assert_eq!(
        AdBlockManager::partition_rules(&rules, &manager.source_exception_matcher().unwrap()),
        [
            vec![
                "DOMAIN,ads.example.test",
                "DOMAIN-KEYWORD,safe1.ads.example.test",
                r"DOMAIN-REGEX,safe1\.ads\.example\.test"
            ],
            vec![
                "GEOIP,CN",
                "IP-ASN,64496,no-resolve",
                "IP-CIDR,192.0.2.0/24,no-resolve",
                "IP-CIDR6,2001:db8::/32,no-resolve"
            ],
            vec![
                "DOMAIN,safe1.ads.example.test,invalid-option",
                "PROCESS-NAME,ad-app",
                r"URL-REGEX,^https://safe1\.ads\.example\.test/"
            ],
        ],
    );
}

#[test]
fn fully_excepted_categories_emit_no_empty_shards() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-empty-shards-{unique}"));
    let mut manager = AdBlockManager::new(root.clone());
    manager
        .source_exceptions
        .insert("safe.ads.example.test".to_string());
    manager
        .rules
        .get_mut("REJECT")
        .unwrap()
        .get_mut("Advertising")
        .unwrap()
        .insert("DOMAIN,safe.ads.example.test".to_string());

    let (paths, artifacts) = manager.generate_ruleset().unwrap();
    assert!(paths.is_empty() && artifacts.is_empty());
    fs::remove_dir_all(root).unwrap();
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
fn benchmark_published_shard_generation() {
    use std::hint::black_box;
    use std::time::Instant;

    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("promax-shard-benchmark-{unique}"));
    let mut manager = AdBlockManager::new(root.clone());
    let mut expected = HashMap::new();
    for entry in fs::read_dir(repo.join("rulesets/AdBlock")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "list")
            || !path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("AdBlock_")
        {
            continue;
        }
        let content = fs::read_to_string(&path).unwrap();
        let category = content
            .lines()
            .find_map(|line| line.strip_prefix("# Category-Key: "))
            .unwrap();
        manager
            .rules
            .get_mut("REJECT")
            .unwrap()
            .get_mut(category)
            .unwrap()
            .extend(
                content
                    .lines()
                    .filter(|line| !line.is_empty() && !line.starts_with('#'))
                    .map(str::to_string),
            );
        expected.insert(path.file_name().unwrap().to_owned(), content);
    }
    assert!(!expected.is_empty());
    let count: usize = manager.rules["REJECT"].values().map(HashSet::len).sum();
    for exception_count in [0, 128] {
        manager.source_exceptions = (0..exception_count)
            .map(|index| format!("safe{index}?.benchmark.example"))
            .collect();
        let mut elapsed = Vec::new();
        let mut module_elapsed = Vec::new();
        let mut module_digest = None;
        for _ in 0..3 {
            let start = Instant::now();
            let (paths, artifacts) = black_box(&manager).generate_ruleset().unwrap();
            elapsed.push(start.elapsed());
            assert_eq!(paths.len(), expected.len());
            assert_eq!(artifacts.len(), expected.len());
            for artifact in &artifacts {
                assert_eq!(
                    artifact.content,
                    expected[artifact.target.file_name().unwrap()],
                    "{}",
                    artifact.target.display()
                );
            }
            let start = Instant::now();
            let module = manager
                .generate_module_with_artifacts(&paths, &artifacts, "github")
                .unwrap();
            module_elapsed.push(start.elapsed());
            let stable_content = module
                .content
                .lines()
                .filter(|line| !line.starts_with("#!date="))
                .collect::<Vec<_>>()
                .join("\n");
            let digest = format!("{:x}", md5::compute(stable_content.as_bytes()));
            if let Some(previous) = &module_digest {
                assert_eq!(&digest, previous);
            }
            module_digest = Some(digest);
            black_box((artifacts, module));
        }
        elapsed.sort_unstable();
        module_elapsed.sort_unstable();
        eprintln!(
            "published shard generation: rules={count}, shards={}, exceptions={exception_count}, rounds=3, median={:?}",
            expected.len(),
            elapsed[1]
        );
        eprintln!(
            "published module generation: rules={count}, exceptions={exception_count}, rounds=3, median={:?}, digest={}",
            module_elapsed[1],
            module_digest.unwrap()
        );
    }
    fs::remove_dir_all(root).unwrap();
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
    fs::create_dir_all(root.join("rulesets/Sources/conf")).unwrap();
    fs::write(
        root.join("rulesets/Sources/conf/SurgeConf_DirectPorts.list"),
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
