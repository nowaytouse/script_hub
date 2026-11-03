//! Ordered subscription transformations with explicitly injected network operations.
use super::{
    MAX_CONTENT, ModelError, NodeIdentity, NodeSource, PrivateJson, Subscription, dns,
    local_region, properties, uncertain_identity, uncertain_token,
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
        #[serde(default)]
        options: dns::Options,
    },
    SetProperty {
        setting: properties::Setting,
    },
    ConditionFilter {
        conditions: Vec<Condition>,
        mode: MatchMode,
        keep: bool,
    },
    NormalizeName {
        case: NameCase,
        trim: bool,
        collapse_whitespace: bool,
    },
    UniqueNames {
        separator: String,
    },
    RegionNames {
        labels: Vec<RegionLabel>,
        separator: String,
        keep_name: bool,
    },
    Shuffle {
        seed: u64,
    },
    BatchRename {
        rules: Vec<RenameRule>,
    },
    Slice {
        skip: usize,
        count: usize,
    },
    DeduplicateName {
        keep: KeepOccurrence,
        ignore_case: bool,
    },
    NetworkFilter {
        networks: Vec<String>,
        keep: bool,
        keep_domains: bool,
    },
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct RenameRule {
    pub pattern: String,
    pub replacement: String,
    pub regex: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum KeepOccurrence {
    First,
    Last,
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub field: Field,
    pub matcher: TextMatch,
    pub value: String,
    pub negate: bool,
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub struct RegionLabel {
    pub region: String,
    pub label: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Name,
    Server,
    Protocol,
    Port,
    Region,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum TextMatch {
    Equals,
    Contains,
    Regex,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    All,
    Any,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum NameCase {
    Preserve,
    Lower,
    Upper,
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
    Dnssec,
    InvalidOptions,
}

pub type Resolver<'a> =
    dyn Fn(&[&str], AddressFamily, &dns::Options) -> Result<dns::Batch, ResolveError> + 'a;

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
    NaturalName,
    Server,
    Port,
}

#[derive(Clone)]
pub struct StageReport {
    pub id: u64,
    pub before: usize,
    pub after: usize,
    pub renamed: usize,
    pub modified: usize,
    pub skipped: usize,
    pub resolution: Option<dns::Report>,
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
                if !matches!(
                    token,
                    "{name}"
                        | "{protocol}"
                        | "{region}"
                        | "{index}"
                        | "{server}"
                        | "{port}"
                        | "{source_index}"
                        | "{index:02}"
                        | "{index:03}"
                        | "{index:04}"
                        | "{index:05}"
                        | "{index:06}"
                ) {
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
            Operation::BatchRename { rules } => {
                if rules.len() > 32
                    || rules
                        .iter()
                        .any(|r| !valid_text(&r.pattern) || !valid_text(&r.replacement))
                    || step.enabled
                        && (rules.is_empty() || rules.iter().any(|r| r.pattern.is_empty()))
                {
                    return Err(ModelError::InvalidEdit);
                }
                if step.enabled {
                    for rule in rules.iter().filter(|r| r.regex) {
                        regex(&rule.pattern)?;
                    }
                }
            }
            Operation::Slice { skip, count } => {
                if *skip > 100_000 || *count > 100_000 || skip.checked_add(*count).is_none() {
                    return Err(ModelError::InvalidEdit);
                }
            }
            Operation::DeduplicateName { .. } => {}
            Operation::NetworkFilter { networks, .. } => {
                if networks.len() > 64
                    || networks.iter().any(|n| !valid_text(n))
                    || step.enabled
                        && (networks.is_empty()
                            || networks.iter().any(|n| n.parse::<ipnet::IpNet>().is_err()))
                {
                    return Err(ModelError::InvalidEdit);
                }
            }
            Operation::ConditionFilter { conditions, .. } => {
                if conditions.len() > 32
                    || conditions.iter().any(|c| !valid_text(&c.value))
                    || step.enabled
                        && (conditions.is_empty() || conditions.iter().any(|c| c.value.is_empty()))
                {
                    return Err(ModelError::InvalidEdit);
                }
                if step.enabled {
                    for condition in conditions {
                        if condition.matcher == TextMatch::Regex {
                            regex(&condition.value)?;
                        }
                    }
                }
            }
            Operation::UniqueNames { separator } => {
                if !valid_text(separator) {
                    return Err(ModelError::InvalidEdit);
                }
            }
            Operation::RegionNames {
                labels, separator, ..
            } => {
                if labels.len() > 64
                    || !valid_text(separator)
                    || labels
                        .iter()
                        .any(|l| !valid_text(&l.region) || !valid_text(&l.label))
                {
                    return Err(ModelError::InvalidEdit);
                }
                if step.enabled {
                    let mut codes = HashSet::new();
                    if labels.is_empty()
                        || labels.iter().any(|l| {
                            l.region.len() != 2
                                || !l.region.bytes().all(|b| b.is_ascii_alphabetic())
                                || l.label.is_empty()
                                || !codes.insert(l.region.to_ascii_uppercase())
                        })
                    {
                        return Err(ModelError::InvalidEdit);
                    }
                }
            }
            Operation::NormalizeName { .. } | Operation::Shuffle { .. } => {}
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
            Operation::ResolveDomain { options, .. } => dns::validate(options, step.enabled)?,
            Operation::Flags { .. }
            | Operation::Sort { .. }
            | Operation::Deduplicate
            | Operation::PortFilter { .. }
            | Operation::IpFilter { .. }
            | Operation::Limit { .. }
            | Operation::Reverse => {}
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

fn natural_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (a, b) = (left.as_bytes(), right.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let (mut ie, mut je) = (i, j);
            while ie < a.len() && a[ie].is_ascii_digit() {
                ie += 1;
            }
            while je < b.len() && b[je].is_ascii_digit() {
                je += 1;
            }
            let mut ai = i;
            let mut bj = j;
            while ai < ie && a[ai] == b'0' {
                ai += 1;
            }
            while bj < je && b[bj] == b'0' {
                bj += 1;
            }
            let cmp = (ie - ai)
                .cmp(&(je - bj))
                .then_with(|| a[ai..ie].cmp(&b[bj..je]));
            if cmp != Ordering::Equal {
                return cmp;
            }
            i = ie;
            j = je;
        } else {
            let cmp = a[i].cmp(&b[j]);
            if cmp != Ordering::Equal {
                return cmp;
            }
            i += 1;
            j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j))
}

fn next_shuffle(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
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
        let mut resolution = None;
        let mut name_bytes = order.iter().map(|&i| names[i].len()).sum();
        let mut remove = vec![false; names.len()];
        let mut duplicate = vec![false; names.len()];
        match &step.operation {
            Operation::BatchRename { rules } => {
                let compiled: Vec<_> = rules
                    .iter()
                    .map(|r| {
                        if r.regex {
                            regex(&r.pattern).map(Some)
                        } else {
                            Ok(None)
                        }
                    })
                    .collect::<Result<_, ModelError>>()?;
                for &i in order.iter() {
                    let mut value = names[i].clone();
                    for (rule, re) in rules.iter().zip(&compiled) {
                        value = Zeroizing::new(if let Some(re) = re {
                            re.replace_all(&value, rule.replacement.as_str())
                                .into_owned()
                        } else {
                            value.replace(&rule.pattern, &rule.replacement)
                        });
                        if !valid_text(&value) {
                            return Err(ModelError::InvalidEdit);
                        }
                    }
                    renamed += usize::from(set_name(names, i, value, &mut name_bytes)?);
                }
            }
            Operation::Slice { skip, count } => {
                let end = skip.checked_add(*count).ok_or(ModelError::InvalidEdit)?;
                for (pos, &i) in order.iter().enumerate() {
                    if pos < *skip || pos >= end {
                        if dependencies[i].is_some() || !dependents[i].is_empty() {
                            skipped += 1;
                        } else {
                            remove[i] = true;
                        }
                    }
                }
            }
            Operation::DeduplicateName { keep, ignore_case } => {
                let key_for = |i: usize| {
                    NodeIdentity(Zeroizing::new(if *ignore_case {
                        names[i].to_lowercase()
                    } else {
                        names[i].to_string()
                    }))
                };
                let mut groups = std::collections::HashMap::new();
                for &i in order.iter() {
                    let protected = dependencies[i].is_some() || !dependents[i].is_empty();
                    let group = groups.entry(key_for(i)).or_insert((i, false));
                    if *keep == KeepOccurrence::Last {
                        group.0 = i;
                    }
                    group.1 |= protected;
                }
                for &i in order.iter() {
                    let (preferred, has_protected) = groups[&key_for(i)];
                    let protected = dependencies[i].is_some() || !dependents[i].is_empty();
                    if protected {
                        if i != preferred {
                            skipped += 1;
                        }
                    } else if has_protected || i != preferred {
                        remove[i] = true;
                        duplicate[i] = true;
                        counts.duplicates += 1;
                    }
                }
            }
            Operation::NetworkFilter {
                networks,
                keep,
                keep_domains,
            } => {
                let parsed: Vec<ipnet::IpNet> = networks
                    .iter()
                    .map(|n| n.parse().map_err(|_| ModelError::InvalidEdit))
                    .collect::<Result<_, _>>()?;
                for &i in order.iter() {
                    let host = mutations[i]
                        .address
                        .as_deref()
                        .unwrap_or(sub.nodes[i].server());
                    remove[i] = match host.parse::<IpAddr>() {
                        Ok(ip) => parsed.iter().any(|n| n.contains(&ip)) != *keep,
                        Err(_) => !*keep_domains,
                    };
                }
            }
            Operation::ConditionFilter {
                conditions,
                mode,
                keep,
            } => {
                let compiled: Vec<_> = conditions
                    .iter()
                    .map(|condition| {
                        if condition.matcher == TextMatch::Regex {
                            regex(&condition.value).map(Some)
                        } else {
                            Ok(None)
                        }
                    })
                    .collect::<Result<_, ModelError>>()?;
                let needs_port = conditions
                    .iter()
                    .any(|condition| condition.field == Field::Port);
                let needs_region = conditions
                    .iter()
                    .any(|condition| condition.field == Field::Region);
                for &i in order.iter() {
                    let port = if needs_port {
                        mutations[i].port.unwrap_or(sub.nodes[i].port).to_string()
                    } else {
                        String::new()
                    };
                    let region = if needs_region {
                        local_region(&names[i])
                            .map(|r| String::from_utf8_lossy(&r).into_owned())
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    let mut matches = conditions.iter().zip(&compiled).map(|(condition, re)| {
                        let actual = match condition.field {
                            Field::Name => names[i].as_str(),
                            Field::Server => mutations[i]
                                .address
                                .as_deref()
                                .unwrap_or(sub.nodes[i].server()),
                            Field::Protocol => protocol(sub.nodes[i].protocol()),
                            Field::Port => &port,
                            Field::Region => &region,
                        };
                        let expected = if condition.field == Field::Protocol
                            && condition.matcher == TextMatch::Equals
                        {
                            protocol(&condition.value)
                        } else {
                            &condition.value
                        };
                        let found = match condition.matcher {
                            TextMatch::Equals if condition.field == Field::Protocol => {
                                actual.eq_ignore_ascii_case(expected)
                            }
                            TextMatch::Equals => actual == expected,
                            TextMatch::Contains => actual.contains(expected),
                            TextMatch::Regex => re.as_ref().is_some_and(|r| r.is_match(actual)),
                        };
                        found != condition.negate
                    });
                    let matched = match mode {
                        MatchMode::All => matches.all(|v| v),
                        MatchMode::Any => matches.any(|v| v),
                    };
                    remove[i] = matched != *keep;
                }
            }
            Operation::NormalizeName {
                case,
                trim,
                collapse_whitespace,
            } => {
                for &i in order.iter() {
                    let input = if *trim {
                        names[i].trim()
                    } else {
                        names[i].as_str()
                    };
                    let input = Zeroizing::new(match case {
                        NameCase::Preserve => input.to_owned(),
                        NameCase::Lower => input.to_lowercase(),
                        NameCase::Upper => input.to_uppercase(),
                    });
                    let mut output = Zeroizing::new(String::new());
                    let mut whitespace = false;
                    for character in input.chars() {
                        if *collapse_whitespace && character.is_whitespace() {
                            if !whitespace {
                                output.push(' ');
                            }
                            whitespace = true;
                        } else {
                            output.push(character);
                            whitespace = false;
                        }
                    }
                    renamed += usize::from(set_name(names, i, output, &mut name_bytes)?);
                }
            }
            Operation::UniqueNames { separator } => {
                let mut reserved: HashSet<_> = order
                    .iter()
                    .map(|&i| NodeIdentity(names[i].clone()))
                    .collect();
                let mut seen = HashSet::new();
                let mut counters = std::collections::HashMap::new();
                for &i in order.iter() {
                    let base = names[i].clone();
                    if seen.insert(NodeIdentity(base.clone())) {
                        continue;
                    }
                    let next = counters.entry(NodeIdentity(base.clone())).or_insert(2usize);
                    loop {
                        let candidate =
                            Zeroizing::new(format!("{}{}{}", base.as_str(), separator, *next));
                        *next = next.checked_add(1).ok_or(ModelError::InvalidEdit)?;
                        if !reserved.contains(&NodeIdentity(candidate.clone())) {
                            reserved.insert(NodeIdentity(candidate.clone()));
                            renamed += usize::from(set_name(names, i, candidate, &mut name_bytes)?);
                            break;
                        }
                    }
                }
            }
            Operation::RegionNames {
                labels,
                separator,
                keep_name,
            } => {
                for &i in order.iter() {
                    let Some(region) = local_region(&names[i]) else {
                        continue;
                    };
                    let Some(label) = labels
                        .iter()
                        .find(|l| l.region.as_bytes().eq_ignore_ascii_case(&region))
                    else {
                        continue;
                    };
                    let value = if *keep_name {
                        format!("{}{}{}", label.label, separator, names[i].as_str())
                    } else {
                        label.label.clone()
                    };
                    renamed +=
                        usize::from(set_name(names, i, Zeroizing::new(value), &mut name_bytes)?);
                }
            }
            Operation::Shuffle { seed } => {
                let mut state = *seed;
                for i in (1..order.len()).rev() {
                    let bound = i as u64 + 1;
                    let threshold = bound.wrapping_neg() % bound;
                    let mut sample = next_shuffle(&mut state);
                    while sample < threshold {
                        sample = next_shuffle(&mut state);
                    }
                    let j = (sample % bound) as usize;
                    order.swap(i, j);
                }
            }
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
                    remove[i] = (*start..=*end)
                        .contains(&mutations[i].port.unwrap_or(sub.nodes[i].port))
                        != *keep;
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
            Operation::ResolveDomain { family, options } => {
                resolution = Some(dns::Report::default());
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
                    let batch =
                        resolver(&hosts, *family, options).map_err(ModelError::Resolution)?;
                    if batch.addresses.len() != indices.len() {
                        return Err(ModelError::Resolution(ResolveError::InvalidResponse));
                    }
                    resolution = Some(batch.report);
                    for (i, address) in indices.into_iter().zip(batch.addresses) {
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
                        } else if let Some(width) = part
                            .strip_prefix("{index:0")
                            .and_then(|p| p.strip_suffix('}'))
                        {
                            let width: usize =
                                width.parse().map_err(|_| ModelError::InvalidEdit)?;
                            result.push_str(&format!("{:0width$}", pos + 1));
                        } else if *part == "{source_index}" {
                            result.push_str(&(i + 1).to_string());
                        } else if *part == "{port}" {
                            result.push_str(
                                &mutations[i].port.unwrap_or(sub.nodes[i].port).to_string(),
                            );
                        } else if *part == "{server}" {
                            result.push_str(
                                mutations[i]
                                    .address
                                    .as_deref()
                                    .unwrap_or(sub.nodes[i].server()),
                            );
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
                        SortKey::NaturalName => natural_cmp(&names[a], &names[b]),
                        SortKey::Server => mutations[a]
                            .address
                            .as_deref()
                            .unwrap_or(sub.nodes[a].server())
                            .cmp(
                                mutations[b]
                                    .address
                                    .as_deref()
                                    .unwrap_or(sub.nodes[b].server()),
                            ),
                        SortKey::Port => mutations[a]
                            .port
                            .unwrap_or(sub.nodes[a].port)
                            .cmp(&mutations[b].port.unwrap_or(sub.nodes[b].port)),
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
            resolution,
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
    fn batch_rename_is_ordered_and_preserves_unknown_node_fields() {
        let input =
            r#"[{"type":"socks","tag":"node12","server":"a","server_port":1,"custom":{"x":1}}]"#;
        let result = run(
            input,
            vec![step(
                1,
                Operation::BatchRename {
                    rules: vec![
                        RenameRule {
                            pattern: r"node(\d+)".into(),
                            replacement: "N$1".into(),
                            regex: true,
                        },
                        RenameRule {
                            pattern: "N".into(),
                            replacement: "Node ".into(),
                            regex: false,
                        },
                    ],
                },
            )],
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(value[0]["tag"], "Node 12");
        assert_eq!(value[0]["custom"]["x"], 1);
        assert_eq!(result.stages[0].renamed, 1);
    }

    #[test]
    fn expanded_template_uses_effective_fields_and_both_indices() {
        let input = r#"[{"type":"socks","tag":"A","server":"a","server_port":1},{"type":"socks","tag":"B","server":"b","server_port":2}]"#;
        let result = run(
            input,
            vec![
                step(1, Operation::Reverse),
                step(
                    2,
                    Operation::SetProperty {
                        setting: properties::Setting::Port { value: 42 },
                    },
                ),
                step(
                    3,
                    Operation::FormatName {
                        template: "{server}:{port}-{source_index}-{index:03}".into(),
                    },
                ),
            ],
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(value[0]["tag"], "b:42-2-001");
        assert_eq!(value[1]["tag"], "a:42-1-002");
    }

    #[test]
    fn network_filter_is_offline_and_domains_are_independent() {
        let input = r#"[{"type":"socks","tag":"A","server":"192.0.2.4","server_port":1},{"type":"socks","tag":"B","server":"2001:db8::1","server_port":2},{"type":"socks","tag":"C","server":"example.invalid","server_port":3}]"#;
        let result = run(
            input,
            vec![step(
                1,
                Operation::NetworkFilter {
                    networks: vec!["192.0.2.0/24".into(), "2001:db8::/32".into()],
                    keep: false,
                    keep_domains: true,
                },
            )],
        )
        .unwrap();
        assert_eq!(result.retained, vec![2]);
    }

    #[test]
    fn name_dedup_last_uses_effective_order_and_unicode_case() {
        let input = r#"[{"type":"socks","tag":"Ä","server":"a","server_port":1},{"type":"socks","tag":"ä","server":"b","server_port":2},{"type":"socks","tag":"C","server":"c","server_port":3}]"#;
        let result = run(
            input,
            vec![
                step(1, Operation::Reverse),
                step(
                    2,
                    Operation::DeduplicateName {
                        keep: KeepOccurrence::Last,
                        ignore_case: true,
                    },
                ),
            ],
        )
        .unwrap();
        assert_eq!(result.retained, vec![2, 0]);
    }

    #[test]
    fn new_operations_validate_drafts_and_bounds() {
        let invalid = Operation::NetworkFilter {
            networks: vec!["bad".into()],
            keep: true,
            keep_domains: true,
        };
        assert!(validate(&[step(1, invalid.clone())]).is_err());
        let mut draft = step(1, invalid);
        draft.enabled = false;
        assert!(validate(&[draft]).is_ok());
        assert!(
            validate(&[step(
                1,
                Operation::Slice {
                    skip: 100_000,
                    count: 100_001
                }
            )])
            .is_err()
        );
        assert!(validate(&[step(1, Operation::BatchRename { rules: vec![] })]).is_err());
        assert!(
            serde_json::from_str::<Operation>(
                r#"{"type":"deduplicate_name","keep":"last","ignore_case":true}"#
            )
            .is_ok()
        );
    }

    #[test]
    fn slice_protects_dependencies_without_cascading() {
        let input = "Base = socks5, base.example, 1080\nChild = socks5, child.example, 1080, underlying-proxy=Base\nLast = socks5, last.example, 1080\n";
        let result = run(input, vec![step(1, Operation::Slice { skip: 2, count: 1 })]).unwrap();
        assert_eq!(result.retained, vec![0, 1, 2]);
        assert_eq!(result.stages[0].skipped, 2);
    }

    #[test]
    fn name_dedup_prioritizes_dependencies_in_either_order() {
        let input = "Base = socks5, base.example, 1080\nChild = socks5, child.example, 1080, underlying-proxy=Base\nbase = socks5, other.example, 1080\n";
        for (reverse, keep, expected_skipped) in [
            (false, KeepOccurrence::First, 0),
            (false, KeepOccurrence::Last, 1),
            (true, KeepOccurrence::First, 1),
            (true, KeepOccurrence::Last, 0),
        ] {
            let mut steps = Vec::new();
            if reverse {
                steps.push(step(1, Operation::Reverse));
            }
            steps.push(step(
                2,
                Operation::DeduplicateName {
                    keep,
                    ignore_case: true,
                },
            ));
            let result = run(input, steps).unwrap();
            assert_eq!(
                result.retained,
                if reverse { vec![1, 0] } else { vec![0, 1] }
            );
            assert_eq!(result.stages.last().unwrap().skipped, expected_skipped);
            assert_eq!(result.duplicates, 1);
            assert_eq!(result.excluded, 0);
        }
    }

    #[test]
    fn name_dedup_uses_prior_renames_and_counts_ordinary_duplicates() {
        let input = "Base = socks5, base.example, 1080\nChild = socks5, child.example, 1080, underlying-proxy=Base\nOther = socks5, other.example, 1080\n";
        let result = run(
            input,
            vec![
                step(
                    1,
                    Operation::ReplaceName {
                        pattern: "Other".into(),
                        replacement: "base".into(),
                        regex: false,
                    },
                ),
                step(
                    2,
                    Operation::DeduplicateName {
                        keep: KeepOccurrence::Last,
                        ignore_case: true,
                    },
                ),
            ],
        )
        .unwrap();
        assert_eq!(result.retained, vec![0, 1]);
        assert_eq!(result.stages[1].skipped, 1);
        assert_eq!(result.duplicates, 1);
        assert_eq!(result.excluded, 0);
    }

    #[test]
    fn batch_rejects_oversized_intermediate_even_if_later_shortened() {
        let input = r#"[{"type":"socks","tag":"A","server":"a","server_port":1}]"#;
        let operation = Operation::BatchRename {
            rules: vec![
                RenameRule {
                    pattern: "A".into(),
                    replacement: "x".repeat(512),
                    regex: false,
                },
                RenameRule {
                    pattern: "x".into(),
                    replacement: "xx".into(),
                    regex: false,
                },
                RenameRule {
                    pattern: "x".into(),
                    replacement: "".into(),
                    regex: false,
                },
            ],
        };
        assert!(run(input, vec![step(1, operation)]).is_err());
    }

    #[test]
    fn template_and_batch_preserve_uri_and_surge_extra_fields() {
        let operation = || Operation::BatchRename {
            rules: vec![RenameRule {
                pattern: "old".into(),
                replacement: "new".into(),
                regex: false,
            }],
        };
        let surge = run(
            "old = socks5, a.example, 1080, custom-option=opaque\n",
            vec![step(1, operation())],
        )
        .unwrap();
        let parsed = Subscription::parse(&surge.content).unwrap();
        assert_eq!(parsed.nodes[0].name(), "new");
        assert!(
            surge
                .content
                .ends_with(" = socks5, a.example, 1080, custom-option=opaque\n")
        );
        let uri = run(
            "trojan://secret@example.com:443?custom=opaque#old\n",
            vec![step(1, operation())],
        )
        .unwrap();
        assert!(uri.content.contains("custom=opaque"));
        assert!(uri.content.contains("#new"));
        let yaml = "proxies:\n  - name: old\n    type: http\n    server: example.com\n    port: 80\n    custom: opaque\n";
        let out = run(yaml, vec![step(1, operation())]).unwrap();
        assert!(out.content.contains("new"));
        assert!(out.content.contains("custom: opaque"));
        assert!(Subscription::parse(&out.content).is_ok());
    }

    #[test]
    fn network_filter_uses_effective_address_and_domains_can_be_excluded() {
        let input = r#"[{"type":"socks","tag":"A","server":"example.invalid","server_port":1}]"#;
        let out = run(
            input,
            vec![
                step(
                    1,
                    Operation::SetProperty {
                        setting: properties::Setting::Server {
                            value: "192.0.2.42".into(),
                        },
                    },
                ),
                step(
                    2,
                    Operation::NetworkFilter {
                        networks: vec!["192.0.2.0/24".into()],
                        keep: true,
                        keep_domains: false,
                    },
                ),
            ],
        )
        .unwrap();
        assert_eq!(out.retained, vec![0]);
        let out = run(
            input,
            vec![step(
                1,
                Operation::NetworkFilter {
                    networks: vec!["192.0.2.0/24".into()],
                    keep: false,
                    keep_domains: false,
                },
            )],
        )
        .unwrap();
        assert!(out.retained.is_empty());
    }

    #[test]
    fn new_operation_limits_and_serde_are_enforced() {
        let rule = RenameRule {
            pattern: "x".into(),
            replacement: "y".into(),
            regex: false,
        };
        assert!(
            validate(&[step(
                1,
                Operation::BatchRename {
                    rules: vec![rule; 33]
                }
            )])
            .is_err()
        );
        assert!(
            validate(&[step(
                1,
                Operation::NetworkFilter {
                    networks: vec!["0.0.0.0/0".into(); 65],
                    keep: true,
                    keep_domains: true
                }
            )])
            .is_err()
        );
        assert!(
            validate(&[step(
                1,
                Operation::Slice {
                    skip: usize::MAX,
                    count: 1
                }
            )])
            .is_err()
        );
        let json =
            r#"{"type":"batch_rename","rules":[{"pattern":"a","replacement":"b","regex":false}]}"#;
        let parsed: Operation = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
        assert!(
            serde_json::from_str::<Operation>(
                r#"{"type":"slice","skip":1,"count":2,"extra":true}"#
            )
            .is_err()
        );
    }

    #[test]
    fn normalize_and_unique_reserve_existing_names() {
        let input = r#"[{"type":"socks","tag":"A","server":"a","server_port":1},{"type":"socks","tag":"a","server":"b","server_port":2},{"type":"socks","tag":"a 2","server":"c","server_port":3}]"#;
        let operations = || {
            vec![
                step(
                    1,
                    Operation::NormalizeName {
                        case: NameCase::Lower,
                        trim: true,
                        collapse_whitespace: true,
                    },
                ),
                step(
                    2,
                    Operation::UniqueNames {
                        separator: " ".into(),
                    },
                ),
                step(
                    3,
                    Operation::NormalizeName {
                        case: NameCase::Lower,
                        trim: true,
                        collapse_whitespace: true,
                    },
                ),
            ]
        };
        let result = run(input, operations()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(value[0]["tag"], "a");
        assert_eq!(value[1]["tag"], "a 3");
        assert_eq!(value[2]["tag"], "a 2");
        assert_eq!(result.stages[2].renamed, 0);
    }

    #[test]
    fn conditions_match_region_unknown_and_protocol_aliases() {
        let input = r#"[{"type":"socks","tag":"[US] alpha","server":"a","server_port":10},{"type":"socks","tag":"plain","server":"b","server_port":20}]"#;
        let condition = |field, matcher, value: &str, negate| Condition {
            field,
            matcher,
            value: value.into(),
            negate,
        };
        let result = run(
            input,
            vec![step(
                1,
                Operation::ConditionFilter {
                    conditions: vec![
                        condition(Field::Protocol, TextMatch::Equals, "socks5", false),
                        condition(Field::Region, TextMatch::Regex, ".", true),
                    ],
                    mode: MatchMode::All,
                    keep: true,
                },
            )],
        )
        .unwrap();
        assert_eq!(result.retained, vec![1]);
    }

    #[test]
    fn region_names_and_seeded_shuffle_are_deterministic() {
        let input = r#"[{"type":"socks","tag":"[US] alpha","server":"a","server_port":1},{"type":"socks","tag":"[JP] beta","server":"b","server_port":2},{"type":"socks","tag":"plain","server":"c","server_port":3}]"#;
        let steps = || {
            vec![
                step(
                    1,
                    Operation::RegionNames {
                        labels: vec![RegionLabel {
                            region: "us".into(),
                            label: "America".into(),
                        }],
                        separator: " / ".into(),
                        keep_name: true,
                    },
                ),
                step(2, Operation::Shuffle { seed: 0 }),
            ]
        };
        let a = run(input, steps()).unwrap();
        let b = run(input, steps()).unwrap();
        assert_eq!(a.content.as_str(), b.content.as_str());
        assert!(a.content.contains("America / [US] alpha"));
        assert!(a.content.contains("plain"));
    }

    #[test]
    fn natural_sort_uses_unbounded_digits_and_stable_equal_numbers() {
        assert_eq!(natural_cmp("node2", "node10"), std::cmp::Ordering::Less);
        assert_eq!(natural_cmp("node0002", "node2"), std::cmp::Ordering::Equal);
        assert_eq!(
            natural_cmp("n99999999999999999999", "n100000000000000000000"),
            std::cmp::Ordering::Less
        );
        let input = r#"[{"type":"socks","tag":"node10","server":"a","server_port":1},{"type":"socks","tag":"node2","server":"b","server_port":2},{"type":"socks","tag":"node02","server":"c","server_port":3}]"#;
        let result = run(
            input,
            vec![step(
                1,
                Operation::Sort {
                    key: SortKey::NaturalName,
                    descending: false,
                },
            )],
        )
        .unwrap();
        assert_eq!(result.retained, vec![1, 2, 0]);
    }

    #[test]
    fn condition_validation_rejects_invalid_enabled_and_bounds() {
        let empty = step(
            1,
            Operation::ConditionFilter {
                conditions: vec![],
                mode: MatchMode::All,
                keep: true,
            },
        );
        assert!(matches!(validate(&[empty]), Err(ModelError::InvalidEdit)));
        let disabled = Step {
            id: 1,
            enabled: false,
            operation: Operation::ConditionFilter {
                conditions: vec![],
                mode: MatchMode::All,
                keep: true,
            },
        };
        assert!(validate(&[disabled]).is_ok());
        let labels = vec![
            RegionLabel {
                region: "us".into(),
                label: "A".into(),
            },
            RegionLabel {
                region: "US".into(),
                label: "B".into(),
            },
        ];
        assert!(matches!(
            validate(&[step(
                2,
                Operation::RegionNames {
                    labels,
                    separator: String::new(),
                    keep_name: false
                }
            )]),
            Err(ModelError::InvalidEdit)
        ));
    }

    #[test]
    fn condition_empty_values_rejected_only_when_enabled() {
        for matcher in [TextMatch::Equals, TextMatch::Contains, TextMatch::Regex] {
            let condition = Condition {
                field: Field::Name,
                matcher,
                value: String::new(),
                negate: false,
            };
            let operation = Operation::ConditionFilter {
                conditions: vec![condition],
                mode: MatchMode::All,
                keep: false,
            };
            let enabled = Step {
                id: 1,
                enabled: true,
                operation: operation.clone(),
            };
            let disabled = Step {
                id: 1,
                enabled: false,
                operation,
            };
            assert!(matches!(validate(&[enabled]), Err(ModelError::InvalidEdit)));
            assert!(validate(&[disabled]).is_ok());
        }
    }

    #[test]
    fn effective_address_and_port_drive_sorts_and_conditions() {
        let sub = Subscription::parse(r#"[{"type":"socks","tag":"a","server":"a.example","server_port":10},{"type":"socks","tag":"b","server":"b.example","server_port":20}]"#).unwrap();
        for key in [SortKey::Server, SortKey::Port] {
            let dependencies = vec![None, None];
            let dependents = vec![vec![], vec![]];
            let mut names = vec![Zeroizing::new("a".into()), Zeroizing::new("b".into())];
            let mut order = vec![0, 1];
            let mut mutations = vec![properties::Mutation::default(); 2];
            mutations[0].address = Some("z.example".into());
            mutations[0].port = Some(30);
            let mut working = Working {
                sub: &sub,
                dependencies: &dependencies,
                dependents: &dependents,
                names: &mut names,
                order: &mut order,
                mutations: &mut mutations,
                resolver: None,
            };
            apply(
                &mut working,
                &[step(
                    1,
                    Operation::Sort {
                        key,
                        descending: false,
                    },
                )],
            )
            .unwrap();
            assert_eq!(order, vec![1, 0]);
        }
        let dependencies = vec![None, None];
        let dependents = vec![vec![], vec![]];
        let mut names = vec![Zeroizing::new("a".into()), Zeroizing::new("b".into())];
        let mut order = vec![0, 1];
        let mut mutations = vec![properties::Mutation::default(); 2];
        mutations[0].address = Some("z.example".into());
        mutations[0].port = Some(30);
        let mut working = Working {
            sub: &sub,
            dependencies: &dependencies,
            dependents: &dependents,
            names: &mut names,
            order: &mut order,
            mutations: &mut mutations,
            resolver: None,
        };
        apply(
            &mut working,
            &[step(
                1,
                Operation::ConditionFilter {
                    conditions: vec![
                        Condition {
                            field: Field::Server,
                            matcher: TextMatch::Equals,
                            value: "z.example".into(),
                            negate: false,
                        },
                        Condition {
                            field: Field::Port,
                            matcher: TextMatch::Equals,
                            value: "30".into(),
                            negate: false,
                        },
                    ],
                    mode: MatchMode::All,
                    keep: true,
                },
            )],
        )
        .unwrap();
        assert_eq!(order, vec![0]);
    }

    #[test]
    fn normalized_dependency_name_updates_detour() {
        let input = r#"[{"type":"socks","tag":"BASE","server":"a","server_port":1},{"type":"socks","tag":"chain","server":"b","server_port":2,"detour":"BASE"}]"#;
        let result = run(
            input,
            vec![step(
                1,
                Operation::NormalizeName {
                    case: NameCase::Lower,
                    trim: true,
                    collapse_whitespace: true,
                },
            )],
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        assert_eq!(value[0]["tag"], "base");
        assert_eq!(value[1]["detour"], "base");
    }
    #[test]
    fn resolver_batch_retains_tls_identity_and_reports_missing() {
        let sub = Subscription::parse(r#"[{"type":"trojan","tag":"a","server":"a.example","server_port":443,"password":"x","tls":{"enabled":true}},{"type":"trojan","tag":"b","server":"b.example","server_port":443,"password":"x","tls":{"enabled":true}}]"#).unwrap();
        let options = editor::Options {
            pipeline: vec![step(
                1,
                Operation::ResolveDomain {
                    family: AddressFamily::V4,
                    options: dns::Options::default(),
                },
            )],
            include: String::new(),
            exclude: String::new(),
            find: String::new(),
            replace: String::new(),
            nodes: Vec::new(),
            deduplicate: false,
        };
        let resolver = |hosts: &[&str], _: AddressFamily, _: &dns::Options| {
            assert_eq!(hosts, &["a.example", "b.example"]);
            Ok(dns::Batch::addresses(vec![
                Some("192.0.2.1".parse().unwrap()),
                None,
            ]))
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
    fn resolution_reports_are_per_step_and_zero_eligible_is_reported() {
        let sub = Subscription::parse(
            r#"[{"type":"socks","tag":"a","server":"a.example","server_port":1080}]"#,
        )
        .unwrap();
        let mut edits = editor::Options::default();
        edits.pipeline = vec![
            step(
                1,
                Operation::ResolveDomain {
                    family: AddressFamily::V4,
                    options: dns::Options::default(),
                },
            ),
            step(2, Operation::Reverse),
            step(
                3,
                Operation::ResolveDomain {
                    family: AddressFamily::V4,
                    options: dns::Options::default(),
                },
            ),
        ];
        let resolver = |_: &[&str], _: AddressFamily, _: &dns::Options| {
            Ok(dns::Batch {
                addresses: vec![Some("192.0.2.1".parse().unwrap())],
                report: dns::Report {
                    queries: 2,
                    cache_hits: 3,
                    ..Default::default()
                },
            })
        };
        let rendered = sub
            .render_processed_with_resolver(
                &[Health::Unknown],
                &EnhanceOptions {
                    rename: false,
                    sort: false,
                },
                &edits,
                &resolver,
            )
            .unwrap();
        let first = rendered.stages[0].resolution.as_ref().unwrap();
        assert_eq!((first.queries, first.cache_hits), (2, 3));
        assert!(rendered.stages[1].resolution.is_none());
        assert_eq!(
            rendered.stages[2].resolution.as_ref(),
            Some(&dns::Report::default())
        );
    }

    #[test]
    fn invalid_dns_options_fail_before_callback_and_disabled_regex_draft_is_allowed() {
        let sub = Subscription::parse(
            r#"[{"type":"socks","tag":"a","server":"a.example","server_port":1080}]"#,
        )
        .unwrap();
        let dns_options = dns::Options {
            include: "[".into(),
            ..Default::default()
        };
        let operation = Operation::ResolveDomain {
            family: AddressFamily::V4,
            options: dns_options,
        };
        let mut edits = editor::Options::default();
        edits.pipeline = vec![step(1, operation.clone())];
        let called = std::cell::Cell::new(false);
        let resolver = |_: &[&str], _: AddressFamily, _: &dns::Options| {
            called.set(true);
            Ok(dns::Batch::addresses(vec![]))
        };
        assert!(matches!(
            sub.render_processed_with_resolver(
                &[Health::Unknown],
                &EnhanceOptions {
                    rename: false,
                    sort: false
                },
                &edits,
                &resolver,
            ),
            Err(ModelError::InvalidEdit)
        ));
        assert!(!called.get());
        assert!(
            validate(&[Step {
                id: 1,
                enabled: false,
                operation
            }])
            .is_ok()
        );
    }

    #[test]
    fn legacy_dns_step_without_options_deserializes_to_defaults() {
        let step: Step = serde_json::from_str(
            r#"{"id":1,"enabled":true,"operation":{"type":"resolve_domain","family":"v4"}}"#,
        )
        .unwrap();
        let Operation::ResolveDomain { options, .. } = &step.operation else {
            panic!("expected DNS operation")
        };
        assert_eq!(
            options.timeout_seconds,
            dns::Options::default().timeout_seconds
        );
        assert_eq!(options.concurrency, dns::Options::default().concurrency);
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
                    options: dns::Options::default(),
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
        let resolver =
            |_: &[&str], _: AddressFamily, _: &dns::Options| Ok(dns::Batch::addresses(vec![]));
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
                        options: dns::Options::default(),
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
        let resolver = |_: &[&str], _: AddressFamily, _: &dns::Options| {
            Ok(dns::Batch::addresses(vec![Some(
                "192.0.2.1".parse().unwrap(),
            )]))
        };
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
        let source = "A = socks5, node.example, 1080\nB = future, node.example, 1234\n";
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
        assert!(out.content.contains("B = future, node.example, 1234"));
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
                    options: dns::Options::default(),
                },
            ),
            step(2, Operation::Deduplicate),
        ];
        let resolver = |_: &[&str], _: AddressFamily, _: &dns::Options| {
            Ok(dns::Batch::addresses(vec![
                Some("1.1.1.1".parse().unwrap()),
                Some("8.8.8.8".parse().unwrap()),
            ]))
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
