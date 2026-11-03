//! Explicit, all-or-nothing export of a prepared node list.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use super::RelayError;
use super::model::{MAX_CONTENT, ModelError, PrivateJson, Subscription, quote_surge, valid_host};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    #[default]
    Original,
    /// Existing Surge policy list or a strict conversion of a supported node list.
    Surge,
    /// Existing Mihomo provider or a strict conversion of a supported node list.
    Mihomo,
    /// Strict projection into a sing-box `outbounds` fragment.
    SingBox,
}

impl OutputFormat {
    pub(super) fn is_original(&self) -> bool {
        *self == Self::Original
    }

    pub const fn extension(self) -> &'static str {
        match self {
            Self::Original | Self::Surge => "txt",
            Self::Mihomo => "yaml",
            Self::SingBox => "json",
        }
    }
}

/// No network or file access. A failed projection returns no output or partial list.
pub fn export_subscription(
    content: &str,
    format: OutputFormat,
) -> Result<Zeroizing<String>, RelayError> {
    let subscription = Subscription::parse(content).map_err(RelayError::Model)?;
    subscription
        .validate_export_dependencies()
        .map_err(RelayError::Model)?;
    match format {
        OutputFormat::Surge if !subscription.is_surge_policy_list() => {
            return surge(&subscription);
        }
        OutputFormat::Mihomo if !subscription.is_mihomo_provider() => {
            return mihomo(&subscription);
        }
        OutputFormat::Surge => {
            for (index, node) in subscription.nodes().iter().enumerate() {
                node.validate_native_surge_export()
                    .map_err(|error| RelayError::ExportNode {
                        index: index + 1,
                        error,
                    })?;
            }
        }
        OutputFormat::SingBox => return sing_box(&subscription),
        _ => {}
    }
    Ok(Zeroizing::new(content.to_owned()))
}

fn sing_box(subscription: &Subscription) -> Result<Zeroizing<String>, RelayError> {
    let mut output = Zeroizing::new(br#"{"outbounds":["#.to_vec());
    for (index, node) in subscription.nodes().iter().enumerate() {
        let mut bytes = Zeroizing::new(Vec::new());
        node.write_export_sing_box(&mut bytes)
            .map_err(|error| RelayError::ExportNode {
                index: index + 1,
                error,
            })?;
        if output.len() + bytes.len() + 3 > MAX_CONTENT {
            return Err(RelayError::Model(ModelError::Render));
        }
        if index != 0 {
            output.push(b',');
        }
        output.extend_from_slice(&bytes);
    }
    output.extend_from_slice(b"]}\n");
    export_text(&output)
}

fn surge(subscription: &Subscription) -> Result<Zeroizing<String>, RelayError> {
    let mut output = Zeroizing::new(String::new());
    let mut expected = Vec::with_capacity(subscription.nodes().len());
    for (index, node) in subscription.nodes().iter().enumerate() {
        let at_node = |error| RelayError::ExportNode {
            index: index + 1,
            error,
        };
        let mut normalized = node.export_sing_box().map_err(at_node)?;
        default_socks_version(&mut normalized.0);
        let line = surge_node(&normalized.0).map_err(at_node)?;
        if output.len() + line.len() + 1 > MAX_CONTENT {
            return Err(RelayError::Model(ModelError::Render));
        }
        output.push_str(&line);
        output.push('\n');
        expected.push(normalized);
    }
    verify_projection(&output, expected, Subscription::is_surge_policy_list)?;
    Ok(output)
}

fn surge_node(source: &Value) -> Result<Zeroizing<String>, ModelError> {
    const COMMON: &[&str] = &["type", "tag", "server", "server_port", "tcp_fast_open"];
    let object = source.as_object().ok_or(ModelError::Unsupported)?;
    let field = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(ModelError::Unsupported)
    };
    let (kind, fields): (&str, &[&str]) = match field("type")? {
        "socks"
            if source["version"] == "5"
                && (object.contains_key("username") == object.contains_key("password")) =>
        {
            ("socks5", &["version", "username", "password", "network"])
        }
        "shadowsocks"
            if matches!(
                field("method")?,
                "aes-128-gcm"
                    | "aes-192-gcm"
                    | "aes-256-gcm"
                    | "chacha20-ietf-poly1305"
                    | "xchacha20-ietf-poly1305"
            ) =>
        {
            ("ss", &["method", "password", "network"])
        }
        kind @ ("trojan" | "anytls") => (kind, &["password", "tls"]),
        _ => return Err(ModelError::Unsupported),
    };
    if object
        .keys()
        .any(|key| !COMMON.contains(&key.as_str()) && !fields.contains(&key.as_str()))
    {
        return Err(ModelError::Unsupported);
    }
    let name = field("tag")?;
    let server = field("server")?;
    let port = source["server_port"]
        .as_u64()
        .filter(|port| (1..=u16::MAX.into()).contains(port))
        .ok_or(ModelError::Unsupported)?;
    if name.trim() != name
        || name.starts_with("//")
        || name.contains([
            '=', ',', '#', ';', '"', '\\', '[', ']', '\u{2028}', '\u{2029}',
        ])
        || [
            "DIRECT",
            "CELLULAR",
            "CELLULAR-ONLY",
            "HYBRID",
            "NO-HYBRID",
            "REJECT",
            "REJECT-DROP",
            "REJECT-NO-DROP",
            "REJECT-TINYGIF",
        ]
        .iter()
        .any(|builtin| name.eq_ignore_ascii_case(builtin))
        || !valid_host(server)
    {
        return Err(ModelError::Unsupported);
    }
    let mut row = Zeroizing::new(format!("{name} = {kind}, {server}, {port}"));
    if matches!(kind, "socks5" | "ss") {
        let udp = match object.get("network") {
            None => true,
            Some(value) if value == "tcp" => false,
            _ => return Err(ModelError::Unsupported),
        };
        write!(row, ", udp-relay={udp}").map_err(|_| ModelError::Render)?;
    }
    if let Some(tfo) = object.get("tcp_fast_open") {
        let tfo = tfo.as_bool().ok_or(ModelError::Unsupported)?;
        write!(row, ", tfo={tfo}").map_err(|_| ModelError::Render)?;
    }
    if kind == "ss" {
        write!(row, ", encrypt-method={}", field("method")?).map_err(|_| ModelError::Render)?;
    }
    for key in ["username", "password"] {
        if object.contains_key(key) || (key == "password" && kind != "socks5") {
            let value = field(key)?;
            if value.contains("#!")
                || value
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
            {
                return Err(ModelError::Unsupported);
            }
            let value = quote_surge(value)?;
            write!(row, ", {key}={}", value.as_str()).map_err(|_| ModelError::Render)?;
        }
    }
    if matches!(kind, "trojan" | "anytls") {
        let tls = source["tls"].as_object().ok_or(ModelError::Unsupported)?;
        if source["tls"]["enabled"] != true
            || tls
                .keys()
                .any(|key| !["enabled", "server_name", "insecure", "alpn"].contains(&key.as_str()))
        {
            return Err(ModelError::Unsupported);
        }
        if let Some(insecure) = tls.get("insecure") {
            if insecure.as_bool() != Some(false) {
                return Err(ModelError::SecurityBlocked);
            }
            row.push_str(", skip-cert-verify=false");
        }
        if let Some(sni) = tls.get("server_name") {
            let sni = sni
                .as_str()
                .filter(|sni| {
                    valid_host(sni)
                        && sni.parse::<std::net::IpAddr>().is_err()
                        && !sni.eq_ignore_ascii_case("off")
                })
                .ok_or(ModelError::Unsupported)?;
            write!(row, ", sni={sni}").map_err(|_| ModelError::Render)?;
        }
        if let Some(alpn) = tls.get("alpn") {
            let protocols = alpn.as_array().ok_or(ModelError::Unsupported)?;
            if protocols.is_empty() {
                return Err(ModelError::Unsupported);
            }
            let mut joined = Zeroizing::new(String::new());
            for value in protocols {
                let protocol = value
                    .as_str()
                    .filter(|value| {
                        (1..=255).contains(&value.len())
                            && !value.contains(['#', ','])
                            && value.bytes().all(|byte| byte.is_ascii_graphic())
                    })
                    .ok_or(ModelError::Unsupported)?;
                if !joined.is_empty() {
                    joined.push(',');
                }
                joined.push_str(protocol);
            }
            let quoted = quote_surge(&joined)?;
            write!(row, ", alpn={}", quoted.as_str()).map_err(|_| ModelError::Render)?;
        }
    }
    Ok(row)
}

fn default_socks_version(source: &mut Value) {
    // An omitted version means SOCKS5 in sing-box.
    if source["type"] == "socks" && source.get("version").is_none() {
        source["version"] = "5".into();
    }
}

fn mihomo(subscription: &Subscription) -> Result<Zeroizing<String>, RelayError> {
    let mut output = Zeroizing::new(b"proxies:\n".to_vec());
    let mut expected = Vec::with_capacity(subscription.nodes().len());
    for (index, node) in subscription.nodes().iter().enumerate() {
        let at_node = |error| RelayError::ExportNode {
            index: index + 1,
            error,
        };
        let mut normalized = node.export_sing_box().map_err(at_node)?;
        default_socks_version(&mut normalized.0);
        let converted = mihomo_node(&normalized.0).map_err(at_node)?;
        // A JSON object is a YAML flow mapping. Serialize it instead of
        // interpolating credentials and names into the YAML document.
        let mut row = Zeroizing::new(Vec::new());
        serde_json::to_writer(&mut *row, &converted.0)
            .map_err(|_| RelayError::Model(ModelError::Render))?;
        if output.len() + row.len() + 5 > MAX_CONTENT {
            return Err(RelayError::Model(ModelError::Render));
        }
        output.extend_from_slice(b"  - ");
        output.extend_from_slice(&row);
        output.push(b'\n');
        expected.push(normalized);
    }
    let output = export_text(&output)?;
    verify_projection(&output, expected, Subscription::is_mihomo_provider)?;
    Ok(output)
}

fn verify_projection(
    output: &str,
    expected: Vec<PrivateJson>,
    matches_format: fn(&Subscription) -> bool,
) -> Result<(), RelayError> {
    let roundtrip =
        Subscription::parse(output).map_err(|_| RelayError::Model(ModelError::Render))?;
    if !matches_format(&roundtrip) || roundtrip.nodes().len() != expected.len() {
        return Err(RelayError::Model(ModelError::Render));
    }
    for (index, (node, original)) in roundtrip.nodes().iter().zip(expected).enumerate() {
        if !node
            .export_sing_box()
            .is_ok_and(|value| value.0 == original.0)
        {
            return Err(RelayError::ExportNode {
                index: index + 1,
                error: ModelError::Unsupported,
            });
        }
    }
    Ok(())
}

fn mihomo_node(source: &Value) -> Result<PrivateJson, ModelError> {
    const COMMON: &[&str] = &["type", "tag", "server", "server_port", "network"];
    let object = source.as_object().ok_or(ModelError::Unsupported)?;
    let (kind, fields): (&str, &[&str]) = match source["type"].as_str() {
        Some("socks")
            if source["version"] == "5"
                && (object.contains_key("username") == object.contains_key("password")) =>
        {
            ("socks5", &["version", "username", "password"])
        }
        Some("shadowsocks")
            if matches!(
                source["method"].as_str(),
                Some("aes-128-gcm" | "aes-256-gcm" | "chacha20-ietf-poly1305")
            ) =>
        {
            ("ss", &["method", "password"])
        }
        _ => return Err(ModelError::Unsupported),
    };
    if object
        .keys()
        .any(|key| !COMMON.contains(&key.as_str()) && !fields.contains(&key.as_str()))
    {
        return Err(ModelError::Unsupported);
    }
    let udp = match object.get("network") {
        None => true,
        Some(value) if value == "tcp" => false,
        _ => return Err(ModelError::Unsupported),
    };
    let mut converted = PrivateJson(json!({
        "type": kind, "name": source["tag"], "server": source["server"],
        "port": source["server_port"], "udp": udp,
    }));
    for (from, to) in [
        ("method", "cipher"),
        ("username", "username"),
        ("password", "password"),
    ] {
        if let Some(value) = object.get(from) {
            converted.0[to] = value.clone();
        }
    }
    Ok(converted)
}

fn export_text(output: &[u8]) -> Result<Zeroizing<String>, RelayError> {
    std::str::from_utf8(output)
        .map(|text| Zeroizing::new(text.to_owned()))
        .map_err(|_| RelayError::Model(ModelError::Render))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURGE: &str = "[Proxy]\r\n# example\r\nalpha = https, example.com, 443\r\n";
    const SURGE_CONVERSION: &str = r#"{"outbounds":[
        {"type":"socks","tag":"Tokyo SOCKS","server":"socks.example","server_port":1080,
         "network":"tcp","username":"alice","password":"synthetic, #p\"\\","tcp_fast_open":true},
        {"type":"shadowsocks","tag":"Shadowsocks","server":"ss.example","server_port":443,
         "method":"aes-192-gcm","password":"synthetic"},
        {"type":"trojan","tag":"Trojan","server":"trojan.example","server_port":443,
         "password":"synthetic","tls":{"enabled":true,"server_name":"tls.example","insecure":false}}
    ]}"#;

    const ANYTLS_JSON: &str = r#"{"outbounds":[{
        "type":"anytls", "tag":"Strict AnyTLS", "server":"proxy.example.invalid", "server_port":443,
        "password":"fixture,phrase", "tcp_fast_open":true,
        "tls":{"enabled":true,"server_name":"tls.example.invalid","insecure":false,"alpn":["h2","http/1.1"]}
    }]}"#;

    #[test]
    fn preserves_native_client_documents_byte_for_byte() {
        let mihomo =
            "proxies:\n  - {name: beta, type: http, server: example.net, port: 80, tfo: true}\n";
        for (format, text) in [
            (OutputFormat::Original, SURGE),
            (OutputFormat::Surge, SURGE),
            (OutputFormat::Mihomo, mihomo),
        ] {
            assert_eq!(export_subscription(text, format).unwrap().as_str(), text);
        }
    }

    #[test]
    fn exports_surge_policy_lines_without_changing_authentication_udp_or_tls() {
        let output = export_subscription(SURGE_CONVERSION, OutputFormat::Surge).unwrap();
        assert_eq!(
            output.as_str(),
            concat!(
                "Tokyo SOCKS = socks5, socks.example, 1080, udp-relay=false, tfo=true, username=\"alice\", password=\"synthetic, #p\\\"\\\\\"\n",
                "Shadowsocks = ss, ss.example, 443, udp-relay=true, encrypt-method=aes-192-gcm, password=\"synthetic\"\n",
                "Trojan = trojan, trojan.example, 443, password=\"synthetic\", skip-cert-verify=false, sni=tls.example\n"
            )
        );
        let restored = export_subscription(&output, OutputFormat::SingBox).unwrap();
        let mut expected: Value = serde_json::from_str(SURGE_CONVERSION).unwrap();
        expected["outbounds"][0]["version"] = "5".into();
        assert_eq!(serde_json::from_str::<Value>(&restored).unwrap(), expected);
    }

    #[test]
    fn converts_a_subscription_of_share_links_directly_to_surge() {
        let input = concat!(
            "socks5://example.invalid:1080#First\n",
            "ss://YWVzLTEyOC1nY206dGVzdA@ss.example:443#Second\n"
        );
        let output = export_subscription(input, OutputFormat::Surge).unwrap();
        assert_eq!(
            output.as_str(),
            concat!(
                "First = socks5, example.invalid, 1080, udp-relay=true\n",
                "Second = ss, ss.example, 443, udp-relay=true, encrypt-method=aes-128-gcm, password=\"test\"\n"
            )
        );
    }

    #[test]
    fn exports_strict_anytls_json_and_share_link_without_losing_tls_options() {
        let uri = "anytls://fixture%2Cphrase@proxy.example.invalid:443?peer=tls.example.invalid&allowInsecure=0&alpn=h2%2Chttp%2F1.1#AnyTLS%20URI\n";
        for (input, name, tfo) in [
            (ANYTLS_JSON, "Strict AnyTLS", ", tfo=true"),
            (uri, "AnyTLS URI", ""),
        ] {
            let output = export_subscription(input, OutputFormat::Surge).unwrap();
            assert_eq!(
                output.as_str(),
                format!(
                    "{name} = anytls, proxy.example.invalid, 443{tfo}, password=\"fixture,phrase\", skip-cert-verify=false, sni=tls.example.invalid, alpn=\"h2,http/1.1\"\n"
                )
            );
            assert!(!output.contains("udp-relay"));
            let expected = export_subscription(input, OutputFormat::SingBox).unwrap();
            let roundtrip = export_subscription(&output, OutputFormat::SingBox).unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&roundtrip).unwrap(),
                serde_json::from_str::<Value>(&expected).unwrap(),
            );
        }
    }

    #[test]
    fn anytls_export_rejects_session_overrides_and_unsafe_or_ambiguous_tls() {
        let input: Value = serde_json::from_str(ANYTLS_JSON).unwrap();
        let mut idle_override = input.clone();
        idle_override["outbounds"][0]["idle_session_timeout"] = json!("5m");
        let mut insecure = input.clone();
        insecure["outbounds"][0]["tls"]["insecure"] = json!(true);
        let mut invalid_alpn = input;
        invalid_alpn["outbounds"][0]["tls"]["alpn"] = json!(["h2,http/1.1"]);
        for (input, expected) in [
            (idle_override, ModelError::Unsupported),
            (insecure, ModelError::SecurityBlocked),
            (invalid_alpn, ModelError::Unsupported),
        ] {
            let error = export_subscription(&input.to_string(), OutputFormat::Surge).unwrap_err();
            assert!(
                matches!(error, RelayError::ExportNode { index: 1, error } if error == expected)
            );
            assert!(!error.to_string().contains("fixture,phrase"));
        }
    }

    #[test]
    #[ignore = "optional Surge parser smoke test; requires SCRIPT_HUB_SURGE_BIN and SCRIPT_HUB_SURGE_TEST_DIR"]
    fn generated_surge_policies_pass_client_config_check() {
        use std::io::Write as _;

        let policies = export_subscription(SURGE_CONVERSION, OutputFormat::Surge).unwrap();
        let anytls = export_subscription(ANYTLS_JSON, OutputFormat::Surge).unwrap();
        let scratch =
            std::path::PathBuf::from(std::env::var_os("SCRIPT_HUB_SURGE_TEST_DIR").unwrap());
        let path = scratch.join("script-hub-surge-check.conf");
        let mut profile = std::fs::File::create_new(&path).unwrap();
        write!(
            profile,
            "[Proxy]\n{}{}\n[Proxy Group]\nCheck = select, Tokyo SOCKS, Shadowsocks, Trojan, Strict AnyTLS\n[Rule]\nFINAL,Check\n",
            policies.as_str(), anytls.as_str()
        )
        .unwrap();
        drop(profile);
        let result = std::process::Command::new(std::env::var_os("SCRIPT_HUB_SURGE_BIN").unwrap())
            .arg("--check")
            .arg(&path)
            .output();
        std::fs::remove_file(path).unwrap();
        let result = result.unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    #[test]
    fn surge_conversion_stops_at_a_reserved_name_unsupported_option_or_unsafe_tls() {
        for (name, kind, options, expected) in [
            (
                "private-udp",
                "socks",
                r#""network":"udp""#,
                ModelError::Unsupported,
            ),
            (
                "private-auth",
                "socks",
                r#""username":"private-user""#,
                ModelError::Unsupported,
            ),
            (
                "private-dial",
                "socks",
                r#""udp_over_tcp":{"enabled":true}"#,
                ModelError::Unsupported,
            ),
            (
                "hybrid",
                "socks",
                r#""version":"5""#,
                ModelError::Unsupported,
            ),
            (
                "private-name=escape",
                "socks",
                r#""version":"5""#,
                ModelError::Unsupported,
            ),
            (
                "private-tls",
                "trojan",
                r#""password":"private-password","tls":{"enabled":true,"insecure":true}"#,
                ModelError::SecurityBlocked,
            ),
        ] {
            let input = format!(
                r#"{{"outbounds":[
                {{"type":"socks","tag":"first","server":"example.com","server_port":1080}},
                {{"type":"{kind}","tag":"{name}","server":"example.net","server_port":1080,{options}}}]}}"#
            );
            let error = export_subscription(&input, OutputFormat::Surge).unwrap_err();
            assert!(
                matches!(error, RelayError::ExportNode { index: 2, error } if error == expected)
            );
            assert!(!error.to_string().contains("private-"));
        }
    }

    #[test]
    fn exports_collection_to_mihomo_and_preserves_node_order_and_networks() {
        let input = r#"{"outbounds":[
            {"type":"socks","tag":"alpha: #1","server":"example.com","server_port":1080,
             "network":"tcp","username":"yes","password":"a: #b"},
            {"type":"shadowsocks","tag":"beta","server":"example.net","server_port":443,
             "method":"aes-256-gcm","password":"synthetic"}]}"#;
        let output = export_subscription(input, OutputFormat::Mihomo).unwrap();
        let nodes: Value = serde_saphyr::from_str(&output).unwrap();
        assert_eq!(
            nodes,
            json!({"proxies":[
                {"type":"socks5","name":"alpha: #1","server":"example.com","port":1080,
                 "udp":false,"username":"yes","password":"a: #b"},
                {"type":"ss","name":"beta","server":"example.net","port":443,
                 "udp":true,"cipher":"aes-256-gcm","password":"synthetic"},
            ]})
        );
        let restored = export_subscription(&output, OutputFormat::SingBox).unwrap();
        let mut expected: Value = serde_json::from_str(input).unwrap();
        expected["outbounds"][0]["version"] = "5".into();
        assert_eq!(serde_json::from_str::<Value>(&restored).unwrap(), expected);
    }

    #[test]
    #[ignore = "optional native parser smoke test; requires SCRIPT_HUB_MIHOMO_BIN and SCRIPT_HUB_MIHOMO_TEST_DIR"]
    fn generated_mihomo_proxies_pass_client_config_check() {
        use base64::Engine;

        let input = r#"{"outbounds":[
            {"type":"socks","tag":"DemoSocks","server":"node.example","server_port":1080,"network":"tcp"},
            {"type":"shadowsocks","tag":"DemoSs","server":"node.example","server_port":443,
             "method":"aes-256-gcm","password":"synthetic-not-a-real-secret"}]}"#;
        let output = export_subscription(input, OutputFormat::Mihomo).unwrap();
        let config = format!(
            "{}proxy-groups:\n  - name: Check\n    type: select\n    proxies: [DemoSocks, DemoSs]\nrules: ['MATCH,Check']\n",
            output.as_str()
        );
        let encoded = base64::engine::general_purpose::STANDARD.encode(config);
        let result = std::process::Command::new(std::env::var_os("SCRIPT_HUB_MIHOMO_BIN").unwrap())
            .arg("-t")
            .arg("-d")
            .arg(std::env::var_os("SCRIPT_HUB_MIHOMO_TEST_DIR").unwrap())
            .args(["-config", &encoded])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    #[test]
    fn mihomo_conversion_refuses_udp_only_and_unsupported_options_without_partial_output() {
        for options in [
            r#""network":"udp""#,
            r#""tcp_fast_open":true"#,
            r#""version":"4a""#,
        ] {
            let input = format!(
                r#"{{"outbounds":[
                {{"type":"socks","tag":"first","server":"example.com","server_port":1080}},
                {{"type":"socks","tag":"private-name","server":"example.net","server_port":1080,{options}}}]}}"#
            );
            let error = export_subscription(&input, OutputFormat::Mihomo).unwrap_err();
            assert!(matches!(error, RelayError::ExportNode { index: 2, .. }));
            assert!(!error.to_string().contains("private-name"));
        }
    }

    #[test]
    fn mihomo_conversion_preserves_surge_udp_defaults() {
        let source =
            "alpha = socks5, example.com, 1080\nbeta = socks5, example.net, 1080, udp-relay=true\n";
        let output = export_subscription(source, OutputFormat::Mihomo).unwrap();
        let nodes: Value = serde_saphyr::from_str(&output).unwrap();
        assert_eq!(nodes["proxies"][0]["udp"], false);
        assert_eq!(nodes["proxies"][1]["udp"], true);
    }

    #[test]
    fn rejects_incomplete_node_chains() {
        let input = "alpha = https, example.com, 443, underlying-proxy=missing\n";
        assert!(matches!(
            export_subscription(input, OutputFormat::Original),
            Err(RelayError::Model(ModelError::InvalidDependency))
        ));
    }

    #[test]
    fn surge_export_rejects_shadowrocket_only_policy() {
        let foreign = "alpha = vless, example.com, 443, username=synthetic\n";
        assert!(matches!(
            export_subscription(foreign, OutputFormat::Surge),
            Err(RelayError::ExportNode { index: 1, .. })
        ));
    }

    #[test]
    fn refuses_surge_external_directives_even_in_original() {
        for directive in [
            "#!include https://example.com/outside.conf\n",
            "beta = http, example.com, 80 #!REQUIREMENT IOS\n",
        ] {
            let source = format!("{SURGE}{directive}");
            assert!(matches!(
                export_subscription(&source, OutputFormat::Original),
                Err(RelayError::Model(ModelError::InvalidInput))
            ));
        }
    }

    #[test]
    fn exports_all_nodes_and_preserves_udp_and_tls_semantics() {
        let input = format!(
            "{SURGE}beta = socks5, example.net, 1080\ngamma = socks5, example.org, 1080, udp-relay=true\n"
        );
        let output = export_subscription(&input, OutputFormat::SingBox).unwrap();
        let expected = serde_json::json!({"outbounds":[
            {"type":"http","tag":"alpha","server":"example.com","server_port":443,"tls":{"enabled":true}},
            {"type":"socks","tag":"beta","server":"example.net","server_port":1080,"version":"5","network":"tcp"},
            {"type":"socks","tag":"gamma","server":"example.org","server_port":1080,"version":"5"}
        ]});
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&output).unwrap(),
            expected
        );
    }

    #[test]
    fn conversion_rejects_unrepresentable_options_at_their_node_without_leaking_values() {
        let input = format!(
            "{SURGE}private-name = https, example.net, 443, test-url=https://example.net/private-token\n"
        );
        let error = export_subscription(&input, OutputFormat::SingBox).unwrap_err();
        assert!(matches!(
            error,
            RelayError::ExportNode {
                index: 2,
                error: ModelError::Unsupported
            }
        ));
        assert!(!error.to_string().contains("private-"));
    }

    #[test]
    fn conversion_refuses_to_silently_downgrade_tls_verification() {
        let input = "alpha = https, example.net, 443, skip-cert-verify=true\n";
        assert!(matches!(
            export_subscription(input, OutputFormat::SingBox),
            Err(RelayError::ExportNode {
                index: 1,
                error: ModelError::SecurityBlocked
            })
        ));
    }
}
