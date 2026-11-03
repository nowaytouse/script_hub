//! Explicit, offline preview of Sub-Store backup contents.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;
use zeroize::{Zeroize, Zeroizing};

use super::{RelayConfig, input, model};

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
    pub operations: Vec<model::pipeline::Step>,
    pub unsupported: usize,
    pub is_file: bool,
}

pub struct Collection {
    pub name: Zeroizing<String>,
    pub subscriptions: Vec<Zeroizing<String>>,
    pub unsupported: usize,
}

pub struct Report {
    pub items: Vec<Item>,
    pub collections: Vec<Collection>,
    pub sensitive_count: usize,
    pub unsupported_count: usize,
}

// Never format source URLs, node contents, settings, or processor arguments.
impl std::fmt::Debug for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Report")
            .field("items", &self.items.len())
            .field("collections", &self.collections.len())
            .field("sensitive_count", &self.sensitive_count)
            .field("unsupported_count", &self.unsupported_count)
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

fn node_content(value: &str) -> Option<Zeroizing<String>> {
    let parsed = model::Subscription::parse(value).ok()?;
    if parsed.nodes().is_empty()
        || parsed
            .nodes()
            .iter()
            .any(|node| node.to_sing_box(false).is_err())
    {
        return None;
    }
    Some(Zeroizing::new(value.to_owned()))
}

fn count_process(value: &Value) -> usize {
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
    };
    for (key, is_file) in [("subs", false), ("files", true)] {
        let Some(values) = root.get(key) else {
            continue;
        };
        let values = values.as_array().ok_or(ImportError::InvalidBackup)?;
        for value in values {
            let Some(name) = name(value) else {
                report.unsupported_count += 1;
                continue;
            };
            let mut unsupported = count_process(value);
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
                    .keys()
                    .filter(|key| {
                        !["name", "source", "url", "content", "process"].contains(&key.as_str())
                    })
                    .count();
            }
            report.unsupported_count += unsupported;
            report.items.push(Item {
                name,
                source,
                content,
                operations: Vec::new(),
                unsupported,
                is_file,
            });
        }
    }
    if let Some(values) = root.get("collections") {
        for value in values.as_array().ok_or(ImportError::InvalidBackup)? {
            let Some(name) = name(value) else {
                report.unsupported_count += 1;
                continue;
            };
            let mut unsupported = count_process(value);
            let mut subscriptions = Vec::new();
            if let Some(refs) = value.get("subscriptions") {
                if let Some(refs) = refs.as_array() {
                    for reference in refs {
                        if let Some(reference) = reference.as_str() {
                            subscriptions.push(Zeroizing::new(reference.to_owned()));
                        } else {
                            unsupported += 1;
                        }
                    }
                } else {
                    unsupported += 1;
                }
            }
            if let Some(object) = value.as_object() {
                unsupported += object
                    .keys()
                    .filter(|key| !["name", "subscriptions", "process"].contains(&key.as_str()))
                    .count();
            }
            report.unsupported_count += unsupported;
            report.collections.push(Collection {
                name,
                subscriptions,
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
