//! Offline smoke demonstration using only synthetic subscriptions and Gist snapshots.

use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use serde::Serialize;
use zeroize::Zeroizing;

use super::{
    RelayConfig, RelayError, RunMode, collection_schedule, compat,
    export::{OutputFormat, export_subscription},
    gist::{self, GistError, Snapshot},
    input::Source,
    library::{Collection, Entry, Workspace},
    library_import,
    model::{
        ModelError, Subscription,
        editor::Options,
        pipeline::{Operation, Step},
    },
    schedule::{MIN_INTERVAL, Schedule},
};

const HTTPS_SOURCE: &str = "https://subscription.invalid/demo";
const GIST_SOURCE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const GIST_DESTINATION: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CACHED: &str = "Cached = socks5, cached.invalid, 1080\n";
const FRESH: &str =
    "Primary = socks5, primary.invalid, 1080\nExclude = socks5, excluded.invalid, 1080\n";
const OFFLINE_PRIMARY: &str =
    "Primary = socks5, primary.invalid, 1080\nAlternate = socks5, primary.invalid, 1080\n";
const SECONDARY: &str = r#"[{"type":"socks","tag":"Secondary","server":"secondary.invalid","server_port":1080,"network":"tcp"}]"#;
const SUBSTORE_BACKUP: &[u8] = br#"{
  "subs": [
    {
      "name": "Backup Primary",
      "source": "local",
      "content": "Primary = socks5, primary.invalid, 1080\nExclude = socks5, excluded.invalid, 1080\n",
      "process": [{"type":"Regex Rename Operator","args":[{"expr":"Primary","now":"Ready"}]}]
    },
    {
      "name": "Backup Secondary",
      "source": "local",
      "content": "Secondary = socks5, secondary.invalid, 1080\n"
    }
  ],
  "collections": [
    {
      "name": "Backup Demo",
      "subscriptions": ["Backup Primary", "Backup Secondary"],
      "process": [{"type":"Regex Filter","args":{"regex":["Exclude$"],"keep":false}}]
    }
  ]
}"#;

/// An original-format Sub-Store backup containing only local, synthetic sample nodes.
pub fn backup() -> &'static [u8] {
    SUBSTORE_BACKUP
}

/// Counts and safety checks from a simulated publication. No output was actually written.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct DemoReport {
    pub backup_subscriptions: usize,
    pub backup_collections: usize,
    pub scheduled_runs: usize,
    pub members: usize,
    pub refreshed_members: usize,
    pub input_nodes: usize,
    pub removed_nodes: usize,
    pub exported_nodes: usize,
    pub planned_uploads: usize,
    pub skipped_unchanged: bool,
    pub source_change_blocked: bool,
}

/// Exercise real relay preparation, export, publication preflight, and scheduling without I/O.
pub fn run() -> Result<DemoReport, RelayError> {
    let imported = compat::import(backup()).map_err(|_| RelayError::InvalidInput)?;
    if imported.unsupported_count != 0 {
        return Err(RelayError::InvalidInput);
    }
    let (workspace, imported) = library_import::import(&Workspace::default(), &imported, true)
        .map_err(|_| RelayError::InvalidInput)?;
    if imported.skipped != 0 {
        return Err(RelayError::InvalidInput);
    }
    let workspace = Workspace::decode(&workspace.encode().map_err(|_| RelayError::InvalidInput)?)
        .map_err(|_| RelayError::InvalidInput)?;
    let collection = workspace
        .collections
        .first()
        .ok_or(RelayError::InvalidInput)?;
    let content = workspace
        .merge(collection.id)
        .map_err(|_| RelayError::InvalidInput)?;
    let preview = super::run(
        &RelayConfig {
            expected_source: Some(content.clone()),
            local_content: Some(content),
            processing: collection.processing.clone(),
            script_source: super::model::script::SourceContext {
                kind: "collection",
                label: Some(collection.name.clone()),
                member_index: None,
            },
            rename: false,
            sort: false,
            test_nodes: false,
            ..Default::default()
        },
        "",
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
    )?;
    if preview.total != 3 || preview.processed != 2 || preview.published || preview.tested {
        return Err(RelayError::Model(ModelError::Render));
    }
    let workspace = fixture([
        (HTTPS_SOURCE, "", CACHED),
        (GIST_SOURCE, "demo.json", SECONDARY),
    ])?;
    let collection = workspace
        .collections
        .first()
        .ok_or(RelayError::InvalidInput)?;
    let id = collection.id;
    let config = RelayConfig {
        local_content: Some(workspace.merge(id).map_err(|_| RelayError::InvalidInput)?),
        processing: collection.processing.clone(),
        output: GIST_DESTINATION.into(),
        rename: false,
        sort: false,
        ..RelayConfig::default()
    };
    let cancel = AtomicBool::new(false);
    let now = Instant::now();
    let mut schedule = Schedule::new(MIN_INTERVAL, now)?;
    let due = now + MIN_INTERVAL;
    if schedule.begin(now) || !schedule.begin(due) || schedule.begin(due) {
        return Err(RelayError::InvalidInput);
    }
    let mut refreshed_members = 0;
    let prepared = collection_schedule::prepare_with(
        &workspace,
        id,
        &config,
        &cancel,
        &|_| {},
        &mut |entry| {
            refreshed_members += 1;
            fetch(entry)
        },
    )?;
    let result = &prepared.result;
    let expected_names = ["1: Ready", "2: Secondary"];
    for format in [
        OutputFormat::Surge,
        OutputFormat::SingBox,
        OutputFormat::Mihomo,
    ] {
        let exported = export_subscription(&result.content, format)?;
        let subscription = Subscription::parse(&exported).map_err(RelayError::Model)?;
        if !subscription
            .nodes()
            .iter()
            .map(super::model::Node::name)
            .eq(expected_names)
        {
            return Err(RelayError::Model(ModelError::Render));
        }
    }
    prepared.recheck_with(&cancel, &mut fetch)?;
    let source_change_blocked = match prepared.recheck_with(&cancel, &mut |entry| {
        if entry.source == HTTPS_SOURCE {
            Ok(https_source(CACHED))
        } else {
            fetch(entry)
        }
    }) {
        Err(RelayError::SourceChanged) => true,
        Err(error) => return Err(error),
        Ok(()) => return Err(RelayError::SourceChanged),
    };

    let previous = Snapshot {
        content: config.local_content.clone(),
        revision: "synthetic-before".into(),
        description: config.title.clone(),
        public: false,
    };
    let planned_uploads = usize::from(
        gist::needs_publish(&previous, &previous, &result.content, &config.title, false)
            .map_err(RelayError::Gist)?,
    );
    let simulated = Snapshot {
        content: Some(result.content.clone()),
        revision: "synthetic-after".into(),
        description: config.title.clone(),
        public: false,
    };
    schedule.finish_status(due, Ok(()));
    let next = due + MIN_INTERVAL;
    if schedule.begin(due) || !schedule.begin(next) || schedule.begin(next) {
        return Err(RelayError::InvalidInput);
    }
    let repeated = collection_schedule::prepare_with(
        &prepared.workspace,
        id,
        &config,
        &cancel,
        &|_| {},
        &mut fetch,
    )?;
    repeated.recheck_with(&cancel, &mut fetch)?;
    let skipped_unchanged = !gist::needs_publish(
        &simulated,
        &simulated,
        &repeated.result.content,
        &simulated.description,
        false,
    )
    .map_err(RelayError::Gist)?;
    if planned_uploads != 1 || !skipped_unchanged {
        return Err(RelayError::Gist(GistError::VerificationFailed));
    }
    schedule.finish_status(next, Ok(()));
    check_schedule(schedule, next)?;
    Ok(DemoReport {
        backup_subscriptions: imported.subscriptions,
        backup_collections: imported.collections,
        scheduled_runs: 2,
        members: collection.members.len(),
        refreshed_members,
        input_nodes: result.total,
        removed_nodes: result.removed,
        exported_nodes: expected_names.len(),
        planned_uploads,
        skipped_unchanged,
        source_change_blocked,
    })
}

/// Synthetic local entries for interactive previews, with no upstream or publication targets.
pub fn workspace() -> Result<Workspace, RelayError> {
    fixture([("", "", OFFLINE_PRIMARY), ("", "", SECONDARY)])
}

fn fixture(sources: [(&str, &str, &str); 2]) -> Result<Workspace, RelayError> {
    let mut workspace = Workspace::default();
    let members = ["Primary", "Secondary"]
        .into_iter()
        .zip(sources)
        .map(|(name, (source, source_file, content))| {
            workspace.add_entry(Entry {
                id: 0,
                name: name.into(),
                source: source.into(),
                source_file: source_file.into(),
                content: content.into(),
                processing: Options::default(),
                publish: Default::default(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| RelayError::InvalidInput)?;
    let mut processing = Options::default();
    processing.pipeline.extend([
        Step {
            id: 1,
            enabled: true,
            operation: Operation::NameFilter {
                pattern: "Exclude$".into(),
                keep: false,
            },
        },
        Step {
            id: 2,
            enabled: true,
            operation: Operation::ReplaceName {
                pattern: "Primary".into(),
                replacement: "Ready".into(),
                regex: false,
            },
        },
        Step {
            id: 3,
            enabled: true,
            operation: Operation::Deduplicate,
        },
    ]);
    workspace
        .add_collection(Collection {
            id: 0,
            name: "Demo".into(),
            members,
            processing,
            publish: Default::default(),
        })
        .map_err(|_| RelayError::InvalidInput)?;
    Ok(workspace)
}

fn fetch(entry: &Entry) -> Result<Source, RelayError> {
    match entry.source.as_str() {
        HTTPS_SOURCE => Ok(https_source(FRESH)),
        GIST_SOURCE => Ok(Source::Gist(gist::Source {
            file: gist::GistFile::parse_source(GIST_SOURCE, &entry.source_file)
                .map_err(RelayError::Gist)?,
            content: Zeroizing::new(SECONDARY.into()),
        })),
        _ => Err(RelayError::InvalidInput),
    }
}

fn https_source(content: &str) -> Source {
    Source::Https {
        url: Zeroizing::new(HTTPS_SOURCE.into()),
        content: Zeroizing::new(content.into()),
    }
}

fn check_schedule(mut schedule: Schedule, last_success: Instant) -> Result<(), RelayError> {
    let next = last_success + MIN_INTERVAL;
    if schedule.next_due() != Some(next) || !schedule.begin(next) {
        return Err(RelayError::InvalidInput);
    }
    let rate_limit = RelayError::Gist(GistError::RateLimited {
        retry_after_seconds: Some(90),
    });
    schedule.finish_status(next, Err(&rate_limit));
    let retry = next + Duration::from_secs(90);
    if schedule.next_due() != Some(retry) || schedule.begin(next) || !schedule.begin(retry) {
        return Err(RelayError::InvalidInput);
    }
    schedule.finish_status(retry, Err(&RelayError::Gist(GistError::UncertainWrite)));
    if !schedule.is_paused() || schedule.next_due().is_some() {
        return Err(RelayError::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_reports_only_synthetic_results() {
        assert_eq!(
            run().unwrap(),
            DemoReport {
                backup_subscriptions: 2,
                backup_collections: 1,
                scheduled_runs: 2,
                members: 2,
                refreshed_members: 2,
                input_nodes: 3,
                removed_nodes: 1,
                exported_nodes: 2,
                planned_uploads: 1,
                skipped_unchanged: true,
                source_change_blocked: true,
            }
        );
    }

    #[test]
    fn interactive_workspace_previews_and_deduplicates_without_remote_targets() {
        let workspace = workspace().unwrap();
        assert_eq!(workspace.subscriptions.len(), 2);
        assert_eq!(workspace.collections.len(), 1);
        assert!(
            workspace
                .subscriptions
                .iter()
                .all(|entry| entry.source.is_empty()
                    && entry.source_file.is_empty()
                    && entry.publish.output.is_empty())
        );
        let collection = &workspace.collections[0];
        assert!(collection.publish.output.is_empty());
        let content = workspace.merge(collection.id).unwrap();
        let result = super::super::run(
            &RelayConfig {
                expected_source: Some(content.clone()),
                local_content: Some(content),
                processing: collection.processing.clone(),
                rename: false,
                sort: false,
                test_nodes: false,
                ..Default::default()
            },
            "",
            super::super::RunMode::Preview,
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap();
        assert_eq!(
            (result.total, result.processed, result.duplicates),
            (3, 2, 1)
        );
        assert!(!result.published && !result.tested && result.source.is_none());
    }
}
