//! Explicit, all-or-nothing refresh of cached collection members.
use std::sync::atomic::{AtomicBool, Ordering};
use zeroize::{Zeroize, Zeroizing};

use super::{Progress, RelayConfig, RelayError, RunMode, library::Workspace, model};

pub fn refresh(
    workspace: &Workspace,
    id: u64,
    token: &str,
    cancel: &AtomicBool,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<Workspace, RelayError> {
    refresh_with(workspace, id, cancel, |entry| {
        let mut config = RelayConfig {
            source: entry.source.clone(),
            source_file: entry.source_file.clone(),
            test_nodes: false,
            rename: false,
            sort: false,
            ..RelayConfig::default()
        };
        let result = super::run(&config, token, RunMode::Import, cancel, progress)
            .map(|result| result.content);
        config.source.zeroize();
        config.source_file.zeroize();
        result
    })
}

pub(super) fn refresh_with(
    workspace: &Workspace,
    id: u64,
    cancel: &AtomicBool,
    mut fetch: impl FnMut(&super::library::Entry) -> Result<Zeroizing<String>, RelayError>,
) -> Result<Workspace, RelayError> {
    workspace.validate().map_err(|_| RelayError::InvalidInput)?;
    let collection = workspace
        .collections
        .iter()
        .find(|c| c.id == id)
        .ok_or(RelayError::InvalidInput)?;
    let mut candidate = workspace.clone();
    for id in &collection.members {
        if cancel.load(Ordering::Relaxed) {
            return Err(RelayError::Cancelled);
        }
        let entry = candidate
            .subscriptions
            .iter_mut()
            .find(|entry| entry.id == *id)
            .ok_or(RelayError::InvalidInput)?;
        if entry.source.trim().is_empty() {
            continue;
        }
        let mut content = fetch(entry)?;
        if cancel.load(Ordering::Relaxed) {
            return Err(RelayError::Cancelled);
        }
        if entry.content != *content
            && (!entry.processing.nodes.is_empty()
                || workspace.collections.iter().any(|collection| {
                    collection.members.contains(id) && !collection.processing.nodes.is_empty()
                }))
            && !model::semantic_equal(&entry.content, &content)
        {
            return Err(RelayError::SourceChanged);
        }
        std::mem::swap(&mut entry.content, &mut *content);
        candidate.validate().map_err(|_| RelayError::InvalidInput)?;
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(RelayError::Cancelled);
    }
    candidate.validate().map_err(|_| RelayError::InvalidInput)?;
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::super::library::{Collection, Entry};
    use super::*;

    fn workspace() -> Workspace {
        let mut workspace = Workspace::default();
        let mut ids = Vec::new();
        for name in ["one", "two"] {
            ids.push(
                workspace
                    .add_entry(Entry {
                        id: 0,
                        name: name.into(),
                        source: "https://example.com/sub".into(),
                        source_file: String::new(),
                        content: "old".into(),
                        processing: Default::default(),
                        publish: Default::default(),
                    })
                    .unwrap(),
            );
        }
        workspace
            .add_collection(Collection {
                id: 0,
                name: "group".into(),
                members: ids,
                processing: Default::default(),
                publish: Default::default(),
            })
            .unwrap();
        workspace
    }

    #[test]
    fn failure_and_cancellation_preserve_all_original_content() {
        let workspace = workspace();
        let cancel = AtomicBool::new(false);
        let mut calls = 0;
        let result = refresh_with(&workspace, 3, &cancel, |_| {
            calls += 1;
            if calls == 2 {
                Err(RelayError::InputUnavailable)
            } else {
                Ok(Zeroizing::new("new".into()))
            }
        });
        assert!(matches!(result, Err(RelayError::InputUnavailable)));
        assert!(workspace.subscriptions.iter().all(|e| e.content == "old"));
        let result = refresh_with(&workspace, 3, &cancel, |_| {
            cancel.store(true, Ordering::Relaxed);
            Ok(Zeroizing::new("new".into()))
        });
        assert!(matches!(result, Err(RelayError::Cancelled)));
        assert!(workspace.subscriptions.iter().all(|e| e.content == "old"));
    }

    #[test]
    fn successful_refresh_returns_new_workspace() {
        let workspace = workspace();
        let result = refresh_with(&workspace, 3, &AtomicBool::new(false), |_| {
            Ok(Zeroizing::new("new".into()))
        })
        .unwrap();
        assert!(result.subscriptions.iter().all(|e| e.content == "new"));
        assert!(workspace.subscriptions.iter().all(|e| e.content == "old"));
    }

    #[test]
    fn changed_source_with_indexed_edits_is_rejected_atomically() {
        for collection_edit in [false, true] {
            let mut workspace = workspace();
            let edit = super::super::model::editor::NodeEdit {
                index: 0,
                included: false,
                name: None,
            };
            if collection_edit {
                workspace.collections[0].processing.nodes.push(edit);
            } else {
                workspace.subscriptions[1].processing.nodes.push(edit);
            }
            let result = refresh_with(&workspace, 3, &AtomicBool::new(false), |entry| {
                Ok(Zeroizing::new(
                    if entry.name == "two" { "new" } else { "old" }.into(),
                ))
            });
            assert!(matches!(result, Err(RelayError::SourceChanged)));
            assert!(
                workspace
                    .subscriptions
                    .iter()
                    .all(|entry| entry.content == "old")
            );
            assert_eq!(
                workspace.subscriptions[1].processing.nodes.len()
                    + workspace.collections[0].processing.nodes.len(),
                1
            );
        }
    }

    #[test]
    fn shared_member_refresh_preserves_other_collections_edits_across_formatting_changes() {
        let workspace = workspace_with_shared_member_edits();
        let refreshed = refresh_with(&workspace, 3, &AtomicBool::new(false), |_| {
            Ok(Zeroizing::new(
                "# updated comment\nA=socks5,a.example.invalid,1080\nB=socks5,b.example.invalid,1080\n"
                    .into(),
            ))
        })
        .unwrap();
        let other = &refreshed.collections[1];
        assert_eq!(other.processing.nodes.len(), 1);
        assert!(!other.processing.nodes[0].included);
        assert!(
            refreshed.subscriptions[0]
                .content
                .starts_with("# updated comment")
        );
    }

    #[test]
    fn shared_member_refresh_rejects_reordered_nodes_with_other_collections_edits() {
        let workspace = workspace_with_shared_member_edits();
        let result = refresh_with(&workspace, 3, &AtomicBool::new(false), |_| {
            Ok(Zeroizing::new(
                "B = socks5, b.example.invalid, 1080\nA = socks5, a.example.invalid, 1080\n".into(),
            ))
        });
        assert!(matches!(result, Err(RelayError::SourceChanged)));
    }

    fn workspace_with_shared_member_edits() -> Workspace {
        let mut workspace = workspace();
        for entry in &mut workspace.subscriptions {
            entry.content =
                "A = socks5, a.example.invalid, 1080\nB = socks5, b.example.invalid, 1080\n".into();
        }
        let mut other = workspace.collections[0].clone();
        other.name = "other group".into();
        other
            .processing
            .nodes
            .push(super::super::model::editor::NodeEdit {
                index: 0,
                included: false,
                name: None,
            });
        workspace.add_collection(other).unwrap();
        workspace
    }
}
