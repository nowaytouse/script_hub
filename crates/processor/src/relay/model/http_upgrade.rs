//! Shared validation for VLESS HTTPUpgrade translations.

use std::net::Ipv6Addr;

use serde_json::Value;

use super::{ModelError, valid_host, valid_label};

pub(in crate::relay) fn valid_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains(['#', '\\'])
        && !path.chars().any(|c| c.is_control() || c.is_whitespace())
        // Surge removes Xray's early-data query parameter and waits for the upgrade.
        && !path.split_once('?').is_some_and(|(_, query)| {
            url::form_urlencoded::parse(query.as_bytes()).any(|(name, _)| name == "ed")
        })
}

pub(in crate::relay) fn valid_authority(authority: &str) -> bool {
    let valid_port = |port: &str| {
        port.bytes().all(|byte| byte.is_ascii_digit())
            && port.parse::<u16>().is_ok_and(|port| port != 0)
    };
    if let Some((ip, port)) = authority
        .strip_prefix('[')
        .and_then(|value| value.split_once(']'))
    {
        return ip.parse::<Ipv6Addr>().is_ok()
            && (port.is_empty() || port.strip_prefix(':').is_some_and(valid_port));
    }
    match authority.split_once(':') {
        Some((host, port)) => valid_host(host) && valid_port(port),
        None => valid_host(authority),
    }
}

pub(in crate::relay) fn authority(host: &str, port: Option<u16>) -> String {
    let mut value = if host.parse::<Ipv6Addr>().is_ok() {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    if let Some(port) = port {
        value.push(':');
        value.push_str(&port.to_string());
    }
    value
}

pub(in crate::relay) fn valid_header(name: &str, value: &str) -> bool {
    [
        "Connection",
        "Upgrade",
        "Host",
        "Sec-WebSocket-Key",
        "Sec-WebSocket-Version",
        "Sec-WebSocket-Protocol",
        "Sec-WebSocket-Extensions",
    ]
    .iter()
    .all(|reserved| !name.eq_ignore_ascii_case(reserved))
        && !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^`_~".contains(&byte))
        && valid_label(value)
        && value.trim() == value
        && !value.contains(['|', '\u{2028}', '\u{2029}'])
        && !value.contains("#!")
}

pub(super) fn insert_header(
    transport: &mut Value,
    name: &str,
    value: &str,
) -> Result<(), ModelError> {
    if name.eq_ignore_ascii_case("Host") {
        if transport.get("host").is_some() || !valid_authority(value) {
            return Err(ModelError::Unsupported);
        }
        transport["host"] = value.into();
    } else {
        if !valid_header(name, value)
            || transport["headers"]
                .as_object()
                .is_some_and(|headers| headers.keys().any(|key| key.eq_ignore_ascii_case(name)))
        {
            return Err(ModelError::Unsupported);
        }
        transport["headers"][name] = value.into();
    }
    Ok(())
}

pub(in crate::relay) fn normalize_defaults(source: &mut Value) -> Result<(), ModelError> {
    if source["type"] != "vless" || source["transport"]["type"] != "httpupgrade" {
        return Ok(());
    }
    let transport = &source["transport"];
    if transport.get("host").is_none() || transport["host"] == "" {
        let server = source["server"].as_str().ok_or(ModelError::Unsupported)?;
        let tls = &source["tls"];
        let host = if tls["enabled"] == true {
            let sni = tls["server_name"].as_str().filter(|sni| !sni.is_empty());
            authority(sni.unwrap_or(server), None)
        } else {
            let port = source["server_port"]
                .as_u64()
                .and_then(|port| u16::try_from(port).ok())
                .ok_or(ModelError::Unsupported)?;
            authority(server, Some(port))
        };
        source["transport"]["host"] = host.into();
    }
    let transport = source["transport"]
        .as_object_mut()
        .ok_or(ModelError::Unsupported)?;
    // An empty path selects `/` and an empty header map sends no custom headers.
    if transport.get("path").is_some_and(|path| path == "") {
        transport.remove("path");
    }
    if transport
        .get("headers")
        .and_then(Value::as_object)
        .is_some_and(|headers| headers.is_empty())
    {
        transport.remove("headers");
    }
    Ok(())
}
