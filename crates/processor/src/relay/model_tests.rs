use super::*;
use serde_json::json;

#[test]
fn substore_dependency_aliases_rename_and_cascade() {
    for key in ["dialer-proxy", "detour", "underlying-proxy"] {
        let source = format!(
            r#"[{{"name":"Base","type":"socks5","server":"base.invalid","port":1080}},{{"name":"Child","type":"socks5","server":"child.invalid","port":1080,"{key}":"Base","future":{{"keep":true}}}}]"#
        );
        let sub = Subscription::parse(&source).unwrap();
        assert_eq!(
            sub.nodes()[1].to_sing_box(true).err(),
            Some(ModelError::Unsupported)
        );
        let renamed = sub
            .render(
                &[Health::Unknown; 2],
                &EnhanceOptions {
                    rename: true,
                    sort: false,
                },
            )
            .unwrap();
        let output: Value = serde_json::from_str(&renamed).unwrap();
        assert_eq!(output[1][key], output[0]["name"]);
        assert_eq!(output[1]["future"]["keep"], true);
        let removed = sub
            .render(&[Health::Failed, Health::Unknown], &unchanged())
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&removed).unwrap(), json!([]));
    }
    assert!(Subscription::parse(r#"[{"name":"A","type":"socks5","server":"a.invalid","port":1080,"detour":"B","dialer-proxy":"B"}]"#).is_err());
}

#[test]
fn substore_udp_projection_is_limited_to_supported_protocols() {
    for (protocol, fields) in [
        ("ss", r#", "cipher":"aes-128-gcm","password":"synthetic""#),
        ("trojan", r#", "password":"synthetic""#),
    ] {
        for udp in [true, false] {
            let source = format!(
                r#"[{{"name":"A","type":"{protocol}","server":"a.invalid","port":443,"udp":{udp}{fields}}}]"#
            );
            let sub = Subscription::parse(&source).unwrap();
            let projected = sub.nodes()[0].to_sing_box(false).unwrap();
            assert_eq!(
                projected.get("network").and_then(Value::as_str),
                if udp { None } else { Some("tcp") }
            );
        }
    }
    let sub = Subscription::parse(
        r#"[{"name":"A","type":"socks5","server":"a.invalid","port":1080,"udp":true}]"#,
    )
    .unwrap();
    assert_eq!(sub.nodes()[0].to_sing_box(false).unwrap()["version"], "5");
    let sub = Subscription::parse(r#"[{"name":"A","type":"ss","server":"a.invalid","port":443,"cipher":"aes-128-gcm","password":"synthetic","udp":"false"}]"#).unwrap();
    assert_eq!(
        sub.nodes()[0].to_sing_box(false).err(),
        Some(ModelError::Unsupported)
    );
}

#[test]
fn substore_json_preserves_unknown_nodes_and_projects_only_known_fields() {
    let source = r#"[{"name":"A","type":"trojan","server":"a.example.invalid","port":443,"password":"synthetic","sni":"identity.example.invalid","skip-cert-verify":true},{"name":"B","type":"future","server":"b.example.invalid","port":1234,"future":{"keep":true}},{"name":"C","type":"socks5","server":"c.example.invalid","port":1080}]"#;
    let sub = Subscription::parse(source).unwrap();
    assert_eq!(sub.format_label(), "Sub-Store JSON");
    assert_eq!(
        sub.nodes()[0].to_sing_box(false).err(),
        Some(ModelError::SecurityBlocked)
    );
    assert_eq!(
        sub.nodes()[0].to_sing_box(true).unwrap()["tls"]["insecure"],
        true
    );
    assert_eq!(
        sub.nodes()[1].to_sing_box(true).err(),
        Some(ModelError::Unsupported)
    );
    let rendered = sub
        .render(
            &[Health::SecurityBlocked, Health::Unknown, Health::Failed],
            &EnhanceOptions::default(),
        )
        .unwrap();
    let output: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(output.as_array().unwrap().len(), 2);
    let retained = output
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node.get("future").is_some())
        .unwrap();
    assert_eq!(retained["future"]["keep"], true);
    assert!(retained["name"].as_str().unwrap().contains('B'));
    assert!(semantic_equal(&rendered, &rendered));
}

#[test]
fn substore_json_rejects_ambiguous_and_invalid_input() {
    for source in [
        r#"[{"name":"A","tag":"A","type":"socks5","server":"a.invalid","port":1}]"#,
        r#"[{"name":"A","type":"socks5","server":"a.invalid","port":1,"server_port":1}]"#,
        r#"[{"name":"A","name":"B","type":"socks5","server":"a.invalid","port":1}]"#,
        r#"[{"name":"","type":"socks5","server":"a.invalid","port":1}]"#,
        r#"[{"name":"A","type":"socks5","server":"a.invalid","port":0}]"#,
        r#"{"subscriptions":[],"settings":{}}"#,
    ] {
        assert!(Subscription::parse(source).is_err());
    }
}

fn unchanged() -> EnhanceOptions {
    EnhanceOptions {
        rename: false,
        sort: false,
    }
}

#[test]
fn producer_node_containers_roundtrip_without_losing_fields() {
    for source in [
        "proxies:\n  - {name: B, type: trojan, server: b.invalid, port: 443, password: synthetic, future: {keep: true}}\n  - {name: A, type: http, server: a.invalid, port: 80}\n",
        r#"{"proxies":[{"name":"B","type":"trojan","server":"b.invalid","port":443,"password":"synthetic","future":{"keep":true,"url":"https://docs.invalid"}},{"name":"A","type":"http","server":"a.invalid","port":80}]}"#,
        "- name: B\n  type: http\n  server: b.invalid\n  port: 80\n  future: {keep: true}\n- name: A\n  type: http\n  server: a.invalid\n  port: 80\n",
        r#"{"outbounds":[{"tag":"B","type":"http","server":"b.invalid","server_port":80,"future":{"keep":true}},{"tag":"A","type":"http","server":"a.invalid","server_port":80}],"endpoints":[]}"#,
    ] {
        let sub = Subscription::parse(source).unwrap();
        assert_eq!(
            &*sub.render(&[Health::Unknown; 2], &unchanged()).unwrap(),
            source
        );
        assert_eq!(
            sub.nodes()[0].to_sing_box(false).err(),
            Some(ModelError::Unsupported)
        );
        let rendered = sub
            .render(&[Health::Unknown; 2], &EnhanceOptions::default())
            .unwrap();
        let parsed = Subscription::parse(&rendered).unwrap();
        assert_eq!(parsed.nodes().len(), 2);
        assert!(rendered.contains("keep"));
        if source.contains("endpoints") {
            assert_eq!(
                serde_json::from_str::<Value>(&rendered).unwrap()["endpoints"],
                json!([])
            );
        }
        assert!(semantic_equal(&rendered, &rendered));
        assert!(!semantic_equal(source, &rendered));
        let removed = sub
            .render(&[Health::Unknown, Health::Failed], &unchanged())
            .unwrap();
        assert_eq!(Subscription::parse(&removed).unwrap().nodes().len(), 1);
    }
}

#[test]
fn uri_and_base64_subscriptions_keep_encoding_unknown_nodes_and_credentials() {
    use base64::{Engine as _, engine::general_purpose::*};
    let source = "# nodes\ntrojan://synthetic@b.invalid:443?sni=identity.invalid#B\nfuture://opaque@a.invalid:1234?keep=value#A\n";
    for encoding in [
        None,
        Some(STANDARD),
        Some(STANDARD_NO_PAD),
        Some(URL_SAFE),
        Some(URL_SAFE_NO_PAD),
    ] {
        let input = encoding
            .as_ref()
            .map_or_else(|| source.to_owned(), |engine| engine.encode(source));
        let sub = Subscription::parse(&input).unwrap();
        assert_eq!(sub.nodes().len(), 2);
        assert_eq!(
            sub.nodes()[0].to_sing_box(false).unwrap()["tls"]["server_name"],
            "identity.invalid"
        );
        assert_eq!(
            sub.nodes()[1].to_sing_box(false).err(),
            Some(ModelError::Unsupported)
        );
        assert_eq!(
            &*sub.render(&[Health::Unknown; 2], &unchanged()).unwrap(),
            &input
        );
        let rendered = sub
            .render(&[Health::Unknown; 2], &EnhanceOptions::default())
            .unwrap();
        let parsed = Subscription::parse(&rendered).unwrap();
        assert_eq!(parsed.nodes().len(), 2);
        let decoded = match encoding {
            Some(_) => String::from_utf8(
                [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD]
                    .iter()
                    .find_map(|engine| engine.decode(rendered.as_bytes()).ok())
                    .unwrap(),
            )
            .unwrap(),
            None => rendered.to_string(),
        };
        assert!(decoded.contains("opaque@a.invalid:1234?keep=value"));
        assert!(decoded.contains("synthetic@b.invalid:443?sni=identity.invalid"));
        assert!(semantic_equal(&rendered, &rendered));
        let removed = sub
            .render(&[Health::Failed, Health::Unknown], &unchanged())
            .unwrap();
        assert_eq!(Subscription::parse(&removed).unwrap().nodes().len(), 1);
    }
}

#[test]
fn structured_nodes_reject_full_configs_nonempty_endpoints_and_ambiguous_names() {
    for input in [
        r#"{"outbounds":[],"endpoints":[{"type":"wireguard"}]}"#,
        r#"{"outbounds":[],"route":{}}"#,
        "proxies: []\nproxy-groups: []",
        "proxies:\n - {name: duplicate, type: http, server: a.invalid, port: 80}\n - {name: duplicate, type: http, server: b.invalid, port: 80}",
        "trojan://p@a.invalid:443#duplicate\ntrojan://p@b.invalid:443#duplicate",
    ] {
        assert!(Subscription::parse(input).is_err());
    }
    assert!(Subscription::parse(&"a".repeat(4 * 1024 * 1024 + 1)).is_err());
}

#[test]
fn yaml_dependencies_and_comments_only_noop_are_preserved() {
    let source = "# retained on no-op\nproxies:\n - {name: Base, type: http, server: a.invalid, port: 80}\n - {name: Child, type: http, server: b.invalid, port: 80, dialer-proxy: Base}\n";
    let sub = Subscription::parse(source).unwrap();
    assert_eq!(
        &*sub.render(&[Health::Unknown; 2], &unchanged()).unwrap(),
        source
    );
    let renamed = sub
        .render(&[Health::Unknown; 2], &EnhanceOptions::default())
        .unwrap();
    let parsed = Subscription::parse(&renamed).unwrap();
    assert!(
        parsed
            .render(&[Health::Unknown; 2], &EnhanceOptions::default())
            .is_ok()
    );
    assert!(semantic_equal(
        source,
        &source.replace("# retained on no-op", "# another comment")
    ));
}

#[test]
fn reality_requires_explicit_utls_and_compact_surge_is_not_a_uri() {
    let source =
        "n=trojan, example.invalid, 443, password=synthetic, test-url=https://example.invalid/";
    assert_eq!(Subscription::parse(source).unwrap().format_label(), "Surge");
    for source in [
        "vless://id@node.invalid:443?security=reality&pbk=synthetic#A",
        r#"{"outbounds":[{"type":"vless","tag":"A","server":"node.invalid","server_port":443,"uuid":"id","tls":{"enabled":true,"reality":{"enabled":true,"public_key":"synthetic"}}}]}"#,
        "proxies:\n - {name: A, type: vless, server: node.invalid, port: 443, uuid: id, tls: true, reality-opts: {public-key: synthetic}}",
    ] {
        let sub = Subscription::parse(source).unwrap();
        assert_eq!(
            sub.nodes()[0].to_sing_box(false).err(),
            Some(ModelError::Unsupported)
        );
        assert_eq!(
            &*sub.render(&[Health::Unknown], &unchanged()).unwrap(),
            source
        );
    }
    let source = "vless://id@node.invalid:443?security=reality&pbk=synthetic&fp=chrome#A";
    assert!(
        Subscription::parse(source).unwrap().nodes()[0]
            .to_sing_box(false)
            .is_ok()
    );
}

#[test]
fn common_mihomo_protocols_project_without_losing_transport_identity() {
    for (protocol, expected, fields) in [
        ("socks5", "socks", json!({})),
        ("http", "http", json!({})),
        (
            "ss",
            "shadowsocks",
            json!({"cipher":"aes-128-gcm","password":"synthetic"}),
        ),
        ("trojan", "trojan", json!({"password":"synthetic"})),
        (
            "anytls",
            "anytls",
            json!({"password":"synthetic","udp":true}),
        ),
        (
            "vmess",
            "vmess",
            json!({"uuid":"123e4567-e89b-12d3-a456-426614174000","cipher":"auto","alterId":0}),
        ),
        (
            "vless",
            "vless",
            json!({"uuid":"123e4567-e89b-12d3-a456-426614174000"}),
        ),
        ("hysteria2", "hysteria2", json!({"password":"synthetic"})),
        (
            "tuic",
            "tuic",
            json!({"uuid":"123e4567-e89b-12d3-a456-426614174000","password":"synthetic","congestion-controller":"cubic","udp-relay-mode":"native"}),
        ),
    ] {
        let mut value = json!({"name":"A","type":protocol,"server":"node.invalid","port":443,"_subName":"subscription"});
        value
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        let input = json!({"proxies":[value]}).to_string();
        let sub = Subscription::parse(&input).unwrap();
        let projected = sub.nodes()[0].to_sing_box(false).unwrap();
        assert_eq!(projected["type"], expected);
        assert_eq!(projected["server"], "node.invalid");
        assert_eq!(projected["server_port"], 443);
        assert!(projected.get("_subName").is_none());
    }
}

#[test]
fn external_surge_is_retained_but_never_projected() {
    let external = "External = external, exec=/synthetic/tool, local-port=1234, args=first, args=\"second,value\", udp-relay=true\n";
    let source = format!(
        "A = anytls, a.invalid, 443, password=synthetic\nB = anytls, b.invalid, 443, password=synthetic\n{external}"
    );
    let subscription = Subscription::parse(&source).unwrap();
    assert_eq!(subscription.nodes().len(), 3);
    for compatibility in [false, true] {
        assert_eq!(
            subscription.nodes()[2].to_sing_box(compatibility).err(),
            Some(ModelError::Unsupported)
        );
    }
    let rendered = subscription
        .render(
            &[Health::Failed, Health::Unknown, Health::Unknown],
            &unchanged(),
        )
        .unwrap();
    assert!(rendered.contains(external));
    assert!(!semantic_equal(
        &source,
        &source.replace("args=first", "args=changed")
    ));
    assert!(semantic_equal(&source, &source));
    let renamed = subscription
        .render(&[Health::Unknown; 3], &EnhanceOptions::default())
        .unwrap();
    assert!(
        renamed
            .contains("exec=/synthetic/tool, local-port=1234, args=first, args=\"second,value\"")
    );
}

#[test]
fn external_surge_requires_unambiguous_exec_and_port() {
    for source in [
        "X = external, local-port=1, args=a",
        "X = external, exec=, local-port=1",
        "X = external, exec=\"   \", local-port=1",
        "X = external, exec=tool, args=a",
        "X = external, exec=tool, local-port=0",
        "X = external, exec=tool, local-port=65536",
        "X = external, exec=tool, local-port=1, exec=other",
        "X = anytls, a.invalid, 443, args=a, args=b",
    ] {
        assert!(Subscription::parse(source).is_err(), "{source}");
    }
}

#[test]
fn external_reference_is_updated_on_rename() {
    let source = "Base = anytls, a.invalid, 443, password=synthetic\nX = external, exec=tool, local-port=1234, args=a, args=b, underlying-proxy=Base\n";
    let subscription = Subscription::parse(source).unwrap();
    let output = subscription
        .render(&[Health::Unknown; 2], &EnhanceOptions::default())
        .unwrap();
    assert!(output.contains("args=a, args=b, underlying-proxy=\"ANYTLS | Base\""));
}

#[test]
fn anytls_projection_preserves_quoted_password_alpn_and_tls_identity() {
    let source = r#""HK, A" = anytls, node.example.invalid, 443, password="p,a=ss#word", sni=identity.example.invalid, alpn="h2,http/1.1", skip-cert-verify=false
"#;
    let subscription = Subscription::parse(source).unwrap();
    let node = &subscription.nodes()[0];
    assert_eq!(node.name(), "HK, A");
    assert_eq!(node.server(), "node.example.invalid");
    assert_eq!(node.protocol(), "anytls");
    assert_eq!(
        node.to_sing_box(false).unwrap(),
        json!({
            "type":"anytls", "tag":"HK, A", "server":"node.example.invalid", "server_port":443,
            "password":"p,a=ss#word", "tls":{"enabled":true,"server_name":"identity.example.invalid","alpn":["h2","http/1.1"],"insecure":false}
        })
    );
    assert_eq!(
        &*subscription
            .render(&[Health::Healthy { latency_ms: 1 }], &unchanged())
            .unwrap(),
        source
    );
}

#[test]
fn insecure_profile_is_security_blocked_until_explicit_compatibility() {
    let subscription = Subscription::parse(
        "A = anytls, node.example.invalid, 443, password=synthetic, skip-cert-verify=true\n",
    )
    .unwrap();
    assert_eq!(
        subscription.nodes()[0].to_sing_box(false).err(),
        Some(ModelError::SecurityBlocked)
    );
    assert_eq!(
        subscription.nodes()[0].to_sing_box(true).unwrap()["tls"]["insecure"],
        true
    );
}

#[test]
fn unknown_surge_field_is_retained_but_never_silently_projected() {
    let source = "# original\nA = anytls, node.example.invalid, 443, password=synthetic, future-option=\"a,b=#\"\n";
    let subscription = Subscription::parse(source).unwrap();
    assert_eq!(
        subscription.nodes()[0].to_sing_box(false).err(),
        Some(ModelError::Unsupported)
    );
    assert_eq!(
        &*subscription
            .render(&[Health::Unknown], &unchanged())
            .unwrap(),
        source
    );
}

#[test]
fn unknown_protocol_is_not_lost_when_another_node_fails() {
    let subscription = Subscription::parse("A = future-protocol, future.example.invalid, 443, future=\"x,y\"\nB = socks5, failed.example.invalid, 1080\n").unwrap();
    let rendered = subscription
        .render(&[Health::Unknown, Health::Failed], &unchanged())
        .unwrap();
    assert_eq!(
        &*rendered,
        "A = future-protocol, future.example.invalid, 443, future=\"x,y\"\n"
    );
}

#[test]
fn native_json_preserves_unknown_fields_and_original_dialect() {
    let source = r#"{"outbounds":[{"type":"anytls","tag":"A","server":"node.example.invalid","server_port":443,"password":"synthetic","tls":{"enabled":true,"server_name":"identity.example.invalid","alpn":["h2"]},"future":{"keep":[1,2]}}]}"#;
    let subscription = Subscription::parse(source).unwrap();
    assert_eq!(
        subscription.nodes()[0].to_sing_box(false).err(),
        Some(ModelError::Unsupported)
    );
    assert_eq!(
        &*subscription
            .render(&[Health::Unknown], &unchanged())
            .unwrap(),
        source
    );
}

#[test]
fn native_projection_preserves_reality_and_transport_without_config_injection() {
    let source = r#"[{"type":"vless","tag":"A","server":"node.example.invalid","server_port":443,"uuid":"11111111-1111-4111-8111-111111111111","flow":"xtls-rprx-vision","tls":{"enabled":true,"server_name":"identity.example.invalid","utls":{"enabled":true,"fingerprint":"chrome"},"reality":{"enabled":true,"public_key":"synthetic-public-key","short_id":"00112233"}}}]"#;
    let subscription = Subscription::parse(source).unwrap();
    assert_eq!(
        subscription.nodes()[0].to_sing_box(false).unwrap(),
        serde_json::from_str::<Value>(source).unwrap()[0]
    );
}

#[test]
fn native_local_files_processes_and_ambient_dialers_are_not_executed() {
    for field in [
        r#""tls":{"enabled":true,"certificate_path":"/private/synthetic"}"#,
        r#""bind_interface":"en0""#,
        r#""routing_mark":1"#,
        r#""plugin":"synthetic-executable""#,
    ] {
        let source = format!(
            r#"[{{"type":"anytls","tag":"A","server":"node.example.invalid","server_port":443,"password":"synthetic",{field}}}]"#
        );
        let subscription = Subscription::parse(&source).unwrap();
        assert_eq!(
            subscription.nodes()[0].to_sing_box(true).err(),
            Some(ModelError::Unsupported)
        );
    }
}

#[test]
fn complete_configs_settings_and_ambiguous_nodes_are_rejected() {
    for source in [
        "[General]\nloglevel=notify\n[Proxy]\nA = socks5, node.example.invalid, 1080\n",
        r#"{"outbounds":[],"route":{"rules":[]}}"#,
        r#"{"outbounds":[],"inbounds":[]}"#,
        r#"{"subscriptions":[],"settings":{}}"#,
        r#"[{"type":"direct","tag":"DIRECT"}]"#,
        "A = anytls, node.example.invalid, 443, password=one, password=two\n",
        "A = socks5, node.example.invalid, 1080\nA = socks5, another.example.invalid, 1080\n",
        "A = socks5, node.example.invalid, 1080, password=\"unterminated\n",
    ] {
        assert!(Subscription::parse(source).is_err());
    }
}

#[test]
fn surge_proxy_section_is_retained_without_admitting_other_sections() {
    let source = "# source\n[Proxy]\nA = socks5, node.example.invalid, 1080\n";
    let subscription = Subscription::parse(source).unwrap();
    assert_eq!(
        &*subscription
            .render(&[Health::Unknown], &unchanged())
            .unwrap(),
        source
    );
}

#[test]
fn failed_dependency_removes_only_required_dependent_closure() {
    let source = "Base = socks5, base.example.invalid, 1080\nChild = socks5, child.example.invalid, 1080, underlying-proxy=Base\nGrandchild = socks5, grand.example.invalid, 1080, underlying-proxy=Child\nKeep = socks5, keep.example.invalid, 1080\n";
    let subscription = Subscription::parse(source).unwrap();
    assert_eq!(
        &*subscription
            .render(
                &[
                    Health::Failed,
                    Health::Unknown,
                    Health::SecurityBlocked,
                    Health::Healthy { latency_ms: 1 }
                ],
                &unchanged()
            )
            .unwrap(),
        "Keep = socks5, keep.example.invalid, 1080\n"
    );
}

#[test]
fn rename_updates_chain_references_and_is_idempotent() {
    let source = "Base = socks5, base.example.invalid, 1080\nChild = socks5, child.example.invalid, 1080, underlying-proxy=Base, password=synthetic\n";
    let subscription = Subscription::parse(source).unwrap();
    let options = EnhanceOptions::default();
    let rendered = subscription
        .render(&[Health::Unknown; 2], &options)
        .unwrap();
    let reparsed = Subscription::parse(&rendered).unwrap();
    assert_eq!(reparsed.nodes().len(), 2);
    assert_ne!(reparsed.nodes()[0].name(), "Base");
    assert_eq!(
        &*reparsed.render(&[Health::Unknown; 2], &options).unwrap(),
        &*rendered
    );
    let without_base = reparsed
        .render(&[Health::Failed, Health::Unknown], &unchanged())
        .unwrap();
    assert_eq!(
        Subscription::parse(&without_base)
            .map(|s| s.nodes().len())
            .unwrap_or(0),
        0
    );
}

#[test]
fn invalid_dependencies_are_visible_instead_of_silently_rewired() {
    for source in [
        "A = socks5, a.example.invalid, 1080, underlying-proxy=Missing\n",
        "A = socks5, a.example.invalid, 1080, underlying-proxy=B\nB = socks5, b.example.invalid, 1080, underlying-proxy=A\n",
    ] {
        let subscription = Subscription::parse(source).unwrap();
        assert_eq!(
            subscription
                .render(
                    &[Health::Unknown; 2][..subscription.nodes().len()],
                    &EnhanceOptions::default()
                )
                .err(),
            Some(ModelError::InvalidDependency)
        );
    }
}

#[test]
fn long_node_names_are_not_subject_to_gist_filename_limit() {
    let name = "节点".repeat(40);
    let source = format!("{name} = socks5, node.example.invalid, 1080\n");
    let subscription = Subscription::parse(&source).unwrap();
    let rendered = subscription
        .render(&[Health::Unknown], &EnhanceOptions::default())
        .unwrap();
    let reparsed = Subscription::parse(&rendered).unwrap();
    assert!(reparsed.nodes()[0].name().contains(&name));
}

#[test]
fn alias_names_remain_distinct_with_identical_endpoints() {
    let subscription = Subscription::parse(
        "A = socks5, same.example.invalid, 1080\nB = socks5, same.example.invalid, 1080\n",
    )
    .unwrap();
    let rendered = subscription
        .render(&[Health::Unknown; 2], &EnhanceOptions::default())
        .unwrap();
    let reparsed = Subscription::parse(&rendered).unwrap();
    assert_eq!(reparsed.nodes().len(), 2);
    assert_ne!(reparsed.nodes()[0].name(), reparsed.nodes()[1].name());
}

#[test]
fn local_region_hints_group_before_protocol_and_keep_names_idempotent() {
    let source = "US West = socks5, us.example.invalid, 1080\n日本 Main = trojan, jp.example.invalid, 443, password=synthetic\nASUS = socks5, unknown.example.invalid, 1080\n";
    let subscription = Subscription::parse(source).unwrap();
    let options = EnhanceOptions::default();
    let rendered = subscription
        .render(&[Health::Unknown; 3], &options)
        .unwrap();
    let reparsed = Subscription::parse(&rendered).unwrap();
    let names: Vec<_> = reparsed.nodes().iter().map(Node::name).collect();
    assert_eq!(
        names,
        [
            "TROJAN | [JP] 日本 Main",
            "SOCKS5 | [US] US West",
            "SOCKS5 | ASUS"
        ]
    );
    assert_eq!(
        reparsed
            .render(&[Health::Unknown; 3], &options)
            .unwrap()
            .as_str(),
        rendered.as_str()
    );
}

#[test]
fn region_sort_can_leave_original_names_and_endpoint_identity_untouched() {
    let source = r#"[{"type":"socks","tag":"US First","server":"same.example.invalid","server_port":443},{"type":"socks","tag":"🇯🇵 Second","server":"same.example.invalid","server_port":443,"future":{"preserve":true}}]"#;
    let subscription = Subscription::parse(source).unwrap();
    let rendered = subscription
        .render(
            &[Health::Unknown; 2],
            &EnhanceOptions {
                rename: false,
                sort: true,
            },
        )
        .unwrap();
    let original: Value = serde_json::from_str(source).unwrap();
    let actual: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(actual, json!([original[1], original[0]]));
}

#[test]
fn region_renaming_updates_chain_references_and_retains_named_aliases() {
    let source = "Japan Base = socks5, same.example.invalid, 1080\n\"[JP] Alias\" = socks5, same.example.invalid, 1080\nUS Child = socks5, child.example.invalid, 1080, underlying-proxy=\"Japan Base\", future=\"keep,a=b\"\n";
    let subscription = Subscription::parse(source).unwrap();
    let rendered = subscription
        .render(&[Health::Unknown; 3], &EnhanceOptions::default())
        .unwrap();
    let reparsed = Subscription::parse(&rendered).unwrap();
    let names: Vec<_> = reparsed.nodes().iter().map(Node::name).collect();
    assert!(names.contains(&"SOCKS5 | [JP] Japan Base"));
    assert!(names.contains(&"SOCKS5 | [JP] Alias"));
    assert!(rendered.contains("underlying-proxy=\"SOCKS5 | [JP] Japan Base\""));
    assert!(rendered.contains("future=\"keep,a=b\""));
    assert!(reparsed.dependencies().is_ok());
}

#[test]
fn health_length_mismatch_cannot_drop_nodes() {
    let subscription = Subscription::parse("A = socks5, node.example.invalid, 1080\n").unwrap();
    assert_eq!(
        subscription.render(&[], &unchanged()).err(),
        Some(ModelError::InvalidHealth)
    );
}

#[test]
fn semantic_equality_ignores_formatting_comments_not_secrets_order_or_transport() {
    let source =
        "# x\nA = anytls, node.example.invalid, 443, password=\"a,b\", alpn=\"h2,http/1.1\"\n";
    assert!(semantic_equal(
        source,
        "A=anytls,node.example.invalid,443,alpn=\"h2,http/1.1\",password=\"a,b\"\n# y\n"
    ));
    assert!(!semantic_equal(
        source,
        "A=anytls,node.example.invalid,443,alpn=\"h2,http/1.1\",password=\"b,a\"\n"
    ));
    assert!(!semantic_equal(
        "A=socks5,a.example.invalid,1080\nB=socks5,b.example.invalid,1080\n",
        "B=socks5,b.example.invalid,1080\nA=socks5,a.example.invalid,1080\n"
    ));
    assert!(!semantic_equal(
        r#"[{"type":"anytls","tag":"A","server":"a.example.invalid","server_port":443,"password":"x"}]"#,
        r#"[{"type":"anytls","tag":"A","server":"a.example.invalid","server_port":443,"password":"y"}]"#
    ));
    assert!(!semantic_equal("not a subscription", "not a subscription"));
}

#[test]
fn common_surge_protocols_project_only_faithful_known_fields() {
    let subscription = Subscription::parse("S = socks5, socks.example.invalid, 1080, username=alice, password=synthetic\nT = trojan, tls.example.invalid, 443, password=synthetic, sni=identity.example.invalid\nSS = ss, ss.example.invalid, 8388, encrypt-method=aes-128-gcm, password=synthetic\n").unwrap();
    assert_eq!(
        subscription.nodes()[0].to_sing_box(false).unwrap(),
        json!({"type":"socks","version":"5","tag":"S","server":"socks.example.invalid","server_port":1080,"username":"alice","password":"synthetic"})
    );
    assert_eq!(
        subscription.nodes()[1].to_sing_box(false).unwrap()["tls"],
        json!({"enabled":true,"server_name":"identity.example.invalid"})
    );
    assert_eq!(
        subscription.nodes()[2].to_sing_box(false).unwrap()["method"],
        "aes-128-gcm"
    );
}

#[test]
fn duplicate_json_keys_cannot_hide_credentials_or_routing_settings() {
    assert!(Subscription::parse(r#"[{"type":"anytls","tag":"A","server":"a.example.invalid","server_port":443,"password":"one","password":"two"}]"#).is_err());
    assert!(Subscription::parse(r#"{"outbounds":[],"outbounds":[]}"#).is_err());
}

#[test]
fn transformations_preserve_unknown_surge_bytes_and_separate_unterminated_final_line() {
    let source =
        "Z = socks5, z.example.invalid, 1080, future=\"a,b\"\nA = socks5, a.example.invalid, 1080";
    let subscription = Subscription::parse(source).unwrap();
    let rendered = subscription
        .render(&[Health::Unknown; 2], &EnhanceOptions::default())
        .unwrap();
    let reparsed = Subscription::parse(&rendered).unwrap();
    assert_eq!(reparsed.nodes().len(), 2);
    assert!(rendered.contains("future=\"a,b\""));
}

#[test]
fn native_chain_rename_and_failed_dependency_removal_preserve_unknown_json_values() {
    let source = r#"{"outbounds":[{"type":"socks","tag":"Base","server":"base.example.invalid","server_port":1080},{"type":"socks","tag":"Child","server":"child.example.invalid","server_port":1080,"detour":"Base","future":{"keep":true}}]}"#;
    let subscription = Subscription::parse(source).unwrap();
    let rendered = subscription
        .render(&[Health::Unknown; 2], &EnhanceOptions::default())
        .unwrap();
    let value: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(
        value["outbounds"][1]["detour"],
        value["outbounds"][0]["tag"]
    );
    assert_eq!(value["outbounds"][1]["future"], json!({"keep":true}));
    let reparsed = Subscription::parse(&rendered).unwrap();
    assert_eq!(
        reparsed.nodes()[1].to_sing_box(false).err(),
        Some(ModelError::Unsupported)
    );
    let removed = reparsed
        .render(&[Health::Failed, Health::Unknown], &unchanged())
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&removed).unwrap(),
        json!({"outbounds":[]})
    );
}

#[test]
fn projection_decodes_escaped_quote_and_backslash_without_changing_password() {
    let source = r#"A = anytls, node.example.invalid, 443, password="a\"b\\c"
"#;
    let subscription = Subscription::parse(source).unwrap();
    assert_eq!(
        subscription.nodes()[0].to_sing_box(false).unwrap()["password"],
        "a\"b\\c"
    );
}

#[test]
fn client_probe_metadata_cannot_override_engine_targets_or_timeouts() {
    let source = "A = anytls, node.example.invalid, 443, password=synthetic, tfo=true, udp-relay=false, block-quic=true, test-url=https://never-fetch.example.invalid/, test-timeout=99999\n";
    let subscription = Subscription::parse(source).unwrap();
    let projected = subscription.nodes()[0].to_sing_box(false).unwrap();
    assert_eq!(projected["tcp_fast_open"], true);
    assert!(projected.get("test-url").is_none());
    assert!(projected.get("test-timeout").is_none());
    assert_eq!(
        &*subscription
            .render(&[Health::Unknown], &unchanged())
            .unwrap(),
        source
    );
    let invalid = Subscription::parse(
        "A = anytls, node.example.invalid, 443, password=synthetic, udp-relay=maybe\n",
    )
    .unwrap();
    assert_eq!(
        invalid.nodes()[0].to_sing_box(false).err(),
        Some(ModelError::Unsupported)
    );
}

#[test]
fn unverified_quoted_escapes_are_preserved_not_probed_or_semantically_collapsed() {
    let source = r#"A = anytls, node.example.invalid, 443, password="a\nb"
"#;
    let literal_backslash = r#"A = anytls, node.example.invalid, 443, password="a\\nb"
"#;
    let subscription = Subscription::parse(source).unwrap();
    assert_eq!(
        subscription.nodes()[0].to_sing_box(false).err(),
        Some(ModelError::Unsupported)
    );
    assert_eq!(
        &*subscription
            .render(&[Health::Unknown], &unchanged())
            .unwrap(),
        source
    );
    assert!(!semantic_equal(source, literal_backslash));

    let ambiguous_name = r#""Base\nA" = socks5, base.example.invalid, 1080
"#;
    let subscription = Subscription::parse(ambiguous_name).unwrap();
    assert_eq!(
        subscription
            .render(&[Health::Unknown], &EnhanceOptions::default())
            .err(),
        Some(ModelError::Unsupported)
    );
    let ambiguous_reference = r#""Base\\nA" = socks5, base.example.invalid, 1080
Child = socks5, child.example.invalid, 1080, underlying-proxy="Base\nA"
"#;
    let subscription = Subscription::parse(ambiguous_reference).unwrap();
    assert_eq!(
        subscription
            .render(&[Health::Unknown; 2], &EnhanceOptions::default())
            .err(),
        Some(ModelError::Unsupported)
    );
}

#[test]
fn inline_comment_quotes_and_backslashes_are_not_node_syntax() {
    let source = r#"A = anytls, node.example.invalid, 443, password="synthetic#data" # note "unfinished\
"#;
    let without_comment = r#"A = anytls, node.example.invalid, 443, password="synthetic#data"
"#;
    let subscription = Subscription::parse(source).unwrap();
    assert_eq!(
        subscription.nodes()[0].to_sing_box(false).unwrap()["password"],
        "synthetic#data"
    );
    assert_eq!(
        &*subscription
            .render(&[Health::Unknown], &unchanged())
            .unwrap(),
        source
    );
    assert!(semantic_equal(source, without_comment));
}

#[test]
fn leading_bom_stays_outside_node_names_and_survives_chain_renaming() {
    let source = "\u{feff}Base = socks5, node.example.invalid, 443\nChild = socks5, node2.example.invalid, 443, underlying-proxy=Base\n";
    let subscription = Subscription::parse(source).unwrap();
    assert_eq!(subscription.nodes()[0].name(), "Base");
    assert_eq!(
        &*subscription
            .render(&[Health::Unknown; 2], &unchanged())
            .unwrap(),
        source
    );
    let rendered = subscription
        .render(&[Health::Unknown; 2], &EnhanceOptions::default())
        .unwrap();
    assert!(rendered.starts_with('\u{feff}'));
    assert_eq!(rendered.matches('\u{feff}').count(), 1);
    let reparsed = Subscription::parse(&rendered).unwrap();
    assert_eq!(reparsed.nodes().len(), 2);
    assert!(
        reparsed
            .nodes()
            .iter()
            .all(|node| !node.name().contains('\u{feff}'))
    );
    assert_eq!(
        &*reparsed
            .render(&[Health::Unknown; 2], &EnhanceOptions::default())
            .unwrap(),
        &*rendered
    );
}
