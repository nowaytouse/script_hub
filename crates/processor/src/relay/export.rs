//! Explicit, all-or-nothing export of a prepared node list.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zeroize::Zeroizing;

use super::RelayError;
use super::model::{MAX_CONTENT, ModelError, PrivateJson, Subscription, quote_surge, valid_host};

mod surge_vless;

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
    pub(super) const fn script_label(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Surge => "surge",
            Self::Mihomo => "mihomo",
            Self::SingBox => "sing-box",
        }
    }

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
        let mut normalized = node.export_surge_projection().map_err(at_node)?;
        default_socks_version(&mut normalized.0);
        let line = surge_node(&normalized.0).map_err(at_node)?;
        surge_vless::normalize(&mut normalized.0);
        if output.len() + line.len() + 1 > MAX_CONTENT {
            return Err(RelayError::Model(ModelError::Render));
        }
        output.push_str(&line);
        output.push('\n');
        expected.push(normalized);
    }
    verify_projection(&output, expected, OutputFormat::Surge)?;
    Ok(output)
}

fn surge_node(source: &Value) -> Result<Zeroizing<String>, ModelError> {
    const COMMON: &[&str] = &[
        "type",
        "tag",
        "server",
        "server_port",
        "tcp_fast_open",
        "detour",
    ];
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
        "vless" => (
            "vless",
            &["uuid", "flow", "packet_encoding", "tls", "transport"],
        ),
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
        || is_surge_builtin_name(name)
        || !valid_host(server)
    {
        return Err(ModelError::Unsupported);
    }
    let mut row = Zeroizing::new(format!("{name} = {kind}, {server}, {port}"));
    if object.contains_key("detour") {
        let quoted = quote_surge(field("detour")?)?;
        write!(row, ", underlying-proxy={}", quoted.as_str()).map_err(|_| ModelError::Render)?;
    }
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
    if kind == "vless" {
        surge_vless::append(source, &mut row)?;
        return Ok(row);
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
        append_surge_tls_parameters(tls, &mut row)?;
    }
    Ok(row)
}

fn append_surge_tls_parameters(
    tls: &serde_json::Map<String, Value>,
    row: &mut Zeroizing<String>,
) -> Result<(), ModelError> {
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
    Ok(())
}

pub(super) fn is_surge_builtin_name(name: &str) -> bool {
    [
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
    verify_projection(&output, expected, OutputFormat::Mihomo)?;
    Ok(output)
}

fn verify_projection(
    output: &str,
    expected: Vec<PrivateJson>,
    format: OutputFormat,
) -> Result<(), RelayError> {
    let roundtrip =
        Subscription::parse(output).map_err(|_| RelayError::Model(ModelError::Render))?;
    let surge = format == OutputFormat::Surge;
    let matches_format = if surge {
        roundtrip.is_surge_policy_list()
    } else {
        roundtrip.is_mihomo_provider()
    };
    if !matches_format || roundtrip.nodes().len() != expected.len() {
        return Err(RelayError::Model(ModelError::Render));
    }
    for (index, (node, original)) in roundtrip.nodes().iter().zip(expected).enumerate() {
        let projected = if surge {
            node.export_surge_projection()
        } else {
            node.export_sing_box()
        };
        if !projected.is_ok_and(|mut value| {
            if surge {
                surge_vless::normalize(&mut value.0);
            }
            value.0 == original.0
        }) {
            return Err(RelayError::ExportNode {
                index: index + 1,
                error: ModelError::Unsupported,
            });
        }
    }
    Ok(())
}

fn mihomo_node(source: &Value) -> Result<PrivateJson, ModelError> {
    const COMMON: &[&str] = &["type", "tag", "server", "server_port", "network", "detour"];
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
    if let Some(detour) = source.get("detour") {
        let target = detour
            .as_str()
            .filter(|target| !target.is_empty())
            .ok_or(ModelError::Unsupported)?;
        converted.0["dialer-proxy"] = target.into();
    }
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

    const VLESS_LINKS: &str = concat!(
        "vless://11111111-1111-4111-8111-111111111111@raw.example.invalid:443?security=tls&flow=xtls-rprx-vision#VLESS%20Vision\n",
        "vless://11111111-1111-4111-8111-111111111111@reality.example.invalid:443?security=reality&flow=xtls-rprx-vision&sni=identity.example.invalid&fp=chrome&pbk=dwrjY6KiAMPxKdlF7T4AWFXXBtsxeHCXtBQYeCOdZV4&sid=0123456789abcdef#VLESS%20REALITY\n",
        "vless://11111111-1111-4111-8111-111111111111@ws.example.invalid:443?security=tls&type=ws&path=%2Fsocket&host=front.example.invalid#VLESS%20WS\n",
        "vless://11111111-1111-4111-8111-111111111111@grpc.example.invalid:443?security=tls&type=grpc&serviceName=fixture&mode=multi&authority=front.example.invalid#VLESS%20gRPC\n",
    );

    #[test]
    fn verified_proxy_chain_roundtrips_between_surge_mihomo_and_sing_box() {
        let source = "Entry = socks5, entry.example.invalid, 1080\nExit = socks5, exit.example.invalid, 1080, underlying-proxy=Entry\n";
        let native = export_subscription(source, OutputFormat::Surge).unwrap();
        assert_eq!(native.as_str(), source);
        let sing_box = export_subscription(source, OutputFormat::SingBox).unwrap();
        let canonical: Value = serde_json::from_str(&sing_box).unwrap();
        assert_eq!(canonical["outbounds"][1]["detour"], "Entry");
        let mihomo = export_subscription(&sing_box, OutputFormat::Mihomo).unwrap();
        assert!(mihomo.contains("\"dialer-proxy\":\"Entry\""));
        let surge = export_subscription(&mihomo, OutputFormat::Surge).unwrap();
        assert!(surge.contains("underlying-proxy="));
        let roundtrip = export_subscription(&surge, OutputFormat::SingBox).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&roundtrip).unwrap(),
            canonical
        );
        let parsed = Subscription::parse(&surge).unwrap();
        assert!(matches!(
            parsed.nodes()[1].to_probe_sing_box(false),
            Err(ModelError::Unsupported)
        ));
    }

    #[test]
    fn sing_box_preserves_valid_udp_over_tcp_encodings() {
        for field in [json!(true), json!({"enabled": false, "version": 1})] {
            let source = json!([{
                "type":"socks", "tag":"sample", "server":"sample.example.invalid",
                "server_port":1080, "udp_over_tcp":field
            }]);
            let exported = export_subscription(&source.to_string(), OutputFormat::SingBox).unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&exported).unwrap()["outbounds"][0]["udp_over_tcp"],
                field
            );
        }
    }

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
        use crate::relay::gist::{GistFile, PolicyLink};
        use std::io::Write as _;

        let policies = export_subscription(SURGE_CONVERSION, OutputFormat::Surge).unwrap();
        let anytls = export_subscription(ANYTLS_JSON, OutputFormat::Surge).unwrap();
        let vless = export_subscription(VLESS_LINKS, OutputFormat::Surge).unwrap();
        let chain = export_subscription(
            r#"[{"type":"socks","tag":"Entry","server":"entry.example.invalid","server_port":1080},
                {"type":"socks","tag":"Exit","server":"exit.example.invalid","server_port":1080,"detour":"Entry"}]"#,
            OutputFormat::Surge,
        ).unwrap();
        let scratch =
            std::path::PathBuf::from(std::env::var_os("SCRIPT_HUB_SURGE_TEST_DIR").unwrap());
        let external = scratch.join("external-sample.txt");
        let mut external_file = std::fs::File::create_new(&external).unwrap();
        writeln!(
            external_file,
            "External sample = https, proxy.example.invalid, 443"
        )
        .unwrap();
        drop(external_file);
        let target = GistFile::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "nodes.txt").unwrap();
        let receipt = PolicyLink::from_raw(
            "https://gist.githubusercontent.com/example/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/raw/nodes.txt",
            &target,
        )
        .unwrap();
        let group = receipt
            .named_policy_group("东京 🇯🇵")
            .unwrap()
            .replace(receipt.subscription_url().as_str(), "external-sample.txt");
        let path = scratch.join("script-hub-surge-check.conf");
        let mut profile = std::fs::File::create_new(&path).unwrap();
        write!(
            profile,
            "[Proxy]\n{}{}{}{}\n[Proxy Group]\n{group}\nCheck = select, Tokyo SOCKS, Shadowsocks, Trojan, Strict AnyTLS, Exit, VLESS Vision, VLESS REALITY, VLESS WS, VLESS gRPC, 东京 🇯🇵\n[Rule]\nFINAL,Check\n",
            policies.as_str(), anytls.as_str(), chain.as_str(), vless.as_str()
        )
        .unwrap();
        drop(profile);
        let result = std::process::Command::new(std::env::var_os("SCRIPT_HUB_SURGE_BIN").unwrap())
            .current_dir(&scratch)
            .arg("--check")
            .arg(&path)
            .output();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(external).unwrap();
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
        let foreign = "alpha = ssr, example.invalid, 443, password=synthetic\n";
        assert!(matches!(
            export_subscription(foreign, OutputFormat::Surge),
            Err(RelayError::ExportNode { index: 1, .. })
        ));
    }

    #[test]
    fn surge_vless_preserves_supported_transport_and_reality_settings() {
        let output = export_subscription(VLESS_LINKS, OutputFormat::Surge).unwrap();
        let rows: Vec<_> = output.lines().collect();
        assert_eq!(rows.len(), 4);
        assert!(rows[0].contains(", tls=true, flow=xtls-rprx-vision"));
        assert!(rows[1].contains(", reality-public-key="));
        assert!(
            rows[1]
                .contains(", reality-short-id=\"0123456789abcdef\", sni=identity.example.invalid")
        );
        assert!(!rows[1].contains(", tls=true"));
        assert!(
            rows[2].contains(
                ", ws=true, ws-path=\"/socket\", ws-headers=\"Host:front.example.invalid\""
            )
        );
        assert!(rows[3].contains(", grpc=true, grpc-service-name=\"fixture\", grpc-mode=multi, grpc-authority=\"front.example.invalid\""));
        assert_eq!(
            export_subscription(&output, OutputFormat::Surge)
                .unwrap()
                .as_str(),
            output.as_str()
        );
    }

    #[test]
    fn surge_vless_converts_mihomo_headers_and_explicit_default_xudp() {
        let provider = r#"proxies:
  - {name: WS, type: vless, server: example.invalid, port: 443, uuid: fixture, udp: true, network: ws, ws-opts: {path: /socket, headers: {host: front.example.invalid, X-Test: fixture}}}
"#;
        let output = export_subscription(provider, OutputFormat::Surge).unwrap();
        assert!(output.contains("ws-headers=\"X-Test:fixture|host:front.example.invalid\""));
        let sing_box = r#"[{"type":"vless","tag":"XUDP","server":"example.invalid","server_port":443,"uuid":"fixture","packet_encoding":"xudp","tls":{"enabled":false}}]"#;
        let output = export_subscription(sing_box, OutputFormat::Surge).unwrap();
        assert_eq!(
            output.as_str(),
            "XUDP = vless, example.invalid, 443, username=\"fixture\"\n"
        );
    }

    #[test]
    fn surge_vless_conversion_refuses_unsupported_or_unsafe_options_without_dropping_nodes() {
        let first = VLESS_LINKS.lines().next().unwrap();
        for (query, expected) in [
            ("security=tls&type=xhttp", ModelError::Unsupported),
            ("security=none&type=grpc", ModelError::Unsupported),
            (
                "security=none&flow=xtls-rprx-vision",
                ModelError::Unsupported,
            ),
            (
                "security=tls&type=ws&flow=xtls-rprx-vision",
                ModelError::Unsupported,
            ),
            ("security=tls&fp=firefox", ModelError::Unsupported),
            ("security=tls&allowInsecure=1", ModelError::SecurityBlocked),
        ] {
            let input = format!(
                "{first}\nvless://synthetic@blocked.example.invalid:443?{query}#Private%20fixture\n"
            );
            let error = export_subscription(&input, OutputFormat::Surge).unwrap_err();
            assert!(
                matches!(error, RelayError::ExportNode { index: 2, error } if error == expected)
            );
            assert_eq!(
                export_subscription(&input, OutputFormat::Original)
                    .unwrap()
                    .as_str(),
                input
            );
        }
    }

    #[test]
    fn native_surge_export_requires_tls_verification_but_original_is_lossless() {
        let source = concat!(
            "[Proxy]\r\n",
            "TLS = https, tls.example.invalid, 443\r\n",
            "Node = anytls, node.example.invalid, 443, password=fixture, skip-cert-verify=true\r\n",
        );
        let result = export_subscription(source, OutputFormat::Surge);
        assert!(matches!(
            result,
            Err(RelayError::ExportNode {
                index: 2,
                error: ModelError::SecurityBlocked,
            })
        ));
        assert_eq!(
            export_subscription(source, OutputFormat::Original)
                .unwrap()
                .as_str(),
            source
        );
        let strict = source.replace("skip-cert-verify=true", "skip-cert-verify=false");
        assert_eq!(
            export_subscription(&strict, OutputFormat::Surge)
                .unwrap()
                .as_str(),
            strict
        );
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
