use super::{ModelError, Node, PrivateJson, parse_bool, required_field, valid_host, valid_label};
use base64::{Engine as _, engine::general_purpose::*};
use serde_json::{Value, json};
use std::collections::HashMap;

pub(in crate::relay) fn valid_surge_reality_key(key: &str) -> bool {
    matches!(key.len(), 43 | 44)
        && [URL_SAFE_NO_PAD, URL_SAFE, STANDARD_NO_PAD, STANDARD]
            .iter()
            .any(|engine| engine.decode(key).is_ok_and(|key| key.len() == 32))
}

pub(in crate::relay) fn valid_surge_reality_short_id(id: &str) -> bool {
    id.len() <= 16 && id.len().is_multiple_of(2) && id.bytes().all(|c| c.is_ascii_hexdigit())
}

fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn header_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^`_|~".contains(&byte))
}

pub(super) fn project(
    node: &Node,
    fields: &HashMap<&str, &str>,
    compatibility: bool,
    surge_target: bool,
) -> Result<Value, ModelError> {
    let get = |key: &str| fields.get(key).copied();
    let boolean = |key| get(key).map(parse_bool).transpose();
    let insecure = boolean("skip-cert-verify")?;
    if insecure == Some(true) && !compatibility {
        return Err(ModelError::SecurityBlocked);
    }
    let allowed = [
        "username",
        "flow",
        "encryption",
        "tls",
        "sni",
        "alpn",
        "skip-cert-verify",
        "reality-public-key",
        "reality-short-id",
        "ws",
        "ws-path",
        "ws-headers",
        "grpc",
        "grpc-service-name",
        "grpc-mode",
        "grpc-authority",
        "tfo",
        "block-quic",
        "test-url",
        "test-timeout",
        "underlying-proxy",
    ];
    if fields.keys().any(|key| !allowed.contains(key))
        || fields.values().any(|value| {
            value.contains("#!")
                || value
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
        })
        || get("encryption").is_some_and(|value| value != "none")
        || get("block-quic").is_some_and(|value| !matches!(value, "true" | "false" | "adaptive"))
        || node.name().contains(['\u{2028}', '\u{2029}'])
        || !valid_host(node.server())
    {
        return Err(ModelError::Unsupported);
    }
    let id = required_field(fields, "username")?;
    if !(uuid(id) || surge_target && id.len() <= 30 && valid_label(id) && id.trim() == id) {
        return Err(ModelError::Unsupported);
    }
    let public_key = get("reality-public-key");
    let short_id = get("reality-short-id");
    let explicit_tls = boolean("tls")?;
    let tls_enabled = public_key.is_some() || explicit_tls == Some(true);
    let sni = get("sni");
    if sni.is_some_and(|value| {
        !valid_host(value)
            || value.parse::<std::net::IpAddr>().is_ok()
            || value.eq_ignore_ascii_case("off")
    }) || (!tls_enabled && (sni.is_some() || get("alpn").is_some() || insecure.is_some()))
        || (short_id.is_some() && public_key.is_none())
        || (public_key.is_some() && (explicit_tls == Some(false) || insecure == Some(true)))
    {
        return Err(ModelError::Unsupported);
    }
    // Surge uses its native TLS stack. Don't invent a uTLS profile for probes or sing-box exports.
    if let Some(key) = public_key
        && (!surge_target
            || sni.is_none()
            || !valid_surge_reality_key(key)
            || short_id.is_some_and(|value| !valid_surge_reality_short_id(value)))
    {
        return Err(ModelError::Unsupported);
    }
    let ws = boolean("ws")? == Some(true);
    let grpc = boolean("grpc")? == Some(true);
    let flow = get("flow");
    if (ws && (grpc || public_key.is_some()))
        || (grpc && !tls_enabled)
        || (!ws && (get("ws-path").is_some() || get("ws-headers").is_some()))
        || (!grpc
            && ["grpc-service-name", "grpc-mode", "grpc-authority"]
                .iter()
                .any(|key| get(key).is_some()))
        || flow.is_some_and(|value| {
            ws || grpc
                || !tls_enabled
                || (value != "xtls-rprx-vision"
                    && !(surge_target && value == "xtls-rprx-vision-udp443"))
        })
    {
        return Err(ModelError::Unsupported);
    }
    let mut out = PrivateJson(json!({
        "type": "vless", "tag": node.name(), "server": node.server(), "server_port": node.port(), "uuid": id,
    }));
    if !surge_target {
        out.0["packet_encoding"] = "xudp".into();
    }
    if let Some(tfo) = boolean("tfo")? {
        out.0["tcp_fast_open"] = tfo.into();
    }
    if let Some(flow) = flow {
        out.0["flow"] = flow.into();
    }
    if tls_enabled {
        out.0["tls"] = json!({"enabled": true});
        if let Some(sni) = sni {
            out.0["tls"]["server_name"] = sni.into();
        }
        if let Some(insecure) = insecure {
            out.0["tls"]["insecure"] = insecure.into();
        }
        if let Some(alpn) = get("alpn") {
            let protocols: Vec<_> = alpn.split(',').map(str::trim).collect();
            if protocols.iter().any(|value| {
                !(1..=255).contains(&value.len())
                    || value.contains('#')
                    || !value.bytes().all(|byte| byte.is_ascii_graphic())
            }) {
                return Err(ModelError::Unsupported);
            }
            out.0["tls"]["alpn"] = json!(protocols);
        }
        if let Some(key) = public_key {
            out.0["tls"]["reality"] = json!({"enabled": true, "public_key": key});
            if let Some(short_id) = short_id {
                out.0["tls"]["reality"]["short_id"] = short_id.into();
            }
        }
    }
    if ws {
        out.0["transport"] = json!({"type":"ws"});
        if let Some(path) = get("ws-path") {
            if !path.starts_with('/') || !valid_label(path) {
                return Err(ModelError::Unsupported);
            }
            out.0["transport"]["path"] = path.into();
        }
        if let Some(headers) = get("ws-headers") {
            let mut seen = std::collections::HashSet::new();
            for header in headers.split('|') {
                let (key, value) = header.split_once(':').ok_or(ModelError::Unsupported)?;
                if !valid_label(value)
                    || !seen.insert(key.to_ascii_lowercase())
                    || !header_name(key)
                {
                    return Err(ModelError::Unsupported);
                }
                let key = if key.eq_ignore_ascii_case("host") {
                    "Host"
                } else {
                    key
                };
                out.0["transport"]["headers"][key] = value.into();
            }
        }
    } else if grpc {
        out.0["transport"] = json!({"type":"grpc"});
        for (key, field) in [
            ("service_name", "grpc-service-name"),
            ("mode", "grpc-mode"),
            ("authority", "grpc-authority"),
        ] {
            if let Some(value) = get(field) {
                let valid = match key {
                    "mode" => matches!(value, "gun" | "multi") && (surge_target || value == "gun"),
                    "authority" => {
                        surge_target
                            && valid_label(value)
                            && value.bytes().all(|byte| byte.is_ascii_graphic())
                    }
                    _ => !value.chars().any(char::is_control),
                };
                if !valid {
                    return Err(ModelError::Unsupported);
                }
                if key != "mode" || surge_target {
                    out.0["transport"][key] = value.into();
                }
            }
        }
    }
    Ok(std::mem::take(&mut out.0))
}
