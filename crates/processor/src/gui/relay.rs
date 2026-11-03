//! Session-only relay state; deliberately separate from preferences and job history.

use super::file_dialog::FileAction;
use super::i18n::Text;
use crate::relay::export::{OutputFormat, export_subscription};
use crate::relay::gist::{GistFile, validate_filename, validate_name};
use crate::relay::{Progress, RelayConfig, RelayError, RelayResult, RunMode};
use eframe::egui;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

pub(super) const SECRET_IDS: [&str; 8] = [
    "relay-token",
    "relay-source",
    "relay-source-file",
    "relay-output",
    "relay-output-file",
    "relay-title",
    "relay-age-identity",
    "relay-age-recipient",
];

pub(super) enum Failure {
    Validation(Text),
    Operation(crate::relay::RelayError),
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum RelayStep {
    #[default]
    Source,
    Review,
    Publish,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum InputMode {
    #[default]
    Link,
    Paste,
    File,
}

pub(super) fn failure_message(
    language: crate::runtime::settings::Language,
    failure: &Failure,
) -> String {
    use crate::relay::model::pipeline::ResolveError;
    use crate::relay::{engine::EngineError, gist::GistError, model::ModelError};
    let key = match failure {
        Failure::Validation(key) => *key,
        Failure::Operation(error) => match error {
            RelayError::InvalidInput => Text::RelayConfigInvalid,
            RelayError::InputUnavailable => Text::RelayInputUnavailable,
            RelayError::InputInvalid => Text::RelaySubscriptionInvalid,
            RelayError::InputHttpStatus(status) => {
                return format!(
                    "{} {status}",
                    super::i18n::text(language, Text::RelayInputStatus)
                );
            }
            RelayError::InputRateLimited {
                retry_after_seconds,
            } => {
                return match retry_after_seconds {
                    Some(seconds) => format!(
                        "{} {seconds} s",
                        super::i18n::text(language, Text::RelayInputLimited)
                    ),
                    None => super::i18n::text(language, Text::RelayInputLimited).to_owned(),
                };
            }
            RelayError::NoHealthyNodes => Text::RelayNoHealthy,
            RelayError::SourceChanged => Text::RelaySourceChanged,
            RelayError::Cancelled => Text::RelayCancelled,
            RelayError::ExportFormat => Text::RelayExportFormatMismatch,
            RelayError::ExportNode { index, .. } => {
                return format!(
                    "{} #{index}",
                    super::i18n::text(language, Text::RelayExportNodeFailed)
                );
            }
            RelayError::Model(error) => match error {
                ModelError::InvalidInput => Text::RelaySubscriptionInvalid,
                ModelError::Unsupported => Text::RelayUnsupported,
                ModelError::SecurityBlocked => Text::RelaySecurityBlocked,
                ModelError::InvalidDependency => Text::RelayDependencyInvalid,
                ModelError::InvalidEdit => Text::RelayEditInvalid,
                ModelError::InvalidHealth | ModelError::Render => Text::RelayRenderFailed,
                ModelError::Resolution(error) => match error {
                    ResolveError::ConsentRequired => Text::ResolutionConsentRequired,
                    ResolveError::Cancelled => Text::ResolutionCancelled,
                    ResolveError::Unavailable => Text::ResolutionUnavailable,
                    ResolveError::InvalidResponse => Text::ResolutionResponseInvalid,
                    ResolveError::Budget => Text::ResolutionBudget,
                    ResolveError::Dnssec => Text::ResolutionDnssec,
                    ResolveError::InvalidOptions => Text::ResolutionOptionsInvalid,
                    ResolveError::RateLimited {
                        retry_after_seconds,
                    } => {
                        return format!(
                            "{} {retry_after_seconds} s",
                            super::i18n::text(language, Text::ResolutionRateLimited)
                        );
                    }
                },
            },
            RelayError::Engine(error) => match error {
                EngineError::InvalidOptions => Text::RelayOptionsInvalid,
                EngineError::Unavailable => Text::RelayEngineUnavailable,
                EngineError::Cancelled => Text::RelayCancelled,
                EngineError::Infrastructure => Text::RelayEngineFailed,
            },
            RelayError::Gist(error) => match error {
                GistError::InvalidTarget => Text::RelayConfigInvalid,
                GistError::PinnedTarget => Text::RelayPinnedTarget,
                GistError::InvalidName => Text::RelayNameInvalid,
                GistError::InvalidToken => Text::RelayTokenInvalid,
                GistError::Cancelled => Text::RelayCancelled,
                GistError::Transport => Text::RelayConnectionFailed,
                GistError::RateLimited {
                    retry_after_seconds,
                } => {
                    return match retry_after_seconds {
                        Some(seconds) => format!(
                            "{} {seconds} s",
                            super::i18n::text(language, Text::RelayRateLimited)
                        ),
                        None => {
                            super::i18n::text(language, Text::RelayRateLimitedUnknown).to_owned()
                        }
                    };
                }
                GistError::HttpStatus(status) => {
                    return format!(
                        "{} {status}",
                        super::i18n::text(language, Text::RelayHttpStatus)
                    );
                }
                GistError::InvalidResponse => Text::RelayResponseInvalid,
                GistError::TooLarge => Text::RelayTooLarge,
                GistError::IncompleteFiles => Text::RelayFileListIncomplete,
                GistError::SelectFile(_) => Text::RelaySelectFile,
                GistError::Conflict => Text::RelayOutputChanged,
                GistError::PublicOutput => Text::RelayPublicOutput,
                GistError::UncertainWrite => Text::RelayWriteUncertain,
                GistError::VerificationFailed => Text::RelayReadbackFailed,
                GistError::CreatedUnverified(created) => {
                    return format!(
                        "{} {}",
                        super::i18n::text(language, Text::RelayReadbackFailed),
                        created.url
                    );
                }
            },
        },
    };
    super::i18n::text(language, key).to_owned()
}

fn archive_error(error: crate::relay::archive::ArchiveError) -> Text {
    use crate::relay::archive::ArchiveError;
    match error {
        ArchiveError::TooLarge => Text::RelayTooLarge,
        ArchiveError::InvalidKey => Text::ArchiveKeyInvalid,
        ArchiveError::Unsupported => Text::ArchiveUnsupported,
        ArchiveError::InvalidArchive | ArchiveError::EncryptionFailed => Text::ArchiveInvalid,
    }
}

fn decode_import(bytes: &[u8]) -> Result<FileCompleted, Text> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(Text::RelayTooLarge);
    }
    if crate::relay::archive::detect(bytes) {
        return Ok(FileCompleted::Locked(Zeroizing::new(bytes.to_vec())));
    }
    let content = std::str::from_utf8(bytes).map_err(|_| Text::RelayBackupInvalid)?;
    if content.len() <= 4 * 1024 * 1024 && crate::relay::model::Subscription::parse(content).is_ok()
    {
        return Ok(FileCompleted::Nodes(Zeroizing::new(content.to_owned())));
    }
    if let Ok(backup) = crate::relay::backup::decode(bytes) {
        return Ok(FileCompleted::Imported(Box::new(backup)));
    }
    crate::relay::compat::import(bytes)
        .map(FileCompleted::Migration)
        .map_err(|_| Text::RelayBackupInvalid)
}

pub(super) struct RelayState {
    pub(super) demo_mode: bool,
    pub workspace: super::subscriptions::State,
    pub step: RelayStep,
    pub input_mode: InputMode,
    pub paste: Zeroizing<String>,
    pub reveal_paste: bool,
    pub schedule: super::relay_schedule::State,
    pub selected_engine: Option<std::path::PathBuf>,
    pub reveal_source: bool,
    pub reveal_token: bool,
    pub reveal_output: bool,
    pub archive_identity: Zeroizing<String>,
    pub reveal_identity: bool,
    pub archive_recipient: Zeroizing<String>,
    pub encrypted_import: Option<Zeroizing<Vec<u8>>>,
    pub migration: Option<Arc<crate::relay::compat::Report>>,
    pub migration_ack: bool,
    cooldown_until: Option<Instant>,
    pub local_content: Option<Zeroizing<String>>,
    pub local_summary: Option<(usize, &'static str)>,
    pub token: Zeroizing<String>,
    pub source: Zeroizing<String>,
    pub source_file: Zeroizing<String>,
    pub output: Zeroizing<String>,
    pub output_file: Zeroizing<String>,
    pub title: Zeroizing<String>,
    pub concurrency: usize,
    pub timeout_seconds: u64,
    pub compatibility: bool,
    pub doh: bool,
    pub allow_resolution: bool,
    pub rename: bool,
    pub sort: bool,
    pub test_nodes: bool,
    pub editor: Option<EditorState>,
    pub preview_names: Vec<Zeroizing<String>>,
    pub preview_endpoints: Vec<Zeroizing<String>>,
    pub allow_public_output: bool,
    pub acknowledge_publish: bool,
    pub allow_plaintext: bool,
    pub result: Option<RelayResult>,
    pub result_format: Option<&'static str>,
    pub output_format: OutputFormat,
    pub publish_format: OutputFormat,
    pub demo_report: Option<Result<crate::relay::demo::DemoReport, RelayError>>,
    pub progress: Option<Progress>,
    pub error: Option<Failure>,
    pub notice: Option<Text>,
    pending: Option<Pending>,
    pub(super) imported_processing: Option<crate::relay::model::editor::Options>,
    file_picker: Option<super::file_dialog::Picker>,
    file_picker_imports_new: bool,
    last_directory: Option<std::path::PathBuf>,
    file_pending: Option<FilePending>,
    completed_config: Option<PrivateConfig>,
    generation: u64,
}

pub(super) struct EditorRow {
    pub index: usize,
    pub original: Zeroizing<String>,
    pub name: Zeroizing<String>,
    pub protocol: Zeroizing<String>,
    pub included: bool,
}

pub(super) struct EditorState {
    pub source: Zeroizing<String>,
    origin: Option<crate::relay::input::Source>,
    pub rows: Vec<EditorRow>,
    pub options: crate::relay::model::editor::Options,
    pub reveal: bool,
}

impl EditorState {
    fn new(
        source: &str,
        origin: Option<crate::relay::input::Source>,
    ) -> Result<Self, crate::relay::model::ModelError> {
        let subscription = crate::relay::model::Subscription::parse(source)?;
        let rows = subscription
            .nodes()
            .iter()
            .enumerate()
            .map(|(index, node)| EditorRow {
                index,
                original: Zeroizing::new(node.name().to_owned()),
                name: Zeroizing::new(node.name().to_owned()),
                protocol: Zeroizing::new(node.protocol().to_owned()),
                included: true,
            })
            .collect();
        Ok(Self {
            source: Zeroizing::new(source.to_owned()),
            origin,
            rows,
            options: Default::default(),
            reveal: false,
        })
    }

    pub(super) fn processing(&self) -> crate::relay::model::editor::Options {
        let mut options = self.options.clone();
        options.nodes = self
            .rows
            .iter()
            .filter(|row| !row.included || row.name != row.original)
            .map(|row| crate::relay::model::editor::NodeEdit {
                index: row.index,
                included: row.included,
                name: (row.name != row.original).then(|| row.name.to_string()),
            })
            .collect();
        options
    }

    pub fn reset(&mut self) {
        self.options = Default::default();
        for row in &mut self.rows {
            row.included = true;
            row.name.zeroize();
            row.name.push_str(&row.original);
        }
        self.reveal = false;
    }
}

struct FilePending {
    worker: JoinHandle<()>,
    receiver: mpsc::Receiver<Result<Option<FileCompleted>, Text>>,
    generation: u64,
    imports_new: bool,
}

enum FileCompleted {
    ExportFailed(RelayError),
    FormatChecked,
    CopyReady(OutputFormat, Zeroizing<String>),
    Nodes(Zeroizing<String>),
    Imported(Box<crate::relay::backup::RelayBackup>),
    Migration(crate::relay::compat::Report),
    Locked(Zeroizing<Vec<u8>>),
    Plan(Vec<crate::relay::model::pipeline::Step>),
    PlanSaved,
    Saved,
    Engine(std::path::PathBuf),
}

struct Pending {
    mode: RunMode,
    worker: JoinHandle<()>,
    receiver: mpsc::Receiver<Result<RelayResult, RelayError>>,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Option<Progress>>>,
    generation: u64,
}

pub(super) struct PrivateConfig(pub(super) RelayConfig);

impl Drop for PrivateConfig {
    fn drop(&mut self) {
        self.0.source.zeroize();
        self.0.source_file.zeroize();
        self.0.output.zeroize();
        self.0.output_file.zeroize();
        self.0.title.zeroize();
    }
}

impl Default for RelayState {
    fn default() -> Self {
        Self::new(false)
    }
}

impl RelayState {
    pub(super) fn new(demo_mode: bool) -> Self {
        let defaults = RelayConfig::default();
        Self {
            demo_mode,
            step: RelayStep::Source,
            workspace: Default::default(),
            input_mode: InputMode::Link,
            paste: Zeroizing::new(String::new()),
            reveal_paste: false,
            schedule: Default::default(),
            selected_engine: if demo_mode {
                None
            } else {
                crate::relay::engine::discover_engine()
            },
            reveal_source: false,
            reveal_token: false,
            reveal_output: false,
            archive_identity: Zeroizing::new(String::new()),
            reveal_identity: false,
            archive_recipient: Zeroizing::new(String::new()),
            encrypted_import: None,
            migration: None,
            migration_ack: false,
            cooldown_until: None,
            local_content: None,
            local_summary: None,
            token: Zeroizing::new(String::new()),
            source: Zeroizing::new(String::new()),
            source_file: Zeroizing::new(String::new()),
            output: Zeroizing::new(String::new()),
            output_file: Zeroizing::new(defaults.output_file),
            title: Zeroizing::new(defaults.title),
            concurrency: defaults.concurrency,
            timeout_seconds: defaults.timeout_seconds,
            compatibility: false,
            doh: false,
            allow_resolution: false,
            rename: false,
            sort: false,
            test_nodes: true,
            editor: None,
            preview_names: Vec::new(),
            preview_endpoints: Vec::new(),
            allow_public_output: false,
            acknowledge_publish: false,
            allow_plaintext: false,
            result: None,
            result_format: None,
            output_format: OutputFormat::Original,
            publish_format: OutputFormat::Original,
            demo_report: None,
            progress: None,
            error: None,
            notice: None,
            pending: None,
            imported_processing: None,
            file_picker: None,
            file_picker_imports_new: false,
            last_directory: None,
            file_pending: None,
            completed_config: None,
            generation: 0,
        }
    }
    pub(super) fn import_paste(&mut self, _ctx: &egui::Context) {
        if self.busy() {
            return;
        }
        if self.paste.len() > 4 * 1024 * 1024 {
            self.error = Some(Failure::Validation(Text::RelayTooLarge));
            return;
        }
        self.decode_import(Zeroizing::new(self.paste.as_bytes().to_vec()), None);
    }

    pub(super) fn clear_pasted_text(&mut self, ctx: &egui::Context) {
        self.paste.zeroize();
        self.reveal_paste = false;
        super::templates::clear_secret_widget(ctx, "relay-paste");
        self.clear_request_error();
        if self.workspace.error == Some(Text::RelayLoadPastedBeforeSave) {
            self.workspace.error = None;
        }
    }

    pub(super) fn unlock_import(&mut self, ctx: &egui::Context) {
        if self.busy() || self.archive_identity.is_empty() {
            return;
        }
        let Some(bytes) = self.encrypted_import.clone() else {
            return;
        };
        let identity = Zeroizing::new(std::mem::take(&mut *self.archive_identity));
        self.reveal_identity = false;
        super::templates::clear_secret_widget(ctx, SECRET_IDS[6]);
        self.decode_import(bytes, Some(identity));
    }

    fn decode_import(&mut self, bytes: Zeroizing<Vec<u8>>, identity: Option<Zeroizing<String>>) {
        let (sender, receiver) = mpsc::sync_channel(1);
        let generation = self.generation;
        self.error = None;
        self.notice = None;
        match thread::Builder::new()
            .name("script-hub-backup-import".into())
            .spawn(move || {
                let result = if let Some(identity) = identity {
                    crate::relay::archive::decrypt(&bytes, identity.trim())
                        .map_err(archive_error)
                        .and_then(|plaintext| decode_import(&plaintext))
                } else {
                    decode_import(&bytes)
                };
                let _ = sender.send(result.map(Some));
            }) {
            Ok(worker) => {
                self.file_pending = Some(FilePending {
                    worker,
                    receiver,
                    generation,
                    imports_new: false,
                })
            }
            Err(_) => self.error = Some(Failure::Validation(Text::RelayFileError)),
        }
    }

    pub(super) fn select_migration(&mut self, ctx: &egui::Context, index: usize) {
        if self.busy() || !self.migration_ack {
            return;
        }
        let Some(report) = &self.migration else {
            return;
        };
        let Ok(config) = report.select_item(index, self.migration_ack) else {
            self.error = Some(Failure::Validation(Text::RelayBackupInvalid));
            return;
        };
        let mut config = PrivateConfig(config);
        if let Some(content) = config.0.local_content.take() {
            self.apply_backup(
                ctx,
                Box::new(crate::relay::backup::RelayBackup {
                    config: config.0.clone(),
                    content,
                }),
            );
        } else {
            self.clear(ctx);
            self.source = Zeroizing::new(std::mem::take(&mut config.0.source));
            self.notice = Some(Text::MigrationReady);
        }
    }

    pub(super) fn add_preset(
        &mut self,
        ctx: &egui::Context,
        preset: crate::relay::model::entrance::Preset,
    ) {
        let Some(editor) = &self.editor else { return };
        let mut steps = editor.options.pipeline.clone();
        let mut id = steps.iter().map(|step| step.id).max().unwrap_or(0);
        for mut step in crate::relay::model::entrance::steps(preset) {
            let Some(next) = id.checked_add(1) else {
                self.error = Some(Failure::Validation(Text::OperationPlanInvalid));
                return;
            };
            id = next;
            step.id = id;
            steps.push(step);
        }
        if crate::relay::model::pipeline::validate(&steps).is_err() {
            self.error = Some(Failure::Validation(Text::OperationPlanInvalid));
            return;
        }
        self.clear_editor_widgets(ctx);
        if let Some(editor) = &mut self.editor {
            editor.options.pipeline = steps;
        }
        self.invalidate_result();
    }

    pub(super) fn schedule_setup_issue(&self) -> Option<Text> {
        let Some(result) = self
            .result
            .as_ref()
            .filter(|result| !result.content.is_empty())
        else {
            return Some(Text::ScheduleNeedsPreview);
        };
        if result.tested && result.healthy == 0 {
            return Some(Text::RelayNoHealthyHelp);
        }
        if self.output.trim().is_empty() {
            return Some(Text::ScheduleNeedsOutput);
        }
        if let Err(error) = self.validate(RunMode::Publish, self.engine_path()) {
            return Some(error);
        }
        if !self.source.trim().is_empty()
            && self.editor.as_ref().is_some_and(|editor| {
                editor
                    .rows
                    .iter()
                    .any(|row| !row.included || row.name != row.original)
            })
        {
            return Some(Text::ScheduleIndexedEdits);
        }
        None
    }

    pub(super) fn arm_schedule(&mut self) {
        if self.demo_mode || self.busy() || self.selecting_file() || self.cooldown_seconds() > 0 {
            return;
        }
        if let Some(issue) = self.schedule_setup_issue() {
            self.error = Some(Failure::Validation(issue));
            return;
        }
        let local_content = if self.source.trim().is_empty() {
            self.local_content.clone()
        } else {
            None
        };
        let mut config = self.private_config(self.engine_path());
        config.0.local_content = local_content;
        config.0.test_nodes = self.result.as_ref().is_some_and(|result| result.tested);
        let result = if let Some(super::subscriptions::Selection::Collection(id)) =
            self.workspace.selected
        {
            match self.collection_schedule_snapshot(id) {
                Ok((snapshot, id)) => {
                    self.schedule
                        .arm_collection(config.0.clone(), &self.token, snapshot, id)
                }
                Err(_) => {
                    self.error = Some(Failure::Validation(Text::WorkspaceDraftInvalid));
                    return;
                }
            }
        } else {
            self.schedule.arm(config.0.clone(), &self.token)
        };
        if result.is_ok() {
            self.notice = None;
        }
        self.error = result.err().map(Failure::Operation);
    }

    pub(super) fn accept_scheduled_collection(
        &mut self,
        mut completed: crate::relay::collection_schedule::Completed,
    ) {
        for refreshed in &mut completed.workspace.subscriptions {
            if refreshed.source.is_empty() || !self.workspace.members.contains(&refreshed.id) {
                continue;
            }
            if let Some(entry) = self.workspace.data.subscriptions.iter_mut().find(|entry| {
                entry.id == refreshed.id
                    && entry.source == refreshed.source
                    && entry.source_file == refreshed.source_file
            }) && entry.content != refreshed.content
            {
                entry.content.zeroize();
                entry.content = std::mem::take(&mut refreshed.content);
                self.workspace.dirty = true;
            }
        }
        self.local_content = Some(completed.base);
        self.accept_scheduled(completed.result);
    }

    pub(super) fn accept_scheduled(&mut self, result: RelayResult) {
        let content = result
            .source
            .as_ref()
            .map(|source| source.content())
            .or(self.local_content.as_deref().map(String::as_str));
        if let Some(content) = content {
            match EditorState::new(content, result.source.clone()) {
                Ok(mut editor) => {
                    if let Some(old) = &self.editor {
                        editor.options = old.options.clone();
                    }
                    if result.source.is_some()
                        && let Some(cached) = &mut self.local_content
                        && cached.as_str() != editor.source.as_str()
                    {
                        *cached = editor.source.clone();
                        self.workspace.draft_dirty |= self.workspace.selected.is_some();
                    }
                    self.editor = Some(editor);
                }
                Err(error) => {
                    self.error = Some(Failure::Operation(RelayError::Model(error)));
                    self.schedule.stop();
                    return;
                }
            }
        }
        if let Ok(subscription) = crate::relay::model::Subscription::parse(&result.content) {
            self.preview_names = subscription
                .nodes()
                .iter()
                .map(|node| Zeroizing::new(node.name().to_owned()))
                .collect();
            self.preview_endpoints = subscription
                .nodes()
                .iter()
                .map(|node| Zeroizing::new(format!("{}:{}", node.server(), node.port())))
                .collect();
            self.result_format = Some(subscription.format_label());
        }
        self.completed_config = Some(self.private_config(self.engine_path()));
        self.notice = None;
        self.error = None;
        self.result = Some(result);
    }

    pub(super) fn clear_editor_widgets(&self, ctx: &egui::Context) {
        if let Some(editor) = &self.editor {
            super::pages::clear_node_operations(ctx, &editor.options.pipeline);
        }
        ctx.data_mut(|data| {
            for id in [
                "relay-include",
                "relay-exclude",
                "relay-find",
                "relay-replace",
                "relay-paste",
            ] {
                data.remove::<egui::text_edit::TextEditState>(egui::Id::unique(id));
            }
            if let Some(editor) = &self.editor {
                for row in &editor.rows {
                    data.remove::<egui::text_edit::TextEditState>(egui::Id::unique((
                        "relay-node-name",
                        row.index,
                    )));
                }
            }
        });
    }
    pub(super) fn engine_path(&self) -> &Path {
        self.selected_engine
            .as_deref()
            .unwrap_or_else(|| Path::new("sing-box"))
    }

    pub(super) fn cooldown_seconds(&self) -> u64 {
        self.cooldown_until
            .max(self.schedule.server_retry_until())
            .and_then(|deadline| deadline.checked_duration_since(Instant::now()))
            .map_or(0, |remaining| remaining.as_secs().saturating_add(1))
    }

    pub(super) fn input_changed(&mut self) {
        let mut processing = self
            .editor
            .as_ref()
            .map(EditorState::processing)
            .or_else(|| self.imported_processing.take());
        if let Some(options) = &mut processing {
            options.nodes.clear();
        }
        self.imported_processing = self.workspace.selected.and(processing);
        self.editor = None;
        self.rename = false;
        self.sort = false;
        self.compatibility = false;
        self.cooldown_until = None;
        self.invalidate_result();
    }

    pub(super) fn invalidate_result(&mut self) {
        if self.workspace.selected.is_some() {
            self.workspace.draft_dirty = true;
        }
        self.schedule.stop();
        self.allow_resolution = false;
        self.preview_names.clear();
        self.preview_endpoints.clear();
        self.error = None;
        self.notice = None;
        self.result = None;
        self.result_format = None;
        self.completed_config = None;
        self.allow_plaintext = false;
        self.acknowledge_publish = false;
        self.allow_public_output = false;
    }

    pub(super) fn clear_request_error(&mut self) {
        self.error = None;
    }
    pub(super) fn busy(&self) -> bool {
        self.pending.is_some()
            || self.file_pending.is_some()
            || self.schedule.running()
            || self.workspace.busy()
    }

    pub(super) fn selecting_file(&self) -> bool {
        self.file_picker.is_some()
    }

    pub(super) fn workspace_import_pending(&self) -> bool {
        self.file_pending
            .as_ref()
            .is_some_and(|pending| pending.imports_new)
    }

    pub(super) fn dismiss_file_picker(&mut self) {
        self.file_picker = None;
        self.file_picker_imports_new = false;
    }

    pub(super) fn show_file_picker(
        &mut self,
        ctx: &egui::Context,
        language: crate::runtime::settings::Language,
        workspace: Option<&Path>,
    ) {
        use super::file_dialog::PickerResponse;
        let Some(picker) = &mut self.file_picker else {
            return;
        };
        let action = picker.action;
        let response = picker.show(ctx, language);
        if !matches!(response, PickerResponse::Pending)
            && let Some(directory) = picker.directory()
        {
            self.last_directory = Some(directory.to_owned());
        }
        match response {
            PickerResponse::Pending => {}
            PickerResponse::Cancelled => self.dismiss_file_picker(),
            PickerResponse::Selected(path) => {
                self.file_picker = None;
                let imports_new = std::mem::take(&mut self.file_picker_imports_new);
                self.dispatch_file_action(action, Some(path), workspace, imports_new);
            }
        }
    }

    pub(super) fn validate(&self, mode: RunMode, engine: &Path) -> Result<(), Text> {
        if self.source.trim().is_empty() && self.local_content.is_none() {
            return Err(Text::RelayInputInvalid);
        }
        if !self.source.trim().is_empty() && self.local_content.is_none() {
            RelayConfig {
                source: self.source.trim().to_owned(),
                source_file: self.source_file.to_string(),
                ..RelayConfig::default()
            }
            .validate(RunMode::Import)
            .map_err(|error| match error {
                RelayError::Gist(crate::relay::gist::GistError::PinnedTarget) => {
                    Text::RelayPinnedTarget
                }
                _ => Text::RelayInputInvalid,
            })?;
        }
        if mode == RunMode::Import {
            return Ok(());
        }
        if mode == RunMode::Preview && self.needs_resolution() && !self.allow_resolution {
            return Err(Text::ResolutionConsentRequired);
        }
        if mode == RunMode::Publish {
            if !self.output.trim().is_empty() {
                let output =
                    GistFile::parse(self.output.trim(), &self.output_file).map_err(|error| {
                        match error {
                            crate::relay::gist::GistError::PinnedTarget => Text::RelayPinnedTarget,
                            _ => Text::RelayOutputInvalid,
                        }
                    })?;
                let source_matches = |source: &str, file: &str| {
                    GistFile::parse_source(source.trim(), file)
                        .is_ok_and(|source| source.id == output.id)
                };
                if source_matches(&self.source, &self.source_file)
                    || matches!(
                        self.workspace.selected,
                        Some(super::subscriptions::Selection::Collection(_))
                    ) && self.workspace.data.subscriptions.iter().any(|entry| {
                        self.workspace.members.contains(&entry.id)
                            && source_matches(&entry.source, &entry.source_file)
                    })
                {
                    return Err(Text::RelayOutputInvalid);
                }
            } else if validate_filename(&self.output_file).is_err() {
                return Err(Text::RelayNameInvalid);
            }
            if validate_name(&self.title).is_err() || self.output_file.chars().count() > 64 {
                return Err(Text::RelayNameInvalid);
            }
        }
        if mode != RunMode::Publish && self.test_nodes && engine.as_os_str().is_empty() {
            return Err(Text::SingboxRequired);
        }
        if mode != RunMode::Publish
            && self.test_nodes
            && (!(1..=8).contains(&self.concurrency) || !(1..=20).contains(&self.timeout_seconds))
        {
            return Err(Text::RelayOptionsInvalid);
        }
        let uses_token = mode == RunMode::Publish
            || (self.local_content.is_none() && (self.test_nodes || self.editor.is_none()));
        if (uses_token
            && !self.token.is_empty()
            && (self.token.len() > 1024
                || !self.token.is_ascii()
                || self
                    .token
                    .bytes()
                    .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())))
            || mode == RunMode::Publish && self.token.is_empty()
        {
            return Err(Text::RelayTokenInvalid);
        }
        if mode == RunMode::Publish && !self.acknowledge_publish {
            return Err(Text::RelayConsentRequired);
        }
        if self
            .local_content
            .as_ref()
            .is_some_and(|content| content.len() > 4 * 1024 * 1024)
        {
            return Err(Text::RelayTooLarge);
        }
        Ok(())
    }

    pub(super) fn private_config(&self, engine: &Path) -> PrivateConfig {
        PrivateConfig(RelayConfig {
            local_content: None,
            source: self.source.trim().to_owned(),
            source_file: self.source_file.to_string(),
            output: self.output.trim().to_owned(),
            output_file: self.output_file.to_string(),
            output_format: self.publish_format,
            title: self.title.to_string(),
            engine: engine.to_path_buf(),
            concurrency: self.concurrency,
            timeout_seconds: self.timeout_seconds,
            compatibility: self.compatibility,
            doh: self.doh,
            allow_resolution: self.allow_resolution,
            rename: self.rename,
            sort: self.sort,
            test_nodes: self.test_nodes,
            allow_public_output: self.allow_public_output,
            processing: self.editor.as_ref().map_or_else(
                || self.imported_processing.clone().unwrap_or_default(),
                EditorState::processing,
            ),
            expected_source: self.editor.as_ref().map(|editor| editor.source.clone()),
        })
    }

    pub(super) fn clear(&mut self, ctx: &egui::Context) {
        self.schedule.stop();
        self.schedule.interval_minutes = crate::relay::schedule::DEFAULT_INTERVAL.as_secs() / 60;
        self.paste.zeroize();
        self.archive_identity.zeroize();
        self.reveal_identity = false;
        self.archive_recipient.zeroize();
        self.encrypted_import = None;
        self.migration = None;
        self.migration_ack = false;
        self.reveal_paste = false;
        self.input_mode = InputMode::Link;
        self.clear_editor_widgets(ctx);
        self.dismiss_file_picker();
        self.imported_processing = None;
        self.last_directory = None;
        self.generation = self.generation.wrapping_add(1);
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Release);
        }
        self.token.zeroize();
        self.step = RelayStep::Source;
        self.reveal_source = false;
        self.reveal_token = false;
        self.reveal_output = false;
        self.cooldown_until = None;
        self.local_content = None;
        self.local_summary = None;
        self.editor = None;
        self.preview_names.clear();
        self.preview_endpoints.clear();
        self.completed_config = None;
        self.source.zeroize();
        self.source_file.zeroize();
        self.output.zeroize();
        self.output_file.zeroize();
        self.title.zeroize();
        let defaults = RelayConfig::default();
        self.output_file.push_str(&defaults.output_file);
        self.title.push_str(&defaults.title);
        self.result = None;
        self.result_format = None;
        self.output_format = OutputFormat::Original;
        self.publish_format = OutputFormat::Original;
        self.progress = None;
        self.error = None;
        self.allow_plaintext = false;
        self.acknowledge_publish = false;
        self.allow_public_output = false;
        self.compatibility = false;
        self.doh = false;
        self.allow_resolution = false;
        self.test_nodes = true;
        self.notice = Some(if self.busy() {
            Text::RelayCancelling
        } else {
            Text::RelayCleared
        });
        for id in SECRET_IDS {
            super::templates::clear_secret_widget(ctx, id);
        }
    }

    pub(super) fn request_cancel(&mut self) {
        self.workspace.cancel.store(true, Ordering::Release);
        self.schedule.stop();
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Release);
            self.notice = Some(Text::RelayCancelling);
        }
        self.allow_plaintext = false;
    }

    pub(super) fn tick(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.workspace.tick(ctx) {
            self.execute_workspace(ctx, action);
        }
        if let Some(failure) = self.workspace.take_failure() {
            self.record_workspace_failure(failure);
        }
        if !ctx.input(|input| input.viewport().focused.unwrap_or(true)) {
            self.clear_editor_widgets(ctx);
            if let Some(editor) = &mut self.editor {
                editor.reveal = false;
            }
            self.allow_plaintext = false;
            self.reveal_source = false;
            self.reveal_token = false;
            self.reveal_output = false;
            self.reveal_paste = false;
            self.reveal_identity = false;
            super::templates::clear_secret_widget(ctx, SECRET_IDS[6]);
        }
        if self.cooldown_seconds() > 0 {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        self.tick_file(ctx);
        let Some(pending) = &self.pending else { return };
        let current = pending.generation == self.generation;
        if current {
            self.progress = *pending
                .progress
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        if !pending.worker.is_finished() {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        let pending = self.pending.take().expect("completed relay worker exists");
        let result = pending.receiver.try_recv();
        let joined = pending.worker.join();
        if !current {
            self.notice = Some(Text::RelayCancelled);
            return;
        }
        self.allow_plaintext = false;
        self.acknowledge_publish = false;
        self.allow_public_output = false;
        self.compatibility = false;
        match (joined, result) {
            (Ok(()), Ok(Ok(mut result))) => {
                if pending.mode == RunMode::Publish {
                    self.completed_config = Some(self.private_config(self.engine_path()));
                }
                if pending.mode == RunMode::Import {
                    match EditorState::new(&result.content, result.source.clone()) {
                        Ok(mut editor) => {
                            if let Some(mut options) = self.imported_processing.take() {
                                for edit in &options.nodes {
                                    if let Some(row) = editor.rows.get_mut(edit.index) {
                                        row.included = edit.included;
                                        if let Some(name) = &edit.name {
                                            row.name = Zeroizing::new(name.clone());
                                        }
                                    }
                                }
                                options.nodes.clear();
                                editor.options = options;
                            }
                            self.editor = Some(editor);
                        }
                        Err(error) => {
                            self.error = Some(Failure::Operation(RelayError::Model(error)));
                            return;
                        }
                    }
                }
                if pending.mode == RunMode::Preview && !self.test_nodes {
                    result.source = self
                        .editor
                        .as_ref()
                        .and_then(|editor| editor.origin.clone());
                }
                self.step = if pending.mode == RunMode::Publish {
                    RelayStep::Publish
                } else {
                    RelayStep::Review
                };
                self.preview_names.clear();
                self.preview_endpoints.clear();
                self.result_format = crate::relay::model::Subscription::parse(&result.content)
                    .ok()
                    .map(|subscription| {
                        self.preview_names = subscription
                            .nodes()
                            .iter()
                            .map(|node| Zeroizing::new(node.name().to_owned()))
                            .collect();
                        self.preview_endpoints = subscription
                            .nodes()
                            .iter()
                            .map(|node| {
                                Zeroizing::new(if node.server().contains(':') {
                                    format!("[{}]:{}", node.server(), node.port())
                                } else {
                                    format!("{}:{}", node.server(), node.port())
                                })
                            })
                            .collect();
                        subscription.format_label()
                    });
                if let Some(created) = &result.created {
                    self.output = Zeroizing::new(created.id.clone());
                    self.workspace.draft_dirty |= self.workspace.selected.is_some();
                    if let Some(config) = &mut self.completed_config {
                        config.0.output = created.id.clone();
                    }
                }
                self.notice = Some(if result.published {
                    Text::RelayPublished
                } else if result.unchanged {
                    Text::RelayUnchanged
                } else {
                    Text::RelayPreviewReady
                });
                if pending.mode == RunMode::Import
                    && (self.rename
                        || self.sort
                        || self
                            .editor
                            .as_ref()
                            .is_some_and(|editor| editor.processing().is_active()))
                {
                    self.result = None;
                    self.completed_config = None;
                    self.notice = Some(Text::OperationRestored);
                } else {
                    self.result = Some(result);
                }
            }
            (Ok(()), Ok(Err(error))) => {
                if matches!(error, RelayError::SourceChanged) && !self.source.is_empty() {
                    self.clear_editor_widgets(ctx);
                    self.input_changed();
                    self.local_content = None;
                    self.local_summary = None;
                    self.step = RelayStep::Source;
                }
                self.record_workspace_failure(Failure::Operation(error));
            }
            _ => self.notice = Some(Text::RelayWorkerStopped),
        }
    }

    fn record_workspace_failure(&mut self, failure: Failure) {
        if let Failure::Operation(error) = &failure {
            if let Some(retry) = crate::relay::schedule::server_retry_after(error) {
                self.cooldown_until = self.cooldown_until.max(Instant::now().checked_add(retry));
            }
            if let RelayError::Gist(crate::relay::gist::GistError::CreatedUnverified(created)) =
                error
            {
                self.output = Zeroizing::new(created.id.clone());
                self.workspace.draft_dirty |= self.workspace.selected.is_some();
            }
        }
        self.error = Some(failure);
    }

    pub(super) fn copy(&mut self, ctx: &egui::Context) {
        if self.allow_plaintext && !self.demo_mode {
            self.start_local_output(self.output_format, true);
            ctx.request_repaint();
        }
    }

    pub(super) fn file_dialog_active(&self) -> bool {
        self.file_pending.is_some()
    }

    pub(super) fn start_format_check(&mut self) {
        self.start_format_check_for(self.output_format);
    }

    pub(super) fn start_format_check_for(&mut self, format: OutputFormat) {
        self.start_local_output(format, false);
    }

    fn start_local_output(&mut self, format: OutputFormat, copy: bool) {
        if self.busy() || self.selecting_file() {
            return;
        }
        let Some(result) = self
            .result
            .as_ref()
            .filter(|result| !result.content.is_empty())
        else {
            return;
        };
        let content = result.content.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        self.error = None;
        self.notice = None;
        match thread::Builder::new()
            .name("script-hub-local-output".into())
            .spawn(move || {
                let result = match export_subscription(&content, format) {
                    Ok(content) if copy => FileCompleted::CopyReady(format, content),
                    Ok(_) => FileCompleted::FormatChecked,
                    Err(error) => FileCompleted::ExportFailed(error),
                };
                let _ = sender.send(Ok(Some(result)));
            }) {
            Ok(worker) => {
                self.file_pending = Some(FilePending {
                    worker,
                    receiver,
                    generation: self.generation,
                    imports_new: false,
                });
            }
            Err(_) => self.error = Some(Failure::Validation(Text::RelayWorkerStopped)),
        }
    }

    pub(super) fn file_action(
        &mut self,
        action: FileAction,
        path: Option<std::path::PathBuf>,
        workspace: Option<&Path>,
    ) {
        self.dispatch_file_action(action, path, workspace, false);
    }

    pub(super) fn start_workspace_import(
        &mut self,
        path: Option<std::path::PathBuf>,
        workspace: Option<&Path>,
    ) {
        self.dispatch_file_action(FileAction::Import, path, workspace, true);
    }

    fn dispatch_file_action(
        &mut self,
        action: FileAction,
        path: Option<std::path::PathBuf>,
        workspace: Option<&Path>,
        imports_new: bool,
    ) {
        if self.demo_mode || self.busy() || self.selecting_file() {
            return;
        }
        if action.is_save() && action != FileAction::ExportEncrypted && !self.allow_plaintext {
            self.error = Some(Failure::Validation(Text::VaultPlaintextConsent));
            return;
        }
        if matches!(action, FileAction::ImportPlan | FileAction::ExportPlan)
            && self.editor.is_none()
        {
            self.error = Some(Failure::Validation(Text::OperationPlanInvalid));
            return;
        }
        let Some(path) = path else {
            if matches!(
                action,
                FileAction::ExportBackup | FileAction::ExportEncrypted | FileAction::ExportNodes(_)
            ) && self
                .result
                .as_ref()
                .is_none_or(|result| result.content.is_empty())
            {
                self.error = Some(Failure::Validation(Text::RelayFileError));
                return;
            }
            if imports_new {
                self.workspace.error = None;
            } else {
                self.error = None;
            }
            self.file_picker = Some(super::file_dialog::Picker::new(
                action,
                self.last_directory.as_deref(),
            ));
            self.file_picker_imports_new = imports_new;
            return;
        };
        let plan = if action == FileAction::ExportPlan {
            self.editor
                .as_ref()
                .map(|editor| editor.options.pipeline.clone())
        } else {
            None
        };
        let payload = if !matches!(
            action,
            FileAction::ExportBackup | FileAction::ExportEncrypted | FileAction::ExportNodes(_)
        ) {
            None
        } else {
            let Some(result) = &self.result else { return };
            if result.content.is_empty() {
                self.error = Some(Failure::Validation(Text::RelayFileError));
                return;
            }
            Some((
                self.completed_config.as_ref().map(|config| {
                    PrivateConfig(RelayConfig {
                        source: config.0.source.clone(),
                        source_file: config.0.source_file.clone(),
                        output: config.0.output.clone(),
                        output_file: config.0.output_file.clone(),
                        output_format: config.0.output_format,
                        title: config.0.title.clone(),
                        processing: config.0.processing.clone(),
                        expected_source: config.0.expected_source.clone(),
                        rename: config.0.rename,
                        sort: config.0.sort,
                        concurrency: config.0.concurrency,
                        timeout_seconds: config.0.timeout_seconds,
                        ..RelayConfig::default()
                    })
                }),
                result.content.clone(),
            ))
        };
        let workspace = workspace.map(Path::to_owned);
        let recipient = self.archive_recipient.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let generation = self.generation;
        if imports_new {
            self.workspace.error = None;
        } else {
            self.error = None;
            self.notice = None;
            self.allow_plaintext = false;
        }
        let worker = thread::Builder::new()
            .name("script-hub-relay-file".into())
            .spawn(move || {
                let work = || -> Result<Option<FileCompleted>, Text> {
                    if matches!(action, FileAction::ChooseEngine) {
                        let path = super::file_dialog::validate_engine(path)
                            .map_err(|_| Text::RelayFileError)?;
                        return Ok(Some(FileCompleted::Engine(path)));
                    }
                    if action == FileAction::ImportPlan {
                        let bytes = Zeroizing::new(
                            crate::vault::read_bounded(&path).map_err(|_| Text::RelayFileError)?,
                        );
                        let steps = crate::relay::plan::decode(&bytes)
                            .map_err(|_| Text::OperationPlanInvalid)?;
                        return Ok(Some(FileCompleted::Plan(steps)));
                    }
                    if matches!(action, FileAction::Import) {
                        let bytes = Zeroizing::new(
                            crate::vault::read_bounded(&path).map_err(|_| Text::RelayFileError)?,
                        );
                        return decode_import(&bytes).map(Some);
                    }
                    if !super::templates::safe_export_destination(&path, workspace.as_deref()) {
                        return Err(Text::RelayFileError);
                    }
                    if let Some(plan) = plan {
                        let bytes = crate::relay::plan::encode(&plan)
                            .map_err(|_| Text::OperationPlanInvalid)?;
                        crate::vault::write_private(&path, &bytes, false).map_err(|error| {
                            match error {
                                crate::vault::VaultError::AlreadyExists => Text::RelayFileExists,
                                _ => Text::RelayFileError,
                            }
                        })?;
                        return Ok(Some(FileCompleted::PlanSaved));
                    }
                    let (config, content) = payload.ok_or(Text::RelayFileError)?;
                    let bytes = if matches!(
                        action,
                        FileAction::ExportBackup | FileAction::ExportEncrypted
                    ) {
                        crate::relay::backup::encode(
                            &config.ok_or(Text::RelayFileError)?.0,
                            &content,
                        )
                        .map_err(|_| Text::RelayBackupInvalid)?
                    } else if let FileAction::ExportNodes(format) = action {
                        match export_subscription(&content, format) {
                            Ok(converted) => Zeroizing::new(converted.as_bytes().to_vec()),
                            Err(error) => return Ok(Some(FileCompleted::ExportFailed(error))),
                        }
                    } else {
                        return Err(Text::RelayFileError);
                    };
                    let bytes = if action == FileAction::ExportEncrypted {
                        crate::relay::archive::encrypt(&bytes, recipient.trim())
                            .map_err(archive_error)?
                    } else {
                        bytes
                    };
                    // Never replace an existing backup or write private node data into the repository.
                    crate::vault::write_private(&path, &bytes, false).map_err(
                        |error| match error {
                            crate::vault::VaultError::AlreadyExists => Text::RelayFileExists,
                            _ => Text::RelayFileError,
                        },
                    )?;
                    Ok(Some(FileCompleted::Saved))
                };
                let _ = sender.send(work());
            });
        match worker {
            Ok(worker) => {
                self.file_pending = Some(FilePending {
                    worker,
                    receiver,
                    generation,
                    imports_new,
                })
            }
            Err(_) if imports_new => self.workspace.error = Some(Text::WorkspaceImportReadFailed),
            Err(_) => self.error = Some(Failure::Validation(Text::RelayFileError)),
        }
    }

    fn tick_file(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.file_pending else {
            return;
        };
        if !pending.worker.is_finished() {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        let Some(pending) = self.file_pending.take() else {
            return;
        };
        let result = pending.receiver.try_recv();
        let joined = pending.worker.join();
        if pending.generation != self.generation {
            return;
        }
        if pending.imports_new {
            match (&joined, &result) {
                (
                    Ok(()),
                    Ok(Ok(Some(
                        FileCompleted::Nodes(_)
                        | FileCompleted::Imported(_)
                        | FileCompleted::Migration(_)
                        | FileCompleted::Locked(_),
                    ))),
                ) => {
                    let directory = self.last_directory.take();
                    self.execute_workspace(ctx, super::subscriptions::Action::NewSubscription);
                    self.last_directory = directory;
                    self.input_mode = InputMode::File;
                }
                (Ok(()), Ok(Err(key))) => {
                    self.workspace.error = Some(if *key == Text::RelayFileError {
                        Text::WorkspaceImportReadFailed
                    } else {
                        *key
                    });
                    return;
                }
                _ => {
                    self.workspace.error = Some(Text::WorkspaceImportReadFailed);
                    return;
                }
            }
        }
        match (joined, result) {
            (Ok(()), Ok(Ok(Some(FileCompleted::Plan(steps))))) => {
                self.apply_plan(ctx, steps);
            }
            (Ok(()), Ok(Ok(Some(FileCompleted::PlanSaved)))) => {
                self.notice = Some(Text::OperationPlanSaved);
            }
            (Ok(()), Ok(Ok(Some(FileCompleted::Migration(report))))) => {
                self.encrypted_import = None;
                self.archive_identity.zeroize();
                self.migration_ack = false;
                self.migration = Some(Arc::new(report));
                self.step = RelayStep::Source;
                self.input_mode = InputMode::File;
            }
            (Ok(()), Ok(Ok(Some(FileCompleted::Locked(bytes))))) => {
                self.encrypted_import = Some(bytes);
                self.migration = None;
                self.step = RelayStep::Source;
                self.input_mode = InputMode::File;
            }
            (Ok(()), Ok(Ok(Some(FileCompleted::Imported(backup))))) => {
                self.apply_backup(ctx, backup);
                self.workspace.draft_dirty = self.workspace.selected.is_some();
            }
            (Ok(()), Ok(Ok(Some(FileCompleted::Nodes(content))))) => {
                if self.workspace.selected.is_some() {
                    self.replace_nodes(ctx, content);
                } else {
                    self.apply_backup(
                        ctx,
                        Box::new(crate::relay::backup::RelayBackup {
                            config: RelayConfig {
                                test_nodes: true,
                                rename: false,
                                sort: false,
                                ..RelayConfig::default()
                            },
                            content,
                        }),
                    );
                }
            }
            (Ok(()), Ok(Ok(Some(FileCompleted::Engine(path))))) => {
                self.selected_engine = Some(path);
                self.notice = Some(Text::RelayEngineSelected);
            }
            (Ok(()), Ok(Ok(Some(FileCompleted::Saved)))) => {
                self.notice = Some(Text::RelayFileSaved)
            }
            (Ok(()), Ok(Ok(Some(FileCompleted::ExportFailed(error))))) => {
                self.error = Some(Failure::Operation(error));
            }
            (Ok(()), Ok(Ok(Some(FileCompleted::FormatChecked)))) => {
                self.notice = Some(Text::RelayExportCheckPassed);
            }
            (Ok(()), Ok(Ok(Some(FileCompleted::CopyReady(format, mut content))))) => {
                if self.allow_plaintext && self.result.is_some() && self.output_format == format {
                    ctx.copy_text(std::mem::take(&mut *content));
                    self.notice = Some(Text::Copied);
                }
            }
            (Ok(()), Ok(Ok(None))) => {}
            (Ok(()), Ok(Err(key))) => self.error = Some(Failure::Validation(key)),
            _ => self.error = Some(Failure::Validation(Text::RelayFileError)),
        }
    }

    fn replace_nodes(&mut self, ctx: &egui::Context, content: Zeroizing<String>) {
        let cooldown = self.cooldown_until;
        self.clear_editor_widgets(ctx);
        self.input_changed();
        self.cooldown_until = cooldown;
        self.clear_pasted_text(ctx);
        self.source.zeroize();
        self.source_file.zeroize();
        self.reveal_source = false;
        for id in [SECRET_IDS[1], SECRET_IDS[2]] {
            super::templates::clear_secret_widget(ctx, id);
        }
        self.migration = None;
        self.encrypted_import = None;
        self.local_summary = crate::relay::model::Subscription::parse(&content)
            .ok()
            .map(|subscription| (subscription.nodes().len(), subscription.format_label()));
        self.local_content = Some(content);
        self.step = RelayStep::Review;
        let engine = self.engine_path().to_owned();
        self.start_with(RunMode::Import, &engine, crate::relay::run);
    }

    pub(super) fn apply_backup(
        &mut self,
        ctx: &egui::Context,
        mut backup: Box<crate::relay::backup::RelayBackup>,
    ) {
        let last_directory = self.last_directory.take();
        let token = self
            .workspace
            .selected
            .is_some()
            .then(|| self.token.clone());
        self.clear(ctx);
        if let Some(token) = token {
            self.token = token;
        }
        self.last_directory = last_directory;
        let config = &mut backup.config;
        self.source = Zeroizing::new(std::mem::take(&mut config.source));
        self.source_file = Zeroizing::new(std::mem::take(&mut config.source_file));
        self.output = Zeroizing::new(std::mem::take(&mut config.output));
        self.output_file = Zeroizing::new(std::mem::take(&mut config.output_file));
        self.publish_format = config.output_format;
        self.title = Zeroizing::new(std::mem::take(&mut config.title));
        self.rename = config.rename;
        self.sort = config.sort;
        self.concurrency = config.concurrency;
        self.timeout_seconds = config.timeout_seconds;
        self.imported_processing = Some(std::mem::take(&mut config.processing));
        self.local_summary = crate::relay::model::Subscription::parse(&backup.content)
            .ok()
            .map(|subscription| (subscription.nodes().len(), subscription.format_label()));
        self.local_content = Some(std::mem::take(&mut backup.content));
        self.notice = Some(Text::RelayImported);
        self.step = RelayStep::Review;
        let engine = self.engine_path().to_owned();
        self.start_with(RunMode::Import, &engine, crate::relay::run);
    }

    pub(super) fn start_with<F>(&mut self, mode: RunMode, engine: &Path, work: F)
    where
        F: FnOnce(
                &RelayConfig,
                &str,
                RunMode,
                &AtomicBool,
                &(dyn Fn(Progress) + Sync),
            ) -> Result<RelayResult, RelayError>
            + Send
            + 'static,
    {
        let offline = match mode {
            RunMode::Import => self.local_content.is_some(),
            RunMode::Preview => {
                !self.test_nodes
                    && !self.needs_resolution()
                    && (self.editor.is_some() || self.local_content.is_some())
            }
            RunMode::Publish => false,
        };
        if self.demo_mode && !offline {
            self.error = Some(Failure::Validation(Text::RelaySandboxOffline));
            return;
        }
        if self.busy() || (!offline && self.cooldown_seconds() > 0) {
            return;
        }
        if let Err(error) = self.validate(mode, engine) {
            self.error = Some(Failure::Validation(error));
            return;
        }
        let mut config = self.private_config(engine);
        if mode == RunMode::Import {
            config.0.expected_source = None;
            config.0.processing = Default::default();
        }
        config.0.local_content = self.local_content.clone();
        if mode == RunMode::Preview
            && !self.test_nodes
            && let Some(editor) = &self.editor
        {
            config.0.local_content = Some(editor.source.clone());
        }
        if mode != RunMode::Publish {
            self.completed_config = Some(self.private_config(engine));
            self.result = None;
            self.preview_names.clear();
            self.preview_endpoints.clear();
            self.result_format = None;
        }
        let token = if mode != RunMode::Publish && config.0.local_content.is_some() {
            Zeroizing::new(String::new())
        } else {
            self.token.clone()
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let progress = Arc::new(Mutex::new(None));
        let worker_progress = Arc::clone(&progress);
        let (sender, receiver) = mpsc::sync_channel(1);
        self.progress = None;
        self.error = None;
        self.notice = None;
        self.allow_plaintext = false;
        match thread::Builder::new()
            .name("script-hub-private-relay".to_owned())
            .spawn(move || {
                let report = |value| {
                    *worker_progress
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(value);
                };
                let result = work(&config.0, &token, mode, &worker_cancel, &report);
                // Cancellation cannot erase accepted-write receipts; Clear discards by generation.
                let _ = sender.send(result);
            }) {
            Ok(worker) => {
                self.pending = Some(Pending {
                    mode,
                    worker,
                    receiver,
                    cancel,
                    progress,
                    generation: self.generation,
                })
            }
            Err(_) => self.notice = Some(Text::RelayWorkerStopped),
        }
    }

    pub(super) fn needs_resolution(&self) -> bool {
        self.editor.as_ref().is_some_and(|editor| {
            crate::relay::model::pipeline::has_resolution(&editor.options.pipeline)
        })
    }

    pub(super) fn resolution_providers(&self) -> Vec<crate::relay::model::dns::Provider> {
        let mut providers = Vec::new();
        if let Some(editor) = &self.editor {
            for step in editor.options.pipeline.iter().filter(|step| step.enabled) {
                if let crate::relay::model::pipeline::Operation::ResolveDomain { options, .. } =
                    &step.operation
                    && !providers.contains(&options.provider)
                {
                    providers.push(options.provider);
                }
            }
        }
        providers
    }

    fn apply_plan(&mut self, ctx: &egui::Context, steps: Vec<crate::relay::model::pipeline::Step>) {
        self.clear_editor_widgets(ctx);
        if let Some(editor) = &mut self.editor {
            editor.options.pipeline = steps;
            self.invalidate_result();
            self.notice = Some(Text::OperationPlanLoaded);
            self.step = RelayStep::Review;
        } else {
            self.error = Some(Failure::Validation(Text::OperationPlanInvalid));
        }
    }
}

impl Drop for RelayState {
    fn drop(&mut self) {
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, mpsc};
    use std::time::{Duration, Instant};

    pub(super) fn fixture() -> RelayState {
        let mut state = RelayState::default();
        state.test_nodes = false;
        state.token.push_str("synthetic-token");
        state.source.push_str("0123456789abcdef0123456789abcdef");
        state.source_file.push_str("input.conf");
        state.output.push_str("fedcba9876543210fedcba9876543210");
        state.output_file.push_str("output.conf");
        state.title.push_str("Synthetic relay");
        state
    }

    #[test]
    fn public_https_import_validation_does_not_require_gist_or_engine() {
        let mut state = fixture();
        state.source = Zeroizing::new("https://example.com/sub?key=synthetic".into());
        state.source_file.clear();
        assert!(state.validate(RunMode::Import, Path::new("")).is_ok());
        state.source = Zeroizing::new("https://127.0.0.1/private".into());
        assert!(state.validate(RunMode::Import, Path::new("")).is_err());
    }

    #[test]
    fn pasted_nodes_import_offline_and_clear_erases_the_input() {
        let ctx = egui::Context::default();
        let mut state = RelayState::default();
        state.input_mode = InputMode::Paste;
        state.paste.push_str("A = socks5, node.example, 1080\n");
        state.import_paste(&ctx);
        finish(&mut state, &ctx);
        assert_eq!(state.editor.as_ref().unwrap().rows.len(), 1);
        assert!(state.source.is_empty());
        assert!(state.result.as_ref().unwrap().source.is_none());
        state.clear(&ctx);
        assert!(state.paste.is_empty() && state.local_content.is_none());
    }

    #[test]
    fn schedule_arming_is_explicit_and_input_edits_stop_it() {
        let mut state = fixture();
        state.result = Some(result());
        state.arm_schedule();
        assert!(!state.schedule.armed());
        state.acknowledge_publish = true;
        state.arm_schedule();
        assert!(state.schedule.armed());
        state.input_changed();
        assert!(!state.schedule.armed());
    }

    #[test]
    fn schedule_rejects_empty_and_zero_healthy_previews_and_respects_cooldown() {
        let mut state = fixture();
        state.acknowledge_publish = true;
        let mut preview = result();
        preview.content.clear();
        state.result = Some(preview);
        state.arm_schedule();
        assert_eq!(
            state.schedule_setup_issue(),
            Some(Text::ScheduleNeedsPreview)
        );
        assert!(!state.schedule.armed());

        let mut preview = result();
        preview.healthy = 0;
        state.result = Some(preview);
        state.arm_schedule();
        assert_eq!(state.schedule_setup_issue(), Some(Text::RelayNoHealthyHelp));
        assert!(!state.schedule.armed());

        state.result = Some(result());
        state.cooldown_until = Instant::now().checked_add(Duration::from_secs(60));
        state.arm_schedule();
        assert!(!state.schedule.armed());
    }

    #[test]
    fn collection_arming_requires_an_independent_target_and_explicit_consent() {
        use crate::relay::library::{Collection, Entry};

        let ctx = egui::Context::default();
        let mut state = fixture();
        let source = state.source.to_string();
        let content = "A = socks5, cached.example, 1080\n";
        let member = state
            .workspace
            .data
            .add_entry(Entry {
                id: 0,
                name: "Member".into(),
                source: source.clone(),
                source_file: state.source_file.to_string(),
                content: content.into(),
                processing: Default::default(),
                publish: Default::default(),
            })
            .unwrap();
        let id = state
            .workspace
            .data
            .add_collection(Collection {
                id: 0,
                name: "Collection".into(),
                members: vec![member],
                processing: Default::default(),
                publish: Default::default(),
            })
            .unwrap();
        state.source.clear();
        state.source_file.clear();
        state.workspace.selected = Some(super::super::subscriptions::Selection::Collection(id));
        state.workspace.members = vec![member];
        state.local_content = Some(state.workspace.data.merge(id).unwrap());
        state.result = Some(result());

        state.arm_schedule();
        assert!(!state.schedule.armed());
        state.acknowledge_publish = true;
        let destination = std::mem::replace(&mut state.output, Zeroizing::new(source));
        state.arm_schedule();
        assert!(!state.schedule.armed());
        state.output = destination;
        state.workspace.name = Zeroizing::new("Unsaved collection name".into());
        state.workspace.draft_dirty = true;
        let (snapshot, snapshot_id) = state.collection_schedule_snapshot(id).unwrap();
        assert_eq!(snapshot_id, id);
        assert_eq!(snapshot.collections[0].name, "Unsaved collection name");
        assert_eq!(
            snapshot.collections[0].publish.output,
            state.output.as_str()
        );
        state.arm_schedule();
        assert!(state.schedule.armed());
        assert!(state.workspace.draft_dirty && !state.workspace.dirty);
        assert_eq!(state.workspace.data.collections[0].name, "Collection");
        assert!(state.schedule.tick(&ctx, true).is_none());
        assert!(!state.schedule.running());
    }

    #[test]
    fn native_presets_append_unique_steps_without_silent_dns_requests() {
        let mut state = fixture();
        state.editor = Some(EditorState::new("A = socks5, node.example, 1080\n", None).unwrap());
        let ctx = egui::Context::default();
        state.add_preset(&ctx, crate::relay::model::entrance::Preset::Safe);
        state.add_preset(&ctx, crate::relay::model::entrance::Preset::Compatible);
        let steps = &state.editor.as_ref().unwrap().options.pipeline;
        assert_eq!(steps.len(), 16);
        assert!(crate::relay::model::pipeline::validate(steps).is_ok());
        assert!(!state.needs_resolution());
    }

    #[test]
    fn scheduled_success_replaces_stale_copy_and_source_binding() {
        let mut state = fixture();
        state.source = Zeroizing::new("https://example.com/sub".into());
        state.source_file.clear();
        state.local_content = Some(Zeroizing::new("A = socks5, cached.example, 1080\n".into()));
        state.workspace.selected = Some(super::super::subscriptions::Selection::Subscription(1));
        let fresh = "B = socks5, fresh.example, 1080\n";
        let mut latest = result();
        latest.content = Zeroizing::new(fresh.into());
        latest.source = Some(crate::relay::input::Source::Https {
            url: Zeroizing::new(state.source.to_string()),
            content: Zeroizing::new(fresh.into()),
        });
        state.notice = Some(Text::RelayPublished);
        state.accept_scheduled(latest);
        assert!(state.notice.is_none());
        assert_eq!(state.result.as_ref().unwrap().content.as_str(), fresh);
        assert_eq!(state.editor.as_ref().unwrap().source.as_str(), fresh);
        assert_eq!(state.local_content.as_ref().unwrap().as_str(), fresh);
        assert!(state.workspace.draft_dirty);
        assert_eq!(state.preview_names[0].as_str(), "B");
        assert_eq!(
            state
                .completed_config
                .as_ref()
                .unwrap()
                .0
                .expected_source
                .as_deref()
                .unwrap()
                .as_str(),
            fresh
        );
    }

    #[test]
    fn dns_consent_is_explicit_and_invalidated_with_the_processing_plan() {
        use crate::relay::model::pipeline::{AddressFamily, Operation, Step};
        let mut state = fixture();
        state.editor = Some(EditorState::new("A = socks5, node.example, 1080\n", None).unwrap());
        state.editor.as_mut().unwrap().options.pipeline.push(Step {
            id: 1,
            enabled: true,
            operation: Operation::ResolveDomain {
                family: AddressFamily::PreferV4,
                options: Default::default(),
            },
        });
        assert!(matches!(
            state.validate(RunMode::Preview, Path::new("sing-box")),
            Err(Text::ResolutionConsentRequired)
        ));
        state.allow_resolution = true;
        assert!(
            state
                .validate(RunMode::Preview, Path::new("sing-box"))
                .is_ok()
        );
        assert!(
            state
                .private_config(Path::new("sing-box"))
                .0
                .allow_resolution
        );
        state.invalidate_result();
        assert!(!state.allow_resolution);
        assert!(state.validate(RunMode::Import, Path::new("")).is_ok());
    }

    #[test]
    fn dns_providers_are_distinct_and_ignore_disabled_steps() {
        use crate::relay::model::{
            dns::{Options, Provider},
            pipeline::{AddressFamily, Operation, Step},
        };
        let mut state = fixture();
        state.editor = Some(EditorState::new("A = socks5, node.example, 1080\n", None).unwrap());
        let step = |id, enabled, provider| Step {
            id,
            enabled,
            operation: Operation::ResolveDomain {
                family: AddressFamily::V4,
                options: Options {
                    provider,
                    ..Default::default()
                },
            },
        };
        state.editor.as_mut().unwrap().options.pipeline = vec![
            step(1, true, Provider::Cloudflare),
            step(2, false, Provider::Google),
            step(3, true, Provider::Cloudflare),
        ];
        assert!(state.resolution_providers() == vec![Provider::Cloudflare]);
        state
            .editor
            .as_mut()
            .unwrap()
            .options
            .pipeline
            .push(step(4, true, Provider::Google));
        assert!(state.resolution_providers() == vec![Provider::Cloudflare, Provider::Google]);
    }

    #[test]
    fn replacing_dns_options_invalidates_preview_and_consent() {
        use crate::relay::model::{
            dns::{Options, Provider},
            pipeline::{AddressFamily, Operation, Step},
        };
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.editor = Some(EditorState::new("A = socks5, node.example, 1080\n", None).unwrap());
        state.allow_resolution = true;
        state.result = Some(result());
        state
            .preview_names
            .push(Zeroizing::new("private name".into()));
        state
            .preview_endpoints
            .push(Zeroizing::new("private.example:443".into()));
        state.apply_plan(
            &ctx,
            vec![Step {
                id: 1,
                enabled: true,
                operation: Operation::ResolveDomain {
                    family: AddressFamily::V4,
                    options: Options {
                        provider: Provider::Google,
                        ..Default::default()
                    },
                },
            }],
        );
        assert!(!state.allow_resolution && state.result.is_none());
        assert!(state.preview_names.is_empty() && state.preview_endpoints.is_empty());
        assert!(state.resolution_providers() == vec![Provider::Google]);
        assert!(matches!(
            state.validate(RunMode::Preview, Path::new("")),
            Err(Text::ResolutionConsentRequired)
        ));
    }

    #[test]
    fn processed_endpoint_preview_brackets_ipv6_and_is_cleared() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.editor = Some(EditorState::new("A = socks5, node.example, 1080\n", None).unwrap());
        state.start_with(RunMode::Preview, Path::new(""), |_, _, _, _, _| {
            let mut result = result();
            result.content = Zeroizing::new(
                r#"[{"type":"socks","tag":"A","server":"2001:db8::1","server_port":1080}]"#.into(),
            );
            Ok(result)
        });
        finish(&mut state, &ctx);
        assert_eq!(
            state
                .preview_endpoints
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
            ["[2001:db8::1]:1080"]
        );
        state.invalidate_result();
        assert!(state.preview_endpoints.is_empty());
        state
            .preview_endpoints
            .push(Zeroizing::new("private.example:443".into()));
        state.clear(&ctx);
        assert!(state.preview_endpoints.is_empty());
    }

    fn result() -> RelayResult {
        RelayResult {
            content: Zeroizing::new("A = socks5, node.example.invalid, 1080\n".to_owned()),
            tested: true,
            source: None,
            created: None,
            total: 2,
            processed: 2,
            excluded: 0,
            duplicates: 0,
            duplicate_skipped: 0,
            stages: Vec::new(),
            healthy: 1,
            failed: 0,
            unknown: 1,
            security_blocked: 0,
            removed: 0,
            baseline_valid: 2,
            details: Vec::new(),
            published: false,
            unchanged: false,
        }
    }

    #[test]
    fn checking_output_format_does_not_grant_plaintext_or_publish_consent() {
        let mut state = fixture();
        state.result = Some(result());
        state.output_format = OutputFormat::SingBox;
        state.start_format_check();
        finish(&mut state, &egui::Context::default());
        assert_eq!(state.notice, Some(Text::RelayExportCheckPassed));
        assert!(state.error.is_none());
        assert!(!state.allow_plaintext && !state.acknowledge_publish);
        assert_eq!(state.result.as_ref().unwrap().content, result().content);
    }

    #[test]
    fn replacing_nodes_keeps_destination_and_drops_old_node_specific_edits() {
        use super::super::subscriptions::Selection;
        use crate::relay::model::pipeline::{Operation, Step};
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.workspace.selected = Some(Selection::Subscription(0));
        let mut editor = EditorState::new(&result().content, None).unwrap();
        editor.rows[0].included = false;
        editor.options.pipeline.push(Step {
            id: 1,
            enabled: true,
            operation: Operation::ReplaceName {
                pattern: "a".into(),
                replacement: "b".into(),
                regex: false,
            },
        });
        state.editor = Some(editor);
        state.schedule.interval_minutes = 30;
        state.token = Zeroizing::new("unfinished token".into());
        state.acknowledge_publish = true;
        state
            .paste
            .push_str("New = socks5, new.example.invalid, 1080\n");
        let output = state.output.clone();
        state.import_paste(&ctx);
        finish(&mut state, &ctx);
        assert_eq!(state.output, output);
        assert_eq!(state.schedule.interval_minutes, 30);
        assert_eq!(state.token.as_str(), "unfinished token");
        assert!(state.paste.is_empty() && state.source.is_empty() && !state.acknowledge_publish);
        let editor = state.editor.as_ref().unwrap();
        assert!(editor.rows[0].included);
        assert_eq!(editor.options.pipeline.len(), 1);
        assert!(
            state.result.is_none(),
            "the retained operation plan requires a fresh preview"
        );
    }

    #[test]
    fn publish_errors_retain_preview_unless_the_source_changed() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.result = Some(result());
        state.result_format = Some("Surge");
        state.preview_names.push(Zeroizing::new("Example".into()));
        state.acknowledge_publish = true;
        state.start_with(RunMode::Publish, Path::new(""), |_, _, _, _, _| {
            Err(RelayError::Gist(crate::relay::gist::GistError::Conflict))
        });
        finish(&mut state, &ctx);
        assert!(state.result.is_some());
        assert_eq!(state.result_format, Some("Surge"));
        assert_eq!(state.preview_names.len(), 1);
        state.acknowledge_publish = true;
        state.start_with(RunMode::Publish, Path::new(""), |_, _, _, _, _| {
            Err(RelayError::SourceChanged)
        });
        finish(&mut state, &ctx);
        assert!(state.result.is_none() && state.step == RelayStep::Source);
    }

    #[test]
    fn restoring_a_processing_plan_requires_preview_and_applies_it_only_once() {
        use crate::relay::model::pipeline::{Operation, Step};
        let ctx = egui::Context::default();
        let content = "A = http, node.invalid, 80\n";
        let mut config = RelayConfig {
            rename: false,
            sort: false,
            ..Default::default()
        };
        config.processing.pipeline.push(Step {
            id: 9,
            enabled: true,
            operation: Operation::ReplaceName {
                pattern: "A".into(),
                replacement: "AA".into(),
                regex: false,
            },
        });
        let bytes = crate::relay::backup::encode(&config, content).unwrap();
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "script-hub-plan-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        crate::vault::write_private(&path, &bytes, false).unwrap();
        let mut state = fixture();
        state.file_action(FileAction::Import, Some(path.clone()), None);
        finish(&mut state, &ctx);
        assert!(
            state.result.is_none(),
            "raw backup input is not a processed result"
        );
        let editor = state.editor.as_ref().unwrap();
        assert_eq!(editor.source.as_str(), content);
        assert_eq!(editor.options.pipeline.len(), 1);
        state.test_nodes = false;
        state.start_with(RunMode::Preview, Path::new(""), crate::relay::run);
        finish(&mut state, &ctx);
        let restored =
            crate::relay::model::Subscription::parse(&state.result.as_ref().unwrap().content)
                .unwrap();
        assert_eq!(restored.nodes()[0].name(), "AA");
        assert_eq!(state.result.as_ref().unwrap().stages.len(), 1);
        state.clear(&ctx);
        assert!(state.imported_processing.is_none());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn substore_preview_requires_selection_and_never_fetches_or_restores_tokens() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.result = Some(result());
        state.paste = Zeroizing::new(r#"{"subs":[{"name":"remote","source":"remote","url":"https://example.com/sub","process":[{"type":"Script Operator"}]}],"settings":{"gistToken":"do-not-restore"}}"#.into());
        state.import_paste(&ctx);
        finish(&mut state, &ctx);
        assert_eq!(state.migration.as_ref().unwrap().items.len(), 1);
        assert!(state.result.is_some());
        state.select_migration(&ctx, 0);
        assert_eq!(state.token.as_str(), "synthetic-token");
        state.migration_ack = true;
        state.select_migration(&ctx, 0);
        assert_eq!(state.source.as_str(), "https://example.com/sub");
        assert!(state.token.is_empty() && state.result.is_none());
        assert!(!state.busy(), "selection must not contact a remote source");
        assert!(state.migration.is_none());
    }

    #[test]
    fn age_import_keeps_failed_attempt_recoverable_and_clears_private_key() {
        use age::{secrecy::ExposeSecret, x25519};
        let key = x25519::Identity::generate();
        let secret = key.to_string();
        let ciphertext = crate::relay::archive::encrypt(
            br#"{"subs":[{"name":"local","source":"local","content":"A = socks5, node.example, 1080\n"}]}"#,
            &key.to_public().to_string(),
        ).unwrap();
        assert!(matches!(
            decode_import(&ciphertext),
            Ok(FileCompleted::Locked(_))
        ));
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.encrypted_import = Some(ciphertext);
        state.archive_identity.push_str("not-a-key");
        state.unlock_import(&ctx);
        finish(&mut state, &ctx);
        assert!(state.archive_identity.is_empty() && state.encrypted_import.is_some());
        assert!(matches!(
            state.error,
            Some(Failure::Validation(Text::ArchiveKeyInvalid))
        ));
        state.archive_identity.push_str(secret.expose_secret());
        state.unlock_import(&ctx);
        finish(&mut state, &ctx);
        assert!(state.archive_identity.is_empty() && state.encrypted_import.is_none());
        assert!(state.migration.is_some());
        state.migration_ack = true;
        state.select_migration(&ctx, 0);
        finish(&mut state, &ctx);
        assert_eq!(state.editor.as_ref().unwrap().rows.len(), 1);
        assert!(state.source.is_empty() && state.token.is_empty());
        state.clear(&ctx);
        assert!(state.migration.is_none() && state.archive_recipient.is_empty());
    }

    #[test]
    fn encrypted_export_roundtrips_without_plaintext_consent() {
        use age::{secrecy::ExposeSecret, x25519};
        let ctx = egui::Context::default();
        let key = x25519::Identity::generate();
        let secret = key.to_string();
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "script-hub-age-export-{}-{}.age",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let mut state = fixture();
        state.result = Some(result());
        state.completed_config = Some(state.private_config(Path::new("")));
        state
            .archive_recipient
            .push_str(&key.to_public().to_string());
        state.file_action(FileAction::ExportEncrypted, Some(path.clone()), None);
        finish(&mut state, &ctx);
        assert!(state.error.is_none());
        let ciphertext = crate::vault::read_bounded(&path).unwrap();
        assert!(crate::relay::archive::detect(&ciphertext));
        let plaintext =
            crate::relay::archive::decrypt(&ciphertext, secret.expose_secret()).unwrap();
        assert!(crate::relay::backup::decode(&plaintext).is_ok());
        assert!(!String::from_utf8_lossy(&ciphertext).contains("synthetic-token"));
        std::fs::remove_file(path).unwrap();
    }

    fn finish(state: &mut RelayState, ctx: &egui::Context) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.busy() && Instant::now() < deadline {
            state.tick(ctx);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!state.busy(), "synthetic worker must finish");
    }

    #[test]
    fn choosing_a_path_is_in_app_and_does_not_dispatch_a_file_operation() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.result = Some(result());
        state.file_action(FileAction::ExportNodes(OutputFormat::Original), None, None);
        assert!(!state.selecting_file(), "export requires plaintext consent");
        assert!(matches!(
            state.error,
            Some(Failure::Validation(Text::VaultPlaintextConsent))
        ));
        state.allow_plaintext = true;
        state.file_action(FileAction::ExportNodes(OutputFormat::Original), None, None);
        assert!(state.selecting_file() && !state.busy());
        assert!(state.allow_plaintext && state.result.is_some());
        state.clear(&ctx);
        assert!(!state.selecting_file() && !state.busy());
        assert!(state.last_directory.is_none());
    }

    #[test]
    fn last_browsed_folder_is_session_only_and_clear_forgets_it() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        let path = std::env::temp_dir().join("synthetic-last-folder");
        state.last_directory = Some(path.clone());
        state.file_action(FileAction::Import, None, None);
        assert!(state.selecting_file() && !state.busy());
        state.clear(&ctx);
        assert!(state.last_directory.is_none());
        assert!(!state.selecting_file());
    }

    #[test]
    fn unreadable_workspace_import_keeps_active_editor_and_reports_error_in_library() {
        use super::super::subscriptions::Selection;

        let ctx = egui::Context::default();
        let mut state = fixture();
        state.workspace.selected = Some(Selection::Collection(7));
        state.workspace.show_editor = true;
        state.workspace.name.push_str("Current collection");
        state.result = Some(result());
        state.start_workspace_import(Some(std::env::temp_dir()), None);
        assert!(state.workspace_import_pending());
        finish(&mut state, &ctx);
        assert!(state.workspace.selected == Some(Selection::Collection(7)));
        assert_eq!(state.workspace.name.as_str(), "Current collection");
        assert!(state.workspace.show_editor && !state.workspace.draft_dirty);
        assert!(state.result.is_some() && !state.workspace_import_pending());
        assert_eq!(state.workspace.error, Some(Text::WorkspaceImportReadFailed));
    }

    #[test]
    fn direct_engine_selection_cannot_bypass_file_validation() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        let previous = state.selected_engine.clone();
        state.file_action(FileAction::ChooseEngine, Some(std::env::temp_dir()), None);
        finish(&mut state, &ctx);
        assert!(state.error.is_some());
        assert_eq!(state.selected_engine, previous);
    }

    #[test]
    fn format_only_and_explicit_secret_creation_do_not_require_an_engine_or_existing_gist() {
        let mut state = fixture();
        state.output.clear();
        assert!(state.validate(RunMode::Preview, Path::new("")).is_ok());
        assert!(state.validate(RunMode::Publish, Path::new("")).is_err());
        state.acknowledge_publish = true;
        assert!(state.validate(RunMode::Publish, Path::new("")).is_ok());
    }

    #[test]
    fn created_target_is_reused_by_the_next_explicit_publish() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.output.clear();
        state.acknowledge_publish = true;
        state.start_with(RunMode::Publish, Path::new(""), |config, _, _, _, _| {
            assert!(config.output.is_empty() && !config.test_nodes);
            let mut result = result();
            result.created = Some(crate::relay::gist::CreatedGist {
                id: "aaaaaaaaaaaaaaaa".to_owned(),
                url: "https://gist.github.com/aaaaaaaaaaaaaaaa".to_owned(),
            });
            result.published = true;
            Ok(result)
        });
        finish(&mut state, &ctx);
        assert_eq!(state.output.as_str(), "aaaaaaaaaaaaaaaa");
        state.acknowledge_publish = true;
        state.start_with(RunMode::Publish, Path::new(""), |config, _, _, _, _| {
            assert_eq!(config.output, "aaaaaaaaaaaaaaaa");
            Ok(result())
        });
        finish(&mut state, &ctx);
        assert!(state.result.is_some());
    }

    #[test]
    fn cancellation_keeps_an_unverified_creation_receipt_for_reuse() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.output.clear();
        state.acknowledge_publish = true;
        let (release, wait) = mpsc::sync_channel(1);
        state.start_with(
            RunMode::Publish,
            Path::new(""),
            move |_, _, _, cancel, _| {
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
                assert!(cancel.load(Ordering::Acquire));
                Err(RelayError::Gist(
                    crate::relay::gist::GistError::CreatedUnverified(
                        crate::relay::gist::CreatedGist {
                            id: "aaaaaaaaaaaaaaaa".to_owned(),
                            url: "https://gist.github.com/aaaaaaaaaaaaaaaa".to_owned(),
                        },
                    ),
                ))
            },
        );
        state.request_cancel();
        release.send(()).unwrap();
        finish(&mut state, &ctx);
        assert_eq!(state.output.as_str(), "aaaaaaaaaaaaaaaa");
        assert!(state.result.is_none());
        assert!(matches!(
            state.error,
            Some(Failure::Operation(RelayError::Gist(
                crate::relay::gist::GistError::CreatedUnverified(_)
            )))
        ));
        state.acknowledge_publish = true;
        assert!(state.validate(RunMode::Publish, Path::new("")).is_ok());
    }

    #[test]
    fn gui_defaults_to_cleaning_while_cli_format_only_is_explicit() {
        let mut state = fixture();
        assert!(RelayState::default().test_nodes);
        assert!(!RelayConfig::default().test_nodes);
        state.test_nodes = true;
        assert_eq!(
            state.validate(RunMode::Preview, Path::new("")),
            Err(Text::SingboxRequired)
        );
    }

    #[test]
    fn relay_validation_rejects_missing_inputs_same_gist_and_unicode_overflow() {
        let engine = std::env::temp_dir().join("synthetic-sing-box");
        assert!(
            RelayState::default()
                .validate(RunMode::Preview, &engine)
                .is_err()
        );
        let mut state = fixture();
        assert!(state.validate(RunMode::Preview, &engine).is_ok());
        state.acknowledge_publish = true;
        state.output = state.source.clone();
        assert!(state.validate(RunMode::Publish, &engine).is_err());
        state = fixture();
        state.acknowledge_publish = true;
        state.title = Zeroizing::new("界".repeat(64));
        state.output_file = Zeroizing::new(format!("{}.conf", "界".repeat(59)));
        assert!(state.validate(RunMode::Publish, &engine).is_ok());
        state.title.push('界');
        assert!(state.validate(RunMode::Publish, &engine).is_err());
        let _ = state.title.pop();
        state.output_file.push('界');
        assert!(state.validate(RunMode::Publish, &engine).is_err());
    }

    #[test]
    fn offline_preview_works_during_cooldown_with_incomplete_publish_settings() {
        let mut state = fixture();
        state.editor =
            Some(EditorState::new("A = socks5, node.example.invalid, 1080\n", None).unwrap());
        state.test_nodes = false;
        state.output = Zeroizing::new("incomplete destination".into());
        state.token = Zeroizing::new("unfinished token".into());
        state.title = Zeroizing::new("a".repeat(65));
        state.cooldown_until = Instant::now().checked_add(Duration::from_secs(30));
        state.invalidate_result();
        state.start_with(RunMode::Preview, Path::new(""), crate::relay::run);
        finish(&mut state, &egui::Context::default());
        assert!(
            state
                .result
                .as_ref()
                .is_some_and(|result| result.total == 1 && !result.tested)
        );
        assert!(state.cooldown_seconds() > 0);
    }

    #[test]
    fn relay_dispatch_preserves_meaningful_filename_spaces() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.source_file = Zeroizing::new(" input.conf ".to_owned());
        state.output_file = Zeroizing::new(" output.conf ".to_owned());
        let (sent, received) = mpsc::sync_channel(1);
        state.start_with(
            RunMode::Preview,
            &std::env::temp_dir().join("synthetic-sing-box"),
            move |config, _, _, _, _| {
                sent.send((config.source_file.clone(), config.output_file.clone()))
                    .unwrap();
                Ok(result())
            },
        );
        finish(&mut state, &ctx);
        assert_eq!(
            received.recv_timeout(Duration::from_secs(2)).unwrap(),
            (" input.conf ".to_owned(), " output.conf ".to_owned())
        );
    }

    #[test]
    fn publication_and_compatibility_are_explicit_while_preview_never_implies_upload() {
        let engine = std::env::temp_dir().join("synthetic-sing-box");
        let mut state = fixture();
        assert!(!state.compatibility && !state.doh && !state.allow_public_output);
        assert!(state.validate(RunMode::Publish, &engine).is_err());
        state.acknowledge_publish = true;
        assert!(state.validate(RunMode::Publish, &engine).is_ok());
        state.acknowledge_publish = false;
        state.start_with(RunMode::Publish, &engine, |_, _, _, _, _| {
            panic!("unacknowledged publication must not dispatch")
        });
        assert!(!state.busy() && state.error.is_some());
        state.start_with(
            RunMode::Preview,
            &engine,
            |config, token, mode, _, progress| {
                assert!(matches!(mode, RunMode::Preview));
                assert_eq!(token, "synthetic-token");
                assert!(!config.compatibility && !config.doh && !config.allow_public_output);
                progress(Progress {
                    stage: crate::relay::RelayStage::Check,
                    checked: 1,
                    total: 2,
                });
                Ok(result())
            },
        );
        finish(&mut state, &egui::Context::default());
        assert!(state.result.is_some() && !state.allow_plaintext);
    }

    #[test]
    fn clearing_cancels_and_discards_late_results_but_keeps_admission_until_reaped() {
        let ctx = egui::Context::default();
        let engine = std::env::temp_dir().join("synthetic-sing-box");
        let mut state = fixture();
        let observed = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&observed);
        let (release, wait) = mpsc::sync_channel(1);
        state.start_with(RunMode::Preview, &engine, move |_, _, _, cancel, _| {
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            flag.store(cancel.load(Ordering::Acquire), Ordering::Release);
            Ok(result())
        });
        assert!(state.busy());
        state.clear(&ctx);
        assert!(
            state.busy(),
            "clearing must not admit a second worker before the first exits"
        );
        assert!(
            state.token.is_empty()
                && state.source.is_empty()
                && state.source_file.is_empty()
                && state.output.is_empty()
                && state.output_file.as_str() == RelayConfig::default().output_file
                && state.title.as_str() == RelayConfig::default().title
        );
        release.send(()).unwrap();
        finish(&mut state, &ctx);
        assert!(observed.load(Ordering::Acquire));
        assert!(state.result.is_none() && state.progress.is_none() && !state.allow_plaintext);
    }

    #[test]
    fn focus_loss_does_not_cancel_an_authorized_background_relay() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        let (release, wait) = mpsc::sync_channel(1);
        state.start_with(
            RunMode::Preview,
            &std::env::temp_dir().join("synthetic-sing-box"),
            move |_, _, _, cancel, _| {
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
                assert!(!cancel.load(Ordering::Acquire));
                Ok(result())
            },
        );
        assert!(state.busy());
        let mut input = egui::RawInput {
            focused: false,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .focused = Some(false);
        let output = ctx.run_ui(input, |ui| state.tick(ui.ctx()));
        output.drop_without_applying_deltas();
        assert!(state.busy() && !state.token.is_empty());
        release.send(()).unwrap();
        finish(&mut state, &ctx);
        assert!(state.result.is_some());
    }

    #[test]
    fn plaintext_copy_requires_fresh_consent_and_clear_removes_secret_widget_state() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.result = Some(result());
        let output = ctx.run_ui(Default::default(), |ui| state.copy(ui.ctx()));
        let copied_without_consent = output
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::CopyText(_)));
        output.drop_without_applying_deltas();
        assert!(!copied_without_consent);
        assert!(!state.busy());
        state.allow_plaintext = true;
        let output = ctx.run_ui(Default::default(), |ui| state.copy(ui.ctx()));
        assert!(state.busy());
        assert!(
            !output
                .platform_output
                .commands
                .iter()
                .any(|command| matches!(command, egui::OutputCommand::CopyText(_)))
        );
        output.drop_without_applying_deltas();
        let output = ctx.run_ui(Default::default(), |ui| finish(&mut state, ui.ctx()));
        let copied = output.platform_output.commands.iter().any(|command| matches!(command, egui::OutputCommand::CopyText(value) if value == "A = socks5, node.example.invalid, 1080\n"));
        output.drop_without_applying_deltas();
        assert!(copied);
        assert_eq!(state.notice, Some(Text::Copied));
        for id in SECRET_IDS {
            egui::text_edit::TextEditState::default().store(&ctx, egui::Id::unique(id));
        }
        state.clear(&ctx);
        assert!(state.result.is_none() && !state.allow_plaintext);
        for id in SECRET_IDS {
            assert!(egui::text_edit::TextEditState::load(&ctx, egui::Id::unique(id)).is_none());
        }
    }

    #[test]
    fn pending_converted_copy_cannot_outlive_its_consent_or_generation() {
        for clear in [false, true] {
            let ctx = egui::Context::default();
            let mut state = fixture();
            state.result = Some(result());
            state.allow_plaintext = true;
            state.output_format = OutputFormat::SingBox;
            state.copy(&ctx);
            assert!(state.busy());
            if clear {
                state.clear(&ctx);
                // Renewing consent must still not deliver the old generation.
                state.result = Some(result());
                state.output_format = OutputFormat::SingBox;
                state.allow_plaintext = true;
            } else {
                state.allow_plaintext = false;
            }
            let output = ctx.run_ui(Default::default(), |ui| finish(&mut state, ui.ctx()));
            assert!(
                !output
                    .platform_output
                    .commands
                    .iter()
                    .any(|command| matches!(command, egui::OutputCommand::CopyText(_)))
            );
            output.drop_without_applying_deltas();
            assert_ne!(state.notice, Some(Text::Copied));
        }
    }

    #[test]
    fn empty_result_cannot_be_copied_or_exported() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        let mut empty = result();
        empty.content.clear();
        state.result = Some(empty);
        state.allow_plaintext = true;
        let output = ctx.run_ui(Default::default(), |ui| state.copy(ui.ctx()));
        assert!(
            !output
                .platform_output
                .commands
                .iter()
                .any(|command| matches!(command, egui::OutputCommand::CopyText(_)))
        );
        output.drop_without_applying_deltas();
        for action in [
            FileAction::ExportNodes(OutputFormat::Original),
            FileAction::ExportBackup,
        ] {
            state.file_action(action, None, None);
            assert!(matches!(
                state.error,
                Some(Failure::Validation(Text::RelayFileError))
            ));
            assert!(!state.file_dialog_active());
        }
    }

    #[test]
    fn editor_keeps_original_indices_and_reset_preserves_source() {
        let source =
            "A = socks5, one.example.invalid, 1080\nB = socks5, two.example.invalid, 1080\n";
        let mut editor = EditorState::new(source, None).unwrap();
        editor.rows[1].included = false;
        editor.rows[0].name = Zeroizing::new("Renamed".into());
        let edits = editor.processing();
        assert_eq!(
            edits
                .nodes
                .iter()
                .map(|edit| edit.index)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        editor.reset();
        assert_eq!(editor.source.as_str(), source);
        assert!(editor.processing().nodes.is_empty());
        assert!(!editor.reveal);
    }

    #[test]
    fn editing_invalidates_result_and_source_change_discards_editor() {
        let mut state = fixture();
        state.editor =
            Some(EditorState::new("A = socks5, node.example.invalid, 1080\n", None).unwrap());
        state.result = Some(result());
        state.acknowledge_publish = true;
        state.invalidate_result();
        assert!(state.result.is_none() && state.editor.is_some() && !state.acknowledge_publish);
        state.input_changed();
        assert!(state.editor.is_none());
    }

    #[test]
    fn pure_preview_uses_local_snapshot_without_node_checks() {
        let mut state = fixture();
        let source = "A = socks5, node.example.invalid, 1080\n";
        state.editor = Some(EditorState::new(source, None).unwrap());
        state.test_nodes = false;
        state.start_with(
            RunMode::Preview,
            Path::new(""),
            move |config, _, _, _, _| {
                assert!(!config.test_nodes);
                assert_eq!(
                    config.local_content.as_deref().map(|s| s.as_str()),
                    Some(source)
                );
                assert_eq!(
                    config.expected_source.as_deref().map(|s| s.as_str()),
                    Some(source)
                );
                Ok(result())
            },
        );
        finish(&mut state, &egui::Context::default());
        assert!(state.result.is_some());
        assert_eq!(
            state
                .preview_names
                .iter()
                .map(|name| name.as_str())
                .collect::<Vec<_>>(),
            ["A"]
        );
        state.invalidate_result();
        assert!(state.preview_names.is_empty());
    }

    #[test]
    fn clear_removes_private_editor_widget_state() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.editor =
            Some(EditorState::new("A = socks5, node.example.invalid, 1080\n", None).unwrap());
        let id = egui::Id::unique(("relay-node-name", 0));
        egui::text_edit::TextEditState::default().store(&ctx, id);
        state.clear(&ctx);
        assert!(state.editor.is_none());
        assert!(egui::text_edit::TextEditState::load(&ctx, id).is_none());
    }

    #[test]
    fn focus_loss_hides_names_and_clears_editor_widget_state() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.editor =
            Some(EditorState::new("A = socks5, node.example.invalid, 1080\n", None).unwrap());
        state.editor.as_mut().unwrap().reveal = true;
        let id = egui::Id::unique(("relay-node-name", 0));
        egui::text_edit::TextEditState::default().store(&ctx, id);
        let mut input = egui::RawInput::default();
        input.viewports.get_mut(&input.viewport_id).unwrap().focused = Some(false);
        let output = ctx.run_ui(input, |ui| state.tick(ui.ctx()));
        output.drop_without_applying_deltas();
        assert!(!state.editor.as_ref().unwrap().reveal);
        assert!(egui::text_edit::TextEditState::load(&ctx, id).is_none());
    }
    #[test]
    fn validation_messages_follow_the_selected_language() {
        let failure = Failure::Validation(Text::RelayConsentRequired);
        assert_eq!(
            failure_message(crate::runtime::settings::Language::ZhCn, &failure),
            super::super::i18n::text(
                crate::runtime::settings::Language::ZhCn,
                Text::RelayConsentRequired
            )
        );
    }

    #[test]
    fn pinned_source_and_output_links_explain_the_latest_target_requirement() {
        let engine = std::env::temp_dir().join("synthetic-sing-box");
        for output in [false, true] {
            let mut state = fixture();
            let url = format!(
                "https://gist.githubusercontent.com/fixture/{}/raw/{}/nodes.conf",
                if output {
                    "fedcba9876543210"
                } else {
                    "0123456789abcdef"
                },
                "a".repeat(40)
            );
            if output {
                state.output = Zeroizing::new(url);
                state.output_file = Zeroizing::new(String::new());
            } else {
                state.source = Zeroizing::new(url);
                state.source_file = Zeroizing::new(String::new());
            }
            let mode = if output {
                RunMode::Publish
            } else {
                RunMode::Preview
            };
            assert_eq!(state.validate(mode, &engine), Err(Text::RelayPinnedTarget));
        }
    }

    #[test]
    fn operation_errors_follow_the_selected_language_without_changing_the_core_error() {
        let error = crate::relay::RelayError::NoHealthyNodes;
        let original = error.to_string();
        let failure = Failure::Operation(error);
        let translated = failure_message(crate::runtime::settings::Language::ZhCn, &failure);
        assert_ne!(translated, original);
        assert!(translated.contains("节点"));
    }

    #[test]
    fn import_is_engine_independent_and_opens_review_without_health_claims() {
        let mut state = fixture();
        state.source.clear();
        state.local_content = Some(Zeroizing::new(
            "A = socks5, node.example.invalid, 1080\n".into(),
        ));
        state.test_nodes = true;
        state.selected_engine = None;
        state.output = Zeroizing::new("not a gist".into());
        state.title = Zeroizing::new("x".repeat(65));
        state.start_with(RunMode::Import, Path::new(""), crate::relay::run);
        finish(&mut state, &egui::Context::default());
        assert!(state.step == RelayStep::Review);
        let result = state.result.as_ref().unwrap();
        assert!(!result.tested && result.healthy == 0 && result.unknown == 1);
        state.output.clear();
        state.title = Zeroizing::new("relay".into());
        state.acknowledge_publish = true;
        assert!(state.validate(RunMode::Publish, Path::new("")).is_ok());
    }

    #[test]
    fn rate_limit_cooldown_survives_processing_edits_but_new_source_can_recover() {
        let mut state = fixture();
        state.start_with(RunMode::Import, Path::new(""), |_, _, _, _, _| {
            Err(RelayError::Gist(
                crate::relay::gist::GistError::RateLimited {
                    retry_after_seconds: Some(30),
                },
            ))
        });
        finish(&mut state, &egui::Context::default());
        assert!(state.cooldown_seconds() > 0);
        let deadline = state.cooldown_until;
        state.invalidate_result();
        assert_eq!(state.cooldown_until, deadline);
        state.start_with(RunMode::Import, Path::new(""), |_, _, _, _, _| {
            panic!("cooldown must not issue another request")
        });
        assert!(!state.busy());
        state.input_changed();
        assert_eq!(state.cooldown_seconds(), 0);
        state.start_with(RunMode::Import, Path::new(""), |_, _, _, _, _| Ok(result()));
        finish(&mut state, &egui::Context::default());
        assert!(state.result.is_some());
    }

    #[test]
    fn file_import_is_transactional_and_backup_exports_preserve_completed_settings() {
        let ctx = egui::Context::default();
        let directory = std::env::temp_dir().join(format!(
            "script-hub-relay-file-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let directory = directory.canonicalize().unwrap();
        let source = directory.join("input.list");
        let content = "A = socks5, node.example.invalid, 1080\n";
        crate::vault::write_private(&source, content.as_bytes(), false).unwrap();
        let mut state = fixture();
        state.last_directory = Some(directory.clone());
        state.file_action(FileAction::Import, Some(source.clone()), None);
        assert!(state.busy());
        finish(&mut state, &ctx);
        assert_eq!(
            state.local_content.as_deref().map(|value| value.as_str()),
            Some(content)
        );
        assert_eq!(state.local_summary, Some((1, "Surge")));
        assert_eq!(state.last_directory.as_ref(), Some(&directory));
        assert!(state.token.is_empty() && state.test_nodes && !state.acknowledge_publish);
        assert!(!state.rename && !state.sort);
        let mut completed = result();
        completed.content = Zeroizing::new(content.into());
        state.result = Some(completed);
        state.completed_config = Some(state.private_config(Path::new("private-engine")));
        state.title.push_str("unapplied edit");
        state.allow_plaintext = true;
        let destination = directory.join("backup.json");
        state.file_action(FileAction::ExportBackup, Some(destination.clone()), None);
        finish(&mut state, &ctx);
        assert!(state.error.is_none());
        let bytes = Zeroizing::new(crate::vault::read_bounded(&destination).unwrap());
        let backup = crate::relay::backup::decode(&bytes).unwrap();
        assert!(!backup.config.title.contains("unapplied edit"));
        assert_eq!(backup.content.as_str(), content);
        let invalid = directory.join("invalid.json");
        crate::vault::write_private(&invalid, b"{bad", false).unwrap();
        state.file_action(FileAction::Import, Some(invalid.clone()), None);
        finish(&mut state, &ctx);
        assert!(state.error.is_some());
        assert_eq!(state.local_content.as_ref().unwrap().as_str(), content);
        assert!(state.result.is_some());
        state.allow_plaintext = true;
        state.file_action(
            FileAction::ExportNodes(OutputFormat::Original),
            Some(destination.clone()),
            None,
        );
        finish(&mut state, &ctx);
        assert!(state.error.is_some());
        assert_eq!(
            crate::vault::read_bounded(&destination).unwrap().as_slice(),
            bytes.as_slice()
        );
        use super::super::subscriptions::Selection;
        state.workspace.selected = Some(Selection::Subscription(7));
        state.output.push_str("old destination");
        state.start_workspace_import(Some(source.clone()), None);
        assert!(state.workspace.selected == Some(Selection::Subscription(7)));
        finish(&mut state, &ctx);
        assert!(state.workspace.selected == Some(Selection::Subscription(0)));
        assert!(state.workspace.draft_dirty && state.workspace.show_editor);
        assert_eq!(state.local_content.as_ref().unwrap().as_str(), content);
        assert!(state.output.is_empty() && state.workspace.error.is_none());
        for path in [source, destination, invalid] {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn worker_failures_keep_typed_status_for_current_language_display() {
        let mut state = fixture();
        state.start_with(
            RunMode::Preview,
            &std::env::temp_dir().join("synthetic-sing-box"),
            |_, _, _, _, _| {
                Err(RelayError::Gist(crate::relay::gist::GistError::HttpStatus(
                    429,
                )))
            },
        );
        finish(&mut state, &egui::Context::default());
        let failure = state.error.as_ref().unwrap();
        for language in [
            crate::runtime::settings::Language::En,
            crate::runtime::settings::Language::ZhCn,
            crate::runtime::settings::Language::Ja,
        ] {
            let message = failure_message(language, failure);
            assert!(message.contains("429"));
            assert!(message.starts_with(super::super::i18n::text(language, Text::RelayHttpStatus)));
        }
    }

    #[test]
    fn operation_plan_files_preserve_source_and_manual_edits_but_reset_consent() {
        use crate::relay::model::pipeline::{AddressFamily, Operation, Step};
        let ctx = egui::Context::default();
        let directory = std::env::temp_dir().join(format!(
            "script-hub-plan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let directory = directory.canonicalize().unwrap();
        let destination = directory.join("operations.json");
        let source = "A = socks5, node.example.invalid, 1080\n";
        let mut state = fixture();
        let mut editor = EditorState::new(source, None).unwrap();
        editor.rows[0].name = Zeroizing::new("Manual name".into());
        editor.options.pipeline = vec![Step {
            id: u64::MAX,
            enabled: true,
            operation: Operation::ResolveDomain {
                family: AddressFamily::PreferV4,
                options: Default::default(),
            },
        }];
        state.editor = Some(editor);
        state.file_action(FileAction::ExportPlan, Some(destination.clone()), None);
        assert!(!destination.exists());
        assert!(matches!(
            state.error,
            Some(Failure::Validation(Text::VaultPlaintextConsent))
        ));
        state.allow_plaintext = true;
        state.file_action(FileAction::ExportPlan, Some(destination.clone()), None);
        finish(&mut state, &ctx);
        assert!(state.error.is_none());
        let original = crate::vault::read_bounded(&destination).unwrap();
        let text = std::str::from_utf8(&original).unwrap();
        assert!(
            !text.contains("node.example.invalid")
                && !text.contains("Manual name")
                && !text.contains("synthetic-token")
        );
        state.editor.as_mut().unwrap().options.pipeline.clear();
        state.result = Some(result());
        state.allow_resolution = true;
        state.acknowledge_publish = true;
        state.file_action(FileAction::ImportPlan, Some(destination.clone()), None);
        finish(&mut state, &ctx);
        let editor = state.editor.as_ref().unwrap();
        assert_eq!(editor.source.as_str(), source);
        assert_eq!(editor.rows[0].name.as_str(), "Manual name");
        assert_eq!(editor.options.pipeline.len(), 1);
        assert!(state.result.is_none() && !state.allow_resolution && !state.acknowledge_publish);
        assert!(state.needs_resolution());
        state.allow_plaintext = true;
        state.file_action(FileAction::ExportPlan, Some(destination.clone()), None);
        finish(&mut state, &ctx);
        assert!(matches!(
            state.error,
            Some(Failure::Validation(Text::RelayFileExists))
        ));
        assert_eq!(crate::vault::read_bounded(&destination).unwrap(), original);
        let invalid = directory.join("invalid.json");
        crate::vault::write_private(&invalid, b"{bad", false).unwrap();
        state.file_action(FileAction::ImportPlan, Some(invalid.clone()), None);
        finish(&mut state, &ctx);
        assert!(matches!(
            state.error,
            Some(Failure::Validation(Text::OperationPlanInvalid))
        ));
        assert_eq!(state.editor.as_ref().unwrap().options.pipeline.len(), 1);
        for path in [destination, invalid] {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }
}
