//! Ordered, local-only subscription transformations.
use super::{
    MAX_CONTENT, ModelError, NodeIdentity, NodeSource, PrivateJson, Subscription, local_region,
    uncertain_identity, uncertain_token,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
            Operation::NameFilter { pattern, .. } => {
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
            Operation::Flags { .. } | Operation::Sort { .. } | Operation::Deduplicate => {}
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
    } = working;
    let mut counts = Counts::default();
    let mut skipped_nodes = HashSet::new();
    let mut reports = Vec::new();
    for step in steps.iter().filter(|s| s.enabled) {
        let before = order.len();
        let mut renamed = 0;
        let mut name_bytes = order.iter().map(|&i| names[i].len()).sum();
        let mut remove = vec![false; names.len()];
        let mut duplicate = vec![false; names.len()];
        match &step.operation {
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
                    } else if let Some(value) = identity(&sub.nodes[i])? {
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
