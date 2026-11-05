//! Explicit, offline preview of Sub-Store backup contents.
mod tags;

use std::fmt::Write as _;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;
use zeroize::{Zeroize, Zeroizing};

use super::{RelayConfig, input, model};
use model::pipeline::{self, Operation, Step};
use model::properties::Setting;

const MAX_ENCODED: usize = 8 * 1024 * 1024;
const MAX_JSON: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportError {
    TooLarge,
    InvalidBackup,
    InvalidItem,
    AcknowledgementRequired,
}

pub struct Item {
    pub name: Zeroizing<String>,
    pub source: Option<Zeroizing<String>>,
    pub content: Option<Zeroizing<String>>,
    pub operations: Vec<Step>,
    pub unsupported: usize,
    pub is_file: bool,
}

pub struct Collection {
    pub name: Zeroizing<String>,
    pub subscriptions: Vec<Zeroizing<String>>,
    pub valid_references: bool,
    pub operations: Vec<Step>,
    pub unsupported: usize,
}

pub struct Report {
    pub items: Vec<Item>,
    pub collections: Vec<Collection>,
    pub sensitive_count: usize,
    pub unsupported_count: usize,
    pub invalid_items: usize,
}

// Never format source URLs, node contents, settings, or processor arguments.
impl std::fmt::Debug for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Report")
            .field("items", &self.items.len())
            .field("collections", &self.collections.len())
            .field("sensitive_count", &self.sensitive_count)
            .field("unsupported_count", &self.unsupported_count)
            .field("invalid_items", &self.invalid_items)
            .finish()
    }
}

struct PrivateValue(Value);
impl Drop for PrivateValue {
    fn drop(&mut self) {
        fn wipe(value: &mut Value) {
            match value {
                Value::String(s) => s.zeroize(),
                Value::Array(a) => a.iter_mut().for_each(wipe),
                Value::Object(o) => {
                    let old = std::mem::take(o);
                    for (mut key, mut value) in old {
                        key.zeroize();
                        wipe(&mut value);
                    }
                }
                _ => {}
            }
        }
        wipe(&mut self.0);
    }
}

fn name(value: &Value) -> Option<Zeroizing<String>> {
    let name = value.get("name")?.as_str()?;
    if name.is_empty() || name.len() > 512 || name.chars().any(char::is_control) {
        return None;
    }
    Some(Zeroizing::new(name.to_owned()))
}

pub(super) fn valid_node_content(value: &str) -> bool {
    model::Subscription::parse(value).is_ok_and(|sub| {
        !sub.nodes().is_empty()
            && sub.validate_export_dependencies().is_ok()
            && sub
                .nodes()
                .iter()
                .all(|node| node.export_sing_box().is_ok())
    })
}

fn node_content(value: &str) -> Option<Zeroizing<String>> {
    valid_node_content(value).then(|| Zeroizing::new(value.to_owned()))
}

fn keep(args: &Value) -> Option<bool> {
    match args.get("keep") {
        None => Some(true),
        Some(value) => value.as_bool(),
    }
}

fn processor_args(item: &Value) -> Option<&Value> {
    if item.as_object()?.keys().any(|key| {
        !["type", "args", "disabled", "customName", "id", "preview"].contains(&key.as_str())
    }) {
        return None;
    }
    Some(item.get("args").unwrap_or(&Value::Null))
}

fn convert_processor(item: &Value, collection: bool) -> Option<Operation> {
    let args = processor_args(item)?;
    match item.get("type")?.as_str()? {
        "Handle Duplicate Operator" if !collection => {
            let args = args.as_object()?;
            if args.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "action" | "field" | "template" | "link" | "position"
                )
            }) || args.get("action")?.as_str()? != "delete"
                || args
                    .get("field")
                    .is_some_and(|field| field != &serde_json::json!(["name"]))
            {
                return None;
            }
            Some(Operation::DeduplicateName {
                keep: pipeline::KeepOccurrence::SubStoreFirst,
                ignore_case: false,
            })
        }
        "Sort Operator" => {
            let descending = match item.get("args") {
                None => false,
                Some(value) => match value.as_str()? {
                    "asc" => false,
                    "desc" => true,
                    _ => return None,
                },
            };
            Some(Operation::Sort {
                key: if collection {
                    pipeline::SortKey::MemberNameUtf16
                } else {
                    pipeline::SortKey::NameUtf16
                },
                descending,
            })
        }
        "Type Filter" => {
            let types = args
                .as_array()
                .or_else(|| args.get("value").and_then(Value::as_array))?;
            if types.is_empty() || types.len() > 64 {
                return None;
            }
            let mut protocols = Vec::with_capacity(types.len());
            for kind in types {
                let protocol = kind.as_str()?;
                let alias = match protocol {
                    // Sub-Store classifies these Surge spellings as one type.
                    "http" => Some("https"),
                    "socks5" => Some("socks5-tls"),
                    "tuic" => Some("tuic-v5"),
                    "ss" | "hysteria2" | "vmess" | "vless" | "trojan" | "anytls" => None,
                    _ => return None,
                };
                for name in std::iter::once(protocol).chain(alias) {
                    if !protocols.iter().any(|p| p == name) {
                        protocols.push(name.to_owned());
                    }
                }
            }
            Some(Operation::ProtocolFilter {
                protocols,
                keep: keep(args)?,
            })
        }
        "Regex Filter" => {
            let expressions = match args.get("regex") {
                None if args.is_object() => &[][..],
                Some(value) => value.as_array()?.as_slice(),
                None => return None,
            };
            let pattern = if expressions.is_empty() {
                r"\b\B".to_owned()
            } else if expressions.len() == 1 && expressions[0].as_str() == Some("") {
                "^".to_owned()
            } else {
                portable_regex(expressions)?.0
            };
            let keep = keep(args)?;
            Some(if collection {
                Operation::MemberNameFilter { pattern, keep }
            } else {
                Operation::NameFilter { pattern, keep }
            })
        }
        "Regex Sort Operator" => {
            let expressions = args
                .as_array()
                .or_else(|| args.get("expressions").and_then(Value::as_array))?;
            if expressions.is_empty()
                || expressions.len() > 64
                || args.as_object().is_some_and(|object| {
                    object
                        .keys()
                        .any(|key| !matches!(key.as_str(), "expressions" | "order"))
                })
            {
                return None;
            }
            use pipeline::RegexSortUnmatched::{Ascending, Descending, Original};
            let unmatched = match args.get("order").filter(|value| !value.is_null()) {
                None => Ascending,
                Some(value) => match value.as_str()? {
                    "" | "asc" => Ascending,
                    "desc" => Descending,
                    "original" => Original,
                    _ => return None,
                },
            };
            let patterns = expressions
                .iter()
                .map(|expr| portable_regex(std::slice::from_ref(expr)).map(|(pattern, _)| pattern))
                .collect::<Option<Vec<_>>>()?;
            Some(if collection {
                Operation::MemberRegexSort {
                    patterns,
                    unmatched,
                }
            } else {
                Operation::RegexSort {
                    patterns,
                    unmatched,
                }
            })
        }
        "Flag Operator" if !collection && args.get("mode")?.as_str()? == "remove" => {
            // Sub-Store removes flags throughout the name, not just its leading country flag.
            Some(Operation::BatchRename {
                rules: vec![pipeline::RenameRule {
                    pattern: r"[\u{1F1E6}-\u{1F1FF}]{2}|🏴‍☠️|🏳️‍🌈".into(),
                    replacement: String::new(),
                    regex: true,
                }],
                trim_each: true,
            })
        }
        kind @ ("Regex Rename Operator" | "Regex Delete Operator") if !collection => {
            let args = args.as_array()?;
            if args.is_empty() || args.len() > 32 {
                return None;
            }
            let rules = args
                .iter()
                .map(|arg| {
                    let (expr, replacement) = if kind == "Regex Delete Operator" {
                        (arg, "")
                    } else {
                        (arg.get("expr")?, arg.get("now")?.as_str()?)
                    };
                    let (pattern, captures) = portable_regex(std::slice::from_ref(expr))?;
                    Some(pipeline::RenameRule {
                        pattern,
                        replacement: portable_replacement(replacement, captures)?,
                        regex: true,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Operation::BatchRename {
                rules,
                trim_each: true,
            })
        }
        _ => None,
    }
}

fn quick_settings(item: &Value) -> Option<Vec<Operation>> {
    let args = processor_args(item)?.as_object()?;
    let mut operations = Vec::new();
    for (key, value) in args {
        let key = key.as_str();
        if !matches!(
            key,
            "udp"
                | "tfo"
                | "scert"
                | "useless"
                | "vmess aead"
                | "reuse"
                | "ecn"
                | "block-quic"
                | "ip-version"
        ) {
            return None;
        }
        if value.is_null() {
            continue;
        }
        let mode = value.as_str()?;
        let setting = match (key, mode) {
            (_, "DEFAULT") | ("useless", "DISABLED") | ("ip-version", "") => continue,
            ("udp", "ENABLED" | "DISABLED") => Setting::Udp {
                enabled: mode == "ENABLED",
            },
            ("tfo", "ENABLED" | "DISABLED") => Setting::TcpFastOpen {
                enabled: mode == "ENABLED",
            },
            ("scert", "DISABLED") => Setting::VerifyTls,
            // A partially migrated operator could bypass a filter or weaken TLS.
            _ => return None,
        };
        operations.push(Operation::SetProperty { setting });
    }
    Some(operations)
}

fn portable_replacement(value: &str, captures: usize) -> Option<String> {
    if value.len() > 512 {
        return None;
    }
    let mut remaining = value;
    let mut replacement = Zeroizing::new(String::with_capacity(value.len()));
    while let Some((literal, suffix)) = remaining.split_once('$') {
        replacement.push_str(literal);
        let consumed = match suffix.as_bytes() {
            [b'$', ..] => {
                replacement.push_str("$$");
                1
            }
            [b'&', ..] => {
                replacement.push_str("${0}");
                1
            }
            [digit @ b'0'..=b'9', rest @ ..] => {
                let first = usize::from(*digit - b'0');
                let second = rest
                    .first()
                    .filter(|digit| digit.is_ascii_digit())
                    .map(|digit| first * 10 + usize::from(*digit - b'0'));
                // ECMAScript falls back to one digit when the two-digit group is absent.
                let (index, consumed) = match second {
                    Some(index) if (1..=captures).contains(&index) => (index, 2),
                    _ if (1..=captures).contains(&first) => (first, 1),
                    _ => return None,
                };
                write!(&mut *replacement, "${{{index}}}").ok()?;
                consumed
            }
            _ => return None,
        };
        remaining = &suffix[consumed..];
    }
    replacement.push_str(remaining);
    (replacement.len() <= 512).then(|| std::mem::take(&mut *replacement))
}

fn portable_regex(patterns: &[Value]) -> Option<(String, usize)> {
    // shortcut: literal alternatives, outer anchors and flat captures until more regex syntax can be proven portable.
    let mut result = Zeroizing::new(String::new());
    let mut captures = 0;
    for pattern in patterns {
        let pattern = pattern.as_str()?;
        if pattern.len() > 512 {
            return None;
        }
        let insensitive = pattern.starts_with("(?i)");
        let pattern = pattern.strip_prefix("(?i)").unwrap_or(pattern);
        if insensitive && !pattern.is_ascii() {
            return None;
        }
        let mut chars = pattern.chars().peekable();
        let (mut in_group, mut branch_start, mut has_atom) = (false, true, false);
        while let Some(ch) = chars.next() {
            match ch {
                '(' if !in_group => {
                    in_group = true;
                    has_atom = false;
                    captures += 1;
                }
                ')' if in_group && has_atom => in_group = false,
                '^' if !in_group && branch_start => {}
                '$' if !in_group && has_atom && matches!(chars.peek(), None | Some('|')) => {}
                '|' if has_atom => has_atom = false,
                ch if ch.is_control() || ".*+?()[]{}\\^$|".contains(ch) => return None,
                _ => has_atom = true,
            }
            branch_start = ch == '|';
        }
        if in_group || !has_atom {
            return None;
        }
        if !result.is_empty() {
            result.push('|');
        }
        if insensitive {
            result.push_str("(?i-u:");
        }
        result.push_str(pattern);
        if insensitive {
            result.push(')');
        }
        if result.len() > 512 {
            return None;
        }
    }
    (!result.is_empty()).then(|| (std::mem::take(&mut *result), captures))
}

fn migrate_process(value: &Value, collection: bool) -> (Vec<Step>, usize) {
    let Some(value) = value.get("process").filter(|v| !v.is_null()) else {
        return (Vec::new(), 0);
    };
    let Some(process) = value.as_array() else {
        return (Vec::new(), 1);
    };
    let mut steps = Vec::new();
    let mut unsupported = 0;
    let mut incomplete = false;
    let mut next_extra_id = process.len() as u64 + 1;
    for (index, item) in process.iter().enumerate() {
        let disabled = item.get("disabled").and_then(Value::as_bool) == Some(true);
        let valid_disabled = item.get("disabled").is_none_or(Value::is_boolean);
        if !incomplete && valid_disabled && steps.len() < 64 {
            let operations =
                if item.get("type").and_then(Value::as_str) == Some("Quick Setting Operator") {
                    quick_settings(item)
                } else if matches!(
                    item.get("type").and_then(Value::as_str),
                    Some("Regex Rename Operator" | "Regex Delete Operator")
                ) && processor_args(item)
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
                {
                    Some(Vec::new())
                } else {
                    convert_processor(item, collection).map(|operation| vec![operation])
                };
            if let Some(operations) = operations.filter(|ops| steps.len() + ops.len() <= 64) {
                let mut migrated = Vec::with_capacity(operations.len());
                for (offset, operation) in operations.into_iter().enumerate() {
                    migrated.push(Step {
                        id: if offset == 0 {
                            index as u64 + 1
                        } else {
                            next_extra_id + offset as u64 - 1
                        },
                        enabled: true,
                        operation,
                    });
                }
                if pipeline::validate(&migrated).is_ok() {
                    next_extra_id += migrated.len().saturating_sub(1) as u64;
                    for step in &mut migrated {
                        step.enabled = !disabled;
                    }
                    steps.extend(migrated);
                    continue;
                }
            }
        }
        // A missing active step can change the data or suppress downstream processors.
        incomplete |= !disabled;
        unsupported += 1;
    }
    (steps, unsupported)
}

fn count_file_process(value: &Value) -> usize {
    match value.get("process") {
        None | Some(Value::Null) => 0,
        Some(Value::Array(process)) => process.len(),
        Some(_) => 1,
    }
}

pub fn import(bytes: &[u8]) -> Result<Report, ImportError> {
    if bytes.len() > MAX_ENCODED {
        return Err(ImportError::TooLarge);
    }
    let input = std::str::from_utf8(bytes)
        .map_err(|_| ImportError::InvalidBackup)?
        .trim();
    let decoded = if input.starts_with('{') {
        Zeroizing::new(input.as_bytes().to_vec())
    } else {
        Zeroizing::new(
            STANDARD
                .decode(input)
                .map_err(|_| ImportError::InvalidBackup)?,
        )
    };
    if decoded.len() > MAX_JSON {
        return Err(ImportError::TooLarge);
    }
    // serde_json enforces its recursion limit before any tree is retained.
    let backup = PrivateValue(
        serde_json::from_slice::<model::UniqueValue>(&decoded)
            .map_err(|_| ImportError::InvalidBackup)?
            .0,
    );
    let root = backup.0.as_object().ok_or(ImportError::InvalidBackup)?;
    if !["subs", "collections", "files", "settings"]
        .iter()
        .any(|key| root.contains_key(*key))
    {
        return Err(ImportError::InvalidBackup);
    }
    let mut report = Report {
        items: Vec::new(),
        collections: Vec::new(),
        sensitive_count: 0,
        unsupported_count: 0,
        invalid_items: 0,
    };
    for (key, is_file) in [("subs", false), ("files", true)] {
        let Some(values) = root.get(key) else {
            continue;
        };
        let values = values.as_array().ok_or(ImportError::InvalidBackup)?;
        for value in values {
            let Some(name) = name(value) else {
                report.unsupported_count += 1;
                report.invalid_items += 1;
                continue;
            };
            let (operations, mut unsupported) = if is_file {
                (Vec::new(), count_file_process(value))
            } else {
                migrate_process(value, false)
            };
            let source_kind = value.get("source").and_then(Value::as_str);
            let source = if source_kind == Some("remote") {
                value
                    .get("url")
                    .and_then(Value::as_str)
                    .filter(|url| {
                        url::Url::parse(url).is_ok_and(|parsed| parsed.scheme() == "https")
                            && input::target(url, "").is_ok()
                    })
                    .map(|url| Zeroizing::new(url.to_owned()))
            } else {
                None
            };
            let content = if source_kind == Some("local") {
                value
                    .get("content")
                    .and_then(Value::as_str)
                    .and_then(node_content)
            } else {
                None
            };
            if source.is_none() && content.is_none() {
                unsupported += 1;
            }
            // Mixed sources and additional options cannot safely be reproduced.
            if source_kind == Some("remote")
                && value
                    .get("content")
                    .is_some_and(|v| !v.is_null() && v != "")
            {
                unsupported += 1;
            }
            if source_kind == Some("local")
                && value.get("url").is_some_and(|v| !v.is_null() && v != "")
            {
                unsupported += 1;
            }
            if let Some(object) = value.as_object() {
                unsupported += object
                    .iter()
                    .filter(|(key, value)| {
                        !["name", "source", "url", "content", "process"].contains(&key.as_str())
                            && (is_file
                                || key.as_str() != "tag"
                                || !tags::is_supported_metadata(value))
                    })
                    .count();
            }
            report.unsupported_count += unsupported;
            report.items.push(Item {
                name,
                source,
                content,
                operations,
                unsupported,
                is_file,
            });
        }
    }
    if let Some(values) = root.get("collections") {
        let values = values.as_array().ok_or(ImportError::InvalidBackup)?;
        let tag_sources = (values.len() <= super::library::MAX_COLLECTIONS).then(|| {
            root.get("subs")
                .and_then(Value::as_array)
                .map_or(&[][..], Vec::as_slice)
        });
        for value in values {
            let Some(name) = name(value) else {
                report.unsupported_count += 1;
                report.invalid_items += 1;
                continue;
            };
            let (operations, mut unsupported) = migrate_process(value, true);
            let mut subscriptions = Vec::new();
            let mut valid_references = true;
            if let Some(refs) = value.get("subscriptions") {
                if let Some(refs) = refs.as_array() {
                    for reference in refs {
                        if let Some(reference) = reference.as_str() {
                            subscriptions.push(Zeroizing::new(reference.to_owned()));
                        } else {
                            unsupported += 1;
                            valid_references = false;
                        }
                    }
                } else {
                    unsupported += 1;
                    valid_references = false;
                }
            } else if value.get("subscriptionTags").is_none_or(Value::is_null) {
                unsupported += 1;
                valid_references = false;
            }
            if valid_references
                && !tags::extend_members(
                    &mut subscriptions,
                    value.get("subscriptionTags"),
                    tag_sources,
                )
            {
                unsupported += 1;
                valid_references = false;
            }
            if let Some(object) = value.as_object() {
                unsupported += object
                    .keys()
                    .filter(|key| {
                        !["name", "subscriptions", "subscriptionTags", "process"]
                            .contains(&key.as_str())
                    })
                    .count();
            }
            report.unsupported_count += unsupported;
            report.collections.push(Collection {
                name,
                subscriptions,
                valid_references,
                operations,
                unsupported,
            });
        }
    }
    if let Some(settings) = root.get("settings") {
        report.sensitive_count += settings.as_object().map_or(1, |s| s.len());
    }
    for (key, value) in root {
        if !["subs", "collections", "files", "settings"].contains(&key.as_str()) {
            report.sensitive_count += value.as_array().map_or(1, Vec::len);
        }
    }
    Ok(report)
}

impl Report {
    pub fn select_item(
        &self,
        index: usize,
        acknowledged: bool,
    ) -> Result<RelayConfig, ImportError> {
        let item = self.items.get(index).ok_or(ImportError::InvalidItem)?;
        if item.unsupported > 0 && !acknowledged {
            return Err(ImportError::AcknowledgementRequired);
        }
        if item.source.is_none() && item.content.is_none() {
            return Err(ImportError::InvalidItem);
        }
        let mut config = RelayConfig::default();
        if let Some(source) = &item.source {
            config.source = source.to_string();
        }
        config.local_content = item.content.clone();
        config.processing.pipeline = item.operations.clone();
        config.rename = false;
        config.sort = false;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_delete_migrates_only_name_and_stops_on_unsupported_active_steps() {
        for args in [
            serde_json::json!({"action":"delete"}),
            serde_json::json!({"action":"delete", "field":["name"], "template":"0 1", "link":"-", "position":"back"}),
        ] {
            let item = serde_json::json!({"type":"Handle Duplicate Operator", "args":args});
            assert!(matches!(
                convert_processor(&item, false),
                Some(Operation::DeduplicateName {
                    keep: pipeline::KeepOccurrence::SubStoreFirst,
                    ignore_case: false
                })
            ));
            assert!(convert_processor(&item, true).is_none());
        }
        for args in [
            serde_json::json!({}),
            serde_json::json!({"action":"rename"}),
            serde_json::json!({"action":"delete", "field":[]}),
            serde_json::json!({"action":"delete", "field":["server"]}),
            serde_json::json!({"action":"delete", "unknown":true}),
        ] {
            let item = serde_json::json!({"process":[
                {"type":"Handle Duplicate Operator", "args":args},
                {"type":"Type Filter", "args":["ss"]}
            ]});
            let (steps, unsupported) = migrate_process(&item, false);
            assert!(steps.is_empty());
            assert_eq!(unsupported, 2);
        }
        let item = serde_json::json!({"process":[
            {"type":"Handle Duplicate Operator", "args":{"action":"rename"}, "disabled":true},
            {"type":"Type Filter", "args":["ss"]}
        ]});
        let (steps, unsupported) = migrate_process(&item, false);
        assert_eq!(unsupported, 1);
        assert_eq!(steps.len(), 1);
    }

    #[test]
    fn previews_without_restoring_secrets_or_executing_scripts() {
        let json = br#"{"subs":[{"name":"remote","source":"remote","url":"https://example.com/sub?token=secret","process":[{"type":"Script Operator","args":{"content":"secret script"}}]}],"collections":[{"name":"group","subscriptions":["remote"]}],"files":[],"settings":{"gistToken":"secret token"}}"#;
        let report = import(json).unwrap();
        assert_eq!(report.items.len(), 1);
        assert_eq!(report.collections[0].subscriptions[0].as_str(), "remote");
        assert_eq!(report.sensitive_count, 1);
        assert!(matches!(
            report.select_item(0, false),
            Err(ImportError::AcknowledgementRequired)
        ));
        assert!(report.select_item(0, true).is_ok());
        let debug = format!("{report:?}");
        assert!(!debug.contains("secret"));
        let encoded = STANDARD.encode(json);
        assert_eq!(import(encoded.as_bytes()).unwrap().items.len(), 1);
        assert!(import(STANDARD.encode(encoded).as_bytes()).is_err());
    }

    #[test]
    fn imports_portable_filters_and_keeps_disabled_steps_in_order() {
        let backup = br#"{"subs":[{"name":"Local","source":"local",
            "content":"[Proxy]\nhk = socks5, node.example.invalid, 1080\nNotHK = socks5, other.example.invalid, 1080\nHK-Web = http, web.example.invalid, 80\n",
            "process":[
              {"type":"Regex Filter","args":{"regex":["(?i)^HK$|^HK-Web$"]}},
              {"type":"Type Filter","args":["socks5"],"disabled":true},
              {"type":"Script Operator","args":{"content":"not runnable"},"disabled":true},
              {"type":"Type Filter","args":{"value":["socks5"],"keep":false}}
            ]}]}"#;
        let report = import(backup).unwrap();
        assert_eq!(report.unsupported_count, 1);
        let config = report.select_item(0, true).unwrap();
        let order: Vec<_> = config
            .processing
            .pipeline
            .iter()
            .map(|step| (step.id, step.enabled))
            .collect();
        assert_eq!(order, [(1, true), (2, false), (4, true)]);
        let source = model::Subscription::parse(config.local_content.as_ref().unwrap()).unwrap();
        let result = source
            .render_processed(
                &[model::Health::Unknown; 3],
                &model::EnhanceOptions {
                    rename: config.rename,
                    sort: config.sort,
                },
                &config.processing,
            )
            .unwrap();
        assert_eq!(result.retained, [2]);
    }

    #[test]
    fn type_filter_maps_the_surge_protocol_aliases_classified_by_substore() {
        let filter = serde_json::json!({"type": "Type Filter", "args": ["http", "socks5", "tuic"]});
        assert!(matches!(
            convert_processor(&filter, true).as_ref(),
            Some(Operation::ProtocolFilter { protocols, .. })
                if protocols == &["http", "https", "socks5", "socks5-tls", "tuic", "tuic-v5"]
        ));
    }

    #[test]
    fn empty_regex_filter_preserves_substore_selection_and_following_steps() {
        for (args, expected) in [
            (r#"{}"#, 0),
            (r#"{"regex":[],"keep":false}"#, 1),
            (r#"{"regex":[""]}"#, 1),
            (r#"{"regex":[""],"keep":false}"#, 0),
        ] {
            let backup = format!(
                r#"{{"subs":[{{"name":"Local","source":"local",
                "content":"[Proxy]\nHK = socks5, node.example.invalid, 1080\n",
                "process":[{{"type":"Regex Filter","args":{args}}},
                  {{"type":"Type Filter","args":["socks5"]}}]}}]}}"#
            );
            let report = import(backup.as_bytes()).unwrap();
            assert_eq!(report.unsupported_count, 0);
            let config = report.select_item(0, false).unwrap();
            assert_eq!(config.processing.pipeline.len(), 2);
            let source =
                model::Subscription::parse(config.local_content.as_ref().unwrap()).unwrap();
            let result = source
                .render_processed(
                    &[model::Health::Unknown],
                    &model::EnhanceOptions {
                        rename: false,
                        sort: false,
                    },
                    &config.processing,
                )
                .unwrap();
            assert_eq!(result.retained.len(), expected, "{args}");
        }
        for args in [serde_json::json!({"regex": null}), serde_json::json!([])] {
            assert!(
                convert_processor(
                    &serde_json::json!({"type":"Regex Filter", "args":args}),
                    false
                )
                .is_none()
            );
        }
    }

    #[test]
    fn empty_collection_regex_filter_uses_member_names() {
        for (regex, expected) in [
            (serde_json::json!([]), r"\b\B"),
            (serde_json::json!([""]), "^"),
        ] {
            let filter = serde_json::json!({"type":"Regex Filter", "args":{"regex":regex}});
            assert!(matches!(
                convert_processor(&filter, true),
                Some(Operation::MemberNameFilter { ref pattern, keep: true }) if pattern == expected
            ));
        }
    }

    #[test]
    fn empty_rename_and_delete_do_not_block_following_steps() {
        for kind in ["Regex Rename Operator", "Regex Delete Operator"] {
            let item = serde_json::json!({"process": [
                {"type": kind, "args": []},
                {"type": kind, "args": [], "disabled": true},
                {"type": "Type Filter", "args": ["ss"]}
            ]});
            let (steps, unsupported) = migrate_process(&item, false);
            assert_eq!(unsupported, 0);
            assert_eq!(steps.len(), 1);
            assert_eq!(steps[0].id, 3);
            let invalid = serde_json::json!({"process": [
                {"type": kind, "args": [], "unknown": true},
                {"type": "Type Filter", "args": ["ss"]}
            ]});
            let (steps, unsupported) = migrate_process(&invalid, false);
            assert!(steps.is_empty());
            assert_eq!(unsupported, 2);
        }
    }

    #[test]
    fn imports_only_supported_sub_store_sorts_transactionally() {
        let missing = serde_json::json!({"type":"Sort Operator"});
        assert!(matches!(
            convert_processor(&missing, false),
            Some(Operation::Sort {
                key: pipeline::SortKey::NameUtf16,
                descending: false
            })
        ));
        let unknown = serde_json::json!({"type":"Sort Operator", "unknown":true});
        assert!(convert_processor(&unknown, false).is_none());
        for kind in [
            "Type Filter",
            "Regex Filter",
            "Regex Sort Operator",
            "Flag Operator",
            "Regex Rename Operator",
            "Regex Delete Operator",
            "Quick Setting Operator",
        ] {
            let missing = serde_json::json!({"type":kind});
            assert!(convert_processor(&missing, false).is_none());
            assert!(quick_settings(&missing).is_none());
        }
        for (order, descending) in [("asc", false), ("desc", true)] {
            for collection in [false, true] {
                let item = serde_json::json!({"type": "Sort Operator", "args": order});
                assert!(
                    matches!(convert_processor(&item, collection), Some(Operation::Sort { key, descending: d })
                    if d == descending && key == if collection { pipeline::SortKey::MemberNameUtf16 } else { pipeline::SortKey::NameUtf16 })
                );
            }
        }
        for args in [
            serde_json::json!("random"),
            serde_json::json!(null),
            serde_json::json!({"order":"asc"}),
        ] {
            let item = serde_json::json!({"process": [
                {"type":"Sort Operator", "args":args},
                {"type":"Type Filter", "args":["ss"]}
            ]});
            let (steps, unsupported) = migrate_process(&item, false);
            assert!(steps.is_empty());
            assert_eq!(unsupported, 2);
        }
        let item = serde_json::json!({"process": [
            {"type":"Regex Delete Operator", "args":["^A"]},
            {"type":"Sort Operator", "args":"asc"}
        ]});
        let (steps, unsupported) = migrate_process(&item, true);
        assert!(steps.is_empty());
        assert_eq!(unsupported, 2);
    }

    #[test]
    fn imports_regex_sort_priorities_and_unmatched_order() {
        for (args, expected) in [
            (r#"["^JP","US"]"#, [2, 3, 0, 4, 5, 1]),
            (
                r#"{"expressions":["^JP","US"],"order":"desc"}"#,
                [2, 3, 0, 1, 5, 4],
            ),
            (
                r#"{"expressions":["^JP","US"],"order":"original"}"#,
                [2, 3, 0, 1, 4, 5],
            ),
        ] {
            let backup = format!(
                r#"{{"subs":[{{"name":"Local","source":"local",
                "content":"[Proxy]\nUS = socks5, us.example.invalid, 1080\nZ = socks5, z.example.invalid, 1080\nJP Z US = socks5, jpz.example.invalid, 1080\nJP A = socks5, jpa.example.invalid, 1080\nA = socks5, a.example.invalid, 1080\nM = socks5, m.example.invalid, 1080\n",
                "process":[{{"type":"Regex Sort Operator","args":{args}}}]}}]}}"#
            );
            let report = import(backup.as_bytes()).unwrap();
            let config = report.select_item(0, false).unwrap();
            let source =
                model::Subscription::parse(config.local_content.as_ref().unwrap()).unwrap();
            let result = source
                .render_processed(
                    &[model::Health::Unknown; 6],
                    &model::EnhanceOptions {
                        rename: false,
                        sort: false,
                    },
                    &config.processing,
                )
                .unwrap();
            assert_eq!(result.retained, expected, "{args}");
        }
    }

    #[test]
    fn imports_safe_quick_settings_before_following_filters_without_colliding_ids() {
        let backup = br#"{"subs":[{"name":"Local","source":"local",
            "content":"[Proxy]\nKeep = socks5, udp.example.invalid, 1080, tfo=true, udp-relay=false\nKeep TLS = trojan, secure.example.invalid, 443, password=fixture\nDrop = socks5, other.example.invalid, 1080",
            "process":[
              {"type":"Quick Setting Operator","args":{"udp":"ENABLED","tfo":"DISABLED","scert":"DISABLED","useless":"DEFAULT","ip-version":"DEFAULT"}},
              {"type":"Regex Filter","args":{"regex":["^Keep"]}}
            ]}]}"#;
        let report = import(backup).unwrap();
        let config = report.select_item(0, false).unwrap();
        let source = model::Subscription::parse(config.local_content.as_ref().unwrap()).unwrap();
        let result = source
            .render_processed(
                &[model::Health::Unknown; 3],
                &model::EnhanceOptions {
                    rename: false,
                    sort: false,
                },
                &config.processing,
            )
            .unwrap();
        assert_eq!(result.retained, [0, 1]);
        for setting in ["udp-relay=true", "tfo=false", "skip-cert-verify=false"] {
            assert!(result.content.contains(setting), "missing {setting}");
        }
    }

    #[test]
    fn imports_subscription_rename_and_delete_with_ordered_global_replacement_and_trim() {
        let backup = br#"{"subs":[{"name":"Local","source":"local",
            "content":"[Proxy]\nold seed seed seed HK = socks5, node.example.invalid, 1080\n",
            "process":[
              {"type":"Regex Rename Operator","args":[{"expr":"^old","now":""},
                {"expr":"^seed","now":"ready"},{"expr":"seed","now":"CA"}]},
              {"type":"Regex Delete Operator","args":["CA "]}
            ]}]}"#;
        let report = import(backup).unwrap();
        assert_eq!(report.unsupported_count, 0);
        let config = report.select_item(0, true).unwrap();
        let source = model::Subscription::parse(config.local_content.as_ref().unwrap()).unwrap();
        let result = source
            .render_processed(
                &[model::Health::Unknown],
                &model::EnhanceOptions {
                    rename: false,
                    sort: false,
                },
                &config.processing,
            )
            .unwrap();
        let rendered = model::Subscription::parse(&result.content).unwrap();
        assert_eq!(rendered.nodes()[0].name(), "ready HK");
    }

    #[test]
    fn collection_rename_is_not_migrated_to_namespaced_nodes() {
        for item in [
            serde_json::json!({"type": "Regex Delete Operator", "args": ["^HK"]}),
            serde_json::json!({"type": "Flag Operator", "args": {"mode": "remove"}}),
        ] {
            assert!(convert_processor(&item, false).is_some());
            assert!(convert_processor(&item, true).is_none());
        }
    }

    #[test]
    fn imports_whole_match_and_literal_dollar_replacements() {
        let backup = br#"{"subs":[{"name":"Local","source":"local",
            "content":"[Proxy]\nHK HK = socks5, node.example.invalid, 1080\n",
            "process":[{"type":"Regex Rename Operator","args":[{"expr":"HK","now":"$$$&Z"}]}]}]}"#;
        let report = import(backup).unwrap();
        assert_eq!(report.unsupported_count, 0);
        let config = report.select_item(0, true).unwrap();
        let source = model::Subscription::parse(config.local_content.as_ref().unwrap()).unwrap();
        let result = source
            .render_processed(
                &[model::Health::Unknown],
                &model::EnhanceOptions {
                    rename: false,
                    sort: false,
                },
                &config.processing,
            )
            .unwrap();
        let rendered = model::Subscription::parse(&result.content).unwrap();
        assert_eq!(rendered.nodes()[0].name(), "$HKZ $HKZ");
    }

    #[test]
    fn imports_numbered_captures_with_ecmascript_replacement_order() {
        let backup = br#"{"subs":[{"name":"Local","source":"local",
            "content":"[Proxy]\njP-one = socks5, node.example.invalid, 1080\nUS = socks5, other.example.invalid, 1080\n",
            "process":[{"type":"Regex Rename Operator","args":[{"expr":"(?i)^(jp|hk)-(one|two)$|^(us)$","now":"$2-$1-$3-$01-$10"}]}]}]}"#;
        let report = import(backup).unwrap();
        assert_eq!(report.unsupported_count, 0);
        let config = report.select_item(0, true).unwrap();
        let source = model::Subscription::parse(config.local_content.as_ref().unwrap()).unwrap();
        let result = source
            .render_processed(
                &[model::Health::Unknown; 2],
                &model::EnhanceOptions {
                    rename: false,
                    sort: false,
                },
                &config.processing,
            )
            .unwrap();
        let rendered = model::Subscription::parse(&result.content).unwrap();
        let names: Vec<_> = rendered.nodes().iter().map(model::Node::name).collect();
        assert_eq!(names, ["one-jP--jP-jP0", "--US--0"]);
    }

    #[test]
    fn numbered_replacement_prefers_existing_two_digit_capture() {
        assert_eq!(portable_replacement("$100", 10).as_deref(), Some("${10}0"));
    }

    #[test]
    fn imports_flag_removal_with_global_matching_and_trim() {
        let backup = r#"{"subs":[{"name":"Local","source":"local",
            "content":"[Proxy]\n🇭🇰 Alpha🏴‍☠️ 🏳️‍🌈🇯🇵 = socks5, node.example.invalid, 1080\nA🇺🇸B🇦 = socks5, other.example.invalid, 1080\n",
            "process":[{"type":"Flag Operator","args":{"mode":"remove","tw":"ws"}}]}]}"#;
        let report = import(backup.as_bytes()).unwrap();
        assert_eq!(report.unsupported_count, 0);
        let config = report.select_item(0, true).unwrap();
        let source = model::Subscription::parse(config.local_content.as_ref().unwrap()).unwrap();
        let result = source
            .render_processed(
                &[model::Health::Unknown; 2],
                &model::EnhanceOptions {
                    rename: false,
                    sort: false,
                },
                &config.processing,
            )
            .unwrap();
        let rendered = model::Subscription::parse(&result.content).unwrap();
        let names: Vec<_> = rendered.nodes().iter().map(model::Node::name).collect();
        assert_eq!(names, ["Alpha", "AB🇦"]);
    }

    #[test]
    fn unsupported_active_processor_blocks_importing_dependent_steps() {
        for kind in [
            r#"{"type":"Script Operator","args":{"content":"unknown"}}"#,
            r#"{"type":"Regex Filter","args":{"regex":["(?=HK)"]}}"#,
            r#"{"type":"Regex Delete Operator","args":["[HK]"]}"#,
            r#"{"type":"Regex Rename Operator","args":[{"expr":"HK","now":"$1-new"}]}"#,
            r#"{"type":"Regex Rename Operator","args":[{"expr":"((HK))","now":"$1"}]}"#,
            r#"{"type":"Regex Rename Operator","args":[{"expr":"(HK|)","now":"$1"}]}"#,
            r#"{"type":"Regex Rename Operator","args":[{"expr":"(HK)?","now":"$1"}]}"#,
            r#"{"type":"Regex Rename Operator","args":[{"expr":"HK","now":"$'-new"}]}"#,
            r#"{"type":"Flag Operator","args":{"mode":"add","tw":"ws"}}"#,
            r#"{"type":"Quick Setting Operator","args":{"udp":"ENABLED","scert":"ENABLED"}}"#,
            r#"{"type":"Quick Setting Operator","args":{"tfo":"ENABLED","useless":"ENABLED"}}"#,
            r#"{"type":"Regex Sort Operator","args":{"expressions":["^HK"],"order":"random"}}"#,
        ] {
            let backup = format!(
                r#"{{"subs":[{{"name":"X","source":"remote","url":"https://example.com/sub","process":[{kind},{{"type":"Type Filter","args":["ss"]}}]}}]}}"#
            );
            let report = import(backup.as_bytes()).unwrap();
            assert_eq!(report.unsupported_count, 2);
            assert!(report.items[0].operations.is_empty());
        }
    }

    #[test]
    fn rejects_invalid_sources_and_non_node_files() {
        let json = br#"{"subs":[{"name":"unsafe","source":"remote","url":"https://127.0.0.1/"}],"files":[{"name":"text","source":"local","content":"not nodes"}]}"#;
        let report = import(json).unwrap();
        assert!(
            report
                .items
                .iter()
                .all(|item| item.source.is_none() && item.content.is_none())
        );
        assert!(matches!(
            report.select_item(0, true),
            Err(ImportError::InvalidItem)
        ));
    }

    #[test]
    fn rejects_duplicate_keys_at_any_depth() {
        for json in [
            br#"{"subs":[{"name":"x","source":"remote","url":"https://example.com/","process":[],"process":[] }]}"#.as_slice(),
            br#"{"settings":{"gistToken":"a","gistToken":"b"}}"#.as_slice(),
        ] {
            assert!(matches!(import(json), Err(ImportError::InvalidBackup)));
        }
    }

    #[test]
    fn remote_source_requires_https_url() {
        let json = br#"{"subs":[{"name":"id","source":"remote","url":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},{"name":"http","source":"remote","url":"http://example.com/"},{"name":"https","source":"remote","url":"https://example.com/"}]}"#;
        let report = import(json).unwrap();
        assert!(report.items[0].source.is_none());
        assert!(report.items[1].source.is_none());
        assert!(report.items[2].source.is_some());
    }
}
