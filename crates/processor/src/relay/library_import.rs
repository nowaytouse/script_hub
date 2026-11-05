//! Offline migration into a validated workspace candidate.
use std::collections::{HashMap, HashSet};

use super::{
    compat,
    library::{self, Collection, Entry, Workspace},
    model::{self, editor::Options},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Invalid,
    AcknowledgementRequired,
    Library(library::Error),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub subscriptions: usize,
    pub collections: usize,
    pub skipped: usize,
}

fn unique_name(name: &str, existing: impl Fn(&str) -> bool, next_id: u64) -> Result<String, Error> {
    if name.is_empty() || name.len() > 512 || name.chars().any(char::is_control) {
        return Err(Error::Invalid);
    }
    if !existing(name) {
        return Ok(name.to_owned());
    }
    let mut suffix_id = next_id;
    loop {
        let suffix = format!(" ({suffix_id})");
        let mut end = (512 - suffix.len()).min(name.len());
        while !name.is_char_boundary(end) {
            end -= 1;
        }
        let candidate = format!("{}{}", &name[..end], suffix);
        if !existing(&candidate) {
            return Ok(candidate);
        }
        suffix_id = suffix_id.checked_add(1).ok_or(Error::Invalid)?;
    }
}

pub fn import(
    workspace: &Workspace,
    report: &compat::Report,
    acknowledged: bool,
) -> Result<(Workspace, Summary), Error> {
    workspace.validate().map_err(Error::Library)?;
    if !acknowledged
        && (report.unsupported_count > 0
            || report
                .items
                .iter()
                .any(|item| item.unsupported > 0 || !item.operations.is_empty())
            || report
                .collections
                .iter()
                .any(|item| item.unsupported > 0 || !item.operations.is_empty()))
    {
        return Err(Error::AcknowledgementRequired);
    }
    let mut names = HashSet::new();
    for item in report.items.iter().filter(|item| !item.is_file) {
        if !names.insert(item.name.as_str()) {
            return Err(Error::Invalid);
        }
    }
    let mut candidate = workspace.clone();
    let mut mapping = HashMap::new();
    let mut summary = Summary {
        skipped: report.invalid_items,
        ..Summary::default()
    };
    let mut cache_bytes = candidate
        .subscriptions
        .iter()
        .try_fold(0usize, |total, item| total.checked_add(item.content.len()))
        .ok_or(Error::Library(library::Error::TooLarge))?;
    for item in &report.items {
        let source = item.source.as_deref().map(String::as_str).unwrap_or("");
        let content = item.content.as_deref().map(String::as_str).unwrap_or("");
        let valid_source = !source.is_empty() && super::input::target(source, "").is_ok();
        let valid_content = !content.is_empty()
            && model::Subscription::parse(content).is_ok_and(|sub| {
                !sub.nodes().is_empty()
                    && sub
                        .nodes()
                        .iter()
                        .all(|node| node.to_sing_box(false).is_ok())
            });
        if (item.is_file && !valid_content) || (!valid_source && !valid_content) {
            summary.skipped += 1;
            continue;
        }
        let Ok(name) = unique_name(
            &item.name,
            |name| {
                candidate
                    .subscriptions
                    .iter()
                    .any(|entry| entry.name == name)
            },
            candidate.next_id,
        ) else {
            summary.skipped += 1;
            continue;
        };
        if candidate.subscriptions.len() >= library::MAX_SUBSCRIPTIONS {
            return Err(Error::Library(library::Error::TooLarge));
        }
        cache_bytes = cache_bytes
            .checked_add(if valid_content { content.len() } else { 0 })
            .ok_or(Error::Library(library::Error::TooLarge))?;
        if cache_bytes > 4 * 1024 * 1024 {
            return Err(Error::Library(library::Error::TooLarge));
        }
        let id = candidate.next_id;
        candidate.next_id = id.checked_add(1).ok_or(Error::Invalid)?;
        let mut processing = Options::default();
        processing.pipeline.clone_from(&item.operations);
        candidate.subscriptions.push(Entry {
            id,
            name,
            source: if valid_source {
                source.to_owned()
            } else {
                String::new()
            },
            source_file: String::new(),
            content: if valid_content {
                content.to_owned()
            } else {
                String::new()
            },
            processing,
            publish: Default::default(),
        });
        if !item.is_file {
            mapping.insert(item.name.as_str(), id);
        }
        summary.subscriptions += 1;
    }
    for collection in &report.collections {
        if !collection.valid_references {
            summary.skipped += 1;
            continue;
        }
        let mut seen = HashSet::new();
        let members: Option<Vec<u64>> = collection
            .subscriptions
            .iter()
            .map(|name| {
                mapping
                    .get(name.as_str())
                    .copied()
                    .filter(|id| seen.insert(*id))
            })
            .collect();
        let Some(members) = members.filter(|members| !members.is_empty()) else {
            summary.skipped += 1;
            continue;
        };
        let Ok(name) = unique_name(
            &collection.name,
            |name| candidate.collections.iter().any(|entry| entry.name == name),
            candidate.next_id,
        ) else {
            summary.skipped += 1;
            continue;
        };
        if candidate.collections.len() >= library::MAX_COLLECTIONS {
            return Err(Error::Library(library::Error::TooLarge));
        }
        let id = candidate.next_id;
        candidate.next_id = id.checked_add(1).ok_or(Error::Invalid)?;
        let mut processing = Options::default();
        processing.pipeline.clone_from(&collection.operations);
        candidate.collections.push(Collection {
            id,
            name,
            members,
            processing,
            publish: Default::default(),
        });
        summary.collections += 1;
    }
    candidate.validate().map_err(Error::Library)?;
    Ok((candidate, summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_overflow_leaves_original_unchanged() {
        let report = compat::import(
            br#"{"subs":[{"name":"A","source":"remote","url":"https://example.com/a"}]}"#,
        )
        .unwrap();
        let original = Workspace {
            version: 1,
            next_id: u64::MAX,
            subscriptions: Vec::new(),
            collections: Vec::new(),
        };
        assert!(matches!(
            import(&original, &report, true),
            Err(Error::Invalid)
        ));
        assert_eq!(original.next_id, u64::MAX);
        assert!(original.subscriptions.is_empty());
    }

    #[test]
    fn imports_collection_without_scripts_and_preserves_original_on_error() {
        let backup = br#"{"subs":[{"name":"A","source":"remote","url":"https://example.com/a","process":[{"type":"script","content":"old script"}]},{"name":"B","source":"remote","url":"https://example.com/b"}],"collections":[{"name":"Both","subscriptions":["A","B"],"process":[{"type":"script"}]}]}"#;
        let report = compat::import(backup).unwrap();
        let original = Workspace::default();
        assert!(matches!(
            import(&original, &report, false),
            Err(Error::AcknowledgementRequired)
        ));
        assert!(original.subscriptions.is_empty());
        let (result, summary) = import(&original, &report, true).unwrap();
        assert_eq!(summary.subscriptions, 2);
        assert_eq!(summary.collections, 1);
        assert_eq!(
            result.collections[0].members,
            result
                .subscriptions
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>()
        );
        assert!(
            result
                .subscriptions
                .iter()
                .all(|item| item.processing.pipeline.is_empty())
        );
        assert!(result.collections[0].processing.pipeline.is_empty());
        assert!(original.subscriptions.is_empty());
    }

    #[test]
    fn bulk_migration_preserves_native_subscription_and_collection_pipelines() {
        let backup = br#"{
          "subs":[{"name":"A","source":"remote","url":"https://example.com/a",
            "process":[{"type":"Type Filter","args":["ss"]}]}],
          "collections":[{"name":"Group","subscriptions":["A"],
            "process":[{"type":"Type Filter","args":{"value":["ss"],"keep":false}},
              {"type":"Regex Filter","args":{"regex":["^HK"]}}]}]
        }"#;
        let report = compat::import(backup).unwrap();
        let original = Workspace::default();
        assert_eq!(report.unsupported_count, 1);
        assert!(matches!(
            import(&original, &report, false),
            Err(Error::AcknowledgementRequired)
        ));
        let (result, _) = import(&original, &report, true).unwrap();
        assert!(matches!(
            &result.subscriptions[0].processing.pipeline[0].operation,
            model::pipeline::Operation::ProtocolFilter { protocols, keep: true } if protocols == &["ss"]
        ));
        assert!(matches!(
            &result.collections[0].processing.pipeline[0].operation,
            model::pipeline::Operation::ProtocolFilter { protocols, keep: false } if protocols == &["ss"]
        ));
        assert_eq!(result.collections[0].processing.pipeline.len(), 1);
    }

    #[test]
    fn malformed_references_never_import_a_partial_collection() {
        let backup = br#"{"subs":[{"name":"A","source":"remote","url":"https://example.com/a"},{"source":"remote","url":"https://example.com/b"}],"collections":[{"name":"Broken","subscriptions":["A",42]}]}"#;
        let report = compat::import(backup).unwrap();
        let (result, summary) = import(&Workspace::default(), &report, true).unwrap();
        assert_eq!(summary.subscriptions, 1);
        assert_eq!(summary.collections, 0);
        assert_eq!(summary.skipped, 2);
        assert!(result.collections.is_empty());
    }

    #[test]
    fn imports_node_file_without_resolving_it_as_a_subscription_reference() {
        let backup = br#"{"files":[{"name":"Nodes","source":"local","content":"[Proxy]\nA = socks5, node.example.invalid, 1080\n"}],"collections":[{"name":"Group","subscriptions":["Nodes"]}]}"#;
        let report = compat::import(backup).unwrap();
        let (result, summary) = import(&Workspace::default(), &report, true).unwrap();
        assert_eq!(summary.subscriptions, 1);
        assert_eq!(summary.collections, 0);
        assert_eq!(summary.skipped, 1);
        assert_eq!(result.subscriptions[0].name, "Nodes");
        assert!(result.collections.is_empty());
    }
}
