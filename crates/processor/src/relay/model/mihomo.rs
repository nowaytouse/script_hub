use super::*;

pub(super) fn parse_yaml(input: &str) -> Result<PrivateJson, ModelError> {
    if input.len() > 8 * 1024 * 1024 {
        return Err(ModelError::InvalidInput);
    }
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_documents: 1,
            max_depth: 32,
            flow_nesting_limit: 32,
            max_nodes: 100_000,
            max_events: 200_000,
            max_aliases: 1024,
            max_anchors: 1024,
            max_recorded_anchor_events: 100_000,
            max_recorded_anchor_bytes: 8 * 1024 * 1024,
            max_total_scalar_bytes: 8 * 1024 * 1024,
        },
        alias_limits: serde_saphyr::alias_limits! {
            max_total_replayed_events: 100_000,
            max_replay_stack_depth: 16,
            max_alias_expansions_per_anchor: 1024,
        },
        duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
        merge_keys: serde_saphyr::MergeKeyPolicy::Error,
        reject_unsupported_tags: true,
        strict_booleans: true,
        no_schema: true,
        with_snippet: false,
    };
    let UniqueValue(value) = serde_saphyr::from_str_with_options(input, options)
        .map_err(|_| ModelError::InvalidInput)?;
    match &value {
        Value::Array(_) => {}
        Value::Object(map) if map.len() == 1 && map.get("proxies").is_some_and(Value::is_array) => {
        }
        _ => return Err(ModelError::InvalidInput),
    }
    Ok(PrivateJson(value))
}

pub(super) fn render(value: &Value) -> Result<Zeroizing<String>, ModelError> {
    serde_saphyr::to_string(value)
        .map(Zeroizing::new)
        .map_err(|_| ModelError::Render)
}

pub(super) fn project(
    node: &Node,
    source: &Value,
    compatibility: bool,
    mihomo_defaults: bool,
) -> Result<Value, ModelError> {
    let object = source.as_object().ok_or(ModelError::Unsupported)?;
    let (protocol, required_tls, fields): (&str, bool, &[&str]) = match node.protocol() {
        "socks5" => ("socks", false, &["username", "password"]),
        "http" => ("http", false, &["username", "password"]),
        "ss" => ("shadowsocks", false, &["cipher", "password"]),
        "trojan" => ("trojan", true, &["password"]),
        "anytls" => ("anytls", true, &["password"]),
        "vmess" => ("vmess", false, &["uuid", "cipher", "alterId"]),
        "vless" => ("vless", false, &["uuid", "flow"]),
        "hysteria2" => ("hysteria2", true, &["password"]),
        "tuic" => (
            "tuic",
            true,
            &[
                "uuid",
                "password",
                "congestion-controller",
                "udp-relay-mode",
            ],
        ),
        _ => return Err(ModelError::Unsupported),
    };
    let common = [
        "name",
        "server",
        "port",
        "type",
        "tls",
        "sni",
        "servername",
        "alpn",
        "skip-cert-verify",
        "udp",
        "network",
        "ws-opts",
        "grpc-opts",
        "reality-opts",
        "client-fingerprint",
        "_subName",
        "_subDisplayName",
        "_collectionName",
        "_collectionDisplayName",
    ];
    if object
        .keys()
        .any(|key| !common.contains(&key.as_str()) && !fields.contains(&key.as_str()))
    {
        return Err(ModelError::Unsupported);
    }
    for key in [
        "_subName",
        "_subDisplayName",
        "_collectionName",
        "_collectionDisplayName",
    ] {
        if object.get(key).is_some_and(|value| !value.is_string()) {
            return Err(ModelError::Unsupported);
        }
    }
    let insecure = object
        .get("skip-cert-verify")
        .map(|v| v.as_bool().ok_or(ModelError::Unsupported))
        .transpose()?;
    if insecure == Some(true) && !compatibility {
        return Err(ModelError::SecurityBlocked);
    }
    let tls = object
        .get("tls")
        .map(|v| v.as_bool().ok_or(ModelError::Unsupported))
        .transpose()?
        .unwrap_or(required_tls);
    if required_tls && !tls || tls && matches!(protocol, "socks" | "shadowsocks") {
        return Err(ModelError::Unsupported);
    }
    let mut out =
        json!({"type":protocol,"tag":node.name(),"server":node.server(),"server_port":node.port});
    if protocol == "socks" {
        out["version"] = "5".into();
    }
    for key in fields {
        if let Some(value) = object.get(*key) {
            if *key == "alterId" {
                if !value.as_u64().is_some_and(|n| n <= u16::MAX.into()) {
                    return Err(ModelError::Unsupported);
                }
                out["alter_id"] = value.clone();
                continue;
            }
            if !value.as_str().is_some_and(valid_label) {
                return Err(ModelError::Unsupported);
            }
            let dest = match *key {
                "cipher" if protocol == "shadowsocks" => "method",
                "cipher" => "security",
                "congestion-controller" => "congestion_control",
                "udp-relay-mode" => "udp_relay_mode",
                _ => key,
            };
            out[dest] = value.clone();
        } else if matches!(*key, "uuid")
            || *key == "password"
                && matches!(
                    protocol,
                    "shadowsocks" | "trojan" | "anytls" | "hysteria2" | "tuic"
                )
            || *key == "cipher" && protocol == "shadowsocks"
        {
            return Err(ModelError::Unsupported);
        }
    }
    if protocol == "vmess" && out.get("security").is_none() {
        out["security"] = "auto".into();
    }
    let udp = object
        .get("udp")
        .map(|v| v.as_bool().ok_or(ModelError::Unsupported))
        .transpose()?
        .or_else(|| mihomo_defaults.then_some(matches!(protocol, "hysteria2" | "tuic")));
    if let Some(udp) = udp {
        match protocol {
            "socks" | "shadowsocks" | "trojan" | "vmess" | "vless" | "hysteria2" | "tuic" => {
                if !udp {
                    out["network"] = "tcp".into();
                }
            }
            "http" if !udp => {}
            "anytls" if udp => {}
            _ => return Err(ModelError::Unsupported),
        }
    }
    let tls_keys = [
        "sni",
        "servername",
        "alpn",
        "skip-cert-verify",
        "reality-opts",
        "client-fingerprint",
    ];
    if !tls && tls_keys.iter().any(|key| object.contains_key(*key)) {
        return Err(ModelError::Unsupported);
    }
    if tls {
        let mut projected = json!({"enabled":true});
        if object.contains_key("sni") && object.contains_key("servername") {
            return Err(ModelError::Unsupported);
        }
        if let Some(sni) = object.get("sni").or_else(|| object.get("servername")) {
            if !sni.as_str().is_some_and(valid_label) {
                return Err(ModelError::Unsupported);
            }
            projected["server_name"] = sni.clone();
        }
        if let Some(alpn) = object.get("alpn") {
            if !alpn.as_array().is_some_and(|items| {
                !items.is_empty() && items.iter().all(|v| v.as_str().is_some_and(valid_label))
            }) {
                return Err(ModelError::Unsupported);
            }
            projected["alpn"] = alpn.clone();
        }
        if let Some(value) = insecure {
            projected["insecure"] = value.into();
        }
        if let Some(fingerprint) = object.get("client-fingerprint") {
            if !fingerprint.as_str().is_some_and(valid_label) {
                return Err(ModelError::Unsupported);
            }
            projected["utls"] = json!({"enabled":true,"fingerprint":fingerprint});
        }
        if let Some(reality) = object.get("reality-opts") {
            if !matches!(protocol, "vmess" | "vless" | "trojan") {
                return Err(ModelError::Unsupported);
            }
            only_keys(reality, &["public-key", "short-id"])?;
            let key = reality
                .get("public-key")
                .and_then(Value::as_str)
                .filter(|s| valid_label(s))
                .ok_or(ModelError::Unsupported)?;
            let mut result = json!({"enabled":true,"public_key":key});
            if let Some(id) = reality.get("short-id") {
                if !id
                    .as_str()
                    .is_some_and(|s| !s.chars().any(char::is_control))
                {
                    return Err(ModelError::Unsupported);
                }
                result["short_id"] = id.clone();
            }
            projected["reality"] = result;
        }
        out["tls"] = projected;
    }
    if let Some(network) = object.get("network") {
        if !matches!(protocol, "vmess" | "vless" | "trojan") {
            return Err(ModelError::Unsupported);
        }
        let transport = match network.as_str() {
            Some("tcp") => {
                if object.contains_key("ws-opts") || object.contains_key("grpc-opts") {
                    return Err(ModelError::Unsupported);
                }
                None
            }
            Some("ws") => {
                if object.contains_key("grpc-opts") {
                    return Err(ModelError::Unsupported);
                }
                let mut result = json!({"type":"ws"});
                if let Some(opts) = object.get("ws-opts") {
                    only_keys(opts, &["path", "headers"])?;
                    if let Some(path) = opts.get("path") {
                        if !path.as_str().is_some_and(valid_label) {
                            return Err(ModelError::Unsupported);
                        }
                        result["path"] = path.clone();
                    }
                    if let Some(headers) = opts.get("headers") {
                        if !headers.as_object().is_some_and(|h| {
                            h.iter()
                                .all(|(k, v)| valid_label(k) && v.as_str().is_some_and(valid_label))
                        }) {
                            return Err(ModelError::Unsupported);
                        }
                        result["headers"] = headers.clone();
                    }
                }
                Some(result)
            }
            Some("grpc") => {
                if object.contains_key("ws-opts") {
                    return Err(ModelError::Unsupported);
                }
                let mut result = json!({"type":"grpc"});
                if let Some(opts) = object.get("grpc-opts") {
                    only_keys(opts, &["grpc-service-name"])?;
                    if let Some(name) = opts.get("grpc-service-name") {
                        if !name.as_str().is_some_and(valid_label) {
                            return Err(ModelError::Unsupported);
                        }
                        result["service_name"] = name.clone();
                    }
                }
                Some(result)
            }
            _ => return Err(ModelError::Unsupported),
        };
        if let Some(transport) = transport {
            out["transport"] = transport;
        }
    } else if object.contains_key("ws-opts") || object.contains_key("grpc-opts") {
        return Err(ModelError::Unsupported);
    }
    validate_native(&out, compatibility)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_node_only_and_rejects_ambiguous_inputs() {
        assert!(
            parse_yaml("proxies:\n  - {name: test, type: http, server: example.com, port: 443}\n")
                .is_ok()
        );
        for input in [
            "proxies: []\nmode: rule",
            "proxies: []\nproxies: []",
            "proxies: !thing []",
            "proxies: []\n---\n[]",
            "proxies: [{1: value}]",
            "proxies: [{<<: {name: x}}]",
        ] {
            assert!(parse_yaml(input).is_err(), "{input}");
        }
    }

    #[test]
    fn projection_rejects_unknown_and_tls_bypass_without_acknowledgement() {
        let node = Node {
            name: Zeroizing::new("test".into()),
            server: Zeroizing::new("example.com".into()),
            protocol: Zeroizing::new("vless".into()),
            port: 443,
            dependency: None,
            source: NodeSource::SubStore(PrivateJson(Value::Null)),
        };
        let mut source = json!({"name":"test","type":"vless","server":"example.com","port":443,"uuid":"test-uuid","tls":true,"network":"ws","ws-opts":{"path":"/socket"}});
        let out = project(&node, &source, false, true).unwrap();
        assert_eq!(out["transport"]["path"], "/socket");
        assert_eq!(out["network"], "tcp");
        assert!(
            project(&node, &source, false, false)
                .unwrap()
                .get("network")
                .is_none()
        );
        source["udp"] = true.into();
        source["_subName"] = "example".into();
        assert!(
            project(&node, &source, false, true)
                .unwrap()
                .get("network")
                .is_none()
        );
        source["unknown"] = true.into();
        assert_eq!(
            project(&node, &source, false, true).unwrap_err(),
            ModelError::Unsupported
        );
        source.as_object_mut().unwrap().remove("unknown");
        source["skip-cert-verify"] = true.into();
        assert_eq!(
            project(&node, &source, false, true).unwrap_err(),
            ModelError::SecurityBlocked
        );
    }

    #[test]
    fn yaml_limits_nesting_and_alias_expansion() {
        let deep = format!("proxies: {}0{}", "[".repeat(40), "]".repeat(40));
        assert!(parse_yaml(&deep).is_err());
        let aliases = format!("proxies: [&a value, {}]", vec!["*a"; 1100].join(", "));
        assert!(parse_yaml(&aliases).is_err());
    }
}
