use super::*;

const PUBLIC_RULE_URL: &str = "https://raw.githubusercontent.com/example/rules/main/ads.yaml";

fn template(content: &str) -> Template {
    Template {
        name: "fixture.yaml".to_owned(),
        client: Client::FlClash,
        content: content.to_owned(),
    }
}

fn fixture(url: &str) -> String {
    // JSON string syntax is also YAML string syntax; no interpolated YAML source values.
    let url = serde_json::to_string(url).unwrap();
    format!(
        "proxy-providers:\n  subscription:\n    type: http\n    url: {url}\n    path: ./providers/subscription.yaml\n    interval: 3600\nproxy-groups:\n  - name: PROXY\n    type: select\n    use: [subscription]\n    proxies: [DIRECT]\nrule-providers:\n  public:\n    type: http\n    behavior: classical\n    format: yaml\n    url: {PUBLIC_RULE_URL}\n    path: ./rules/public.yaml\n    interval: 86400\nrules:\n  - RULE-SET,public,REJECT\n  - MATCH,PROXY\n"
    )
}

fn binding(url: &str) -> BTreeMap<String, String> {
    BTreeMap::from([("SUBSCRIPTION_1".to_owned(), url.to_owned())])
}

const DNS_FIXTURE: &str = "
ipv6: false
dns:
  ipv6: false
  default-nameserver: ['https://192.0.2.53/dns-query']
  nameserver: ['https://resolver.example/dns-query']
  nameserver-policy:
    example.test: ['https://regional.example/dns-query#DIRECT']
tun:
  stack: mixed
  dns-hijack: ['udp://any:53']
";

#[test]
fn windows_dns_opt_in_preserves_source_and_round_trips_the_local_profile() {
    let source = template(&format!("{}{DNS_FIXTURE}", fixture("{{SUBSCRIPTION_1}}")));
    let subscriptions = binding("https://subscriptions.example/fixture");
    let normal = parse(&super::super::render(&source, &subscriptions).unwrap()).unwrap();
    assert!(normal.0["tun"]["strict-route"].is_null());
    assert!(normal.0["dns"]["nameserver-policy"]["geosite:cn"].is_null());

    let guarded = super::super::render_flclash_windows_dns(&source, &subscriptions).unwrap();
    let effective = parse(&guarded).unwrap();
    assert_eq!(effective.0["tun"]["enable"], true);
    assert_eq!(effective.0["tun"]["auto-route"], true);
    assert_eq!(effective.0["tun"]["strict-route"], true);
    assert_eq!(effective.0["tun"]["stack"], "mixed");
    assert_eq!(
        effective.0["tun"]["dns-hijack"],
        serde_json::json!(["udp://any:53", "tcp://any:53"])
    );
    assert_eq!(
        effective.0["dns"]["nameserver-policy"]["geosite:cn"],
        CN_DOH
    );
    assert_eq!(
        effective.0["dns"]["nameserver-policy"]["example.test"],
        normal.0["dns"]["nameserver-policy"]["example.test"]
    );
    for key in [
        "ipv6",
        "rules",
        "proxy-groups",
        "proxy-providers",
        "rule-providers",
    ] {
        assert_eq!(effective.0[key], normal.0[key], "{key}");
    }
    let imported = super::super::sanitize(&template(&guarded)).unwrap();
    assert!(!imported.content.contains("subscriptions.example"));
    let reapplied = super::super::render_flclash_windows_dns(&imported, &subscriptions).unwrap();
    assert_eq!(reapplied.as_str(), guarded.as_str());
}

#[test]
fn windows_dns_opt_in_rejects_plaintext_unsafe_fragments_and_unpinned_bootstrap() {
    for (before, after) in [
        ("https://resolver.example/dns-query", "192.0.2.53"),
        (
            "https://resolver.example/dns-query",
            "https://resolver.example/dns-query#skip-cert-verify=true",
        ),
        (
            "https://192.0.2.53/dns-query",
            "https://bootstrap.example/dns-query",
        ),
        (
            "https://regional.example/dns-query#DIRECT",
            "udp://192.0.2.54",
        ),
    ] {
        let dns = DNS_FIXTURE.replace(before, after);
        let source = template(&format!("{}{dns}", fixture("{{SUBSCRIPTION_1}}")));
        assert!(
            super::super::render_flclash_windows_dns(
                &source,
                &binding("https://subscriptions.example/fixture")
            )
            .is_err()
        );
    }
}

#[test]
fn flclash_client_has_its_own_snake_case_wire_name() {
    assert_eq!(
        serde_json::to_string(&Client::FlClash).unwrap(),
        "\"fl_clash\""
    );
    assert_eq!(
        serde_json::from_str::<Client>("\"fl_clash\"").unwrap(),
        Client::FlClash
    );
}

#[test]
fn sanitize_only_replaces_subscription_fields_and_discards_comments() {
    let source = format!(
        "# {{{{SUBSCRIPTION_55}}}} https://comments.example/?token=comment-secret\n{}",
        fixture("https://subscriptions.example/list?token=fixture-private-marker")
    );
    let original = template(&source);
    let sanitized = super::super::sanitize(&original).unwrap();
    assert_eq!(original.content, source);
    assert!(!sanitized.content.contains("fixture-private-marker"));
    assert!(!sanitized.content.contains("comment-secret"));
    assert!(!sanitized.content.contains("SUBSCRIPTION_55"));
    assert!(sanitized.content.contains(PUBLIC_RULE_URL));
    assert_eq!(
        super::super::subscription_slots(&sanitized),
        ["SUBSCRIPTION_1"]
    );
    let repeated = super::super::sanitize(&sanitized).unwrap();
    assert_eq!(repeated.content, sanitized.content);
    let rendered = super::super::render(
        &sanitized,
        &binding("https://subscriptions.example/replacement?token=new-fixture"),
    )
    .unwrap();
    assert!(rendered.contains(PUBLIC_RULE_URL));
    assert!(rendered.contains("new-fixture"));
    let parsed = parse(&rendered).unwrap();
    assert_eq!(
        parsed.0["rules"],
        serde_json::json!(["RULE-SET,public,REJECT", "MATCH,PROXY"])
    );
}

#[test]
fn sanitizing_rejects_copies_of_path_authenticated_subscriptions() {
    let subscription = "https://subscriptions.example/private-fixture-marker";
    let source = fixture(subscription);
    let mut named = template(&source);
    named.name = format!("profile {subscription}");
    for input in [
        template(&source.replace(PUBLIC_RULE_URL, subscription)),
        template(&source.replace(
            "    type: select",
            &format!("    type: select\n    filter: 'match {subscription}'"),
        )),
        named,
    ] {
        let error = super::super::sanitize(&input)
            .err()
            .expect("duplicate subscription");
        assert!(!error.to_string().contains("private-fixture-marker"));
    }
    assert!(super::super::sanitize(&template(&source)).is_ok());
    let slotted = fixture("{{SUBSCRIPTION_1}}").replace(PUBLIC_RULE_URL, subscription);
    assert!(super::super::render(&template(&slotted), &binding(subscription)).is_err());
}

#[test]
fn render_structurally_preserves_quoted_urls_without_yaml_injection() {
    let single =
        fixture("{{SUBSCRIPTION_1}}").replace("\"{{SUBSCRIPTION_1}}\"", "'{{SUBSCRIPTION_1}}'");
    for source in [fixture("{{SUBSCRIPTION_1}}"), single] {
        let url = "https://subscriptions.example/list?q='quoted',\"double\"&token=fixture";
        let rendered = super::super::render(&template(&source), &binding(url)).unwrap();
        let parsed = parse(&rendered).unwrap();
        assert_eq!(parsed.0["proxy-providers"]["subscription"]["url"], url);
        assert_eq!(parsed.0.as_object().unwrap().len(), 4);
        assert!(rendered.contains(PUBLIC_RULE_URL));
        assert!(super::super::is_flclash_config(&rendered));
    }
    for url in [
        "https://subscriptions.example/list\nlisteners: []",
        "https://subscriptions.example/list\rnext",
        "https://subscriptions.example/list?q=a b",
        "https://subscriptions.example/list?q={{value}}",
        "https://user:password@subscriptions.example/list",
        "https://subscriptions.example/list#fragment",
        "http://subscriptions.example/list",
        "file:///subscriptions.yaml",
    ] {
        assert!(
            super::super::render(&template(&fixture("{{SUBSCRIPTION_1}}")), &binding(url)).is_err()
        );
    }
}

#[test]
fn subscription_slots_come_only_from_actual_provider_urls() {
    let content = format!(
        "# {{{{SUBSCRIPTION_33}}}}\n{}",
        fixture("{{SUBSCRIPTION_1}}")
    );
    let source = template(&content);
    assert_eq!(
        super::super::subscription_slots(&source),
        ["SUBSCRIPTION_1"]
    );
    assert!(super::super::render(&source, &BTreeMap::new()).is_err());
    let mut extra = binding("https://subscriptions.example/list");
    extra.insert(
        "SUBSCRIPTION_33".to_owned(),
        "https://subscriptions.example/extra".to_owned(),
    );
    assert!(super::super::render(&source, &extra).is_err());
    let wrong = BTreeMap::from([(
        "SUBSCRIPTION_2".to_owned(),
        "https://subscriptions.example/list".to_owned(),
    )]);
    assert!(super::super::render(&source, &wrong).is_err());
    assert!(
        super::super::render(
            &template(&fixture("https://subscriptions.example/raw")),
            &binding("https://subscriptions.example/list")
        )
        .is_err()
    );
    for invalid in [
        "{{SUBSCRIPTION_0}}",
        "{{SUBSCRIPTION_01}}",
        "{{SUBSCRIPTION_65}}",
        "{{SUBSCRIPTION_1}",
        "https://subscriptions.example/{{SUBSCRIPTION_1}}",
    ] {
        let source = template(&fixture(invalid));
        assert!(super::super::subscription_slots(&source).is_empty());
        assert!(
            super::super::render(&source, &binding("https://subscriptions.example/list")).is_err()
        );
    }
}

#[test]
fn rejects_ambiguous_or_unbounded_yaml_without_exposing_source() {
    let valid = fixture("{{SUBSCRIPTION_1}}");
    let malformed = [
        format!("{valid}\nrules: [MATCH,DIRECT]\n"),
        valid.replace(
            "type: http\n    url:",
            "type: http\n    url: https://subscriptions.example/duplicate-secret\n    url:",
        ),
        valid.replace("  subscription:\n", "  subscription: {}\n  subscription:\n"),
        format!("{valid}\n---\nprivate: duplicate-secret\n"),
        valid.replace("type: http", "type: !private http"),
        valid.replace("    interval: 3600", "    interval: &interval 3600"),
        valid.replace("    interval: 3600", "    interval: *missing"),
        valid.replace("    interval: 3600", "    <<: {interval: 3600}"),
        valid.replace("    interval: 3600", "    interval: [unterminated"),
        valid.replace(
            "    interval: 3600",
            "    ? [complex, key]\n    : unsupported",
        ),
        "proxy-providers: null".to_owned(),
        "".to_owned(),
        "\0".to_owned(),
    ];
    for source in malformed {
        assert!(!super::super::is_flclash_config(&source));
        match super::super::sanitize(&template(&source)) {
            Err(error) => assert!(!error.to_string().contains("duplicate-secret")),
            Ok(_) => panic!("unsupported YAML was accepted"),
        }
    }
    let oversized = " ".repeat(MAX_TEMPLATE_BYTES + 1);
    assert!(!super::super::is_flclash_config(&oversized));
    assert!(matches!(
        super::super::sanitize(&template(&oversized)),
        Err(VaultError::TooLarge)
    ));
}

#[test]
fn rejects_inline_nodes_credentials_and_unsupported_structures() {
    let valid = fixture("https://subscriptions.example/list?token=allowed-only-here");
    for source in [
        format!(
            "{valid}\nproxies: [{{name: real-node, type: ss, server: example.com, port: 443, cipher: aes-128-gcm, password: private}}]\n"
        ),
        format!("{valid}\nproxies: [{{name: real-node, type: direct}}]\n"),
        valid.replace("    type: http", "    type: inline\n    payload: []"),
        valid.replace("    type: http", "    type: file"),
        valid.replace(
            "    interval: 3600",
            "    interval: 3600\n    header: {Authorization: fixture-secret}",
        ),
        valid.replace(
            "    interval: 3600",
            "    interval: 3600\n    override: {password: fixture-secret}",
        ),
        format!("{valid}\nlisteners: []\n"),
        format!("{valid}\nsecret: fixture-secret\n"),
        format!("{valid}\nexternal-controller: 127.0.0.1:9090\n"),
        valid.replace(
            PUBLIC_RULE_URL,
            "https://rules.example/list?%74oken=fixture-secret",
        ),
        valid.replace(PUBLIC_RULE_URL, "https://user:password@rules.example/list"),
        format!(
            "{valid}\ndns:\n  nameserver: ['https://dns.example/query?token=fixture-secret']\n"
        ),
        valid.replace(
            "    path: ./providers/subscription.yaml",
            "    path: ../private/subscription.yaml",
        ),
        valid.replace(
            "    path: ./providers/subscription.yaml",
            "    path: C:/private/subscription.yaml",
        ),
        valid.replace(
            "    path: ./providers/subscription.yaml",
            "    path: /private/subscription.yaml",
        ),
        valid.replace("    type: select", "    type: smart"),
    ] {
        assert!(!super::super::is_flclash_config(&source));
        assert!(super::super::sanitize(&template(&source)).is_err());
    }
}

#[test]
fn rejects_credentials_after_url_delimiters_in_public_fields() {
    let valid = fixture("{{SUBSCRIPTION_1}}");
    for delimiter in ["'", "\"", ",", "<", ">"] {
        for key in ["%74oken", "%6bey", "%61uth", "access%5ftoken", "api%5fkey"] {
            let private_url =
                format!("https://public.example/path{delimiter}tail?{key}=fixture-private-marker");
            let quoted = serde_json::to_string(&private_url).unwrap();
            for source in [
                valid.replace(PUBLIC_RULE_URL, &quoted),
                format!("{valid}\ndns:\n  nameserver: [{quoted}]\n"),
                format!("{valid}\ngeox-url:\n  mmdb: {quoted}\n"),
                valid.replace(
                    "    interval: 3600",
                    &format!("    interval: 3600\n    health-check:\n      url: {quoted}"),
                ),
                valid.replace(
                    "    type: select",
                    &format!("    type: select\n    icon: {quoted}"),
                ),
                valid.replace(
                    "    type: select",
                    &format!("    type: select\n    filter: {quoted}"),
                ),
            ] {
                assert!(!super::super::is_flclash_config(&source));
                match super::super::sanitize(&template(&source)) {
                    Err(error) => assert!(!error.to_string().contains("fixture-private-marker")),
                    Ok(_) => panic!("private public-field URL was accepted"),
                }
            }
        }
    }
    let dns_credentials = format!(
        "{valid}\ndns:\n  nameserver: ['tls://fixture-user:fixture-password@dns.example:853']\n"
    );
    assert!(super::super::sanitize(&template(&dns_credentials)).is_err());
    // Subscription credentials are still confined to the field that is replaced.
    let sanitized = super::super::sanitize(&template(&fixture(
        "https://subscriptions.example/path'quoted?%74oken=fixture-private-marker",
    )))
    .unwrap();
    assert!(!sanitized.content.contains("fixture-private-marker"));
    assert!(sanitized.content.contains(PUBLIC_RULE_URL));
}

#[test]
fn detector_requires_supported_config_shape_and_accepts_import_export() {
    for url in [
        "https://subscriptions.example/list?token=private-fixture",
        "{{SUBSCRIPTION_1}}",
    ] {
        let source = fixture(url);
        assert!(super::super::is_flclash_config(&source));
        let sanitized = super::super::sanitize(&template(&source)).unwrap();
        assert!(super::super::is_flclash_config(&sanitized.content));
        let rendered = super::super::render(
            &sanitized,
            &binding("https://subscriptions.example/fixture"),
        )
        .unwrap();
        assert!(super::super::is_flclash_config(&rendered));
    }
    for invalid in [
        "arbitrary: yaml",
        "proxy-providers: {}\nproxy-groups: []\nrules: []",
        "proxy-providers: {}\nproxy-groups: [PROXY]\nrules: [MATCH,DIRECT]",
        "[Proxy Group]\nPROXY = select, DIRECT",
    ] {
        assert!(!super::super::is_flclash_config(invalid));
    }
}

#[test]
fn preserves_group_order_inline_rules_and_full_protection_actions() {
    let source = fixture("{{SUBSCRIPTION_1}}")
        .replace("  - name: PROXY", "  - name: 开启防护\n    type: select\n    proxies: [PASS-RULE]\n    hidden: true\n  - name: 兼容放行\n    type: select\n    proxies: [PASS]\n    hidden: true\n  - name: PROXY")
        + "sub-rules:\n  protection:\n    - MATCH,开启防护\n    - RULE-SET,public,REJECT\n    - MATCH,PASS\n";
    let sanitized = super::super::sanitize(&template(&source)).unwrap();
    let parsed = parse(&sanitized.content).unwrap();
    assert_eq!(parsed.0["proxy-groups"][0]["name"], "开启防护");
    assert_eq!(parsed.0["proxy-groups"][1]["name"], "兼容放行");
    assert_eq!(parsed.0["proxy-groups"][2]["name"], "PROXY");
    assert_eq!(
        parsed.0["sub-rules"]["protection"],
        serde_json::json!(["MATCH,开启防护", "RULE-SET,public,REJECT", "MATCH,PASS"])
    );
}
