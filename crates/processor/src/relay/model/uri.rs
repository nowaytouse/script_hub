use super::*;
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};

pub(super) struct UriNode {
    pub(super) name: Zeroizing<String>,
    pub(super) server: Zeroizing<String>,
    pub(super) protocol: Zeroizing<String>,
    pub(super) port: u16,
    raw: Zeroizing<String>,
    data: PrivateJson,
    vmess: bool,
}

fn decode64(input: &str) -> Result<Zeroizing<String>, ModelError> {
    let bytes = [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD]
        .iter()
        .find_map(|engine| engine.decode(input).ok())
        .ok_or(ModelError::InvalidInput)?;
    let bytes = Zeroizing::new(bytes);
    std::str::from_utf8(&bytes)
        .map(|text| Zeroizing::new(text.to_owned()))
        .map_err(|_| ModelError::InvalidInput)
}

fn decode(input: &str) -> Result<String, ModelError> {
    let bytes = input.as_bytes();
    for (i, byte) in bytes.iter().enumerate() {
        if *byte == b'%'
            && (i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit())
        {
            return Err(ModelError::InvalidInput);
        }
    }
    percent_encoding::percent_decode_str(input)
        .decode_utf8()
        .map(|v| v.into_owned())
        .map_err(|_| ModelError::InvalidInput)
}

fn encode(input: &str) -> String {
    percent_encoding::utf8_percent_encode(input, percent_encoding::NON_ALPHANUMERIC).to_string()
}

pub(super) fn parse(line: &str, index: usize) -> Result<UriNode, ModelError> {
    let (scheme, rest) = line.split_once("://").ok_or(ModelError::InvalidInput)?;
    if !scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return Err(ModelError::InvalidInput);
    }
    let scheme = scheme.to_ascii_lowercase();
    let vmess = scheme == "vmess";
    let mut private = PrivateJson(json!({}));
    let data = private.0.as_object_mut().ok_or(ModelError::InvalidInput)?;
    let (server, port, name) = if vmess {
        let (payload, fragment) = rest
            .split_once('#')
            .map_or((rest, None), |(a, b)| (a, Some(b)));
        let decoded = decode64(payload)?;
        let UniqueValue(value) =
            serde_json::from_str(&decoded).map_err(|_| ModelError::InvalidInput)?;
        *data = match value {
            Value::Object(object) => object,
            _ => return Err(ModelError::InvalidInput),
        };
        let server = data
            .get("add")
            .and_then(Value::as_str)
            .filter(|v| valid_label(v))
            .ok_or(ModelError::InvalidInput)?
            .to_owned();
        let port = data
            .get("port")
            .and_then(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<u16>().ok())
                    .or_else(|| v.as_u64().and_then(|n| u16::try_from(n).ok()))
            })
            .filter(|v| *v != 0)
            .ok_or(ModelError::InvalidInput)?;
        if data.get("ps").is_some_and(|v| !v.is_string()) {
            return Err(ModelError::InvalidInput);
        }
        let name = match fragment {
            Some(fragment) => decode(fragment)?,
            None => data
                .get("ps")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("node-{}", index + 1)),
        };
        if !valid_label(&name) {
            return Err(ModelError::InvalidInput);
        }
        (server, port, name)
    } else {
        let (body, fragment) = rest
            .split_once('#')
            .map_or((rest, None), |(a, b)| (a, Some(b)));
        let name = match fragment {
            Some(v) => {
                let v = decode(v)?;
                if !valid_label(&v) {
                    return Err(ModelError::InvalidInput);
                }
                v
            }
            None => format!("node-{}", index + 1),
        };
        let legacy;
        let is_legacy = scheme == "ss" && !body.contains('@');
        let body = if is_legacy {
            legacy = decode64(body)?;
            legacy.as_str()
        } else {
            body
        };
        let (authority, query) = if is_legacy {
            (body, None)
        } else {
            body.split_once('?')
                .map_or((body, None), |(a, b)| (a, Some(b)))
        };
        let authority = authority.strip_suffix('/').unwrap_or(authority);
        let (credentials, hostport) = match authority.rsplit_once('@') {
            Some(value) => value,
            None if matches!(scheme.as_str(), "socks" | "socks5" | "http" | "https") => {
                ("", authority)
            }
            None => return Err(ModelError::InvalidInput),
        };
        let (host, port) = if let Some(ip) = hostport.strip_prefix('[') {
            let (host, port) = ip.split_once("]:").ok_or(ModelError::InvalidInput)?;
            host.parse::<std::net::Ipv6Addr>()
                .map_err(|_| ModelError::InvalidInput)?;
            (host, port)
        } else {
            hostport.rsplit_once(':').ok_or(ModelError::InvalidInput)?
        };
        if !valid_label(host)
            || host
                .chars()
                .any(|c| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '@' | '%' | '[' | ']'))
            || host.contains(':') && !hostport.starts_with('[')
        {
            return Err(ModelError::InvalidInput);
        }
        let port = port
            .parse::<u16>()
            .ok()
            .filter(|v| *v != 0)
            .ok_or(ModelError::InvalidInput)?;
        let creds = if is_legacy {
            Zeroizing::new(credentials.to_owned())
        } else if !credentials.is_empty()
            && (scheme == "ss" || scheme == "socks")
            && !credentials.contains(':')
        {
            let encoded = Zeroizing::new(decode(credentials)?);
            decode64(&encoded)?
        } else {
            Zeroizing::new(decode(credentials)?)
        };
        if !valid_label(&creds)
            && !(credentials.is_empty()
                && matches!(scheme.as_str(), "socks" | "socks5" | "http" | "https"))
        {
            return Err(ModelError::InvalidInput);
        }
        if !credentials.is_empty()
            && matches!(
                scheme.as_str(),
                "ss" | "tuic" | "socks" | "socks5" | "http" | "https"
            )
        {
            let raw_pair = !is_legacy && credentials.contains(':');
            let (left, right) = if raw_pair {
                credentials.split_once(':')
            } else {
                creds.split_once(':')
            }
            .ok_or(ModelError::InvalidInput)?;
            let left = Zeroizing::new(if raw_pair {
                decode(left)?
            } else {
                left.to_owned()
            });
            let right = Zeroizing::new(if raw_pair {
                decode(right)?
            } else {
                right.to_owned()
            });
            if !valid_label(&left) || !valid_label(&right) {
                return Err(ModelError::InvalidInput);
            }
            data.insert("first".into(), left.to_string().into());
            data.insert("second".into(), right.to_string().into());
        }
        data.insert("credentials".into(), creds.to_string().into());
        data.insert("query".into(), json!({}));
        let params = data
            .get_mut("query")
            .and_then(Value::as_object_mut)
            .ok_or(ModelError::InvalidInput)?;
        if let Some(query) = query {
            for pair in query.split('&') {
                let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                let key = decode(key)?;
                if key.is_empty() || params.insert(key, Value::String(decode(value)?)).is_some() {
                    return Err(ModelError::InvalidInput);
                }
            }
        }
        (host.to_owned(), port, name)
    };
    Ok(UriNode {
        name: Zeroizing::new(name),
        server: Zeroizing::new(server),
        protocol: Zeroizing::new(scheme),
        port,
        raw: Zeroizing::new(line.to_owned()),
        data: private,
        vmess,
    })
}

impl UriNode {
    pub(super) fn same_effective(left: &str, right: &str) -> Result<bool, ModelError> {
        let a = parse(left, 0)?;
        let b = parse(right, 0)?;
        Ok(a.server == b.server
            && a.port == b.port
            && a.name == b.name
            && a.protocol == b.protocol
            && a.data.0 == b.data.0)
    }

    pub(super) fn render_mutation(
        &self,
        name: &str,
        mutation: &super::properties::Mutation,
    ) -> Result<Zeroizing<String>, ModelError> {
        use super::properties::{Setting, hostname};
        if mutation.is_empty() {
            return if name == self.name.as_str() {
                Ok(self.raw.clone())
            } else {
                self.render_name(name)
            };
        }
        if !valid_label(name) {
            return Err(ModelError::InvalidEdit);
        }
        if self.vmess {
            let mut data = PrivateJson(self.data.0.clone());
            let obj = data.0.as_object_mut().ok_or(ModelError::Unsupported)?;
            let tls = match obj.get("tls") {
                None => false,
                Some(Value::String(s)) if s.is_empty() => false,
                Some(Value::String(s)) if s == "tls" => true,
                _ => return Err(ModelError::Unsupported),
            };
            if mutation.address.is_some() && tls && !hostname(&self.server) {
                return Err(ModelError::Unsupported);
            }
            if obj.get("sni").is_some_and(|v| !v.is_string()) {
                return Err(ModelError::Unsupported);
            }
            if let Some(address) = &mutation.address {
                match obj.get("net") {
                    None | Some(Value::String(_))
                        if obj.get("net").is_none()
                            || obj
                                .get("net")
                                .and_then(Value::as_str)
                                .is_some_and(|net| net.is_empty() || net == "tcp") =>
                    {
                        if obj
                            .get("host")
                            .is_some_and(|host| !host.as_str().is_some_and(str::is_empty))
                        {
                            return Err(ModelError::Unsupported);
                        }
                    }
                    Some(Value::String(net))
                        if net == "ws"
                            && obj
                                .get("host")
                                .and_then(Value::as_str)
                                .is_some_and(valid_label) => {}
                    _ => return Err(ModelError::Unsupported),
                }
                if tls
                    && obj
                        .get("sni")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                    && !mutation
                        .settings
                        .iter()
                        .any(|s| matches!(s, Setting::ServerName { .. }))
                {
                    obj.insert("sni".into(), self.server.as_str().into());
                }
                obj.insert("add".into(), address.as_str().into());
            }
            if let Some(port) = mutation.port {
                let value = match obj.get("port") {
                    Some(Value::String(_)) => Value::String(port.to_string()),
                    Some(Value::Number(_)) => port.into(),
                    _ => return Err(ModelError::Unsupported),
                };
                obj.insert("port".into(), value);
            }
            for setting in &mutation.settings {
                match setting {
                    Setting::ServerName { value } if tls => {
                        obj.insert("sni".into(), value.as_str().into());
                    }
                    Setting::Alpn { values } if tls => {
                        if obj.get("alpn").is_some_and(|v| !v.is_string()) {
                            return Err(ModelError::Unsupported);
                        }
                        obj.insert("alpn".into(), values.join(",").into());
                    }
                    _ => return Err(ModelError::Unsupported),
                }
            }
            if name != self.name.as_str() {
                obj.insert("ps".into(), name.into());
            }
            let serialized =
                Zeroizing::new(serde_json::to_string(&data.0).map_err(|_| ModelError::Render)?);
            let mut result = Zeroizing::new(format!(
                "vmess://{}",
                STANDARD.encode(serialized.as_bytes())
            ));
            if self.raw.contains('#') {
                result.push('#');
                if name == self.name.as_str() {
                    result.push_str(
                        self.raw
                            .split_once('#')
                            .map(|(_, f)| f)
                            .ok_or(ModelError::Render)?,
                    );
                } else {
                    result.push_str(&encode(name));
                }
            }
            return Ok(result);
        }
        if !matches!(
            self.protocol.as_str(),
            "ss" | "trojan"
                | "anytls"
                | "vless"
                | "hy2"
                | "hysteria2"
                | "tuic"
                | "http"
                | "https"
                | "socks"
                | "socks5"
        ) {
            return Err(ModelError::Unsupported);
        }
        if self.protocol.as_str() == "ss"
            && !self
                .raw
                .split_once('#')
                .map_or(self.raw.as_str(), |(body, _)| body)
                .contains('@')
        {
            return Err(ModelError::Unsupported);
        }
        let params = self
            .data
            .0
            .get("query")
            .and_then(Value::as_object)
            .ok_or(ModelError::Unsupported)?;
        let get = |key: &str| params.get(key).and_then(Value::as_str);
        if mutation.address.is_some() {
            if matches!(self.protocol.as_str(), "http" | "https") {
                return Err(ModelError::Unsupported);
            }
            match get("type") {
                None | Some("tcp") if get("host").is_none() => {}
                Some("ws") if get("host").is_some_and(valid_label) => {}
                _ => return Err(ModelError::Unsupported),
            }
        }
        let required = matches!(
            self.protocol.as_str(),
            "trojan" | "anytls" | "hy2" | "hysteria2" | "tuic" | "https"
        );
        let security = get("security").unwrap_or(if required { "tls" } else { "none" });
        if !matches!(security, "none" | "tls" | "reality")
            || required && security == "none"
            || security == "reality" && self.protocol.as_str() != "vless"
            || security != "none" && matches!(self.protocol.as_str(), "socks" | "socks5")
        {
            return Err(ModelError::Unsupported);
        }
        let tls = security != "none";
        if mutation.address.is_some() && tls && !hostname(&self.server) {
            return Err(ModelError::Unsupported);
        }
        if get("sni").is_some() && get("peer").is_some()
            || get("insecure").is_some() && get("allowInsecure").is_some()
            || get("sni")
                .or_else(|| get("peer"))
                .is_some_and(|s| !valid_label(s))
            || !tls
                && ["sni", "peer", "alpn", "insecure", "allowInsecure"]
                    .iter()
                    .any(|k| get(k).is_some())
        {
            return Err(ModelError::Unsupported);
        }
        let mut changes: Vec<(&str, String)> = Vec::new();
        if mutation.address.is_some()
            && tls
            && get("sni").is_none()
            && get("peer").is_none()
            && !mutation
                .settings
                .iter()
                .any(|s| matches!(s, Setting::ServerName { .. }))
        {
            changes.push(("sni", self.server.as_str().to_owned()));
        }
        for setting in &mutation.settings {
            match setting {
                Setting::VerifyTls if tls => {
                    let key = if get("allowInsecure").is_some() {
                        "allowInsecure"
                    } else {
                        "insecure"
                    };
                    if get(key).is_some_and(|v| !matches!(v, "0" | "1" | "true" | "false")) {
                        return Err(ModelError::Unsupported);
                    }
                    changes.push((key, "0".into()));
                }
                Setting::ServerName { value } if tls => changes.push((
                    if get("peer").is_some() { "peer" } else { "sni" },
                    value.clone(),
                )),
                Setting::Alpn { values } if tls => changes.push(("alpn", values.join(","))),
                _ => return Err(ModelError::Unsupported),
            }
        }
        let (without_fragment, fragment) = self
            .raw
            .split_once('#')
            .map_or((self.raw.as_str(), None), |(a, b)| (a, Some(b)));
        let (prefix, query) = without_fragment
            .split_once('?')
            .map_or((without_fragment, None), |(a, b)| (a, Some(b)));
        let (_, body) = prefix.split_once("://").ok_or(ModelError::Render)?;
        let authority = body.strip_suffix('/').unwrap_or(body);
        let hostport = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let host_start = prefix.len() - body.len() + authority.len() - hostport.len();
        let host_end = if hostport.starts_with('[') {
            host_start + hostport.find(']').ok_or(ModelError::Render)? + 1
        } else {
            host_start + hostport.rfind(':').ok_or(ModelError::Render)?
        };
        let mut result = Zeroizing::new(String::new());
        result.push_str(&prefix[..host_start]);
        if let Some(address) = &mutation.address {
            if address.parse::<std::net::Ipv6Addr>().is_ok() {
                result.push('[');
                result.push_str(address);
                result.push(']');
            } else {
                result.push_str(address);
            }
        } else {
            result.push_str(&prefix[host_start..host_end]);
        }
        if let Some(port) = mutation.port {
            result.push(':');
            result.push_str(&port.to_string());
            result.push_str(&prefix[host_start + hostport.len()..]);
        } else {
            result.push_str(&prefix[host_end..]);
        }
        let mut first = true;
        if let Some(query) = query {
            for pair in query.split('&') {
                let key = decode(pair.split_once('=').map_or(pair, |(k, _)| k))?;
                result.push(if first { '?' } else { '&' });
                first = false;
                if let Some((_, value)) = changes.iter().find(|(k, _)| *k == key) {
                    result.push_str(&encode(&key));
                    result.push('=');
                    result.push_str(&encode(value));
                } else {
                    result.push_str(pair);
                }
            }
        }
        for (key, value) in &changes {
            if !params.contains_key(*key) {
                result.push(if first { '?' } else { '&' });
                first = false;
                result.push_str(&encode(key));
                result.push('=');
                result.push_str(&encode(value));
            }
        }
        if let Some(fragment) = fragment {
            result.push('#');
            if name == self.name.as_str() {
                result.push_str(fragment);
            } else {
                result.push_str(&encode(name));
            }
        } else if name != self.name.as_str() {
            result.push('#');
            result.push_str(&encode(name));
        }
        Ok(result)
    }

    pub(super) fn original(&self) -> &str {
        &self.raw
    }

    pub(super) fn render_name(&self, name: &str) -> Result<Zeroizing<String>, ModelError> {
        if !valid_label(name) {
            return Err(ModelError::Render);
        }
        if self.vmess {
            let mut value = PrivateJson(self.data.0.clone());
            if let Some(Value::String(old)) = value.0.get_mut("ps") {
                old.zeroize();
            }
            value.0["ps"] = name.into();
            let serialized =
                Zeroizing::new(serde_json::to_string(&value.0).map_err(|_| ModelError::Render)?);
            let mut rendered = Zeroizing::new(format!(
                "vmess://{}",
                STANDARD.encode(serialized.as_bytes())
            ));
            if self.raw.contains('#') {
                rendered.push('#');
                rendered.push_str(&encode(name));
            }
            Ok(rendered)
        } else {
            Ok(Zeroizing::new(format!(
                "{}#{}",
                self.raw.split('#').next().ok_or(ModelError::Render)?,
                encode(name)
            )))
        }
    }

    pub(super) fn project(&self, compatibility: bool) -> Result<Value, ModelError> {
        let data = self.data.0.as_object().ok_or(ModelError::Unsupported)?;
        if self.vmess {
            return self.project_vmess(compatibility);
        }
        let params = data
            .get("query")
            .and_then(Value::as_object)
            .ok_or(ModelError::Unsupported)?;
        let get = |key: &str| params.get(key).and_then(Value::as_str);
        let protocol = match self.protocol.as_str() {
            "ss" => "shadowsocks",
            "hy2" | "hysteria2" => "hysteria2",
            "socks" | "socks5" => "socks",
            "http" | "https" => "http",
            "vless" | "trojan" | "anytls" | "tuic" => self.protocol.as_str(),
            _ => return Err(ModelError::Unsupported),
        };
        let mut out = json!({"type":protocol,"tag":self.name.as_str(),"server":self.server.as_str(),"server_port":self.port});
        let creds = data["credentials"]
            .as_str()
            .ok_or(ModelError::Unsupported)?;
        let pair = || -> Result<(&str, &str), ModelError> {
            Ok((
                data.get("first")
                    .and_then(Value::as_str)
                    .ok_or(ModelError::Unsupported)?,
                data.get("second")
                    .and_then(Value::as_str)
                    .ok_or(ModelError::Unsupported)?,
            ))
        };
        match protocol {
            "shadowsocks" => {
                let (method, password) = pair()?;
                out["method"] = method.into();
                out["password"] = password.into();
            }
            "vless" => out["uuid"] = creds.into(),
            "tuic" => {
                let (uuid, password) = pair()?;
                out["uuid"] = uuid.into();
                out["password"] = password.into();
            }
            "socks" | "http" => {
                if !creds.is_empty() {
                    let (username, password) = pair()?;
                    out["username"] = username.into();
                    out["password"] = password.into();
                }
                if protocol == "socks" {
                    out["version"] = "5".into();
                }
            }
            _ => out["password"] = creds.into(),
        }
        let allowed = [
            "security",
            "type",
            "encryption",
            "flow",
            "sni",
            "peer",
            "alpn",
            "allowInsecure",
            "insecure",
            "fp",
            "pbk",
            "sid",
            "host",
            "path",
            "serviceName",
        ];
        if params.keys().any(|k| !allowed.contains(&k.as_str())) {
            return Err(ModelError::Unsupported);
        }
        if get("encryption").is_some() && protocol != "vless"
            || get("type").is_some() && !matches!(protocol, "vless" | "trojan")
        {
            return Err(ModelError::Unsupported);
        }
        if get("encryption").is_some_and(|v| v != "none") {
            return Err(ModelError::Unsupported);
        }
        if let Some(flow) = get("flow") {
            if protocol != "vless" || !valid_label(flow) {
                return Err(ModelError::Unsupported);
            }
            out["flow"] = flow.into();
        }
        let security = get("security").unwrap_or(
            if matches!(protocol, "trojan" | "anytls" | "hysteria2" | "tuic")
                || self.protocol.as_str() == "https"
            {
                "tls"
            } else {
                "none"
            },
        );
        if !matches!(security, "none" | "tls" | "reality")
            || security == "none"
                && (matches!(protocol, "trojan" | "anytls" | "hysteria2" | "tuic")
                    || self.protocol.as_str() == "https")
            || security == "reality" && protocol != "vless"
        {
            return Err(ModelError::Unsupported);
        }
        let mut tls = json!({"enabled":true});
        if get("sni").is_some() && get("peer").is_some() {
            return Err(ModelError::Unsupported);
        }
        if let Some(sni) = get("sni").or_else(|| get("peer")) {
            if !valid_label(sni) {
                return Err(ModelError::Unsupported);
            }
            tls["server_name"] = sni.into();
        }
        if let Some(alpn) = get("alpn") {
            let items: Vec<_> = alpn.split(',').collect();
            if items.iter().any(|s| !valid_label(s)) {
                return Err(ModelError::Unsupported);
            }
            tls["alpn"] = json!(items);
        }
        if get("insecure").is_some() && get("allowInsecure").is_some() {
            return Err(ModelError::Unsupported);
        }
        if let Some(insecure) = get("insecure").or_else(|| get("allowInsecure")) {
            let value = match insecure {
                "1" | "true" => true,
                "0" | "false" => false,
                _ => return Err(ModelError::Unsupported),
            };
            if value && !compatibility {
                return Err(ModelError::SecurityBlocked);
            }
            tls["insecure"] = value.into();
        }
        if let Some(fp) = get("fp") {
            if !valid_label(fp) {
                return Err(ModelError::Unsupported);
            }
            tls["utls"] = json!({"enabled":true,"fingerprint":fp});
        }
        if security == "reality" {
            let public_key = get("pbk")
                .filter(|s| valid_label(s))
                .ok_or(ModelError::Unsupported)?;
            tls["reality"] = json!({"enabled":true,"public_key":public_key});
            if let Some(sid) = get("sid") {
                tls["reality"]["short_id"] = sid.into();
            }
        } else if get("pbk").is_some() || get("sid").is_some() {
            return Err(ModelError::Unsupported);
        }
        if security != "none" {
            if matches!(protocol, "shadowsocks" | "socks") {
                return Err(ModelError::Unsupported);
            }
            out["tls"] = tls;
        } else if ["sni", "peer", "alpn", "insecure", "allowInsecure", "fp"]
            .iter()
            .any(|k| get(k).is_some())
        {
            return Err(ModelError::Unsupported);
        }
        match get("type").unwrap_or("tcp") {
            "tcp" => {
                if ["host", "path", "serviceName"]
                    .iter()
                    .any(|k| get(k).is_some())
                {
                    return Err(ModelError::Unsupported);
                }
            }
            "ws" if matches!(protocol, "vless" | "trojan") => {
                if get("serviceName").is_some() {
                    return Err(ModelError::Unsupported);
                }
                let mut transport = json!({"type":"ws"});
                if let Some(path) = get("path") {
                    transport["path"] = path.into();
                }
                if let Some(host) = get("host") {
                    transport["headers"] = json!({"Host":host});
                }
                out["transport"] = transport;
            }
            "grpc" if matches!(protocol, "vless" | "trojan") => {
                if get("host").is_some() || get("path").is_some() {
                    return Err(ModelError::Unsupported);
                }
                let mut transport = json!({"type":"grpc"});
                if let Some(service) = get("serviceName") {
                    transport["service_name"] = service.into();
                }
                out["transport"] = transport;
            }
            _ => return Err(ModelError::Unsupported),
        }
        validate_native(&out, compatibility)?;
        Ok(out)
    }

    fn project_vmess(&self, compatibility: bool) -> Result<Value, ModelError> {
        let d = self.data.0.as_object().ok_or(ModelError::Unsupported)?;
        let allowed = [
            "v", "ps", "add", "port", "id", "aid", "scy", "net", "type", "host", "path", "tls",
            "sni", "alpn", "fp",
        ];
        if d.keys().any(|k| !allowed.contains(&k.as_str())) {
            return Err(ModelError::Unsupported);
        }
        if d.iter()
            .any(|(k, v)| !v.is_string() && !(matches!(k.as_str(), "port" | "aid") && v.is_u64()))
        {
            return Err(ModelError::Unsupported);
        }
        let s = |key: &str| d.get(key).and_then(Value::as_str);
        if s("v").is_some_and(|v| v != "2") {
            return Err(ModelError::Unsupported);
        }
        let id = s("id")
            .filter(|v| valid_label(v))
            .ok_or(ModelError::Unsupported)?;
        let aid = d
            .get("aid")
            .map(|v| {
                v.as_str()
                    .and_then(|s| s.parse::<u16>().ok())
                    .or_else(|| v.as_u64().and_then(|n| u16::try_from(n).ok()))
                    .ok_or(ModelError::Unsupported)
            })
            .transpose()?
            .unwrap_or(0);
        let mut out = json!({"type":"vmess","tag":self.name.as_str(),"server":self.server.as_str(),"server_port":self.port,"uuid":id,"security":s("scy").filter(|s| !s.is_empty()).unwrap_or("auto"),"alter_id":aid});
        if s("type").is_some_and(|v| !v.is_empty() && v != "none") {
            return Err(ModelError::Unsupported);
        }
        match s("net").unwrap_or("tcp") {
            "tcp" | "" => {
                if ["host", "path"]
                    .iter()
                    .any(|k| s(k).is_some_and(|v| !v.is_empty()))
                {
                    return Err(ModelError::Unsupported);
                }
            }
            "ws" => {
                let mut t = json!({"type":"ws"});
                if let Some(path) = s("path").filter(|v| !v.is_empty()) {
                    t["path"] = path.into();
                }
                if let Some(host) = s("host").filter(|v| !v.is_empty()) {
                    t["headers"] = json!({"Host":host});
                }
                out["transport"] = t;
            }
            _ => return Err(ModelError::Unsupported),
        }
        match s("tls").unwrap_or("") {
            "tls" => {
                let mut tls = json!({"enabled":true});
                if let Some(sni) = s("sni").filter(|v| !v.is_empty()) {
                    tls["server_name"] = sni.into();
                }
                if let Some(alpn) = s("alpn").filter(|v| !v.is_empty()) {
                    tls["alpn"] = json!(alpn.split(',').collect::<Vec<_>>());
                }
                if let Some(fp) = s("fp").filter(|v| !v.is_empty()) {
                    tls["utls"] = json!({"enabled":true,"fingerprint":fp});
                }
                out["tls"] = tls;
            }
            "" => {
                if ["sni", "alpn", "fp"]
                    .iter()
                    .any(|k| s(k).is_some_and(|v| !v.is_empty()))
                {
                    return Err(ModelError::Unsupported);
                }
            }
            _ => return Err(ModelError::Unsupported),
        }
        validate_native(&out, compatibility)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ss_and_legacy_preserve_credentials_and_rename() {
        for uri in [
            "ss://YWVzLTEyOC1nY206cGFzcw@example.invalid:443#old".to_owned(),
            format!(
                "ss://{}#old",
                STANDARD.encode("aes-128-gcm:pass@example.invalid:443")
            ),
        ] {
            let node = parse(&uri, 0).unwrap();
            assert_eq!(node.project(false).unwrap()["password"], "pass");
            assert!(
                node.render_name("新 名")
                    .unwrap()
                    .ends_with("#%E6%96%B0%20%E5%90%8D")
            );
        }
    }

    #[test]
    fn unknown_and_security_options_fail_closed() {
        let unknown = parse("trojan://pw@example.invalid:443?newOption=secret#n", 0).unwrap();
        assert_eq!(unknown.project(false), Err(ModelError::Unsupported));
        assert!(
            unknown
                .render_name("x")
                .unwrap()
                .contains("newOption=secret")
        );
        let insecure = parse("trojan://pw@[::1]:443?allowInsecure=1#n", 0).unwrap();
        assert_eq!(insecure.project(false), Err(ModelError::SecurityBlocked));
        assert_eq!(insecure.project(true).unwrap()["tls"]["insecure"], true);
    }

    #[test]
    fn malformed_and_duplicate_query_rejected() {
        assert!(parse("trojan://p%XX@example.invalid:443#n", 0).is_err());
        assert!(parse("trojan://pw@example.invalid:0#n", 0).is_err());
        assert!(parse("trojan://pw@example.invalid:443?sni=a&sni=b#n", 0).is_err());
        assert!(parse("trojan://pw@bad host.invalid:443#n", 0).is_err());
        assert!(parse("trojan://pw@bad/path.invalid:443#n", 0).is_err());
    }

    #[test]
    fn unauthenticated_proxies_and_vmess_fragments() {
        for input in [
            "http://example.invalid:8080#n",
            "socks5://example.invalid:1080#n",
        ] {
            let node = parse(input, 0).unwrap();
            assert!(node.project(false).unwrap().get("password").is_none());
        }
        let input = format!(
            "vmess://{}#outer%20name",
            STANDARD.encode(r#"{"ps":"inner","add":"example.invalid","port":443,"id":"id"}"#)
        );
        let node = parse(&input, 0).unwrap();
        assert_eq!(node.name.as_str(), "outer name");
        assert_eq!(
            parse(&node.render_name("renamed").unwrap(), 0)
                .unwrap()
                .name
                .as_str(),
            "renamed"
        );
    }

    #[test]
    fn vmess_json_and_vless_transport() {
        let payload = STANDARD.encode(r#"{"v":"2","ps":"old","add":"example.invalid","port":"443","id":"id","aid":"0","net":"ws","path":"/x","tls":"tls"}"#);
        let node = parse(&format!("vmess://{payload}"), 0).unwrap();
        assert_eq!(node.project(false).unwrap()["transport"]["path"], "/x");
        assert_eq!(
            parse(&node.render_name("new").unwrap(), 0)
                .unwrap()
                .name
                .as_str(),
            "new"
        );
        let node = parse(
            "vless://id@example.invalid:443?type=ws&security=tls&path=%2Fx#n",
            0,
        )
        .unwrap();
        assert_eq!(node.project(false).unwrap()["transport"]["path"], "/x");
    }

    #[test]
    fn encoded_credentials_and_legacy_question_mark() {
        let socks = format!(
            "socks://{}@example.invalid:1080#n",
            encode(&STANDARD.encode("user:pass"))
        );
        assert_eq!(
            parse(&socks, 0).unwrap().project(false).unwrap()["username"],
            "user"
        );
        let ss = format!(
            "ss://{}@example.invalid:443#n",
            encode(&STANDARD.encode("aes-128-gcm:pass"))
        );
        assert_eq!(
            parse(&ss, 0).unwrap().project(false).unwrap()["password"],
            "pass"
        );
        let legacy = format!(
            "ss://{}#n",
            STANDARD.encode("aes-128-gcm:p?ss@example.invalid:443")
        );
        assert_eq!(
            parse(&legacy, 0).unwrap().project(false).unwrap()["password"],
            "p?ss"
        );
        let socks = parse("socks5://user%3Aname:pass@example.invalid:1080#n", 0).unwrap();
        assert_eq!(socks.project(false).unwrap()["username"], "user:name");
    }
}
