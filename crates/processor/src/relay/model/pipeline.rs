//! Ordered subscription transformations with explicitly injected network operations.
use super::{
    MAX_CONTENT, ModelError, NodeIdentity, NodeSource, PrivateJson, Subscription, local_region,
    properties, uncertain_identity, uncertain_token,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::IpAddr;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub id: u64,
    pub enabled: bool,
    pub operation: Operation,
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    NameFilter {
        pattern: String,
        keep: bool,
    },
    ProtocolFilter {
        protocols: Vec<String>,
        keep: bool,
    },
    ReplaceName {
        pattern: String,
        replacement: String,
        regex: bool,
    },
    FormatName {
        template: String,
    },
    Flags {
        add: bool,
    },
    Sort {
        key: SortKey,
        descending: bool,
    },
    RegexSort {
        patterns: Vec<String>,
    },
    Deduplicate,
    HostFilter {
        pattern: String,
        keep: bool,
    },
    PortFilter {
        start: u16,
        end: u16,
        keep: bool,
    },
    RegionFilter {
        regions: Vec<String>,
        keep: bool,
        keep_unknown: bool,
    },
    IpFilter {
        family: AddressFamily,
        keep_domains: bool,
    },
    Limit {
        count: usize,
    },
    Reverse,
    ResolveDomain {
        family: AddressFamily,
    },
    SetProperty {
        setting: properties::Setting,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum AddressFamily {
    V4,
    V6,
    PreferV4,
    PreferV6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolveError {
    ConsentRequired,
    Cancelled,
    Unavailable,
    InvalidResponse,
    RateLimited { retry_after_seconds: u64 },
    Budget,
}

pub type Resolver<'a> =
    dyn Fn(&[&str], AddressFamily) -> Result<Vec<Option<IpAddr>>, ResolveError> + 'a;

pub fn has_resolution(steps: &[Step]) -> bool {
    steps
        .iter()
        .any(|step| step.enabled && matches!(step.operation, Operation::ResolveDomain { .. }))
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    Name,
    Protocol,
    Region,
}

#[derive(Clone)]
pub struct StageReport {
    pub id: u64,
    pub before: usize,
    pub after: usize,
    pub renamed: usize,
    pub modified: usize,
    pub skipped: usize,
}

fn valid_text(s: &str) -> bool {
    s.len() <= 512 && !s.chars().any(char::is_control)
}
fn set_name(
    names: &mut [Zeroizing<String>],
    i: usize,
    value: Zeroizing<String>,
    total: &mut usize,
) -> Result<bool, ModelError> {
    if value.is_empty() || !valid_text(&value) {
        return Err(ModelError::InvalidEdit);
    }
    *total = total
        .checked_sub(names[i].len())
        .and_then(|n| n.checked_add(value.len()))
        .ok_or(ModelError::InvalidEdit)?;
    if *total > MAX_CONTENT {
        return Err(ModelError::InvalidEdit);
    }
    let changed = names[i] != value;
    names[i] = value;
    Ok(changed)
}
fn regex(pattern: &str) -> Result<regex::Regex, ModelError> {
    if !valid_text(pattern) {
        return Err(ModelError::InvalidEdit);
    }
    regex::RegexBuilder::new(pattern)
        .size_limit(1024 * 1024)
        .build()
        .map_err(|_| ModelError::InvalidEdit)
}

fn template_parts(template: &str) -> Result<Vec<&str>, ModelError> {
    if !valid_text(template) {
        return Err(ModelError::InvalidEdit);
    }
    let mut parts = Vec::new();
    let mut rest = template;
    while !rest.is_empty() {
        match rest.find(['{', '}']) {
            None => {
                parts.push(rest);
                break;
            }
            Some(i) => {
                if i > 0 {
                    parts.push(&rest[..i]);
                }
                if !rest[i..].starts_with('{') {
                    return Err(ModelError::InvalidEdit);
                }
                let end = rest[i..].find('}').ok_or(ModelError::InvalidEdit)? + i;
                let token = &rest[i..=end];
                if !matches!(token, "{name}" | "{protocol}" | "{region}" | "{index}") {
                    return Err(ModelError::InvalidEdit);
                }
                parts.push(token);
                rest = &rest[end + 1..];
            }
        }
    }
    Ok(parts)
}

pub fn validate(steps: &[Step]) -> Result<(), ModelError> {
    if steps.len() > 64 {
        return Err(ModelError::InvalidEdit);
    }
    let mut ids = HashSet::new();
    for step in steps {
        if !ids.insert(step.id) {
            return Err(ModelError::InvalidEdit);
        }
        match &step.operation {
            Operation::NameFilter { pattern, .. } | Operation::HostFilter { pattern, .. } => {
                if !valid_text(pattern) {
                    return Err(ModelError::InvalidEdit);
                }
                if step.enabled {
                    if pattern.is_empty() {
                        return Err(ModelError::InvalidEdit);
                    }
                    regex(pattern)?;
                }
            }
            Operation::ProtocolFilter { protocols, .. } => {
                if protocols.len() > 64
                    || protocols.iter().any(|s| !valid_text(s))
                    || step.enabled
                        && (protocols.is_empty() || protocols.iter().any(String::is_empty))
                {
                    return Err(ModelError::InvalidEdit);
                }
            }
            Operation::ReplaceName {
                pattern,
                replacement,
                regex: is_regex,
            } => {
                if !valid_text(pattern)
                    || !valid_text(replacement)
                    || step.enabled && pattern.is_empty()
                {
                    return Err(ModelError::InvalidEdit);
                }
                if step.enabled && *is_regex {
                    regex(pattern)?;
                }
            }
            Operation::FormatName { template } => {
                if !valid_text(template) {
                    return Err(ModelError::InvalidEdit);
                }
                if step.enabled {
                    if template.is_empty() {
                        return Err(ModelError::InvalidEdit);
                    }
                    template_parts(template)?;
                }
            }
            Operation::RegexSort { patterns } => {
                if patterns.len() > 64
                    || patterns.iter().any(|p| !valid_text(p))
                    || step.enabled
                        && (patterns.is_empty() || patterns.iter().any(String::is_empty))
                {
                    return Err(ModelError::InvalidEdit);
                }
                if step.enabled {
                    for pattern in patterns {
                        regex(pattern)?;
                    }
                }
            }
            Operation::PortFilter { start, end, .. }
                if step.enabled && (*start == 0 || start > end) =>
            {
                return Err(ModelError::InvalidEdit);
            }
            Operation::IpFilter {
                family: AddressFamily::PreferV4 | AddressFamily::PreferV6,
                ..
            } if step.enabled => return Err(ModelError::InvalidEdit),
            Operation::RegionFilter { regions, .. } => {
                if regions.len() > 64
                    || regions.iter().any(|r| !valid_text(r))
                    || step.enabled
                        && (regions.is_empty()
                            || regions.iter().any(|r| {
                                r.len() != 2 || !r.bytes().all(|b| b.is_ascii_alphabetic())
                            }))
                {
                    return Err(ModelError::InvalidEdit);
                }
            }
            Operation::Limit { count } if *count > 100_000 => return Err(ModelError::InvalidEdit),
            Operation::SetProperty { setting } => {
                match setting {
                    properties::Setting::ServerName { value }
                        if value.len() > 512 || value.chars().any(char::is_control) =>
                    {
                        return Err(ModelError::InvalidEdit);
                    }
                    properties::Setting::Alpn { values }
                        if values.len() > 32 || values.iter().any(|v| !valid_text(v)) =>
                    {
                        return Err(ModelError::InvalidEdit);
                    }
                    _ => {}
                }
                if step.enabled {
                    properties::validate_setting(setting)?;
                }
            }
            Operation::Flags { .. }
            | Operation::Sort { .. }
            | Operation::Deduplicate
            | Operation::PortFilter { .. }
            | Operation::IpFilter { .. }
            | Operation::Limit { .. }
            | Operation::Reverse
            | Operation::ResolveDomain { .. } => {}
        }
    }
    Ok(())
}

// Identical conservative identity rules to the legacy editor; credentials remain zeroized.
pub(super) fn identity(node: &super::Node) -> Result<Option<Zeroizing<String>>, ModelError> {
    let value = match &node.source {
        NodeSource::Native(source) | NodeSource::SubStore(source) | NodeSource::Mihomo(source)
            if matches!(
                node.protocol(),
                "anytls"
                    | "ss"
                    | "shadowsocks"
                    | "socks"
                    | "socks5"
                    | "http"
                    | "https"
                    | "trojan"
                    | "vmess"
                    | "vless"
                    | "tuic"
                    | "hysteria2"
            ) =>
        {
            let mut value = PrivateJson(source.0.clone());
            if let Some(object) = value.0.as_object_mut() {
                object.remove(if matches!(node.source, NodeSource::Native(_)) {
                    "tag"
                } else {
                    "name"
                });
            }
            Some(Zeroizing::new(
                serde_json::to_string(&value.0).map_err(|_| ModelError::Render)?,
            ))
        }
        NodeSource::Surge(source)
            if matches!(
                node.protocol(),
                "anytls" | "ss" | "socks5" | "http" | "https" | "trojan" | "vmess"
            ) && uncertain_identity(source).iter().all(Option::is_none)
                && source
                    .fields
                    .iter()
                    .all(|f| uncertain_token(source, f.value_range.clone()).is_none()) =>
        {
            let mut raw = source.raw.clone();
            raw.replace_range(source.name_range.clone(), "");
            Some(raw)
        }
        _ => None,
    };
    Ok(value)
}

pub(super) struct Working<'a> {
    pub sub: &'a Subscription,
    pub dependencies: &'a [Option<usize>],
    pub dependents: &'a [Vec<usize>],
    pub names: &'a mut [Zeroizing<String>],
    pub order: &'a mut Vec<usize>,
    pub mutations: &'a mut [properties::Mutation],
    pub resolver: Option<&'a Resolver<'a>>,
}

#[derive(Default)]
pub(super) struct Counts {
    pub excluded: usize,
    pub duplicates: usize,
    pub skipped: usize,
}

fn protocol(value: &str) -> &str {
    if value.eq_ignore_ascii_case("ss") || value.eq_ignore_ascii_case("shadowsocks") {
        "shadowsocks"
    } else if value.eq_ignore_ascii_case("socks") || value.eq_ignore_ascii_case("socks5") {
        "socks"
    } else if value.eq_ignore_ascii_case("hy2") || value.eq_ignore_ascii_case("hysteria2") {
        "hysteria2"
    } else {
        value
    }
}

pub(super) fn apply(
    working: &mut Working<'_>,
    steps: &[Step],
) -> Result<(Vec<StageReport>, Counts), ModelError> {
    let Working {
        sub,
        dependencies,
        dependents,
        names,
        order,
        mutations,
        resolver,
    } = working;
    let mut counts = Counts::default();
    let mut skipped_nodes = HashSet::new();
    let mut reports = Vec::new();
    for step in steps.iter().filter(|s| s.enabled) {
        let before = order.len();
        let mut renamed = 0;
        let mut modified = 0;
        let mut skipped = 0;
        let mut name_bytes = order.iter().map(|&i| names[i].len()).sum();
        let mut remove = vec![false; names.len()];
        let mut duplicate = vec![false; names.len()];
        match &step.operation {
            Operation::HostFilter { pattern, keep } => {
                let re = regex(pattern)?;
                for &i in order.iter() {
                    let host = mutations[i]
                        .address
                        .as_deref()
                        .unwrap_or(sub.nodes[i].server());
                    remove[i] = re.is_match(host) != *keep;
                }
            }
            Operation::PortFilter { start, end, keep } => {
                for &i in order.iter() {
                    remove[i] = (*start..=*end).contains(&sub.nodes[i].port) != *keep;
                }
            }
            Operation::RegionFilter {
                regions,
                keep,
                keep_unknown,
            } => {
                for &i in order.iter() {
                    remove[i] = match local_region(&names[i]) {
                        Some(region) => {
                            regions
                                .iter()
                                .any(|r| r.as_bytes().eq_ignore_ascii_case(&region))
                                != *keep
                        }
                        None => !*keep_unknown,
                    };
                }
            }
            Operation::IpFilter {
                family,
                keep_domains,
            } => {
                for &i in order.iter() {
                    let host = mutations[i]
                        .address
                        .as_deref()
                        .unwrap_or(sub.nodes[i].server());
                    remove[i] = match host.parse::<IpAddr>() {
                        Ok(IpAddr::V4(_)) => {
                            matches!(family, AddressFamily::V6 | AddressFamily::PreferV6)
                        }
                        Ok(IpAddr::V6(_)) => {
                            matches!(family, AddressFamily::V4 | AddressFamily::PreferV4)
                        }
                        Err(_) => !*keep_domains,
                    };
                }
            }
            Operation::Limit { count } => {
                for &i in order.iter().skip(*count) {
                    remove[i] = true;
                }
            }
            Operation::Reverse => order.reverse(),
            Operation::SetProperty { setting } => {
                for &i in order.iter() {
                    match properties::set_setting(&sub.nodes[i], &mut mutations[i], setting) {
                        Ok(changed) => modified += usize::from(changed),
                        Err(ModelError::Unsupported) => skipped += 1,
                        Err(error) => return Err(error),
                    }
                }
            }
            Operation::ResolveDomain { family } => {
                let mut indices = Vec::new();
                for &i in order.iter() {
                    let host = mutations[i]
                        .address
                        .as_deref()
                        .unwrap_or(sub.nodes[i].server());
                    if properties::hostname(host) {
                        indices.push(i);
                    } else {
                        skipped += 1;
                    }
                }
                if !indices.is_empty() {
                    let resolver =
                        resolver.ok_or(ModelError::Resolution(ResolveError::ConsentRequired))?;
                    let hosts: Vec<_> = indices
                        .iter()
                        .map(|&i| {
                            mutations[i]
                                .address
                                .as_deref()
                                .unwrap_or(sub.nodes[i].server())
                        })
                        .collect();
                    let addresses = resolver(&hosts, *family).map_err(ModelError::Resolution)?;
                    if addresses.len() != indices.len() {
                        return Err(ModelError::Resolution(ResolveError::InvalidResponse));
                    }
                    for (i, address) in indices.into_iter().zip(addresses) {
                        let Some(address) = address else {
                            skipped += 1;
                            continue;
                        };
                        if matches!(
                            (*family, address),
                            (AddressFamily::V4, IpAddr::V6(_)) | (AddressFamily::V6, IpAddr::V4(_))
                        ) {
                            return Err(ModelError::Resolution(ResolveError::InvalidResponse));
                        }
                        match properties::set_address(&sub.nodes[i], &mut mutations[i], address) {
                            Ok(changed) => modified += usize::from(changed),
                            Err(ModelError::Unsupported) => skipped += 1,
                            Err(error) => return Err(error),
                        }
                    }
                }
            }
            Operation::NameFilter { pattern, keep } => {
                let re = regex(pattern)?;
                for &i in order.iter() {
                    remove[i] = re.is_match(&names[i]) != *keep;
                }
            }
            Operation::ProtocolFilter { protocols, keep } => {
                for &i in order.iter() {
                    remove[i] = protocols.iter().any(|p| {
                        protocol(p).eq_ignore_ascii_case(protocol(sub.nodes[i].protocol()))
                    }) != *keep;
                }
            }
            Operation::ReplaceName {
                pattern,
                replacement,
                regex: is_regex,
            } => {
                let re = if *is_regex {
                    Some(regex(pattern)?)
                } else {
                    None
                };
                for &i in order.iter() {
                    let value = Zeroizing::new(if let Some(re) = &re {
                        re.replace_all(&names[i], replacement.as_str()).into_owned()
                    } else {
                        names[i].replace(pattern, replacement)
                    });
                    renamed += usize::from(set_name(names, i, value, &mut name_bytes)?);
                }
            }
            Operation::FormatName { template } => {
                let parts = template_parts(template)?;
                for (pos, &i) in order.iter().enumerate() {
                    let region = local_region(&names[i])
                        .map(|r| String::from_utf8_lossy(&r).into_owned())
                        .unwrap_or_default();
                    let mut result = Zeroizing::new(String::new());
                    for part in &parts {
                        if *part == "{index}" {
                            result.push_str(&(pos + 1).to_string());
                        } else {
                            result.push_str(match *part {
                                "{name}" => &names[i],
                                "{protocol}" => sub.nodes[i].protocol(),
                                "{region}" => &region,
                                text => text,
                            });
                        }
                    }
                    renamed += usize::from(set_name(names, i, result, &mut name_bytes)?);
                }
            }
            Operation::Flags { add } => {
                for &i in order.iter() {
                    if *add {
                        if let Some(region) = local_region(&names[i]) {
                            let flag: String = region
                                .into_iter()
                                .filter_map(|c| char::from_u32(0x1f1e6 + u32::from(c - b'A')))
                                .collect();
                            if !names[i].contains(&flag) {
                                let value = Zeroizing::new(format!("{flag} {}", names[i].as_str()));
                                renamed += usize::from(set_name(names, i, value, &mut name_bytes)?);
                            }
                        }
                    } else {
                        let mut chars = names[i].chars();
                        if chars.next().is_some_and(|c| ('🇦'..='🇿').contains(&c))
                            && chars.next().is_some_and(|c| ('🇦'..='🇿').contains(&c))
                        {
                            let value = Zeroizing::new(chars.as_str().trim_start().to_owned());
                            renamed += usize::from(set_name(names, i, value, &mut name_bytes)?);
                        }
                    }
                }
            }
            Operation::Sort { key, descending } => {
                let mut regions = Vec::new();
                if *key == SortKey::Region {
                    regions.resize(names.len(), *b"ZZ");
                    for &i in order.iter() {
                        regions[i] = local_region(&names[i]).unwrap_or(*b"ZZ");
                    }
                }
                order.sort_by(|&a, &b| {
                    let cmp = match key {
                        SortKey::Name => names[a].cmp(&names[b]),
                        SortKey::Protocol => {
                            protocol(sub.nodes[a].protocol()).cmp(protocol(sub.nodes[b].protocol()))
                        }
                        SortKey::Region => regions[a].cmp(&regions[b]),
                    };
                    if *descending { cmp.reverse() } else { cmp }
                });
            }
            Operation::RegexSort { patterns } => {
                let regexes: Vec<_> = patterns
                    .iter()
                    .map(|p| regex(p))
                    .collect::<Result<_, _>>()?;
                let mut priorities = vec![regexes.len(); names.len()];
                for &i in order.iter() {
                    priorities[i] = regexes
                        .iter()
                        .position(|r| r.is_match(&names[i]))
                        .unwrap_or(regexes.len());
                }
                order.sort_by_key(|&i| priorities[i]);
            }
            Operation::Deduplicate => {
                let mut seen = HashSet::new();
                for &i in order.iter() {
                    if dependencies[i].is_some() || !dependents[i].is_empty() {
                        skipped_nodes.insert(i);
                    } else if let Some(mut value) = identity(&sub.nodes[i])? {
                        let mutation = Zeroizing::new(
                            serde_json::to_string(&mutations[i]).map_err(|_| ModelError::Render)?,
                        );
                        value.push('\0');
                        value.push_str(&mutation);
                        if !seen.insert(NodeIdentity(value)) {
                            remove[i] = true;
                            duplicate[i] = true;
                            counts.duplicates += 1;
                        }
                    } else {
                        skipped_nodes.insert(i);
                    }
                }
            }
        }
        let mut pending: Vec<_> = order.iter().copied().filter(|&i| remove[i]).collect();
        while let Some(i) = pending.pop() {
            for &child in &dependents[i] {
                if !remove[child] {
                    remove[child] = true;
                    pending.push(child);
                }
            }
        }
        counts.excluded += order
            .iter()
            .filter(|&&i| remove[i] && !duplicate[i])
            .count();
        order.retain(|&i| !remove[i]);
        let mut total = 0usize;
        for &i in order.iter() {
            if names[i].is_empty() || !valid_text(&names[i]) {
                return Err(ModelError::InvalidEdit);
            }
            total = total
                .checked_add(names[i].len())
                .ok_or(ModelError::InvalidEdit)?;
            if total > MAX_CONTENT {
                return Err(ModelError::InvalidEdit);
            }
        }
        reports.push(StageReport {
            id: step.id,
            before,
            after: order.len(),
            renamed,
            modified,
            skipped,
        });
    }
    let mut used = HashSet::new();
    if order.iter().any(|&i| !used.insert(names[i].as_str())) {
        return Err(ModelError::InvalidEdit);
    }
    counts.skipped = skipped_nodes.len();
    Ok((reports, counts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::model::{EnhanceOptions, Health, editor};
    #[test]
    fn resolver_batch_retains_tls_identity_and_reports_missing() {
        let sub = Subscription::parse(r#"[{"type":"trojan","tag":"a","server":"a.example","server_port":443,"password":"x","tls":{"enabled":true}},{"type":"trojan","tag":"b","server":"b.example","server_port":443,"password":"x","tls":{"enabled":true}}]"#).unwrap();
        let options = editor::Options {
            pipeline: vec![step(
                1,
                Operation::ResolveDomain {
                    family: AddressFamily::V4,
                },
            )],
            include: String::new(),
            exclude: String::new(),
            find: String::new(),
            replace: String::new(),
            nodes: Vec::new(),
            deduplicate: false,
        };
        let resolver = |hosts: &[&str], _: AddressFamily| {
            assert_eq!(hosts, &["a.example", "b.example"]);
            Ok(vec![Some("192.0.2.1".parse().unwrap()), None])
        };
        let result = sub
            .render_processed_with_resolver(
                &[Health::Unknown; 2],
                &EnhanceOptions {
                    rename: false,
                    sort: false,
                },
                &options,
                &resolver,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(value[0]["server"], "192.0.2.1");
        assert_eq!(value[0]["tls"]["server_name"], "a.example");
        assert_eq!(value[1]["server"], "b.example");
        assert_eq!(
            (result.stages[0].modified, result.stages[0].skipped),
            (1, 1)
        );
    }

    #[test]
    fn resolver_requires_consent_and_valid_batch() {
        let sub = Subscription::parse(
            r#"[{"type":"http","tag":"a","server":"a.example","server_port":80}]"#,
        )
        .unwrap();
        let options = editor::Options {
            pipeline: vec![step(
                1,
                Operation::ResolveDomain {
                    family: AddressFamily::V4,
                },
            )],
            include: String::new(),
            exclude: String::new(),
            find: String::new(),
            replace: String::new(),
            nodes: Vec::new(),
            deduplicate: false,
        };
        let enhance = EnhanceOptions {
            rename: false,
            sort: false,
        };
        assert!(matches!(
            sub.render_processed(&[Health::Unknown], &enhance, &options),
            Err(ModelError::Resolution(ResolveError::ConsentRequired))
        ));
        let resolver = |_: &[&str], _: AddressFamily| Ok(vec![]);
        assert!(matches!(
            sub.render_processed_with_resolver(&[Health::Unknown], &enhance, &options, &resolver),
            Err(ModelError::Resolution(ResolveError::InvalidResponse))
        ));
    }

    #[test]
    fn resolved_host_filter_uses_effective_address() {
        let sub = Subscription::parse(
            r#"[{"type":"socks","tag":"a","server":"a.example","server_port":80}]"#,
        )
        .unwrap();
        let options = editor::Options {
            pipeline: vec![
                step(
                    1,
                    Operation::ResolveDomain {
                        family: AddressFamily::V4,
                    },
                ),
                step(
                    2,
                    Operation::HostFilter {
                        pattern: "^192\\.0\\.2\\.1$".into(),
                        keep: true,
                    },
                ),
            ],
            include: String::new(),
            exclude: String::new(),
            find: String::new(),
            replace: String::new(),
            nodes: Vec::new(),
            deduplicate: false,
        };
        let resolver = |_: &[&str], _: AddressFamily| Ok(vec![Some("192.0.2.1".parse().unwrap())]);
        let result = sub
            .render_processed_with_resolver(
                &[Health::Unknown],
                &EnhanceOptions {
                    rename: false,
                    sort: false,
                },
                &options,
                &resolver,
            )
            .unwrap();
        assert_eq!(result.retained, vec![0]);
    }

    #[test]
    fn local_filters_reverse_and_limit_apply_in_sequence() {
        let source = "JP A = socks5, a.example, 1080\nUS B = socks5, b.example, 443\nJP C = socks5, 1.1.1.1, 443\nOther = socks5, 2606:4700:4700::1111, 443\n";
        let out = run(
            source,
            vec![
                step(
                    1,
                    Operation::PortFilter {
                        start: 443,
                        end: 443,
                        keep: true,
                    },
                ),
                step(
                    2,
                    Operation::RegionFilter {
                        regions: vec!["JP".into()],
                        keep: true,
                        keep_unknown: true,
                    },
                ),
                step(
                    3,
                    Operation::IpFilter {
                        family: AddressFamily::V4,
                        keep_domains: false,
                    },
                ),
                step(4, Operation::Reverse),
                step(5, Operation::Limit { count: 1 }),
            ],
        )
        .unwrap();
        assert_eq!(out.retained, vec![2]);
        assert_eq!(
            out.stages.iter().map(|s| s.after).collect::<Vec<_>>(),
            [3, 2, 1, 1, 1]
        );
    }

    #[test]
    fn host_filter_cascades_dependencies_and_reverse_changes_limit_choice() {
        let source = "Base = socks5, base.example, 1080\nChild = socks5, child.example, 1080, underlying-proxy=Base\nLast = socks5, last.example, 1080\n";
        let out = run(
            source,
            vec![step(
                1,
                Operation::HostFilter {
                    pattern: "^base\\.".into(),
                    keep: false,
                },
            )],
        )
        .unwrap();
        assert_eq!(out.retained, vec![2]);
        let out = run(
            source,
            vec![
                step(1, Operation::Reverse),
                step(2, Operation::Limit { count: 1 }),
            ],
        )
        .unwrap();
        assert_eq!(out.retained, vec![2]);
    }

    #[test]
    fn properties_report_actual_changes_and_preserve_unsupported_nodes() {
        let source =
            "A = socks5, node.example, 1080\nB = external, exec=synthetic, local-port=1234\n";
        let out = run(
            source,
            vec![
                step(
                    1,
                    Operation::SetProperty {
                        setting: properties::Setting::TcpFastOpen { enabled: true },
                    },
                ),
                step(
                    2,
                    Operation::SetProperty {
                        setting: properties::Setting::TcpFastOpen { enabled: true },
                    },
                ),
            ],
        )
        .unwrap();
        assert_eq!((out.stages[0].modified, out.stages[0].skipped), (1, 1));
        assert_eq!((out.stages[1].modified, out.stages[1].skipped), (0, 1));
        assert!(out.content.contains("tfo=true"));
        let reparsed = Subscription::parse(&out.content).unwrap();
        let NodeSource::Surge(node) = &reparsed.nodes()[0].source else {
            panic!("expected Surge node");
        };
        assert!(
            node.fields
                .iter()
                .any(|field| field.key == "tfo" && field.value.as_str() == "true")
        );
        assert!(out.content.contains("B = external, exec=synthetic"));
    }

    #[test]
    fn rejects_invalid_filter_semantics_before_processing() {
        for operation in [
            Operation::PortFilter {
                start: 443,
                end: 80,
                keep: true,
            },
            Operation::RegionFilter {
                regions: vec!["Japan".into()],
                keep: true,
                keep_unknown: true,
            },
            Operation::IpFilter {
                family: AddressFamily::PreferV4,
                keep_domains: true,
            },
            Operation::SetProperty {
                setting: properties::Setting::Alpn { values: vec![] },
            },
        ] {
            assert!(matches!(
                validate(&[step(1, operation)]),
                Err(ModelError::InvalidEdit)
            ));
        }
    }

    #[test]
    fn deduplication_does_not_merge_nodes_with_distinct_mutations() {
        let source = r#"[{"type":"socks","tag":"a","server":"node.example","server_port":1080},{"type":"socks","tag":"b","server":"node.example","server_port":1080}]"#;
        let sub = Subscription::parse(source).unwrap();
        let mut edits = editor::Options::default();
        edits.pipeline = vec![
            step(
                1,
                Operation::ResolveDomain {
                    family: AddressFamily::V4,
                },
            ),
            step(2, Operation::Deduplicate),
        ];
        let resolver = |_: &[&str], _: AddressFamily| {
            Ok(vec![
                Some("1.1.1.1".parse().unwrap()),
                Some("8.8.8.8".parse().unwrap()),
            ])
        };
        let result = sub
            .render_processed_with_resolver(
                &[Health::Unknown; 2],
                &EnhanceOptions {
                    rename: false,
                    sort: false,
                },
                &edits,
                &resolver,
            )
            .unwrap();
        assert_eq!(result.retained, [0, 1]);
        assert_eq!(result.duplicates, 0);
    }
    fn step(id: u64, operation: Operation) -> Step {
        Step {
            id,
            enabled: true,
            operation,
        }
    }
    fn run(input: &str, pipeline: Vec<Step>) -> Result<editor::Rendered, ModelError> {
        let sub = Subscription::parse(input)?;
        sub.render_processed(
            &vec![Health::Unknown; sub.nodes().len()],
            &EnhanceOptions {
                rename: false,
                sort: false,
            },
            &editor::Options {
                pipeline,
                include: String::new(),
                exclude: String::new(),
                find: String::new(),
                replace: String::new(),
                nodes: Vec::new(),
                deduplicate: false,
            },
        )
    }
    #[test]
    fn ordered_operations_and_reports() {
        let input = "[{\"type\":\"http\",\"tag\":\"alpha\",\"server\":\"a\",\"server_port\":80},{\"type\":\"http\",\"tag\":\"beta\",\"server\":\"b\",\"server_port\":80}]";
        let out = run(
            input,
            vec![
                step(
                    1,
                    Operation::ReplaceName {
                        pattern: "alpha".into(),
                        replacement: "zeta".into(),
                        regex: false,
                    },
                ),
                step(
                    2,
                    Operation::NameFilter {
                        pattern: "zeta".into(),
                        keep: true,
                    },
                ),
            ],
        )
        .unwrap();
        assert_eq!(out.retained, vec![0]);
        assert_eq!(out.stages.len(), 2);
        assert_eq!(out.stages[0].renamed, 1);
        assert_eq!(out.stages[1].after, 1);
        assert!(out.content.contains("zeta"));
    }
    #[test]
    fn sort_before_dedup_selects_first() {
        let input = "[{\"type\":\"http\",\"tag\":\"z\",\"server\":\"same\",\"server_port\":80},{\"type\":\"http\",\"tag\":\"a\",\"server\":\"same\",\"server_port\":80}]";
        let out = run(
            input,
            vec![
                step(
                    1,
                    Operation::Sort {
                        key: SortKey::Name,
                        descending: false,
                    },
                ),
                step(2, Operation::Deduplicate),
            ],
        )
        .unwrap();
        assert_eq!(out.retained, vec![1]);
        assert_eq!(out.duplicates, 1);
        assert_eq!(out.stages[0].renamed, 0);
    }
    #[test]
    fn dependency_rename_and_removal_cascade() {
        let input = "[{\"type\":\"http\",\"tag\":\"base\",\"server\":\"a\",\"server_port\":80},{\"type\":\"http\",\"tag\":\"chain\",\"server\":\"b\",\"server_port\":80,\"detour\":\"base\"}]";
        let out = run(
            input,
            vec![step(
                1,
                Operation::ReplaceName {
                    pattern: "base".into(),
                    replacement: "renamed".into(),
                    regex: false,
                },
            )],
        )
        .unwrap();
        assert!(out.content.contains("\"detour\": \"renamed\""));
        let out = run(
            input,
            vec![step(
                1,
                Operation::NameFilter {
                    pattern: "base".into(),
                    keep: false,
                },
            )],
        )
        .unwrap();
        assert!(out.retained.is_empty());
        assert_eq!(out.excluded, 2);
    }
    #[test]
    fn exact_noop_and_invalid_steps() {
        let input = "[{\"type\":\"http\",\"tag\":\"a\",\"server\":\"a\",\"server_port\":80}]";
        let mut disabled = step(1, Operation::Flags { add: true });
        disabled.enabled = false;
        assert_eq!(run(input, vec![disabled]).unwrap().content.as_str(), input);
        assert!(
            run(
                input,
                vec![step(
                    1,
                    Operation::FormatName {
                        template: "{bad}".into()
                    }
                )]
            )
            .is_err()
        );
        assert!(
            validate(
                &(0..65)
                    .map(|id| step(id, Operation::Deduplicate))
                    .collect::<Vec<_>>()
            )
            .is_err()
        );
        assert!(
            validate(&[step(
                1,
                Operation::NameFilter {
                    pattern: String::new(),
                    keep: true
                }
            )])
            .is_err()
        );
        assert!(
            validate(&[step(
                1,
                Operation::ProtocolFilter {
                    protocols: vec![],
                    keep: true
                }
            )])
            .is_err()
        );
        assert!(validate(&[step(1, Operation::RegexSort { patterns: vec![] })]).is_err());
        assert!(
            validate(&[step(
                1,
                Operation::FormatName {
                    template: String::new()
                }
            )])
            .is_err()
        );
        let mut unfinished = step(
            1,
            Operation::NameFilter {
                pattern: "[".into(),
                keep: true,
            },
        );
        unfinished.enabled = false;
        assert_eq!(
            run(input, vec![unfinished]).unwrap().content.as_str(),
            input
        );
    }
    #[test]
    fn protocol_aliases_match() {
        let input = "[{\"type\":\"socks\",\"tag\":\"a\",\"server\":\"a\",\"server_port\":80}]";
        let out = run(
            input,
            vec![step(
                1,
                Operation::ProtocolFilter {
                    protocols: vec!["socks5".into()],
                    keep: true,
                },
            )],
        )
        .unwrap();
        assert_eq!(out.retained, vec![0]);
    }

    #[test]
    fn regex_priority_sort_keeps_the_input_order_inside_each_group() {
        let source = "US A = http, us.invalid, 80\nJP Z = http, jp1.invalid, 80\nJP A = http, jp2.invalid, 80\nOther = http, other.invalid, 80\n";
        let out = run(
            source,
            vec![step(
                1,
                Operation::RegexSort {
                    patterns: vec!["^JP".into(), "^US".into()],
                },
            )],
        )
        .unwrap();
        assert_eq!(out.retained, [1, 2, 0, 3]);
        assert_eq!((out.stages[0].before, out.stages[0].after), (4, 4));
    }

    #[test]
    fn local_flag_addition_and_removal_cancel_without_changing_source_bytes() {
        let source = "JP Tokyo = http, node.invalid, 80\n";
        let added = run(source, vec![step(1, Operation::Flags { add: true })]).unwrap();
        let parsed = Subscription::parse(&added.content).unwrap();
        assert_eq!(parsed.nodes()[0].name(), "🇯🇵 JP Tokyo");
        let restored = run(
            source,
            vec![
                step(1, Operation::Flags { add: true }),
                step(2, Operation::Flags { add: false }),
            ],
        )
        .unwrap();
        assert_eq!(restored.content.as_str(), source);
        assert_eq!(
            restored
                .stages
                .iter()
                .map(|stage| stage.renamed)
                .collect::<Vec<_>>(),
            [1, 1]
        );
    }

    #[test]
    fn repeated_deduplication_counts_each_unsupported_identity_once() {
        let source = "trojan://synthetic@node.invalid:443#A\n";
        let out = run(
            source,
            vec![
                step(1, Operation::Deduplicate),
                step(2, Operation::Deduplicate),
            ],
        )
        .unwrap();
        assert_eq!((out.retained.len(), out.duplicate_skipped), (1, 1));
        assert_eq!(out.content.as_str(), source);
    }

    #[test]
    fn a_colliding_name_template_is_rejected_without_mutating_the_source() {
        let source = "A = http, one.invalid, 80\nB = http, two.invalid, 80\n";
        assert!(matches!(
            run(
                source,
                vec![step(
                    1,
                    Operation::FormatName {
                        template: "Same".into()
                    }
                )]
            ),
            Err(ModelError::InvalidEdit)
        ));
        assert_eq!(Subscription::parse(source).unwrap().nodes()[0].name(), "A");
    }
    #[test]
    fn disabled_incomplete_pipeline_preserves_ambiguous_surge_exactly() {
        let source = "\"Base\\nA\" = socks5, base.example.invalid, 1080\n";
        let mut unfinished = step(
            1,
            Operation::NameFilter {
                pattern: "[".into(),
                keep: true,
            },
        );
        unfinished.enabled = false;
        let out = run(source, vec![unfinished]).unwrap();
        assert_eq!(out.content.as_str(), source);
        assert!(out.stages.is_empty());
    }
    #[test]
    fn uri_and_yaml_survive_processing() {
        let uri = "trojan://secret@example.com:443#alpha\n";
        let out = run(
            uri,
            vec![step(
                1,
                Operation::ReplaceName {
                    pattern: "alpha".into(),
                    replacement: "beta".into(),
                    regex: false,
                },
            )],
        )
        .unwrap();
        assert!(Subscription::parse(&out.content).is_ok());
        assert!(out.content.contains("beta"));
        let yaml =
            "proxies:\n  - name: alpha\n    type: http\n    server: example.com\n    port: 80\n";
        let out = run(
            yaml,
            vec![step(
                1,
                Operation::FormatName {
                    template: "{index}-{name}".into(),
                },
            )],
        )
        .unwrap();
        assert!(Subscription::parse(&out.content).is_ok());
        assert!(out.content.contains("1-alpha"));
    }
}
