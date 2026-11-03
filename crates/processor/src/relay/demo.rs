//! Offline smoke demonstration using only synthetic subscriptions and Gist snapshots.

use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use serde::Serialize;
use zeroize::Zeroizing;

use super::{
    RelayConfig, RelayError, collection_schedule,
    export::{OutputFormat, export_subscription},
    gist::{self, GistError, Snapshot},
    input::Source,
    library::{Collection, Entry, Workspace},
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
const CACHED: &str = "Cached = http, cached.invalid, 8080\n";
const FRESH: &str =
    "Primary = http, primary.invalid, 8080\nExclude = http, excluded.invalid, 8080\n";
const SECONDARY: &str =
    r#"[{"type":"http","tag":"Secondary","server":"secondary.invalid","server_port":8080}]"#;

/// Counts and safety checks from a simulated publication. No output was actually written.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct DemoReport {
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
    let (workspace, id) = fixture()?;
    let collection = workspace
        .collections
        .first()
        .ok_or(RelayError::InvalidInput)?;
    let config = RelayConfig {
        local_content: Some(workspace.merge(id).map_err(|_| RelayError::InvalidInput)?),
        processing: collection.processing.clone(),
        output: GIST_DESTINATION.into(),
        rename: false,
        sort: false,
        ..RelayConfig::default()
    };
    let cancel = AtomicBool::new(false);
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
    let exported = export_subscription(&result.content, OutputFormat::SingBox)?;
    let subscription = Subscription::parse(&exported).map_err(RelayError::Model)?;
    if !subscription
        .nodes()
        .iter()
        .map(super::model::Node::name)
        .eq(["1: Ready", "2: Secondary"])
    {
        return Err(RelayError::Model(ModelError::Render));
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
        description: config.title,
        public: false,
    };
    let skipped_unchanged = !gist::needs_publish(
        &simulated,
        &simulated,
        &result.content,
        &simulated.description,
        false,
    )
    .map_err(RelayError::Gist)?;
    if planned_uploads != 1 || !skipped_unchanged {
        return Err(RelayError::Gist(GistError::VerificationFailed));
    }
    check_schedule()?;
    Ok(DemoReport {
        members: collection.members.len(),
        refreshed_members,
        input_nodes: result.total,
        removed_nodes: result.removed,
        exported_nodes: subscription.nodes().len(),
        planned_uploads,
        skipped_unchanged,
        source_change_blocked,
    })
}

fn fixture() -> Result<(Workspace, u64), RelayError> {
    let mut workspace = Workspace::default();
    let members = [
        ("Primary", HTTPS_SOURCE, "", CACHED),
        ("Secondary", GIST_SOURCE, "demo.json", SECONDARY),
    ]
    .into_iter()
    .map(|(name, source, source_file, content)| {
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
    ]);
    let id = workspace
        .add_collection(Collection {
            id: 0,
            name: "Demo".into(),
            members,
            processing,
            publish: Default::default(),
        })
        .map_err(|_| RelayError::InvalidInput)?;
    Ok((workspace, id))
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

fn check_schedule() -> Result<(), RelayError> {
    let now = Instant::now();
    let mut schedule = Schedule::new(MIN_INTERVAL, now)?;
    let due = now + MIN_INTERVAL;
    if schedule.begin(now) || !schedule.begin(due) || schedule.begin(due) {
        return Err(RelayError::InvalidInput);
    }
    schedule.finish_status(due, Ok(()));
    let next = due + MIN_INTERVAL;
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
}
