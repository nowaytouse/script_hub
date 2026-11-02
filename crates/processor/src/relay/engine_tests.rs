use super::*;
use serde_json::json;

#[test]
fn engine_lookup_ignores_relative_directories_and_preserves_order() {
    let root = std::env::temp_dir().join(format!("engine-discovery-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let first = root.join("first");
    let second = root.join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    std::fs::write(first.join("sing-box"), []).unwrap();
    std::fs::write(second.join("sing-box"), []).unwrap();
    assert_eq!(
        find_in_directories(
            "sing-box".as_ref(),
            [PathBuf::from("."), second.clone(), first]
        ),
        Some(second.join("sing-box").canonicalize().unwrap())
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn one_verified_https_success_wins_over_an_unavailable_target() {
    assert_eq!(
        classify(
            [true, true],
            &[vec![Attempt::Transport], vec![Attempt::Success(42)]],
            true
        ),
        Health::Healthy { latency_ms: 42 }
    );
}

#[test]
fn verified_success_does_not_wait_for_another_test_target() {
    let mut calls = 0;
    let result = probe_targets(
        [true, true],
        &AtomicBool::new(false),
        || Ok(true),
        |_| {
            calls += 1;
            assert_eq!(calls, 1, "a verified node must not run another HTTPS probe");
            Attempt::Success(7)
        },
    )
    .unwrap();
    assert_eq!(result, Health::Healthy { latency_ms: 7 });
}

#[test]
fn failed_nodes_still_require_all_four_transport_failures() {
    let mut calls = 0;
    let result = probe_targets(
        [true, true],
        &AtomicBool::new(false),
        || Ok(true),
        |_| {
            calls += 1;
            Attempt::Transport
        },
    )
    .unwrap();
    assert_eq!((result, calls), (Health::Failed, 4));
}

#[test]
fn host_aliases_share_one_active_scheduling_permit() {
    let hosts = Mutex::new(HashSet::new());
    let first = HostPermit::try_acquire(&hosts, "NODE.example.invalid")
        .unwrap()
        .unwrap();
    assert!(
        HostPermit::try_acquire(&hosts, "node.example.invalid.")
            .unwrap()
            .is_none()
    );
    assert!(
        HostPermit::try_acquire(&hosts, "other.example.invalid")
            .unwrap()
            .is_some()
    );
    drop(first);
    assert!(
        HostPermit::try_acquire(&hosts, "node.example.invalid.")
            .unwrap()
            .is_some()
    );
    assert_eq!(host_key("[2001:DB8::1]"), host_key("2001:db8:0:0:0:0:0:1"));
}

#[test]
fn red_requires_two_baseline_valid_targets_and_repeated_transport_failure() {
    let failures = [
        vec![Attempt::Transport, Attempt::Transport],
        vec![Attempt::Transport, Attempt::Transport],
    ];
    assert_eq!(classify([true, true], &failures, true), Health::Failed);
    assert_eq!(classify([true, false], &failures, true), Health::Unknown);
    assert_eq!(classify([true, true], &failures, false), Health::Unknown);
}

#[test]
fn http_policy_reply_or_incomplete_attempts_cannot_be_red() {
    assert_eq!(
        classify(
            [true, true],
            &[
                vec![Attempt::Policy],
                vec![Attempt::Transport, Attempt::Transport]
            ],
            true
        ),
        Health::Unknown
    );
    assert_eq!(
        classify(
            [true, true],
            &[vec![Attempt::Transport], vec![Attempt::Transport]],
            true
        ),
        Health::Unknown
    );
}

#[test]
fn synthesized_config_keeps_tls_identity_and_has_no_direct_fallback() {
    let outbound = json!({"type":"anytls","server":"node.example.invalid","server_port":443,"password":"synthetic","tls":{"enabled":true,"server_name":"identity.example.invalid","alpn":["h2"]}});
    let config =
        isolated_config(&outbound, 12345, "synthetic-user", "synthetic-auth", false).unwrap();
    assert_eq!(config["outbounds"].as_array().unwrap().len(), 1);
    assert_eq!(config["outbounds"][0]["tls"], outbound["tls"]);
    assert_eq!(config["route"]["final"], "candidate");
    assert_eq!(config["inbounds"][0]["listen"], "127.0.0.1");
    assert_eq!(
        config["inbounds"][0]["users"][0]["password"],
        "synthetic-auth"
    );
}

#[test]
fn doh_does_not_rewrite_endpoint_or_enable_insecure_tls() {
    let outbound = json!({"type":"anytls","server":"node.example.invalid","server_port":443,"password":"synthetic","tls":{"enabled":true}});
    let config = isolated_config(&outbound, 12345, "u", "p", true).unwrap();
    assert_eq!(config["dns"]["servers"][0]["type"], "https");
    assert_eq!(config["outbounds"][0]["server"], "node.example.invalid");
    assert!(config["outbounds"][0]["tls"].get("insecure").is_none());
}

#[test]
fn external_resources_or_local_service_addresses_are_not_executed() {
    for outbound in [
        json!({"type":"direct","server":"node.example.invalid","server_port":443}),
        json!({"type":"anytls","server":"127.0.0.1","server_port":443}),
        json!({"type":"anytls","server":"node.example.invalid","server_port":443,"tls":{"certificate_path":"private-file"}}),
        json!({"type":"anytls","server":"node.example.invalid","server_port":443,"detour":"ambient"}),
    ] {
        assert!(isolated_config(&outbound, 12345, "u", "p", false).is_err());
    }
}

#[test]
fn cancelled_batch_does_not_start_engine_or_network() {
    let subscription =
        Subscription::parse("A = anytls, node.example.invalid, 443, password=synthetic\n").unwrap();
    let cancel = AtomicBool::new(true);
    let result = check_nodes(&subscription, &ProbeOptions::default(), &cancel, &|_, _| {});
    assert!(matches!(result, Err(EngineError::Cancelled)));
}

#[test]
fn zero_or_excessive_concurrency_is_rejected_before_network() {
    let subscription =
        Subscription::parse("A = anytls, node.example.invalid, 443, password=synthetic\n").unwrap();
    for concurrency in [0, 9] {
        let options = ProbeOptions {
            concurrency,
            ..ProbeOptions::default()
        };
        let result = check_nodes(&subscription, &options, &AtomicBool::new(false), &|_, _| {});
        assert!(matches!(result, Err(EngineError::InvalidOptions)));
    }
}

#[cfg(unix)]
#[test]
fn child_is_reaped_when_checking_is_cancelled() {
    let child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
    let mut process = Process {
        child,
        writer: None,
        finished: false,
    };
    assert_eq!(
        process.checked(&AtomicBool::new(true)),
        Err(EngineError::Cancelled)
    );
    process.finish().unwrap();
    assert!(process.child.try_wait().unwrap().is_some());
    process.finish().unwrap();
}

#[test]
fn local_address_variants_are_rejected_without_resolving() {
    for server in [
        "LOCALHOST",
        "node.localhost",
        "Node.LOCAL",
        "127.0.0.1",
        "[::ffff:127.0.0.1]",
        "169.254.169.254",
        "100.64.0.1",
    ] {
        assert!(unsafe_address(server));
    }
    assert!(!unsafe_address("node.example.invalid"));
}

#[test]
fn measurement_cache_does_not_probe_named_aliases_again() {
    let subscription = Subscription::parse("A = anytls, node.example.invalid, 443, password=synthetic\nB = anytls, node.example.invalid, 443, password=synthetic\n").unwrap();
    let first = subscription.nodes()[0].to_sing_box(false).unwrap();
    let second = subscription.nodes()[1].to_sing_box(false).unwrap();
    assert_eq!(
        measurement_key(&first).unwrap().as_str(),
        measurement_key(&second).unwrap().as_str()
    );
}

#[test]
fn protocol_security_and_local_errors_are_not_dead_node_evidence() {
    for error in [
        ureq::Error::Tls("synthetic certificate policy"),
        ureq::Error::LargeResponseHeader(1, 2),
        ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        ureq::Error::Io(std::io::Error::other("ambiguous SOCKS failure")),
    ] {
        let attempts = [
            vec![error_attempt(&error), error_attempt(&error)],
            vec![error_attempt(&error), error_attempt(&error)],
        ];
        assert_eq!(classify([true, true], &attempts, true), Health::Unknown);
    }
    let timeout = ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::TimedOut));
    let attempts = [
        vec![error_attempt(&timeout), error_attempt(&timeout)],
        vec![error_attempt(&timeout), error_attempt(&timeout)],
    ];
    assert_eq!(classify([true, true], &attempts, true), Health::Failed);
}

#[test]
fn listener_requires_rfc1929_and_rejects_a_wrong_password() {
    use std::io::Read;
    for mode in 0..3 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !stopped.load(Ordering::Acquire) && Instant::now() < deadline {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut greeting = [0; 3];
                if stream.read_exact(&mut greeting).is_err() {
                    continue;
                }
                assert_eq!(greeting, [5, 1, 2]);
                if mode == 0 {
                    stream.write_all(&[5, 0]).unwrap();
                    continue;
                }
                stream.write_all(&[5, 2]).unwrap();
                let mut head = [0; 2];
                stream.read_exact(&mut head).unwrap();
                assert_eq!(head[0], 1);
                let mut username = vec![0; head[1] as usize];
                stream.read_exact(&mut username).unwrap();
                let mut len = [0];
                stream.read_exact(&mut len).unwrap();
                let mut password = vec![0; len[0] as usize];
                stream.read_exact(&mut password).unwrap();
                let valid =
                    mode == 1 || (username == b"fixture-user" && password == b"fixture-password");
                stream.write_all(&[1, if valid { 0 } else { 1 }]).unwrap();
            }
        });
        let authenticated = listener_authenticated(port, "fixture-user", "fixture-password");
        stop.store(true, Ordering::Release);
        worker.join().unwrap();
        assert_eq!(authenticated, mode == 2);
    }
}

#[test]
#[ignore = "requires an installed sing-box; validates synthetic configuration only"]
fn installed_engine_accepts_synthetic_config_without_network() {
    let engine = executable(Path::new("sing-box")).unwrap();
    for source in [
        "A = anytls, node.example.invalid, 443, password=synthetic, alpn=\"h2,http/1.1\"\n",
        r#"[{"name":"A","type":"ss","server":"node.example.invalid","port":443,"cipher":"aes-128-gcm","password":"synthetic","udp":false}]"#,
        r#"[{"name":"A","type":"trojan","server":"node.example.invalid","port":443,"password":"synthetic","udp":true}]"#,
    ] {
        let subscription = Subscription::parse(source).unwrap();
        let outbound = subscription.nodes()[0].to_sing_box(false).unwrap();
        for doh in [false, true] {
            let config = SecretJson(
                isolated_config(&outbound, 12345, "synthetic-user", "synthetic-auth", doh).unwrap(),
            );
            let mut process = Process::spawn(&engine, "check", &config.0).unwrap();
            assert!(process.checked(&AtomicBool::new(false)).unwrap());
            assert!(process.child.try_wait().unwrap().is_some());
        }
    }
}

#[test]
#[ignore = "requires installed sing-box; performs loopback authentication only"]
fn installed_engine_listener_authentication_matches_the_adapter() {
    let engine = executable(Path::new("sing-box")).unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let outbound =
        json!({"type":"socks","server":"node.example.invalid","server_port":443,"version":"5"});
    let config = SecretJson(
        isolated_config(&outbound, port, "synthetic-user", "synthetic-auth", false).unwrap(),
    );
    let mut process = Process::spawn(&engine, "run", &config.0).unwrap();
    assert!(
        process
            .ready(
                port,
                "synthetic-user",
                "synthetic-auth",
                &AtomicBool::new(false)
            )
            .unwrap()
    );
    process.finish().unwrap();
    assert!(process.child.try_wait().unwrap().is_some());
}
