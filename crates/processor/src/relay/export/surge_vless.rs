use std::fmt::Write as _;

use serde_json::Value;
use zeroize::Zeroizing;

use super::{ModelError, append_surge_tls_parameters, quote_surge};
use crate::relay::model::{valid_surge_reality_key, valid_surge_reality_short_id};

pub(super) fn append(source: &Value, row: &mut Zeroizing<String>) -> Result<(), ModelError> {
    if source
        .get("packet_encoding")
        .is_some_and(|value| value != "xudp")
    {
        return Err(ModelError::Unsupported);
    }
    let username = source["uuid"].as_str().ok_or(ModelError::Unsupported)?;
    let uuid = username.split('-').map(str::len).eq([8, 4, 4, 4, 12])
        && username
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-');
    if !uuid && !(1..=30).contains(&username.len()) {
        return Err(ModelError::Unsupported);
    }
    append_string(row, "username", username)?;

    let tls = source
        .get("tls")
        .filter(|tls| tls.as_object().is_none_or(|tls| !tls_disabled(tls)));
    let reality = tls.and_then(|tls| tls.get("reality"));
    if let Some(tls) = tls {
        let tls = tls.as_object().ok_or(ModelError::Unsupported)?;
        if tls.get("enabled") != Some(&Value::Bool(true))
            || tls.keys().any(|key| {
                ![
                    "enabled",
                    "server_name",
                    "insecure",
                    "alpn",
                    "reality",
                    "utls",
                ]
                .contains(&key.as_str())
            })
        {
            return Err(ModelError::Unsupported);
        }
        if let Some(utls) = tls.get("utls") {
            let utls = utls.as_object().ok_or(ModelError::Unsupported)?;
            if utls.get("enabled").and_then(Value::as_bool).is_none()
                || utls
                    .get("fingerprint")
                    .is_some_and(|fingerprint| fingerprint != "chrome")
                || utls
                    .keys()
                    .any(|key| !["enabled", "fingerprint"].contains(&key.as_str()))
            {
                return Err(ModelError::Unsupported);
            }
        }
        if let Some(reality) = reality {
            let reality = reality.as_object().ok_or(ModelError::Unsupported)?;
            if reality.get("enabled") != Some(&Value::Bool(true))
                || reality
                    .keys()
                    .any(|key| !["enabled", "public_key", "short_id"].contains(&key.as_str()))
                || !tls.contains_key("server_name")
            {
                return Err(ModelError::Unsupported);
            }
            let key = reality
                .get("public_key")
                .and_then(Value::as_str)
                .filter(|key| valid_surge_reality_key(key))
                .ok_or(ModelError::Unsupported)?;
            write!(row, ", reality-public-key={key}").map_err(|_| ModelError::Render)?;
            if let Some(id) = reality.get("short_id") {
                let id = id
                    .as_str()
                    .filter(|id| valid_surge_reality_short_id(id))
                    .ok_or(ModelError::Unsupported)?;
                append_string(row, "reality-short-id", id)?;
            }
        } else {
            row.push_str(", tls=true");
        }
        append_surge_tls_parameters(tls, row)?;
    }

    let transport = source.get("transport");
    if let Some(flow) = source.get("flow") {
        let flow = flow
            .as_str()
            .filter(|flow| matches!(*flow, "xtls-rprx-vision" | "xtls-rprx-vision-udp443"))
            .ok_or(ModelError::Unsupported)?;
        if tls.is_none() || transport.is_some() {
            return Err(ModelError::Unsupported);
        }
        write!(row, ", flow={flow}").map_err(|_| ModelError::Render)?;
    }
    if let Some(transport) = transport {
        let options = transport.as_object().ok_or(ModelError::Unsupported)?;
        match transport["type"].as_str() {
            Some("ws")
                if reality.is_none()
                    && options
                        .keys()
                        .all(|key| ["type", "path", "headers"].contains(&key.as_str())) =>
            {
                row.push_str(", ws=true");
                if let Some(path) = options.get("path") {
                    let path = path
                        .as_str()
                        .filter(|path| path.starts_with('/'))
                        .ok_or(ModelError::Unsupported)?;
                    append_string(row, "ws-path", path)?;
                }
                if let Some(headers) = options.get("headers") {
                    let headers = headers
                        .as_object()
                        .filter(|headers| !headers.is_empty())
                        .ok_or(ModelError::Unsupported)?;
                    let mut joined = Zeroizing::new(String::new());
                    for (key, value) in headers {
                        if key.is_empty()
                            || !key.bytes().all(|byte| {
                                byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^`_~".contains(&byte)
                            })
                        {
                            return Err(ModelError::Unsupported);
                        }
                        let value = value
                            .as_str()
                            .filter(|value| !value.is_empty() && !value.contains('|'))
                            .ok_or(ModelError::Unsupported)?;
                        if !joined.is_empty() {
                            joined.push('|');
                        }
                        write!(joined, "{key}:{value}").map_err(|_| ModelError::Render)?;
                    }
                    append_string(row, "ws-headers", &joined)?;
                }
            }
            Some("grpc")
                if tls.is_some()
                    && options.keys().all(|key| {
                        ["type", "service_name", "mode", "authority"].contains(&key.as_str())
                    }) =>
            {
                row.push_str(", grpc=true");
                if let Some(service) = options.get("service_name") {
                    append_string(
                        row,
                        "grpc-service-name",
                        service.as_str().ok_or(ModelError::Unsupported)?,
                    )?;
                }
                if let Some(mode) = options.get("mode") {
                    let mode = mode
                        .as_str()
                        .filter(|mode| matches!(*mode, "gun" | "multi"))
                        .ok_or(ModelError::Unsupported)?;
                    write!(row, ", grpc-mode={mode}").map_err(|_| ModelError::Render)?;
                }
                if let Some(authority) = options.get("authority") {
                    let authority = authority
                        .as_str()
                        .filter(|authority| {
                            !authority.is_empty()
                                && authority.bytes().all(|byte| byte.is_ascii_graphic())
                        })
                        .ok_or(ModelError::Unsupported)?;
                    append_string(row, "grpc-authority", authority)?;
                }
            }
            _ => return Err(ModelError::Unsupported),
        }
    }
    Ok(())
}

fn append_string(row: &mut Zeroizing<String>, key: &str, value: &str) -> Result<(), ModelError> {
    if value.contains("#!")
        || value
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        return Err(ModelError::Unsupported);
    }
    write!(row, ", {key}={}", quote_surge(value)?.as_str()).map_err(|_| ModelError::Render)
}

fn tls_disabled(tls: &serde_json::Map<String, Value>) -> bool {
    tls.len() == 1 && tls.get("enabled") == Some(&Value::Bool(false))
}

pub(super) fn normalize(source: &mut Value) {
    if source["type"] != "vless" {
        return;
    }
    let Some(object) = source.as_object_mut() else {
        return;
    };
    // Surge and sing-box use XUDP by default; Surge uses its own TLS ClientHello.
    if object
        .get("packet_encoding")
        .is_some_and(|value| value == "xudp")
    {
        object.remove("packet_encoding");
    }
    if let Some(tls) = object.get_mut("tls").and_then(Value::as_object_mut) {
        tls.remove("utls");
        if tls_disabled(tls) {
            object.remove("tls");
        }
    }
    if let Some(headers) = object
        .get_mut("transport")
        .and_then(|transport| transport.get_mut("headers"))
        .and_then(Value::as_object_mut)
        && let Some(alias) = headers
            .keys()
            .find(|key| *key != "Host" && key.eq_ignore_ascii_case("host"))
            .cloned()
        && let Some(host) = headers.remove(&alias)
    {
        headers.insert("Host".into(), host);
    }
}
