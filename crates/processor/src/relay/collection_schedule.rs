//! Refresh every remote member before publishing one collection.

use std::sync::atomic::AtomicBool;

use zeroize::{Zeroize, Zeroizing};

use super::{
    Progress, Publication, RelayConfig, RelayError, RelayResult, RelayStage, RunMode, active, gist,
    input,
    library::{Entry, Workspace},
    library_refresh, model, publication_payload, publish_result, read_destination,
    validate_prepared,
};

pub struct Completed {
    pub workspace: Workspace,
    pub base: Zeroizing<String>,
    pub result: RelayResult,
}

pub(super) struct Prepared {
    pub(super) workspace: Workspace,
    pub(super) base: Zeroizing<String>,
    pub(super) result: RelayResult,
    sources: Vec<(u64, Option<gist::GistFile>)>,
}

pub(crate) fn edits_depend_on_remote_order(entry: &Entry, collection_edits: bool) -> bool {
    !entry.source.trim().is_empty() && (collection_edits || !entry.processing.nodes.is_empty())
}

/// Check the frozen workspace, destination, and edit bindings without reading remote data.
pub fn validate(workspace: &Workspace, id: u64, config: &RelayConfig) -> Result<(), RelayError> {
    workspace.validate().map_err(|_| RelayError::InvalidInput)?;
    let collection = workspace
        .collections
        .iter()
        .find(|collection| collection.id == id)
        .ok_or(RelayError::InvalidInput)?;
    if collection.members.is_empty()
        || !config.source.trim().is_empty()
        || !config.source_file.is_empty()
        || config.output.trim().is_empty()
    {
        return Err(RelayError::InvalidInput);
    }
    config.validate(RunMode::Publish)?;
    let output = gist::GistFile::parse(config.output.trim(), &config.output_file)
        .map_err(RelayError::Gist)?;
    for entry in workspace
        .subscriptions
        .iter()
        .filter(|entry| collection.members.contains(&entry.id))
    {
        if model::pipeline::has_resolution(&entry.processing.pipeline) {
            return Err(RelayError::Model(model::ModelError::Unsupported));
        }
        if edits_depend_on_remote_order(
            entry,
            !config.processing.nodes.is_empty() || !collection.processing.nodes.is_empty(),
        ) {
            return Err(RelayError::InvalidInput);
        }
        if !entry.source.trim().is_empty()
            && matches!(
                input::target(entry.source.trim(), &entry.source_file)?,
                input::Target::Gist(source) if output.id == source.id
            )
        {
            return Err(RelayError::InvalidInput);
        }
    }
    Ok(())
}

/// Prepare and verify the refreshed collection before one existing-Gist publication.
pub fn run(
    workspace: &Workspace,
    id: u64,
    config: &RelayConfig,
    token: &str,
    cancel: &AtomicBool,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<Completed, RelayError> {
    let client = gist::GistClient::new(token).map_err(RelayError::Gist)?;
    run_with(workspace, id, config, &client, cancel, progress)
}

fn run_with(
    workspace: &Workspace,
    id: u64,
    config: &RelayConfig,
    client: &gist::GistClient,
    cancel: &AtomicBool,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<Completed, RelayError> {
    let mut fetch =
        |entry: &Entry| input::read(client, entry.source.trim(), &entry.source_file, cancel);
    let mut prepared = prepare_with(workspace, id, config, cancel, progress, &mut fetch)?;
    let converted = publication_payload(config, &prepared.result, cancel)?;
    let destination = read_destination(config, client, cancel)?.ok_or(RelayError::InvalidInput)?;
    prepared.recheck_with(cancel, &mut fetch)?;
    publish_result(
        config,
        client,
        &mut prepared.result,
        Publication {
            destination: Some(destination),
            converted,
        },
        cancel,
        progress,
    )?;
    progress(Progress {
        stage: RelayStage::Complete,
        checked: prepared.result.total,
        total: prepared.result.total,
    });
    Ok(Completed {
        workspace: prepared.workspace,
        base: prepared.base,
        result: prepared.result,
    })
}

pub(super) fn prepare_with(
    workspace: &Workspace,
    id: u64,
    config: &RelayConfig,
    cancel: &AtomicBool,
    progress: &(dyn Fn(Progress) + Sync),
    fetch: &mut impl FnMut(&Entry) -> Result<input::Source, RelayError>,
) -> Result<Prepared, RelayError> {
    validate(workspace, id, config)?;
    active(cancel)?;
    progress(Progress {
        stage: RelayStage::Read,
        checked: 0,
        total: 0,
    });
    let mut sources = Vec::new();
    let workspace = library_refresh::refresh_with(workspace, id, cancel, |entry| {
        let (file, content) = fetch_member(entry, fetch)?;
        sources.push((entry.id, file));
        Ok(content)
    })?;
    let base = workspace
        .merge(id)
        .map_err(|_| RelayError::Model(model::ModelError::Unsupported))?;
    if !config.processing.nodes.is_empty()
        && !config
            .expected_source
            .as_ref()
            .is_some_and(|expected| model::semantic_equal(&base, expected))
    {
        return Err(RelayError::SourceChanged);
    }
    let mut fresh = config.clone();
    fresh.local_content = Some(base.clone());
    fresh.expected_source = Some(base.clone());
    fresh.script_source = model::script::SourceContext {
        kind: "collection",
        label: workspace
            .collections
            .iter()
            .find(|entry| entry.id == id)
            .and_then(|entry| RelayConfig::script_source_label(&entry.name)),
        member_index: None,
    };
    let result = super::run(&fresh, "", RunMode::Preview, cancel, &|value| {
        if value.stage != RelayStage::Complete {
            progress(value);
        }
    });
    fresh.output.zeroize();
    fresh.output_file.zeroize();
    fresh.title.zeroize();
    let result = result?;
    validate_prepared(config, &result)?;
    Ok(Prepared {
        workspace,
        base,
        result,
        sources,
    })
}

impl Prepared {
    pub(super) fn recheck_with(
        &self,
        cancel: &AtomicBool,
        fetch: &mut impl FnMut(&Entry) -> Result<input::Source, RelayError>,
    ) -> Result<(), RelayError> {
        for (id, expected_file) in &self.sources {
            active(cancel)?;
            let entry = self
                .workspace
                .subscriptions
                .iter()
                .find(|entry| entry.id == *id)
                .ok_or(RelayError::InvalidInput)?;
            let (file, content) = fetch_member(entry, fetch)?;
            let same_file = match (file.as_ref(), expected_file) {
                (Some(file), Some(expected)) => {
                    file.id == expected.id && file.filename == expected.filename
                }
                (None, None) => true,
                _ => false,
            };
            if !same_file || !model::semantic_equal(&content, &entry.content) {
                return Err(RelayError::SourceChanged);
            }
        }
        active(cancel)
    }
}

fn fetch_member(
    entry: &Entry,
    fetch: &mut impl FnMut(&Entry) -> Result<input::Source, RelayError>,
) -> Result<(Option<gist::GistFile>, Zeroizing<String>), RelayError> {
    match (
        input::target(entry.source.trim(), &entry.source_file)?,
        fetch(entry)?,
    ) {
        (input::Target::Gist(target), input::Source::Gist(gist::Source { file, content }))
            if target.id == file.id
                && (target.filename.is_empty() || target.filename == file.filename) =>
        {
            Ok((Some(file), content))
        }
        (input::Target::Https(target), input::Source::Https { url, content })
            if target.as_str() == url.as_str() =>
        {
            Ok((None, content))
        }
        _ => Err(RelayError::SourceChanged),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::{
        gist::tests::{Fixture, ID as DESTINATION, INPUT_ID as GIST, api, document, response},
        library::Collection,
    };

    const OLD: &str = "Old = socks5, old.example, 1080\n";
    const NEW: &str = "New = socks5, new.example, 1080\n";

    fn fixture() -> (Workspace, u64, RelayConfig) {
        let mut workspace = Workspace::default();
        let mut members = Vec::new();
        for (name, source) in [("remote", GIST), ("local", "")] {
            members.push(
                workspace
                    .add_entry(Entry {
                        id: 0,
                        name: name.into(),
                        source: source.into(),
                        source_file: String::new(),
                        content: OLD.into(),
                        processing: Default::default(),
                        publish: Default::default(),
                    })
                    .unwrap(),
            );
        }
        let id = workspace
            .add_collection(Collection {
                id: 0,
                name: "group".into(),
                members,
                processing: Default::default(),
                publish: Default::default(),
            })
            .unwrap();
        let config = RelayConfig {
            local_content: Some(workspace.merge(id).unwrap()),
            output: DESTINATION.into(),
            rename: false,
            sort: false,
            ..Default::default()
        };
        (workspace, id, config)
    }

    fn source(filename: &str, content: &str) -> input::Source {
        input::Source::Gist(gist::Source {
            file: gist::GistFile::parse(GIST, filename).unwrap(),
            content: Zeroizing::new(content.into()),
        })
    }

    #[test]
    fn prepare_uses_fresh_remote_data_without_changing_local_or_original_cache() {
        let (workspace, id, config) = fixture();
        let prepared = prepare_with(
            &workspace,
            id,
            &config,
            &AtomicBool::new(false),
            &|_| {},
            &mut |entry| {
                assert_eq!(entry.name, "remote");
                Ok(source("nodes.list", NEW))
            },
        )
        .unwrap();
        assert_eq!(prepared.workspace.subscriptions[0].content, NEW);
        assert_eq!(prepared.workspace.subscriptions[1].content, OLD);
        assert_eq!(workspace.subscriptions[0].content, OLD);
        assert_eq!(prepared.result.total, 2);
        assert!(prepared.result.content.contains("new.example"));
    }

    #[test]
    fn same_gist_as_any_member_is_rejected_before_fetch() {
        let (workspace, id, mut config) = fixture();
        config.output = format!("https://gist.github.com/example/{GIST}");
        assert!(matches!(
            prepare_with(
                &workspace,
                id,
                &config,
                &AtomicBool::new(false),
                &|_| {},
                &mut |_| panic!("must reject the source alias before fetching"),
            ),
            Err(RelayError::InvalidInput)
        ));
    }

    #[test]
    fn recheck_rejects_a_changed_file_even_with_identical_nodes() {
        let (workspace, id, config) = fixture();
        let cancel = AtomicBool::new(false);
        let prepared = prepare_with(&workspace, id, &config, &cancel, &|_| {}, &mut |_| {
            Ok(source("nodes.list", NEW))
        })
        .unwrap();
        assert!(matches!(
            prepared.recheck_with(&cancel, &mut |_| Ok(source("other.list", NEW))),
            Err(RelayError::SourceChanged)
        ));
    }

    #[test]
    fn recheck_rejects_changed_nodes() {
        let (workspace, id, config) = fixture();
        let cancel = AtomicBool::new(false);
        let prepared = prepare_with(&workspace, id, &config, &cancel, &|_| {}, &mut |_| {
            Ok(source("nodes.list", NEW))
        })
        .unwrap();
        assert!(matches!(
            prepared.recheck_with(&cancel, &mut |_| Ok(source("nodes.list", OLD))),
            Err(RelayError::SourceChanged)
        ));
    }

    #[test]
    fn remote_members_prevent_unattended_index_edits() {
        let (workspace, id, mut config) = fixture();
        config.processing.nodes.push(model::editor::NodeEdit {
            index: 0,
            included: false,
            name: None,
        });
        assert!(matches!(
            validate(&workspace, id, &config),
            Err(RelayError::InvalidInput)
        ));
    }

    #[test]
    fn changed_member_after_destination_read_never_starts_a_gist_write() {
        let (mut workspace, id, mut config) = fixture();
        workspace.subscriptions[0].source =
            format!("https://gist.githubusercontent.com/example/{GIST}/raw/nodes.txt");
        config.output_file = "nodes.txt".into();
        let fixture = Fixture::new(vec![
            Some(response(200, "Content-Type: text/plain\r\n", NEW)),
            api(document(OLD)),
            Some(response(200, "Content-Type: text/plain\r\n", OLD)),
        ]);
        assert!(matches!(
            run_with(
                &workspace,
                id,
                &config,
                &fixture.client(),
                &AtomicBool::new(false),
                &|_| {},
            ),
            Err(RelayError::SourceChanged)
        ));
        assert_eq!(fixture.request_methods(), ["GET", "GET", "GET"]);
    }

    #[test]
    fn incompatible_collection_format_stops_before_destination_and_final_source_reads() {
        let (mut workspace, id, mut config) = fixture();
        workspace.subscriptions[0].source =
            format!("https://gist.githubusercontent.com/example/{GIST}/raw/nodes.txt");
        workspace.subscriptions[1].content = "Old = http, old.example, 80\n".into();
        config.output_file = "nodes.txt".into();
        config.output_format = super::super::export::OutputFormat::Mihomo;
        let fixture = Fixture::new(vec![Some(response(
            200,
            "Content-Type: text/plain\r\n",
            NEW,
        ))]);
        assert!(matches!(
            run_with(
                &workspace,
                id,
                &config,
                &fixture.client(),
                &AtomicBool::new(false),
                &|_| {},
            ),
            Err(RelayError::ExportNode { index: 2, .. })
        ));
        assert_eq!(fixture.request_methods(), ["GET"]);
    }

    #[test]
    fn successful_run_updates_one_gist_and_returns_fresh_cache_after_readback() {
        let (mut workspace, id, mut config) = fixture();
        workspace.subscriptions[0].source =
            format!("https://gist.githubusercontent.com/example/{GIST}/raw/nodes.txt");
        config.output_file = "nodes.txt".into();
        config.title = "relay".into();
        config.output_format = super::super::export::OutputFormat::Mihomo;
        let cancel = AtomicBool::new(false);
        let preview = prepare_with(&workspace, id, &config, &cancel, &|_| {}, &mut |_| {
            Ok(source("nodes.txt", NEW))
        })
        .unwrap();
        let exported = super::super::export::export_subscription(
            &preview.result.content,
            config.output_format,
        )
        .unwrap();
        assert_ne!(exported.as_str(), preview.result.content.as_str());
        let fixture = Fixture::new(vec![
            Some(response(200, "Content-Type: text/plain\r\n", NEW)),
            api(document(OLD)),
            Some(response(200, "Content-Type: text/plain\r\n", NEW)),
            api(document(OLD)),
            Some(response(200, "", "")),
            api(document(&exported)),
        ]);
        let completed =
            run_with(&workspace, id, &config, &fixture.client(), &cancel, &|_| {}).unwrap();
        assert!(completed.result.published);
        assert_eq!(completed.result.content, preview.result.content);
        assert_eq!(completed.workspace.subscriptions[0].content, NEW);
        assert_eq!(
            fixture.request_methods(),
            ["GET", "GET", "GET", "GET", "PATCH", "GET"]
        );
    }
}
