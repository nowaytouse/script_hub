//! Loss-aware mutations to the original subscription dialect.
use super::*;
use serde::Serialize;
use std::net::IpAddr;
use zeroize::ZeroizeOnDrop;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(tag = "property", rename_all = "snake_case", deny_unknown_fields)]
pub enum Setting {
    Server { value: String },
    Port { value: u16 },
    Udp { enabled: bool },
    UdpOverTcp { enabled: bool },
    TcpFastOpen { enabled: bool },
    VerifyTls,
    ServerName { value: String },
    Alpn { values: Vec<String> },
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Zeroize, ZeroizeOnDrop)]
pub(super) struct Mutation {
    pub(super) address: Option<String>,
    pub(super) port: Option<u16>,
    pub(super) settings: Vec<Setting>,
}

impl Mutation {
    pub(super) fn is_empty(&self) -> bool {
        self.address.is_none() && self.port.is_none() && self.settings.is_empty()
    }
}

fn kind(setting: &Setting) -> u8 {
    match setting {
        Setting::Server { .. } => 5,
        Setting::Port { .. } => 6,
        Setting::Udp { .. } => 0,
        Setting::UdpOverTcp { .. } => 7,
        Setting::TcpFastOpen { .. } => 1,
        Setting::VerifyTls => 2,
        Setting::ServerName { .. } => 3,
        Setting::Alpn { .. } => 4,
    }
}

pub(super) fn hostname(value: &str) -> bool {
    value.len() <= 253
        && value.parse::<IpAddr>().is_err()
        && value
            .strip_suffix('.')
            .unwrap_or(value)
            .split('.')
            .all(|label| {
                !label.is_empty()
                    && label.len() <= 63
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
}

pub fn validate_setting(setting: &Setting) -> Result<(), ModelError> {
    let valid = |s: &str| {
        !s.is_empty() && s.len() <= 255 && !s.chars().any(|c| c.is_control() || c.is_whitespace())
    };
    match setting {
        Setting::Server { value } if value.parse::<IpAddr>().is_err() && !hostname(value) => {
            Err(ModelError::InvalidEdit)
        }
        Setting::Port { value: 0 } => Err(ModelError::InvalidEdit),
        Setting::ServerName { value } if !valid(value) => Err(ModelError::InvalidEdit),
        Setting::Alpn { values }
            if values.is_empty()
                || values.len() > 32
                || values.iter().any(|v| !valid(v) || v.contains(',')) =>
        {
            Err(ModelError::InvalidEdit)
        }
        _ => Ok(()),
    }
}

pub(super) fn set_setting(
    node: &Node,
    mutation: &mut Mutation,
    setting: &Setting,
) -> Result<bool, ModelError> {
    validate_setting(setting)?;
    match setting {
        Setting::Server { value } => return set_host(node, mutation, value),
        Setting::Port { value } => {
            if mutation.port.unwrap_or(node.port) == *value {
                return Ok(false);
            }
            let mut candidate = mutation.clone();
            candidate.port = (*value != node.port).then_some(*value);
            snapshot(node, &candidate)?;
            *mutation = candidate;
            return Ok(true);
        }
        _ => {}
    }
    let mut candidate = mutation.clone();
    candidate.settings.retain(|old| kind(old) != kind(setting));
    candidate.settings.push(setting.clone());
    candidate.settings.sort_by_key(kind);
    if candidate == *mutation {
        return Ok(false);
    }
    let before = snapshot(node, mutation)?;
    let after = snapshot(node, &candidate)?;
    if before == after
        || matches!(node.source, NodeSource::Uri(_))
            && uri::UriNode::same_effective(&before, &after)?
    {
        return Ok(false);
    }
    *mutation = candidate;
    Ok(true)
}

pub(super) fn set_address(
    node: &Node,
    mutation: &mut Mutation,
    address: IpAddr,
) -> Result<bool, ModelError> {
    set_host(node, mutation, &address.to_string())
}

fn same_host(left: &str, right: &str) -> bool {
    match (left.parse::<IpAddr>(), right.parse::<IpAddr>()) {
        (Ok(a), Ok(b)) => a == b,
        (Err(_), Err(_)) => left
            .strip_suffix('.')
            .unwrap_or(left)
            .eq_ignore_ascii_case(right.strip_suffix('.').unwrap_or(right)),
        _ => false,
    }
}

fn set_host(node: &Node, mutation: &mut Mutation, host: &str) -> Result<bool, ModelError> {
    let host = match host.parse::<IpAddr>() {
        Ok(ip) => ip.to_string(),
        Err(_) if hostname(host) => host.to_owned(),
        Err(_) => return Err(ModelError::InvalidEdit),
    };
    if same_host(mutation.address.as_deref().unwrap_or(node.server()), &host) {
        return Ok(false);
    }
    let mut candidate = mutation.clone();
    candidate.address = (!same_host(node.server(), &host)).then_some(host);
    snapshot(node, &candidate)?;
    *mutation = candidate;
    Ok(true)
}

fn snapshot(node: &Node, mutation: &Mutation) -> Result<Zeroizing<String>, ModelError> {
    match &node.source {
        NodeSource::Native(source) | NodeSource::SubStore(source) | NodeSource::Mihomo(source) => {
            let mut value = PrivateJson(source.0.clone());
            apply_json(node, &mut value.0, mutation)?;
            serde_json::to_string(&value.0)
                .map(Zeroizing::new)
                .map_err(|_| ModelError::Render)
        }
        NodeSource::Surge(source) => {
            let mut edits = Vec::new();
            surge_edits(node, mutation, &mut edits)?;
            edits.sort_by_key(|edit| std::cmp::Reverse(edit.0.start));
            let mut raw = source.raw.clone();
            for (range, value) in edits {
                raw.replace_range(range, &value);
            }
            Ok(raw)
        }
        NodeSource::Uri(source) => source.render_mutation(node.name(), mutation),
    }
}

fn json_tls(node: &Node, value: &Value) -> Result<bool, ModelError> {
    let required = matches!(
        node.protocol(),
        "trojan" | "anytls" | "hysteria2" | "tuic" | "https"
    );
    match &node.source {
        NodeSource::Native(_) => match value.get("tls") {
            None if required => Err(ModelError::Unsupported),
            None => Ok(false),
            Some(Value::Object(tls)) => {
                let enabled = tls
                    .get("enabled")
                    .map(|v| v.as_bool().ok_or(ModelError::Unsupported))
                    .transpose()?
                    .unwrap_or(false);
                if required && !enabled {
                    return Err(ModelError::Unsupported);
                }
                Ok(enabled)
            }
            _ => Err(ModelError::Unsupported),
        },
        _ => {
            let enabled = value
                .get("tls")
                .map(|v| v.as_bool().ok_or(ModelError::Unsupported))
                .transpose()?
                .unwrap_or(required);
            if required && !enabled {
                return Err(ModelError::Unsupported);
            }
            Ok(enabled)
        }
    }
}

/// The only protocol settings exposed to scripts, independently of source dialect.
#[derive(Clone, Default)]
pub(super) struct ScriptFields {
    pub udp: Option<bool>,
    pub udp_over_tcp: Option<bool>,
    pub tfo: Option<bool>,
    pub tls: Option<bool>,
    pub verify_tls: Option<bool>,
    pub sni: Option<String>,
    pub alpn: Option<Vec<String>>,
    pub transport: Option<String>,
}

fn bounded_text(value: &str) -> Option<String> {
    (!value.is_empty()
        && value.len() <= 255
        && !value.chars().any(|c| c.is_control() || c.is_whitespace()))
    .then(|| value.to_owned())
}

fn bounded_alpn<'a>(items: impl Iterator<Item = &'a str>) -> Option<Vec<String>> {
    let items = items
        .take(33)
        .map(|item| (!item.contains(',')).then(|| bounded_text(item)).flatten())
        .collect::<Option<Vec<_>>>()?;
    (1..=32).contains(&items.len()).then_some(items)
}

fn transport(protocol: &str, explicit: Option<&str>) -> Option<String> {
    let kind = match (explicit, protocol) {
        (Some(kind), _) => kind,
        (None, "hysteria2" | "hy2" | "tuic" | "tuic-v5") => "quic",
        (
            None,
            "socks" | "socks5" | "socks5-tls" | "ss" | "shadowsocks" | "http" | "https" | "trojan"
            | "anytls" | "vmess" | "vless",
        ) => "tcp",
        _ => return None,
    };
    matches!(
        kind,
        "tcp" | "ws" | "http" | "httpupgrade" | "grpc" | "quic"
    )
    .then(|| kind.to_owned())
}

fn native_script_fields(node: &Node, value: &Value, tls: Option<bool>) -> ScriptFields {
    let protocol = node.protocol();
    let parameters = value.get("tls");
    let normal_tls = tls == Some(true)
        && !parameters.is_some_and(|value| {
            value
                .get("reality")
                .and_then(|value| value.get("enabled"))
                .and_then(Value::as_bool)
                == Some(true)
        });
    ScriptFields {
        udp: match value.get("network") {
            Some(network) if network == "tcp" => Some(false),
            Some(network) if network == "udp" => Some(true),
            None if matches!(protocol, "http" | "https") => Some(false),
            None if matches!(
                protocol,
                "socks"
                    | "socks5"
                    | "ss"
                    | "shadowsocks"
                    | "trojan"
                    | "anytls"
                    | "vmess"
                    | "vless"
                    | "hysteria2"
                    | "hy2"
                    | "tuic"
            ) =>
            {
                Some(true)
            }
            _ => None,
        },
        udp_over_tcp: matches!(protocol, "socks" | "shadowsocks")
            .then(|| {
                value.get("udp_over_tcp").map_or(Some(false), |value| {
                    value.as_bool().or_else(|| {
                        value
                            .as_object()?
                            .get("enabled")
                            .map_or(Some(false), Value::as_bool)
                    })
                })
            })
            .flatten(),
        tfo: value
            .get("tcp_fast_open")
            .map_or(Some(false), Value::as_bool),
        tls,
        verify_tls: normal_tls
            .then(|| {
                parameters
                    .and_then(|value| value.get("insecure"))
                    .map_or(Some(true), |value| {
                        value.as_bool().map(|insecure| !insecure)
                    })
            })
            .flatten(),
        sni: (tls == Some(true)
            && !parameters.is_some_and(|value| {
                value.get("disable_sni").and_then(Value::as_bool) == Some(true)
            }))
        .then(|| {
            effective_sni(
                parameters.and_then(|value| value.get("server_name")),
                node.server(),
            )
        })
        .flatten(),
        alpn: (tls == Some(true))
            .then(|| json_alpn(parameters?.get("alpn")?))
            .flatten(),
        transport: transport(
            protocol,
            value
                .get("transport")
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
        ),
    }
}

fn effective_sni(value: Option<&Value>, server: &str) -> Option<String> {
    match value {
        Some(value) => bounded_text(value.as_str()?),
        None if hostname(server) => bounded_text(server),
        None => None,
    }
}

fn json_alpn(value: &Value) -> Option<Vec<String>> {
    let array = value.as_array()?;
    (array.len() <= 32 && array.iter().all(Value::is_string)).then_some(())?;
    bounded_alpn(array.iter().filter_map(Value::as_str))
}

pub(super) fn script_fields(node: &Node, mutation: &Mutation) -> ScriptFields {
    let protocol = node.protocol();
    let mut fields = match &node.source {
        NodeSource::Native(value) => {
            native_script_fields(node, &value.0, json_tls(node, &value.0).ok())
        }
        NodeSource::Uri(source) => {
            source
                .project(true)
                .ok()
                .map(PrivateJson)
                .map_or_else(ScriptFields::default, |value| {
                    let tls = value
                        .0
                        .get("tls")
                        .and_then(|value| value.get("enabled"))
                        .map_or(Some(false), Value::as_bool);
                    native_script_fields(node, &value.0, tls)
                })
        }
        NodeSource::SubStore(value) | NodeSource::Mihomo(value) => {
            let value = &value.0;
            let tls = json_tls(node, value).ok();
            ScriptFields {
                udp: value.get("udp").and_then(Value::as_bool),
                tfo: value.get("tfo").and_then(Value::as_bool),
                tls,
                verify_tls: (tls == Some(true) && value.get("reality-opts").is_none())
                    .then(|| {
                        value.get("skip-cert-verify").map_or(Some(true), |value| {
                            value.as_bool().map(|insecure| !insecure)
                        })
                    })
                    .flatten(),
                sni: (tls == Some(true))
                    .then(|| {
                        effective_sni(value.get(identity_key(node, value).ok()?), node.server())
                    })
                    .flatten(),
                alpn: (tls == Some(true))
                    .then(|| json_alpn(value.get("alpn")?))
                    .flatten(),
                transport: transport(protocol, value.get("network").and_then(Value::as_str)),
                ..Default::default()
            }
        }
        NodeSource::Surge(source) => {
            let field = |key| {
                source
                    .fields
                    .iter()
                    .find(|field| field.key == key)
                    .filter(|field| uncertain_token(source, field.value_range.clone()).is_none())
                    .map(|field| field.value.as_str())
            };
            let boolean = |key| field(key).and_then(|value| parse_bool(value).ok());
            let tls_default = Some(matches!(
                protocol,
                "https" | "socks5-tls" | "trojan" | "anytls" | "hysteria2" | "tuic" | "tuic-v5"
            ));
            let tls = field("tls").map_or(tls_default, |value| parse_bool(value).ok());
            ScriptFields {
                udp: boolean("udp-relay").or(match protocol {
                    "socks5" | "socks5-tls" | "ss" | "http" | "https" => Some(false),
                    "anytls" => Some(true),
                    _ => None,
                }),
                tfo: field("tfo").map_or(Some(false), |value| parse_bool(value).ok()),
                tls,
                verify_tls: (tls == Some(true)
                    && field("server-cert-fingerprint-sha256").is_none())
                .then(|| {
                    field("skip-cert-verify").map_or(Some(true), |value| {
                        parse_bool(value).ok().map(|insecure| !insecure)
                    })
                })
                .flatten(),
                sni: (tls == Some(true))
                    .then(|| match field("sni") {
                        Some(value) if value.eq_ignore_ascii_case("off") => None,
                        Some(value) => bounded_text(value),
                        None if hostname(node.server()) => bounded_text(node.server()),
                        None => None,
                    })
                    .flatten(),
                alpn: (tls == Some(true))
                    .then(|| bounded_alpn(field("alpn")?.split(',').map(str::trim)))
                    .flatten(),
                transport: transport(protocol, None),
                ..Default::default()
            }
        }
    };
    for setting in &mutation.settings {
        match setting {
            Setting::Udp { enabled } => fields.udp = Some(*enabled),
            Setting::UdpOverTcp { enabled } => fields.udp_over_tcp = Some(*enabled),
            Setting::TcpFastOpen { enabled } => fields.tfo = Some(*enabled),
            Setting::VerifyTls if fields.verify_tls.is_some() => fields.verify_tls = Some(true),
            Setting::ServerName { value } if matches!(node.source, NodeSource::Surge(_)) => {
                fields.sni = (!value.eq_ignore_ascii_case("off"))
                    .then(|| bounded_text(value))
                    .flatten();
            }
            Setting::ServerName { value }
                if fields.sni.is_some()
                    || !matches!(node.source, NodeSource::Native(ref source) if source.0["tls"]["disable_sni"] == true) =>
            {
                fields.sni = bounded_text(value)
            }
            Setting::Alpn { values } => fields.alpn = Some(values.clone()),
            _ => {}
        }
    }
    fields
}

fn explicit_host(headers: &Value) -> bool {
    let Some(headers) = headers.as_object() else {
        return false;
    };
    let hosts: Vec<_> = headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case("host"))
        .collect();
    hosts.len() == 1
        && hosts[0]
            .1
            .as_str()
            .is_some_and(|s| !s.is_empty() && !s.chars().any(char::is_control))
}

fn safe_transport(node: &Node, value: &Value, native: bool) -> bool {
    if matches!(node.protocol(), "http" | "https") {
        return false;
    }
    if native {
        return match value.get("transport") {
            None => true,
            Some(Value::Object(transport))
                if transport.get("type").and_then(Value::as_str) == Some("ws") =>
            {
                transport.get("headers").is_some_and(explicit_host)
            }
            _ => false,
        };
    }
    match value.get("network") {
        None | Some(Value::String(_))
            if value.get("network").is_none()
                || value.get("network").and_then(Value::as_str) == Some("tcp") =>
        {
            value.get("ws-opts").is_none() && value.get("grpc-opts").is_none()
        }
        Some(Value::String(network)) if network == "ws" && value.get("grpc-opts").is_none() => {
            value
                .get("ws-opts")
                .and_then(|opts| opts.get("headers"))
                .is_some_and(explicit_host)
        }
        _ => false,
    }
}

fn identity_key(node: &Node, value: &Value) -> Result<&'static str, ModelError> {
    let key = if matches!(node.source, NodeSource::Mihomo(_))
        && matches!(node.protocol(), "vmess" | "vless")
    {
        "servername"
    } else {
        "sni"
    };
    let other = if key == "sni" { "servername" } else { "sni" };
    if value.get(other).is_some() {
        return Err(ModelError::Unsupported);
    }
    if value.get(key).is_some_and(|v| {
        !v.as_str()
            .is_some_and(|s| !s.is_empty() && !s.chars().any(char::is_control))
    }) {
        return Err(ModelError::Unsupported);
    }
    Ok(key)
}

pub(super) fn apply_json(
    node: &Node,
    value: &mut Value,
    mutation: &Mutation,
) -> Result<(), ModelError> {
    if mutation.is_empty() {
        return Ok(());
    }
    if !matches!(
        node.protocol(),
        "socks5"
            | "socks"
            | "http"
            | "https"
            | "ss"
            | "shadowsocks"
            | "trojan"
            | "anytls"
            | "vmess"
            | "vless"
            | "hysteria2"
            | "tuic"
    ) {
        return Err(ModelError::Unsupported);
    }
    let native = matches!(node.source, NodeSource::Native(_));
    let tls = json_tls(node, value)?;
    if let Some(address) = &mutation.address {
        if !safe_transport(node, value, native) {
            return Err(ModelError::Unsupported);
        }
        if tls && !hostname(node.server()) {
            return Err(ModelError::Unsupported);
        }
        if tls {
            if native {
                let obj = value
                    .get_mut("tls")
                    .and_then(Value::as_object_mut)
                    .ok_or(ModelError::Unsupported)?;
                match obj.get("server_name") {
                    None => {
                        obj.insert("server_name".into(), node.server().into());
                    }
                    Some(Value::String(s)) if !s.is_empty() => {}
                    _ => return Err(ModelError::Unsupported),
                }
            } else {
                let key = identity_key(node, value)?;
                match value.get(key) {
                    None => {
                        value[key] = node.server().into();
                    }
                    Some(Value::String(s)) if !s.is_empty() => {}
                    _ => return Err(ModelError::Unsupported),
                }
            }
        }
        value["server"] = address.as_str().into();
    }
    if let Some(port) = mutation.port {
        let key = if native { "server_port" } else { "port" };
        if !value.get(key).is_some_and(|v| v.as_u64().is_some()) {
            return Err(ModelError::Unsupported);
        }
        value[key] = port.into();
    }
    for setting in &mutation.settings {
        match setting {
            Setting::UdpOverTcp { enabled }
                if native && matches!(node.protocol(), "socks" | "shadowsocks") =>
            {
                if let Some(current) = value.get_mut("udp_over_tcp") {
                    if current.is_boolean() {
                        *current = (*enabled).into();
                    } else {
                        let options = current.as_object_mut().ok_or(ModelError::Unsupported)?;
                        if options
                            .keys()
                            .any(|key| !matches!(key.as_str(), "enabled" | "version"))
                            || options
                                .get("enabled")
                                .is_some_and(|value| !value.is_boolean())
                            || options
                                .get("version")
                                .is_some_and(|value| !matches!(value.as_u64(), Some(1 | 2)))
                        {
                            return Err(ModelError::Unsupported);
                        }
                        options.insert("enabled".into(), (*enabled).into());
                    }
                } else {
                    value["udp_over_tcp"] = json!({"enabled": enabled});
                }
            }
            Setting::TcpFastOpen { enabled } if native => {
                if value.get("tcp_fast_open").is_some_and(|v| !v.is_boolean()) {
                    return Err(ModelError::Unsupported);
                }
                value["tcp_fast_open"] = (*enabled).into();
            }
            Setting::Udp { enabled }
                if !native
                    && matches!(
                        node.protocol(),
                        "socks5"
                            | "ss"
                            | "trojan"
                            | "vmess"
                            | "vless"
                            | "hysteria2"
                            | "tuic"
                            | "anytls"
                    ) =>
            {
                if node.protocol() == "anytls" && !*enabled {
                    return Err(ModelError::Unsupported);
                }
                if value.get("udp").is_some_and(|v| !v.is_boolean()) {
                    return Err(ModelError::Unsupported);
                }
                value["udp"] = (*enabled).into();
            }
            Setting::VerifyTls if tls && native => {
                let obj = value
                    .get_mut("tls")
                    .and_then(Value::as_object_mut)
                    .ok_or(ModelError::Unsupported)?;
                if obj.get("insecure").is_some_and(|v| !v.is_boolean()) {
                    return Err(ModelError::Unsupported);
                }
                obj.insert("insecure".into(), false.into());
            }
            Setting::VerifyTls if tls => {
                if value
                    .get("skip-cert-verify")
                    .is_some_and(|v| !v.is_boolean())
                {
                    return Err(ModelError::Unsupported);
                }
                value["skip-cert-verify"] = false.into();
            }
            Setting::ServerName { value: name } if tls && native => {
                let obj = value
                    .get_mut("tls")
                    .and_then(Value::as_object_mut)
                    .ok_or(ModelError::Unsupported)?;
                if obj
                    .get("server_name")
                    .is_some_and(|v| !v.as_str().is_some_and(|s| !s.is_empty()))
                {
                    return Err(ModelError::Unsupported);
                }
                obj.insert("server_name".into(), name.as_str().into());
            }
            Setting::ServerName { value: name } if tls => {
                let key = identity_key(node, value)?;
                value[key] = name.as_str().into();
            }
            Setting::Alpn { values } if tls && native => {
                value
                    .get_mut("tls")
                    .and_then(Value::as_object_mut)
                    .ok_or(ModelError::Unsupported)?
                    .insert("alpn".into(), json!(values));
            }
            Setting::Alpn { values } if tls => value["alpn"] = json!(values),
            _ => return Err(ModelError::Unsupported),
        }
    }
    Ok(())
}

pub(super) fn surge_edits(
    node: &Node,
    mutation: &Mutation,
    edits: &mut Vec<(Range<usize>, Zeroizing<String>)>,
) -> Result<(), ModelError> {
    if mutation.is_empty() {
        return Ok(());
    }
    let NodeSource::Surge(source) = &node.source else {
        return Err(ModelError::Unsupported);
    };
    if !matches!(
        node.protocol(),
        "socks5" | "http" | "https" | "ss" | "trojan" | "anytls" | "vmess"
    ) || uncertain_identity(source).iter().any(Option::is_some)
    {
        return Err(ModelError::Unsupported);
    }
    let field = |key: &str| source.fields.iter().find(|f| f.key == key);
    if field("sni").is_some_and(|f| {
        f.value.is_empty() || uncertain_token(source, f.value_range.clone()).is_some()
    }) {
        return Err(ModelError::Unsupported);
    }
    let required = matches!(node.protocol(), "trojan" | "anytls" | "https");
    let tls = match field("tls") {
        Some(f) => parse_bool(&f.value)?,
        None => required,
    };
    if required && !tls {
        return Err(ModelError::Unsupported);
    }
    if let Some(address) = &mutation.address {
        if matches!(node.protocol(), "http" | "https")
            || source.fields.iter().any(|f| {
                ![
                    "tls",
                    "sni",
                    "alpn",
                    "skip-cert-verify",
                    "tfo",
                    "udp-relay",
                    "block-quic",
                    "test-url",
                    "test-timeout",
                    "password",
                    "encrypt-method",
                    "username",
                    "alterid",
                    "underlying-proxy",
                ]
                .contains(&f.key.as_str())
            })
        {
            return Err(ModelError::Unsupported);
        }
        if tls && !hostname(node.server()) {
            return Err(ModelError::Unsupported);
        }
        edits.push((source.server_range.clone(), quote_surge(address)?));
    }
    if let Some(port) = mutation.port {
        if uncertain_token(source, source.port_range.clone()).is_some() {
            return Err(ModelError::Unsupported);
        }
        edits.push((source.port_range.clone(), Zeroizing::new(port.to_string())));
    }
    let mut additions = Vec::<(String, Zeroizing<String>)>::new();
    let mut set = |key: &str, value: String| -> Result<(), ModelError> {
        let encoded = if matches!(key, "udp-relay" | "tfo" | "skip-cert-verify") {
            Zeroizing::new(value)
        } else {
            quote_surge(&value)?
        };
        if let Some(f) = field(key) {
            if uncertain_token(source, f.value_range.clone()).is_some() {
                return Err(ModelError::Unsupported);
            }
            edits.push((f.value_range.clone(), encoded));
        } else {
            additions.push((key.to_owned(), encoded));
        }
        Ok(())
    };
    if mutation.address.is_some()
        && tls
        && field("sni").is_none()
        && !mutation
            .settings
            .iter()
            .any(|s| matches!(s, Setting::ServerName { .. }))
    {
        set("sni", node.server().to_owned())?;
    }
    for setting in &mutation.settings {
        match setting {
            Setting::Udp { enabled } => set("udp-relay", enabled.to_string())?,
            Setting::TcpFastOpen { enabled } => set("tfo", enabled.to_string())?,
            Setting::VerifyTls if tls => set("skip-cert-verify", "false".into())?,
            Setting::ServerName { value } if tls => set("sni", value.clone())?,
            Setting::Alpn { values } if tls => set("alpn", values.join(","))?,
            _ => return Err(ModelError::Unsupported),
        }
    }
    drop(set);
    if !additions.is_empty() {
        let code_end = surge_code_end(&source.raw);
        let pos = source.raw[..code_end]
            .trim_end_matches(char::is_whitespace)
            .len();
        let suffix = additions
            .into_iter()
            .map(|(k, v)| format!(", {k}={}", v.as_str()))
            .collect::<String>();
        edits.push((pos..pos, Zeroizing::new(suffix)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_reads_explicit_and_previously_mutated_protocol_values() {
        let parsed = Subscription::parse(concat!(
            "n = trojan, node.example.invalid, 443, password=fixture, ",
            "skip-cert-verify=true, tfo=false, sni=before.example.invalid, alpn=\"h2,http/1.1\"\n",
        ))
        .unwrap();
        let node = &parsed.nodes()[0];
        let mut mutation = Mutation::default();
        let fields = script_fields(node, &mutation);
        assert_eq!(fields.tls, Some(true));
        assert_eq!(fields.verify_tls, Some(false));
        assert_eq!(fields.tfo, Some(false));
        assert_eq!(fields.sni.as_deref(), Some("before.example.invalid"));
        assert_eq!(fields.alpn.unwrap(), ["h2", "http/1.1"]);
        assert_eq!(fields.transport.as_deref(), Some("tcp"));
        for setting in [
            Setting::VerifyTls,
            Setting::TcpFastOpen { enabled: true },
            Setting::ServerName {
                value: "after.example.invalid".into(),
            },
            Setting::Alpn {
                values: vec!["http/1.1".into()],
            },
        ] {
            assert!(set_setting(node, &mut mutation, &setting).unwrap());
        }
        let fields = script_fields(node, &mutation);
        assert_eq!(fields.verify_tls, Some(true));
        assert_eq!(fields.tfo, Some(true));
        assert_eq!(fields.sni.as_deref(), Some("after.example.invalid"));
        assert_eq!(fields.alpn.unwrap(), ["http/1.1"]);

        let uri = Subscription::parse(
            "trojan://fixture@node.example.invalid:443?peer=tls.example.invalid&alpn=h2#test",
        )
        .unwrap();
        let fields = script_fields(&uri.nodes()[0], &Mutation::default());
        assert_eq!(fields.verify_tls, Some(true));
        assert_eq!(fields.sni.as_deref(), Some("tls.example.invalid"));
        assert_eq!(fields.alpn.unwrap(), ["h2"]);
    }

    #[test]
    fn udp_over_tcp_toggles_only_known_sing_box_mappings_and_preserves_version() {
        let parsed = Subscription::parse(r#"[{"type":"socks","tag":"n","server":"node.example.invalid","server_port":1080,"udp_over_tcp":{"enabled":false,"version":1}}]"#).unwrap();
        let node = &parsed.nodes()[0];
        let mut mutation = Mutation::default();
        assert_eq!(script_fields(node, &mutation).udp_over_tcp, Some(false));
        let enable = Setting::UdpOverTcp { enabled: true };
        assert!(set_setting(node, &mut mutation, &enable).unwrap());
        assert_eq!(script_fields(node, &mutation).udp_over_tcp, Some(true));
        let output: Value = serde_json::from_str(&snapshot(node, &mutation).unwrap()).unwrap();
        assert_eq!(
            output["udp_over_tcp"],
            json!({"enabled": true, "version": 1})
        );
        let parsed = Subscription::parse("n = socks5, node.example.invalid, 1080\n").unwrap();
        assert_eq!(
            set_setting(&parsed.nodes()[0], &mut Mutation::default(), &enable),
            Err(ModelError::Unsupported)
        );
    }

    #[test]
    fn server_and_port_mutations_preserve_dialect_and_identity() {
        for (input, port_key) in [
            (
                r#"[{"type":"trojan","tag":"n","server":"old.example","server_port":443,"password":"x","tls":{"enabled":true},"extra":"kept"}]"#,
                "server_port",
            ),
            (
                r#"{"proxies":[{"type":"trojan","name":"n","server":"old.example","port":443,"password":"x","extra":"kept"}]}"#,
                "port",
            ),
            (
                r#"[{"type":"trojan","name":"n","server":"old.example","port":443,"password":"x","extra":"kept"}]"#,
                "port",
            ),
        ] {
            let sub = Subscription::parse(input).unwrap();
            let node = &sub.nodes()[0];
            let mut mutation = Mutation::default();
            assert!(set_setting(node, &mut mutation, &Setting::Port { value: 8443 }).unwrap());
            assert!(
                set_setting(
                    node,
                    &mut mutation,
                    &Setting::Server {
                        value: "new.example".into()
                    }
                )
                .unwrap()
            );
            assert!(set_address(node, &mut mutation, "2001:db8::1".parse().unwrap()).unwrap());
            let output: Value = serde_json::from_str(&snapshot(node, &mutation).unwrap()).unwrap();
            assert_eq!(output[port_key], 8443);
            assert_eq!(output["server"], "2001:db8::1");
            assert_eq!(output["extra"], "kept");
            let identity = if port_key == "server_port" {
                &output["tls"]["server_name"]
            } else {
                &output["sni"]
            };
            assert_eq!(identity, "old.example");
        }
    }

    #[test]
    fn port_and_host_noops_validate_and_preserve_prior_mutation() {
        let sub = Subscription::parse(
            r#"[{"type":"socks","tag":"n","server":"192.0.2.4","server_port":1080}]"#,
        )
        .unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        assert!(!set_setting(node, &mut mutation, &Setting::Port { value: 1080 }).unwrap());
        assert!(
            !set_setting(
                node,
                &mut mutation,
                &Setting::Server {
                    value: "192.0.2.4".into()
                }
            )
            .unwrap()
        );
        assert!(
            set_setting(
                node,
                &mut mutation,
                &Setting::Server {
                    value: "new.example".into()
                }
            )
            .unwrap()
        );
        let before = mutation.clone();
        for host in [
            "",
            "https://example.com",
            "a..example",
            "[::1]",
            "example.com:443",
            "a.example..",
        ] {
            assert!(matches!(
                set_setting(node, &mut mutation, &Setting::Server { value: host.into() }),
                Err(ModelError::InvalidEdit)
            ));
            assert!(mutation == before);
        }
        assert!(matches!(
            set_setting(node, &mut mutation, &Setting::Port { value: 0 }),
            Err(ModelError::InvalidEdit)
        ));
        assert!(mutation == before);
    }

    #[test]
    fn surge_port_and_server_preserve_line_endings() {
        let sub = Subscription::parse(
            "n = trojan, old.example, 443, password=x, sni=identity.example\r\n",
        )
        .unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        set_setting(node, &mut mutation, &Setting::Port { value: 8443 }).unwrap();
        set_setting(
            node,
            &mut mutation,
            &Setting::Server {
                value: "new.example".into(),
            },
        )
        .unwrap();
        let output = snapshot(node, &mutation).unwrap();
        let parsed = Subscription::parse(&output).unwrap();
        let node = parsed.nodes()[0].to_sing_box(false).unwrap();
        assert_eq!(node["server"], "new.example");
        assert_eq!(node["server_port"], 8443);
        assert!(output.contains("sni=identity.example"));
        assert!(output.ends_with("\r\n"));
    }

    #[test]
    fn uri_port_and_ipv6_host_preserve_query_and_fragment() {
        let sub = Subscription::parse(
            "trojan://secret@old.example:443?x=a%2Fb&peer=identity.example#name",
        )
        .unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        set_setting(node, &mut mutation, &Setting::Port { value: 8443 }).unwrap();
        set_setting(
            node,
            &mut mutation,
            &Setting::Server {
                value: "2001:db8::1".into(),
            },
        )
        .unwrap();
        let output = snapshot(node, &mutation).unwrap();
        assert_eq!(
            output.as_str(),
            "trojan://secret@[2001:db8::1]:8443?x=a%2Fb&peer=identity.example#name"
        );
    }

    #[test]
    fn equivalent_uri_values_do_not_rewrite_original_formatting() {
        let sub =
            Subscription::parse("trojan://secret@Old.Example:00443?peer=identity.example#name")
                .unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        assert!(
            !set_setting(
                node,
                &mut mutation,
                &Setting::Server {
                    value: "old.example.".into()
                }
            )
            .unwrap()
        );
        assert!(!set_setting(node, &mut mutation, &Setting::Port { value: 443 }).unwrap());
        assert!(mutation.is_empty());
        assert_eq!(
            snapshot(node, &mutation).unwrap().as_str(),
            "trojan://secret@Old.Example:00443?peer=identity.example#name"
        );
    }

    #[test]
    fn vmess_port_preserves_string_or_number() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        for port in [json!("443"), json!(443)] {
            let source = json!({"v":"2", "ps":"n", "add":"old.example", "port":port, "id":"id", "tls":"tls", "extra":"kept"});
            let input = format!(
                "vmess://{}",
                STANDARD.encode(serde_json::to_vec(&source).unwrap())
            );
            let sub = Subscription::parse(&input).unwrap();
            let node = &sub.nodes()[0];
            let mut mutation = Mutation::default();
            set_setting(node, &mut mutation, &Setting::Port { value: 8443 }).unwrap();
            let output = snapshot(node, &mutation).unwrap();
            let decoded = STANDARD
                .decode(output.strip_prefix("vmess://").unwrap())
                .unwrap();
            let value: Value = serde_json::from_slice(&decoded).unwrap();
            assert_eq!(
                value["port"],
                if source["port"].is_string() {
                    json!("8443")
                } else {
                    json!(8443)
                }
            );
            assert_eq!(value["extra"], "kept");
        }
    }

    #[test]
    fn unsupported_host_change_keeps_existing_port() {
        let sub = Subscription::parse("trojan://secret@192.0.2.1:443#n").unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        set_setting(node, &mut mutation, &Setting::Port { value: 8443 }).unwrap();
        let before = mutation.clone();
        assert!(matches!(
            set_setting(
                node,
                &mut mutation,
                &Setting::Server {
                    value: "new.example".into()
                }
            ),
            Err(ModelError::Unsupported)
        ));
        assert!(mutation == before);
    }

    #[test]
    fn native_address_preserves_explicit_tls_identity() {
        let sub = Subscription::parse(r#"[{"type":"trojan","tag":"n","server":"origin.example","server_port":443,"password":"x","tls":{"enabled":true,"server_name":"identity.example"}}]"#).unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        assert!(set_address(node, &mut mutation, "2001:db8::1".parse().unwrap()).unwrap());
        let output: Value = serde_json::from_str(&snapshot(node, &mutation).unwrap()).unwrap();
        assert_eq!(output["server"], "2001:db8::1");
        assert_eq!(output["tls"]["server_name"], "identity.example");
    }

    #[test]
    fn mihomo_address_retains_implicit_hostname() {
        let sub = Subscription::parse(r#"{"proxies":[{"name":"n","type":"trojan","server":"origin.example","port":443,"password":"x"}]}"#).unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        set_address(node, &mut mutation, "192.0.2.1".parse().unwrap()).unwrap();
        let output: Value = serde_json::from_str(&snapshot(node, &mutation).unwrap()).unwrap();
        assert_eq!(output["server"], "192.0.2.1");
        assert_eq!(output["sni"], "origin.example");
    }

    #[test]
    fn mihomo_tls_identity_uses_protocol_native_key() {
        for (protocol, key) in [
            ("vmess", "servername"),
            ("vless", "servername"),
            ("trojan", "sni"),
        ] {
            let input = format!(
                r#"{{"proxies":[{{"name":"n","type":"{protocol}","server":"origin.example","port":443,"tls":true}}]}}"#
            );
            let sub = Subscription::parse(&input).unwrap();
            let node = &sub.nodes()[0];
            let mut mutation = Mutation::default();
            set_address(node, &mut mutation, "192.0.2.1".parse().unwrap()).unwrap();
            let output: Value = serde_json::from_str(&snapshot(node, &mutation).unwrap()).unwrap();
            assert_eq!(output[key], "origin.example");
            assert!(
                output
                    .get(if key == "sni" { "servername" } else { "sni" })
                    .is_none()
            );
        }
    }

    #[test]
    fn mihomo_vmess_preserves_explicit_servername() {
        let sub = Subscription::parse(r#"{"proxies":[{"name":"n","type":"vmess","server":"origin.example","port":443,"tls":true,"servername":"identity.example"}]}"#).unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        set_address(node, &mut mutation, "192.0.2.1".parse().unwrap()).unwrap();
        let output: Value = serde_json::from_str(&snapshot(node, &mutation).unwrap()).unwrap();
        assert_eq!(output["servername"], "identity.example");
        assert!(output.get("sni").is_none());
    }

    #[test]
    fn strict_verification_and_last_setting_win() {
        let sub = Subscription::parse(r#"[{"type":"trojan","tag":"n","server":"origin.example","server_port":443,"password":"x","tls":{"enabled":true,"insecure":true}}]"#).unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        set_setting(node, &mut mutation, &Setting::VerifyTls).unwrap();
        set_setting(node, &mut mutation, &Setting::TcpFastOpen { enabled: true }).unwrap();
        set_setting(
            node,
            &mut mutation,
            &Setting::TcpFastOpen { enabled: false },
        )
        .unwrap();
        let output: Value = serde_json::from_str(&snapshot(node, &mutation).unwrap()).unwrap();
        assert_eq!(output["tls"]["insecure"], false);
        assert_eq!(output["tcp_fast_open"], false);
    }

    #[test]
    fn unsupported_and_invalid_do_not_mutate() {
        let sub = Subscription::parse(
            r#"[{"type":"http","tag":"n","server":"origin.example","server_port":80}]"#,
        )
        .unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        assert!(matches!(
            set_setting(node, &mut mutation, &Setting::Udp { enabled: true }),
            Err(ModelError::Unsupported)
        ));
        assert!(matches!(
            set_setting(
                node,
                &mut mutation,
                &Setting::ServerName {
                    value: String::new()
                }
            ),
            Err(ModelError::InvalidEdit)
        ));
        assert!(mutation.is_empty());
    }

    #[test]
    fn surge_address_keeps_explicit_sni_and_other_fields() {
        let sub = Subscription::parse(
            "n = trojan, origin.example, 443, password=x, sni=identity.example\n",
        )
        .unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        set_address(node, &mut mutation, "192.0.2.1".parse().unwrap()).unwrap();
        let result = snapshot(node, &mutation).unwrap();
        let parsed = Subscription::parse(&result).unwrap();
        let projected = parsed.nodes()[0].to_sing_box(false).unwrap();
        assert_eq!(projected["server"], "192.0.2.1");
        assert_eq!(projected["tls"]["server_name"], "identity.example");
        assert_eq!(projected["password"], "x");
    }

    #[test]
    fn uri_ipv6_keeps_unknown_query_and_adds_identity() {
        let sub =
            Subscription::parse("trojan://secret@origin.example:443?unknown=a%2Fb#name\n").unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        set_address(node, &mut mutation, "2001:db8::1".parse().unwrap()).unwrap();
        let rendered = snapshot(node, &mutation).unwrap();
        assert!(rendered.contains("@[2001:db8::1]:443?unknown=a%2Fb&"));
        let reparsed = Subscription::parse(&rendered).unwrap();
        assert_eq!(reparsed.nodes()[0].server(), "2001:db8::1");
        assert!(rendered.contains("sni=origin%2Eexample"));
    }

    #[test]
    fn uri_explicit_peer_is_preserved() {
        let sub = Subscription::parse("vless://id@origin.example:443?security=tls&type=ws&peer=identity.example&host=front.example#name\n").unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        set_address(node, &mut mutation, "192.0.2.1".parse().unwrap()).unwrap();
        let rendered = snapshot(node, &mutation).unwrap();
        assert!(rendered.contains("peer=identity.example&host=front.example"));
        assert!(!rendered.contains("sni="));
    }

    #[test]
    fn vmess_uri_keeps_unknown_fields_and_tls_identity() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let input = json!({"v":"2","ps":"n","add":"origin.example","port":"443",
            "id":"id","tls":"tls","net":"ws","sni":"identity.example","host":"front.example","extra":"kept"});
        let encoded = STANDARD.encode(serde_json::to_vec(&input).unwrap());
        let sub = Subscription::parse(&format!("vmess://{encoded}")).unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        set_address(node, &mut mutation, "2001:db8::2".parse().unwrap()).unwrap();
        let rendered = snapshot(node, &mutation).unwrap();
        let decoded = STANDARD
            .decode(rendered.strip_prefix("vmess://").unwrap())
            .unwrap();
        let value: Value = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(value["add"], "2001:db8::2");
        assert_eq!(value["sni"], "identity.example");
        assert_eq!(value["host"], "front.example");
        assert_eq!(value["extra"], "kept");
    }

    #[test]
    fn ws_address_requires_explicit_host_in_each_dialect() {
        for (input, allowed) in [
            (
                r#"[{"type":"vmess","tag":"n","server":"origin.example","server_port":443,"uuid":"x","transport":{"type":"ws","headers":{"Host":"front.example"}}}]"#,
                true,
            ),
            (
                r#"[{"type":"vmess","tag":"n","server":"origin.example","server_port":443,"uuid":"x","transport":{"type":"ws"}}]"#,
                false,
            ),
            (
                r#"{"proxies":[{"type":"vmess","name":"n","server":"origin.example","port":443,"uuid":"x","network":"ws","ws-opts":{"headers":{"Host":"front.example"}}}]}"#,
                true,
            ),
            (
                r#"{"proxies":[{"type":"vmess","name":"n","server":"origin.example","port":443,"uuid":"x","network":"ws"}]}"#,
                false,
            ),
            (
                "vless://id@origin.example:443?security=tls&type=ws&host=front.example#n",
                true,
            ),
            (
                "vless://id@origin.example:443?security=tls&type=ws#n",
                false,
            ),
        ] {
            let sub = Subscription::parse(input).unwrap();
            let mut mutation = Mutation::default();
            let result = set_address(&sub.nodes()[0], &mut mutation, "192.0.2.1".parse().unwrap());
            assert_eq!(result.is_ok(), allowed);
            if allowed {
                let rendered = snapshot(&sub.nodes()[0], &mutation).unwrap();
                assert!(rendered.contains("front.example"));
                if rendered.starts_with('{') {
                    assert!(serde_json::from_str::<Value>(&rendered).is_ok());
                } else {
                    assert!(Subscription::parse(&rendered).is_ok());
                }
            } else {
                assert!(mutation.is_empty());
            }
        }
    }

    #[test]
    fn vmess_uri_websocket_without_host_is_not_rewritten() {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        for (host, allowed) in [("", false), ("front.example", true)] {
            let input = json!({"v":"2","ps":"n","add":"origin.example","port":"443","id":"id",
                "tls":"tls","net":"ws","host":host});
            let encoded = STANDARD.encode(serde_json::to_vec(&input).unwrap());
            let sub = Subscription::parse(&format!("vmess://{encoded}")).unwrap();
            let mut mutation = Mutation::default();
            assert_eq!(
                set_address(&sub.nodes()[0], &mut mutation, "192.0.2.1".parse().unwrap()).is_ok(),
                allowed
            );
        }
    }

    #[test]
    fn surge_insertions_precede_comment_and_keep_crlf() {
        let sub =
            Subscription::parse("n = trojan, origin.example, 443, password=x  # note\r\n").unwrap();
        let node = &sub.nodes()[0];
        let mut mutation = Mutation::default();
        set_address(node, &mut mutation, "192.0.2.1".parse().unwrap()).unwrap();
        set_setting(node, &mut mutation, &Setting::TcpFastOpen { enabled: true }).unwrap();
        let output = snapshot(node, &mutation).unwrap();
        assert!(output.ends_with("  # note\r\n"));
        let parsed = Subscription::parse(&output).unwrap();
        let outbound = parsed.nodes()[0].to_sing_box(false).unwrap();
        assert_eq!(outbound["server"], "192.0.2.1");
        assert_eq!(outbound["tls"]["server_name"], "origin.example");
        assert_eq!(outbound["tcp_fast_open"], true);
    }
}
