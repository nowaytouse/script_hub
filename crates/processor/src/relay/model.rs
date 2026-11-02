use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    ops::Range,
};

use serde::{
    Deserialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Value, json};
use zeroize::{Zeroize, Zeroizing};

mod mihomo;
mod uri;

const MAX_CONTENT: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Health {
    Healthy { latency_ms: u64 },
    Failed,
    Unknown,
    SecurityBlocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelError {
    InvalidInput,
    Unsupported,
    SecurityBlocked,
    InvalidHealth,
    InvalidDependency,
    Render,
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "expected an unambiguous node-only subscription",
            Self::Unsupported => "node cannot be faithfully and safely projected to the engine",
            Self::SecurityBlocked => {
                "original TLS verification bypass requires compatibility acknowledgement"
            }
            Self::InvalidHealth => "node and health counts differ",
            Self::InvalidDependency => "node dependencies are missing or cyclic",
            Self::Render => "subscription could not be rendered",
        })
    }
}

impl std::error::Error for ModelError {}

pub struct Node {
    name: Zeroizing<String>,
    server: Zeroizing<String>,
    protocol: Zeroizing<String>,
    port: u16,
    dependency: Option<Zeroizing<String>>,
    source: NodeSource,
}

enum NodeSource {
    Surge(SurgeNode),
    Native(PrivateJson),
    SubStore(PrivateJson),
    Mihomo(PrivateJson),
    Uri(uri::UriNode),
}

struct SurgeNode {
    raw: Zeroizing<String>,
    name_range: Range<usize>,
    protocol_range: Range<usize>,
    server_range: Range<usize>,
    fields: Vec<Field>,
}

struct Field {
    key: String,
    value: Zeroizing<String>,
    value_range: Range<usize>,
}

struct PrivateJson(Value);

impl Drop for PrivateJson {
    fn drop(&mut self) {
        fn clear(value: &mut Value) {
            match value {
                Value::String(value) => value.zeroize(),
                Value::Array(values) => values.iter_mut().for_each(clear),
                Value::Object(values) => values.values_mut().for_each(clear),
                _ => {}
            }
        }
        clear(&mut self.0);
    }
}

impl Node {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn server(&self) -> &str {
        &self.server
    }
    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    pub fn to_sing_box(&self, compatibility: bool) -> Result<Value, ModelError> {
        if self.protocol() == "external" || self.dependency.is_some() {
            return Err(ModelError::Unsupported);
        }
        match &self.source {
            NodeSource::Surge(source) => self.project_surge(source, compatibility),
            NodeSource::Native(source) => {
                validate_native(&source.0, compatibility)?;
                Ok(source.0.clone())
            }
            NodeSource::SubStore(source) => self.project_substore(&source.0, compatibility),
            NodeSource::Mihomo(source) => mihomo::project(self, &source.0, compatibility, true),
            NodeSource::Uri(source) => source.project(compatibility),
        }
    }

    fn project_substore(&self, source: &Value, compatibility: bool) -> Result<Value, ModelError> {
        mihomo::project(self, source, compatibility, false)
    }

    fn project_surge(&self, source: &SurgeNode, compatibility: bool) -> Result<Value, ModelError> {
        if uncertain_identity(source).iter().any(Option::is_some)
            || source
                .fields
                .iter()
                .any(|field| uncertain_token(source, field.value_range.clone()).is_some())
        {
            return Err(ModelError::Unsupported);
        }
        let fields: HashMap<&str, &str> = source
            .fields
            .iter()
            .map(|field| (field.key.as_str(), field.value.as_str()))
            .collect();
        let boolean = |key: &str| -> Result<Option<bool>, ModelError> {
            fields.get(key).map(|value| parse_bool(value)).transpose()
        };
        let insecure = boolean("skip-cert-verify")?;
        if insecure == Some(true) && !compatibility {
            return Err(ModelError::SecurityBlocked);
        }
        let (protocol, required_tls, protocol_fields): (&str, bool, &[&str]) = match self.protocol()
        {
            "anytls" => ("anytls", true, &["password"]),
            "trojan" => ("trojan", true, &["password"]),
            "ss" => ("shadowsocks", false, &["password", "encrypt-method"]),
            "socks5" => ("socks", false, &["username", "password"]),
            "http" => ("http", false, &["username", "password"]),
            "https" => ("http", true, &["username", "password"]),
            "vmess" => ("vmess", false, &["username", "encrypt-method", "alterid"]),
            _ => return Err(ModelError::Unsupported),
        };
        let common = [
            "tls",
            "sni",
            "alpn",
            "skip-cert-verify",
            "tfo",
            "udp-relay",
            "block-quic",
            "test-url",
            "test-timeout",
        ];
        if fields
            .keys()
            .any(|key| !common.contains(key) && !protocol_fields.contains(key))
        {
            return Err(ModelError::Unsupported);
        }
        for key in ["udp-relay", "block-quic"] {
            boolean(key)?;
        }
        let explicit_tls = boolean("tls")?;
        if required_tls && explicit_tls == Some(false) {
            return Err(ModelError::Unsupported);
        }
        let tls_enabled = required_tls || explicit_tls == Some(true);
        if (protocol == "shadowsocks" || protocol == "socks") && tls_enabled {
            return Err(ModelError::Unsupported);
        }
        if !tls_enabled
            && ["sni", "alpn", "skip-cert-verify"]
                .iter()
                .any(|key| fields.contains_key(key))
        {
            return Err(ModelError::Unsupported);
        }
        let mut outbound = json!({"type":protocol,"tag":self.name(),"server":self.server(),"server_port":self.port});
        if let Some(tfo) = boolean("tfo")? {
            outbound["tcp_fast_open"] = tfo.into();
        }
        match protocol {
            "socks" => {
                outbound["version"] = "5".into();
            }
            "shadowsocks" => {
                outbound["method"] = required_field(&fields, "encrypt-method")?.into();
            }
            "vmess" => {
                outbound["uuid"] = required_field(&fields, "username")?.into();
                outbound["security"] = fields
                    .get("encrypt-method")
                    .copied()
                    .unwrap_or("auto")
                    .into();
                if let Some(alter_id) = fields.get("alterid") {
                    outbound["alter_id"] = alter_id
                        .parse::<u16>()
                        .map_err(|_| ModelError::Unsupported)?
                        .into();
                }
            }
            _ => {}
        }
        if matches!(protocol, "anytls" | "trojan" | "shadowsocks") {
            outbound["password"] = required_field(&fields, "password")?.into();
        } else if protocol != "vmess" {
            for key in ["username", "password"] {
                if let Some(value) = fields.get(key) {
                    outbound[key] = (*value).into();
                }
            }
        }
        if tls_enabled {
            let mut tls = json!({"enabled":true});
            if let Some(sni) = fields.get("sni") {
                if sni.is_empty() {
                    return Err(ModelError::Unsupported);
                }
                tls["server_name"] = (*sni).into();
            }
            if let Some(insecure) = insecure {
                tls["insecure"] = insecure.into();
            }
            if let Some(alpn) = fields.get("alpn") {
                let protocols: Vec<_> = alpn.split(',').map(str::trim).collect();
                if protocols
                    .iter()
                    .any(|value| value.is_empty() || value.chars().any(char::is_control))
                {
                    return Err(ModelError::Unsupported);
                }
                tls["alpn"] = json!(protocols);
            }
            outbound["tls"] = tls;
        }
        Ok(outbound)
    }
}

pub struct Subscription {
    original: Zeroizing<String>,
    nodes: Vec<Node>,
    dialect: Dialect,
    encoding: Option<SubscriptionEncoding>,
}

enum Dialect {
    Surge(Vec<Line>),
    NativeArray,
    NativeObject { endpoints: bool },
    SubStoreArray,
    Proxies { yaml: bool },
    YamlArray,
    Uri(Vec<Line>),
}

#[derive(Clone, Copy)]
enum SubscriptionEncoding {
    Standard,
    StandardNoPad,
    UrlSafe,
    UrlSafeNoPad,
}

impl SubscriptionEncoding {
    fn engine(self) -> base64::engine::GeneralPurpose {
        use base64::engine::general_purpose::*;
        match self {
            Self::Standard => STANDARD,
            Self::StandardNoPad => STANDARD_NO_PAD,
            Self::UrlSafe => URL_SAFE,
            Self::UrlSafeNoPad => URL_SAFE_NO_PAD,
        }
    }
}

enum Line {
    Literal(Zeroizing<String>),
    Node,
}

impl Subscription {
    pub fn parse(input: &str) -> Result<Self, ModelError> {
        use base64::Engine as _;
        if input.len() > MAX_CONTENT {
            return Err(ModelError::InvalidInput);
        }
        let trimmed = input.trim_start_matches('\u{feff}').trim_start();
        let (nodes, dialect, encoding) = if let Ok((nodes, dialect)) = parse_plain(input) {
            (nodes, dialect, None)
        } else {
            let compact = Zeroizing::new(trimmed.split_ascii_whitespace().collect::<String>());
            let mut decoded = None;
            for encoding in [
                SubscriptionEncoding::Standard,
                SubscriptionEncoding::StandardNoPad,
                SubscriptionEncoding::UrlSafe,
                SubscriptionEncoding::UrlSafeNoPad,
            ] {
                let Ok(bytes) = encoding.engine().decode(compact.as_bytes()) else {
                    continue;
                };
                let bytes = Zeroizing::new(bytes);
                let Ok(text) = std::str::from_utf8(&bytes) else {
                    continue;
                };
                if let Ok((nodes, dialect)) = parse_uri(text) {
                    decoded = Some((nodes, dialect, Some(encoding)));
                    break;
                }
            }
            decoded.ok_or(ModelError::InvalidInput)?
        };
        let mut names = HashSet::new();
        if nodes.is_empty() || nodes.iter().any(|node| !names.insert(node.name())) {
            return Err(ModelError::InvalidInput);
        }
        Ok(Self {
            original: Zeroizing::new(input.to_owned()),
            nodes,
            dialect,
            encoding,
        })
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn format_label(&self) -> &'static str {
        match self.dialect {
            Dialect::Surge(_) => "Surge",
            Dialect::NativeArray | Dialect::NativeObject { .. } => "sing-box JSON",
            Dialect::SubStoreArray => "Sub-Store JSON",
            Dialect::Proxies { yaml: false } => "mihomo / Shadowrocket JSON",
            Dialect::Proxies { yaml: true } | Dialect::YamlArray => "mihomo / Shadowrocket YAML",
            Dialect::Uri(_) if self.encoding.is_some() => "Base64 URI subscription",
            Dialect::Uri(_) => "URI subscription",
        }
    }

    pub fn render(
        &self,
        health: &[Health],
        options: &EnhanceOptions,
    ) -> Result<Zeroizing<String>, ModelError> {
        if health.len() != self.nodes.len() {
            return Err(ModelError::InvalidHealth);
        }
        if !options.rename && !options.sort && !health.contains(&Health::Failed) {
            return Ok(self.original.clone());
        }
        if (options.rename || health.contains(&Health::Failed))
            && self.nodes.iter().any(|node| {
                let NodeSource::Surge(source) = &node.source else {
                    return false;
                };
                uncertain_token(source, source.name_range.clone()).is_some()
                    || source.fields.iter().any(|field| {
                        field.key == "underlying-proxy"
                            && uncertain_token(source, field.value_range.clone()).is_some()
                    })
            })
        {
            return Err(ModelError::Unsupported);
        }
        let dependencies = self.dependencies()?;
        let mut dependents = vec![Vec::new(); self.nodes.len()];
        for (index, dependency) in dependencies.iter().enumerate() {
            if let Some(dependency) = dependency {
                dependents[*dependency].push(index);
            }
        }
        let mut removed = vec![false; self.nodes.len()];
        let mut pending: Vec<_> = health
            .iter()
            .enumerate()
            .filter_map(|(index, health)| (*health == Health::Failed).then_some(index))
            .collect();
        while let Some(index) = pending.pop() {
            if !removed[index] {
                removed[index] = true;
                pending.extend(&dependents[index]);
            }
        }
        let mut names: Vec<Zeroizing<String>> = Vec::with_capacity(self.nodes.len());
        let mut regions = Vec::with_capacity(self.nodes.len());
        let mut used = HashSet::new();
        for node in &self.nodes {
            let prefix = format!("{} | ", node.protocol().to_uppercase());
            let original = node.name().strip_prefix(&prefix).unwrap_or(node.name());
            let region = local_region(original);
            regions.push(region.unwrap_or(*b"ZZ"));
            let base = if !options.rename {
                node.name().to_owned()
            } else {
                let body = if let Some([first, second]) = region {
                    let marker = format!("[{}{}] ", char::from(first), char::from(second));
                    if original.starts_with(&marker) {
                        original.to_owned()
                    } else {
                        format!("{marker}{original}")
                    }
                } else {
                    original.to_owned()
                };
                format!("{prefix}{body}")
            };
            let mut candidate = base.clone();
            let mut suffix = 2usize;
            while !used.insert(candidate.clone()) {
                candidate = format!("{base} #{suffix}");
                suffix += 1;
            }
            names.push(Zeroizing::new(candidate));
        }
        let mut order: Vec<_> = (0..self.nodes.len())
            .filter(|index| !removed[*index])
            .collect();
        if options.sort {
            order.sort_by(|left, right| {
                regions[*left]
                    .cmp(&regions[*right])
                    .then_with(|| names[*left].cmp(&names[*right]))
            });
        }
        if order.iter().copied().eq(0..self.nodes.len())
            && names
                .iter()
                .zip(&self.nodes)
                .all(|(name, node)| name.as_str() == node.name())
        {
            return Ok(self.original.clone());
        }
        let output = match &self.dialect {
            Dialect::Surge(lines) => {
                let mut output = Zeroizing::new(String::new());
                let mut order = order.into_iter();
                for line in lines {
                    match line {
                        Line::Literal(value) => output.push_str(value),
                        Line::Node => {
                            if let Some(index) = order.next() {
                                let NodeSource::Surge(source) = &self.nodes[index].source else {
                                    return Err(ModelError::Render);
                                };
                                let mut raw = source.raw.clone();
                                let mut edits = Vec::new();
                                if let Some(dependency) = dependencies[index]
                                    && names[dependency].as_str() != self.nodes[dependency].name()
                                {
                                    let field = source
                                        .fields
                                        .iter()
                                        .find(|field| field.key == "underlying-proxy")
                                        .ok_or(ModelError::InvalidDependency)?;
                                    edits.push((
                                        field.value_range.clone(),
                                        quote_surge(&names[dependency])?,
                                    ));
                                }
                                if names[index].as_str() != self.nodes[index].name() {
                                    edits.push((
                                        source.name_range.clone(),
                                        quote_surge(&names[index])?,
                                    ));
                                }
                                edits.sort_by_key(|edit| std::cmp::Reverse(edit.0.start));
                                for (range, replacement) in edits {
                                    raw.replace_range(range, &replacement);
                                }
                                output.push_str(&raw);
                                if !raw.ends_with('\n') {
                                    output.push('\n');
                                }
                            }
                        }
                    }
                }
                Ok(output)
            }
            Dialect::Uri(lines) => {
                let mut output = Zeroizing::new(String::new());
                let mut order = order.into_iter();
                for line in lines {
                    match line {
                        Line::Literal(value) => output.push_str(value),
                        Line::Node => {
                            if let Some(index) = order.next() {
                                let NodeSource::Uri(source) = &self.nodes[index].source else {
                                    return Err(ModelError::Render);
                                };
                                if names[index].as_str() == self.nodes[index].name() {
                                    output.push_str(source.original());
                                } else {
                                    output.push_str(&source.render_name(&names[index])?);
                                }
                                output.push('\n');
                            }
                        }
                    }
                }
                Ok(output)
            }
            Dialect::NativeArray
            | Dialect::NativeObject { .. }
            | Dialect::SubStoreArray
            | Dialect::Proxies { .. }
            | Dialect::YamlArray => {
                let mut values = Vec::with_capacity(order.len());
                for index in order {
                    let source = match &self.nodes[index].source {
                        NodeSource::Native(source)
                        | NodeSource::SubStore(source)
                        | NodeSource::Mihomo(source) => source,
                        _ => return Err(ModelError::Render),
                    };
                    let mut value = source.0.clone();
                    value[if matches!(
                        self.nodes[index].source,
                        NodeSource::SubStore(_) | NodeSource::Mihomo(_)
                    ) {
                        "name"
                    } else {
                        "tag"
                    }] = names[index].as_str().into();
                    if let Some(dependency) = dependencies[index] {
                        let key = if matches!(
                            self.nodes[index].source,
                            NodeSource::SubStore(_) | NodeSource::Mihomo(_)
                        ) {
                            ["dialer-proxy", "detour", "underlying-proxy"]
                                .into_iter()
                                .find(|key| value.get(*key).is_some())
                                .ok_or(ModelError::InvalidDependency)?
                        } else {
                            "detour"
                        };
                        value[key] = names[dependency].as_str().into();
                    }
                    values.push(value);
                }
                let output = PrivateJson(
                    if matches!(
                        self.dialect,
                        Dialect::NativeArray | Dialect::SubStoreArray | Dialect::YamlArray
                    ) {
                        Value::Array(values)
                    } else if matches!(self.dialect, Dialect::Proxies { .. }) {
                        json!({"proxies":values})
                    } else if matches!(self.dialect, Dialect::NativeObject { endpoints: true }) {
                        json!({"outbounds":values,"endpoints":[]})
                    } else {
                        json!({"outbounds":values})
                    },
                );
                if matches!(
                    self.dialect,
                    Dialect::YamlArray | Dialect::Proxies { yaml: true }
                ) {
                    mihomo::render(&output.0)
                } else {
                    serde_json::to_string_pretty(&output.0)
                        .map(Zeroizing::new)
                        .map_err(|_| ModelError::Render)
                }
            }
        }?;
        use base64::Engine as _;
        let output = match self.encoding {
            Some(encoding) => Zeroizing::new(encoding.engine().encode(output.as_bytes())),
            None => output,
        };
        if output.len() > MAX_CONTENT {
            return Err(ModelError::Render);
        }
        Ok(output)
    }

    fn dependencies(&self) -> Result<Vec<Option<usize>>, ModelError> {
        let names: HashMap<_, _> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.name(), index))
            .collect();
        let dependencies: Vec<_> = self
            .nodes
            .iter()
            .map(|node| {
                node.dependency
                    .as_ref()
                    .map(|name| {
                        names
                            .get(name.as_str())
                            .copied()
                            .ok_or(ModelError::InvalidDependency)
                    })
                    .transpose()
            })
            .collect::<Result<_, _>>()?;
        let mut state = vec![0u8; self.nodes.len()];
        for index in 0..self.nodes.len() {
            let mut trail = Vec::new();
            let mut cursor = Some(index);
            while let Some(index) = cursor {
                match state[index] {
                    2 => break,
                    1 => return Err(ModelError::InvalidDependency),
                    _ => {
                        state[index] = 1;
                        trail.push(index);
                        cursor = dependencies[index];
                    }
                }
            }
            for index in trail {
                state[index] = 2;
            }
        }
        Ok(dependencies)
    }
}

pub struct EnhanceOptions {
    pub rename: bool,
    pub sort: bool,
}

impl Default for EnhanceOptions {
    fn default() -> Self {
        Self {
            rename: true,
            sort: true,
        }
    }
}

// Regions are local name hints, not endpoint geolocation.
fn local_region(name: &str) -> Option<[u8; 2]> {
    if let [b'[', first, second, b']', b' ', ..] = name.as_bytes()
        && first.is_ascii_uppercase()
        && second.is_ascii_uppercase()
    {
        return Some([*first, *second]);
    }
    let indicators = '\u{1f1e6}'..='\u{1f1ff}';
    let mut letters = name.chars().peekable();
    while let Some(first) = letters.next() {
        if let Some(second) = letters.peek()
            && indicators.contains(&first)
            && indicators.contains(second)
        {
            return Some([
                b'A' + (first as u32 - 0x1f1e6) as u8,
                b'A' + (*second as u32 - 0x1f1e6) as u8,
            ]);
        }
    }
    const HINTS: [([u8; 2], &[&str]); 8] = [
        (*b"HK", &["香港", "HK", "HONG KONG", "HONG-KONG"]),
        (*b"JP", &["日本", "东京", "大阪", "JP", "JAPAN"]),
        (*b"US", &["美国", "美國", "US", "USA", "UNITED STATES"]),
        (*b"SG", &["新加坡", "狮城", "SG", "SINGAPORE"]),
        (*b"TW", &["台湾", "臺灣", "TW", "TAIWAN"]),
        (*b"GB", &["英国", "英國", "UK", "GB", "BRITAIN"]),
        (*b"DE", &["德国", "德國", "DE", "GERMANY"]),
        (*b"KR", &["韩国", "韓國", "KR", "KOREA"]),
    ];
    let upper = Zeroizing::new(name.to_ascii_uppercase());
    for (region, hints) in HINTS {
        if hints.iter().any(|hint| {
            if !hint.is_ascii() {
                return upper.contains(*hint);
            }
            upper.match_indices(*hint).any(|(start, _)| {
                upper[..start]
                    .chars()
                    .next_back()
                    .is_none_or(|letter| !letter.is_ascii_alphanumeric() && letter != '_')
                    && upper[start + hint.len()..]
                        .chars()
                        .next()
                        .is_none_or(|letter| !letter.is_ascii_alphanumeric() && letter != '_')
            })
        }) {
            return Some(region);
        }
    }
    None
}

pub fn semantic_equal(left: &str, right: &str) -> bool {
    let (Ok(left), Ok(right)) = (Subscription::parse(left), Subscription::parse(right)) else {
        return false;
    };
    if std::mem::discriminant(&left.dialect) != std::mem::discriminant(&right.dialect)
        || left.nodes.len() != right.nodes.len()
    {
        return false;
    }
    if let (Dialect::NativeObject { endpoints: a }, Dialect::NativeObject { endpoints: b }) =
        (&left.dialect, &right.dialect)
        && a != b
    {
        return false;
    }
    left.nodes.iter().zip(&right.nodes).all(|(left, right)| {
        if left.name() != right.name()
            || left.server() != right.server()
            || left.protocol() != right.protocol()
            || left.port != right.port
        {
            return false;
        }
        let external = left.protocol() == "external";
        match (&left.source, &right.source) {
            (NodeSource::Native(left), NodeSource::Native(right))
            | (NodeSource::SubStore(left), NodeSource::SubStore(right))
            | (NodeSource::Mihomo(left), NodeSource::Mihomo(right)) => left.0 == right.0,
            (NodeSource::Uri(left), NodeSource::Uri(right)) => left.original() == right.original(),
            (NodeSource::Surge(left), NodeSource::Surge(right)) => {
                if external {
                    return left.raw == right.raw;
                }
                uncertain_identity(left) == uncertain_identity(right)
                    && semantic_fields(left) == semantic_fields(right)
            }
            _ => false,
        }
    })
}

fn semantic_fields(source: &SurgeNode) -> BTreeMap<&str, (&str, Option<&str>)> {
    source
        .fields
        .iter()
        .map(|field| {
            (
                field.key.as_str(),
                (
                    field.value.as_str(),
                    uncertain_token(source, field.value_range.clone()),
                ),
            )
        })
        .collect()
}

fn uncertain_identity(source: &SurgeNode) -> [Option<&str>; 3] {
    [
        source.name_range.clone(),
        source.protocol_range.clone(),
        source.server_range.clone(),
    ]
    .map(|range| uncertain_token(source, range))
}

fn uncertain_token(source: &SurgeNode, range: Range<usize>) -> Option<&str> {
    let raw = &source.raw[range];
    let mut characters = raw.strip_prefix('"')?.strip_suffix('"')?.chars();
    while let Some(character) = characters.next() {
        if character == '\\' && !matches!(characters.next(), Some('"' | '\\')) {
            return Some(raw);
        }
    }
    None
}

fn required_field<'a>(fields: &'a HashMap<&str, &str>, key: &str) -> Result<&'a str, ModelError> {
    fields
        .get(key)
        .copied()
        .filter(|value| !value.is_empty())
        .ok_or(ModelError::Unsupported)
}

fn parse_bool(value: &str) -> Result<bool, ModelError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ModelError::Unsupported),
    }
}

fn valid_label(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_control)
}

fn trimmed_range(input: &str, range: Range<usize>) -> Range<usize> {
    let value = &input[range.clone()];
    let leading = value.len() - value.trim_start().len();
    let start = range.start + leading;
    start..start + value.trim().len()
}

fn unquoted_positions(input: &str, delimiter: char) -> Result<Vec<usize>, ModelError> {
    let mut positions = Vec::new();
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && character == '\\' {
            escaped = true;
            continue;
        }
        if character == '"' {
            quoted = !quoted;
        }
        if !quoted && character == delimiter {
            positions.push(index);
        }
    }
    if quoted || escaped {
        return Err(ModelError::InvalidInput);
    }
    Ok(positions)
}

fn surge_code_end(input: &str) -> usize {
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && character == '\\' {
            escaped = true;
            continue;
        }
        if character == '"' {
            quoted = !quoted;
        }
        if !quoted
            && character == '#'
            && input[..index]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace)
        {
            return index;
        }
    }
    input.len()
}

fn decode_surge(input: &str) -> Result<Zeroizing<String>, ModelError> {
    let input = input.trim();
    if !input.starts_with('"') {
        if input.contains('"') {
            return Err(ModelError::InvalidInput);
        }
        return Ok(Zeroizing::new(input.to_owned()));
    }
    if !input.ends_with('"') || input.len() < 2 {
        return Err(ModelError::InvalidInput);
    }
    let mut output = Zeroizing::new(String::new());
    let mut characters = input[1..input.len() - 1].chars();
    while let Some(character) = characters.next() {
        match character {
            '"' => return Err(ModelError::InvalidInput),
            '\\' => match characters.next() {
                Some('"') => output.push('"'),
                Some('\\') => output.push('\\'),
                // Preserve uncertain escapes for roundtrip; projection and transformations reject them.
                Some(character) => {
                    output.push('\\');
                    output.push(character);
                }
                None => return Err(ModelError::InvalidInput),
            },
            _ => output.push(character),
        }
    }
    Ok(output)
}

fn quote_surge(input: &str) -> Result<Zeroizing<String>, ModelError> {
    serde_json::to_string(input)
        .map(Zeroizing::new)
        .map_err(|_| ModelError::Render)
}

fn parse_surge(input: &str) -> Result<(Vec<Node>, Dialect), ModelError> {
    let mut nodes = Vec::new();
    let mut lines = Vec::new();
    for (line_index, line) in input.split_inclusive('\n').enumerate() {
        let view = if line_index == 0 {
            line.trim_start_matches('\u{feff}')
        } else {
            line
        };
        if view.len() != line.len() {
            lines.push(Line::Literal(Zeroizing::new(
                line[..line.len() - view.len()].to_owned(),
            )));
        }
        let line = view;
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with(';')
            || trimmed.starts_with("//")
            || trimmed.eq_ignore_ascii_case("[Proxy]")
        {
            lines.push(Line::Literal(Zeroizing::new(line.to_owned())));
            continue;
        }
        if trimmed.starts_with('[') || line.contains('\0') {
            return Err(ModelError::InvalidInput);
        }
        let comment = surge_code_end(line);
        let meaningful = &line[..comment];
        let equals = unquoted_positions(meaningful, '=')?
            .into_iter()
            .next()
            .ok_or(ModelError::InvalidInput)?;
        let name_range = trimmed_range(line, 0..equals);
        let name = decode_surge(&line[name_range.clone()])?;
        let rest = equals + 1;
        let mut boundaries = vec![rest];
        boundaries.extend(
            unquoted_positions(&meaningful[rest..], ',')?
                .into_iter()
                .map(|index| rest + index + 1),
        );
        let mut ranges = Vec::new();
        for (index, start) in boundaries.iter().enumerate() {
            let end = boundaries
                .get(index + 1)
                .map(|next| next - 1)
                .unwrap_or(meaningful.len());
            ranges.push(trimmed_range(line, *start..end));
        }
        if ranges.len() < 3 {
            return Err(ModelError::InvalidInput);
        }
        let protocol_range = ranges[0].clone();
        let protocol = Zeroizing::new(decode_surge(&line[protocol_range.clone()])?.to_lowercase());
        let external = protocol.as_str() == "external";
        let server_range = if external {
            protocol_range.clone()
        } else {
            ranges[1].clone()
        };
        let mut fields = Vec::new();
        let mut keys = HashSet::new();
        for range in ranges.iter().skip(if external { 1 } else { 3 }).cloned() {
            let relative = unquoted_positions(&line[range.clone()], '=')?
                .into_iter()
                .next()
                .ok_or(ModelError::InvalidInput)?;
            let equal = range.start + relative;
            let key = line[range.start..equal].trim().to_lowercase();
            if !valid_label(&key) || (!keys.insert(key.clone()) && !(external && key == "args")) {
                return Err(ModelError::InvalidInput);
            }
            let value_range = trimmed_range(line, equal + 1..range.end);
            fields.push(Field {
                key,
                value: decode_surge(&line[value_range.clone()])?,
                value_range,
            });
        }
        let (server, port) = if external {
            let exec = fields
                .iter()
                .find(|field| field.key == "exec")
                .ok_or(ModelError::InvalidInput)?;
            if !valid_label(exec.value.trim()) {
                return Err(ModelError::InvalidInput);
            }
            let port = fields
                .iter()
                .find(|field| field.key == "local-port")
                .ok_or(ModelError::InvalidInput)?
                .value
                .parse::<u16>()
                .map_err(|_| ModelError::InvalidInput)?;
            (Zeroizing::new("127.0.0.1".to_owned()), port)
        } else {
            (
                decode_surge(&line[ranges[1].clone()])?,
                decode_surge(&line[ranges[2].clone()])?
                    .parse::<u16>()
                    .map_err(|_| ModelError::InvalidInput)?,
            )
        };
        if !valid_label(&name) || !valid_label(&protocol) || !valid_label(&server) || port == 0 {
            return Err(ModelError::InvalidInput);
        }
        let dependency = fields
            .iter()
            .find(|field| field.key == "underlying-proxy")
            .map(|field| field.value.clone());
        nodes.push(Node {
            name,
            server,
            protocol,
            port,
            dependency,
            source: NodeSource::Surge(SurgeNode {
                raw: Zeroizing::new(line.to_owned()),
                name_range,
                protocol_range,
                server_range,
                fields,
            }),
        });
        lines.push(Line::Node);
    }
    Ok((nodes, Dialect::Surge(lines)))
}

fn parse_plain(input: &str) -> Result<(Vec<Node>, Dialect), ModelError> {
    let trimmed = input.trim_start_matches('\u{feff}').trim_start();
    let first = trimmed
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with(';'))
        .ok_or(ModelError::InvalidInput)?;
    if first.split_once("://").is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    }) {
        return parse_uri(input);
    }
    if first.eq_ignore_ascii_case("[Proxy]") {
        return parse_surge(input);
    }
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && let Ok(result) = parse_native(trimmed)
    {
        return Ok(result);
    }
    if let Ok(result) = parse_surge(input) {
        return Ok(result);
    }
    parse_structured(mihomo::parse_yaml(trimmed)?, true)
}

fn parse_uri(input: &str) -> Result<(Vec<Node>, Dialect), ModelError> {
    let mut nodes = Vec::new();
    let mut lines = Vec::new();
    if input.starts_with('\u{feff}') {
        lines.push(Line::Literal(Zeroizing::new("\u{feff}".into())));
    }
    let input = input.trim_start_matches('\u{feff}');
    for line in input.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            lines.push(Line::Literal(Zeroizing::new(line.to_owned())));
            continue;
        }
        let source = uri::parse(trimmed, nodes.len())?;
        nodes.push(Node {
            name: source.name.clone(),
            server: source.server.clone(),
            protocol: source.protocol.clone(),
            port: source.port,
            dependency: None,
            source: NodeSource::Uri(source),
        });
        lines.push(Line::Node);
    }
    if nodes.is_empty() {
        return Err(ModelError::InvalidInput);
    }
    Ok((nodes, Dialect::Uri(lines)))
}

fn parse_native(input: &str) -> Result<(Vec<Node>, Dialect), ModelError> {
    let root = PrivateJson(
        serde_json::from_str::<UniqueValue>(input)
            .map_err(|_| ModelError::InvalidInput)?
            .0,
    );
    parse_structured(root, false)
}

fn parse_structured(mut root: PrivateJson, yaml: bool) -> Result<(Vec<Node>, Dialect), ModelError> {
    let (outbounds, dialect) = match &mut root.0 {
        Value::Array(values) => {
            let substore = values
                .first()
                .and_then(Value::as_object)
                .is_some_and(|v| v.contains_key("name"));
            (
                values,
                if yaml && substore {
                    Dialect::YamlArray
                } else if yaml {
                    return Err(ModelError::InvalidInput);
                } else if substore {
                    Dialect::SubStoreArray
                } else {
                    Dialect::NativeArray
                },
            )
        }
        Value::Object(object)
            if !yaml
                && object.contains_key("outbounds")
                && object
                    .keys()
                    .all(|key| key == "outbounds" || key == "endpoints")
                && object
                    .get("endpoints")
                    .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty)) =>
        {
            let endpoints = object.contains_key("endpoints");
            (
                object
                    .get_mut("outbounds")
                    .and_then(Value::as_array_mut)
                    .ok_or(ModelError::InvalidInput)?,
                Dialect::NativeObject { endpoints },
            )
        }
        Value::Object(object) if object.len() == 1 && object.contains_key("proxies") => (
            object
                .get_mut("proxies")
                .and_then(Value::as_array_mut)
                .ok_or(ModelError::InvalidInput)?,
            Dialect::Proxies { yaml },
        ),
        _ => return Err(ModelError::InvalidInput),
    };
    let mut nodes = Vec::with_capacity(outbounds.len());
    for value in outbounds.drain(..) {
        let source = PrivateJson(value);
        let object = source.0.as_object().ok_or(ModelError::InvalidInput)?;
        let substore = matches!(
            dialect,
            Dialect::SubStoreArray | Dialect::YamlArray | Dialect::Proxies { .. }
        );
        if if substore {
            object.contains_key("tag") || object.contains_key("server_port")
        } else {
            object.contains_key("name") || object.contains_key("port")
        } {
            return Err(ModelError::InvalidInput);
        }
        let text = |key: &str| -> Result<Zeroizing<String>, ModelError> {
            object
                .get(key)
                .and_then(Value::as_str)
                .filter(|value| valid_label(value))
                .map(|value| Zeroizing::new(value.to_owned()))
                .ok_or(ModelError::InvalidInput)
        };
        let name = text(if substore { "name" } else { "tag" })?;
        let server = text("server")?;
        let protocol = text("type")?;
        if matches!(
            protocol.as_str(),
            "direct" | "block" | "dns" | "selector" | "urltest"
        ) {
            return Err(ModelError::InvalidInput);
        }
        let port = object
            .get(if substore { "port" } else { "server_port" })
            .and_then(Value::as_u64)
            .and_then(|port| u16::try_from(port).ok())
            .filter(|port| *port > 0)
            .ok_or(ModelError::InvalidInput)?;
        let keys: &[&str] = if substore {
            &["dialer-proxy", "detour", "underlying-proxy"]
        } else {
            &["detour"]
        };
        let mut found = keys.iter().filter_map(|key| object.get(*key));
        let dependency = found
            .next()
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| valid_label(value))
                    .map(|value| Zeroizing::new(value.to_owned()))
                    .ok_or(ModelError::InvalidInput)
            })
            .transpose()?;
        if found.next().is_some() {
            return Err(ModelError::InvalidInput);
        }
        nodes.push(Node {
            name,
            server,
            protocol,
            port,
            dependency,
            source: if matches!(dialect, Dialect::YamlArray | Dialect::Proxies { .. }) {
                NodeSource::Mihomo(source)
            } else if substore {
                NodeSource::SubStore(source)
            } else {
                NodeSource::Native(source)
            },
        });
    }
    Ok((nodes, dialect))
}

fn only_keys(value: &Value, allowed: &[&str]) -> Result<(), ModelError> {
    let object = value.as_object().ok_or(ModelError::Unsupported)?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(ModelError::Unsupported);
    }
    Ok(())
}

fn validate_native(value: &Value, compatibility: bool) -> Result<(), ModelError> {
    if value
        .get("tls")
        .and_then(|tls| tls.get("insecure"))
        .and_then(Value::as_bool)
        == Some(true)
        && !compatibility
    {
        return Err(ModelError::SecurityBlocked);
    }
    let common = ["type", "tag", "server", "server_port", "tcp_fast_open"];
    let protocol_fields: &[&str] = match value["type"].as_str() {
        Some("anytls") => &[
            "password",
            "tls",
            "idle_session_check_interval",
            "idle_session_timeout",
            "min_idle_session",
        ],
        Some("shadowsocks") => &["method", "password", "network", "udp_over_tcp"],
        Some("socks") => &["version", "username", "password", "network", "udp_over_tcp"],
        Some("http") => &["username", "password", "tls", "headers"],
        Some("trojan") => &["password", "tls", "transport", "network"],
        Some("vmess") => &[
            "uuid",
            "security",
            "alter_id",
            "global_padding",
            "authenticated_length",
            "packet_encoding",
            "tls",
            "transport",
            "network",
        ],
        Some("vless") => &[
            "uuid",
            "flow",
            "packet_encoding",
            "tls",
            "transport",
            "network",
        ],
        Some("tuic") => &[
            "uuid",
            "password",
            "congestion_control",
            "udp_relay_mode",
            "udp_over_stream",
            "zero_rtt_handshake",
            "heartbeat",
            "tls",
            "network",
        ],
        Some("hysteria2") => &["password", "obfs", "up_mbps", "down_mbps", "tls", "network"],
        _ => return Err(ModelError::Unsupported),
    };
    let object = value.as_object().ok_or(ModelError::Unsupported)?;
    if object
        .keys()
        .any(|key| !common.contains(&key.as_str()) && !protocol_fields.contains(&key.as_str()))
    {
        return Err(ModelError::Unsupported);
    }
    if let Some(tfo) = value.get("tcp_fast_open")
        && !tfo.is_boolean()
    {
        return Err(ModelError::Unsupported);
    }
    if let Some(tls) = value.get("tls") {
        only_keys(
            tls,
            &[
                "enabled",
                "disable_sni",
                "server_name",
                "insecure",
                "alpn",
                "min_version",
                "max_version",
                "cipher_suites",
                "curve_preferences",
                "utls",
                "reality",
            ],
        )?;
        for key in ["enabled", "disable_sni", "insecure"] {
            if tls.get(key).is_some_and(|value| !value.is_boolean()) {
                return Err(ModelError::Unsupported);
            }
        }
        if let Some(utls) = tls.get("utls") {
            only_keys(utls, &["enabled", "fingerprint"])?;
        }
        if let Some(reality) = tls.get("reality") {
            only_keys(reality, &["enabled", "public_key", "short_id"])?;
            if reality.get("enabled") == Some(&Value::Bool(true))
                && (tls.get("enabled") != Some(&Value::Bool(true))
                    || tls.get("utls").and_then(|v| v.get("enabled")) != Some(&Value::Bool(true)))
            {
                return Err(ModelError::Unsupported);
            }
        }
    }
    if let Some(transport) = value.get("transport") {
        let keys: &[&str] = match transport.get("type").and_then(Value::as_str) {
            Some("ws") => &[
                "type",
                "path",
                "headers",
                "max_early_data",
                "early_data_header_name",
            ],
            Some("http") => &[
                "type",
                "host",
                "path",
                "method",
                "headers",
                "idle_timeout",
                "ping_timeout",
            ],
            Some("httpupgrade") => &["type", "host", "path", "headers"],
            Some("grpc") => &[
                "type",
                "service_name",
                "idle_timeout",
                "ping_timeout",
                "permit_without_stream",
            ],
            Some("quic") => &["type"],
            _ => return Err(ModelError::Unsupported),
        };
        only_keys(transport, keys)?;
    }
    if let Some(obfs) = value.get("obfs") {
        only_keys(obfs, &["type", "password"])?;
    }
    if let Some(uot) = value.get("udp_over_tcp") {
        only_keys(uot, &["enabled", "version"])?;
    }
    Ok(())
}

// Reject duplicate JSON keys rather than silently selecting one credential or setting.
struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("JSON without duplicate keys")
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|value| UniqueValue(Value::Number(value)))
                    .ok_or_else(|| E::custom("invalid JSON number"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(value.into()))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                self.visit_unit()
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element::<UniqueValue>()? {
                    values.push(value.0);
                }
                Ok(UniqueValue(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = Map::new();
                while let Some((key, value)) = map.next_entry::<String, UniqueValue>()? {
                    if values.insert(key, value.0).is_some() {
                        return Err(de::Error::custom("duplicate JSON key"));
                    }
                }
                Ok(UniqueValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
