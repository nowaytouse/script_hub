mod bulk_sources;
mod health;
mod library_edit;
mod library_sync;
mod migration;
mod nodes;
mod output;
mod pipeline;
mod preview_details;
mod relay_save;
mod schedule;
mod session;
mod view;

use super::input::TextInput;
use crate::desktop::i18n::{Text, text};
use crate::relay::{
    self, Progress, RelayConfig, RelayResult, RunMode,
    export::{OutputFormat, export_subscription},
    library::{Collection, Entry, Workspace},
    library_store::Store,
    model::Subscription,
    session_schedule::PrivateConfig,
};
use crate::runtime::settings::Language;
use gpui::{
    App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, PathPromptOptions,
    Subscription as UiSubscription,
};
use pipeline::{PipelineChanged, PipelineEditor};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use zeroize::{Zeroize, Zeroizing};

const MAX_INPUT: usize = 4 * 1024 * 1024;
const MAX_BACKUP: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Item {
    Entry(u64),
    Collection(u64),
}

enum Source {
    Link,
    Local(Zeroizing<String>),
    Saved(Item),
}

enum FileMode {
    Nodes,
    LibraryNodes,
}

struct Preview {
    config: PrivateConfig,
    result: RelayResult,
    source_revision: SourceRevision,
    details: preview_details::Details,
    public_gist_consent: output::PublicGistConsent,
}

#[derive(Clone)]
enum SourceRevision {
    Link(Zeroizing<String>, Zeroizing<String>),
    Bound,
}

struct ReviewedSource {
    revision: SourceRevision,
    content: Zeroizing<String>,
    nodes: usize,
}

impl SourceRevision {
    fn matches(&self, url: &str, file: &str) -> bool {
        match self {
            Self::Link(expected_url, expected_file) => {
                expected_url.as_str() == url.trim() && expected_file.as_str() == file.trim()
            }
            Self::Bound => true,
        }
    }
}

struct Run {
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Option<Progress>>>,
}

enum Completed {
    Unlocked(Arc<Store>, Option<Workspace>),
    Saved(Workspace, &'static str),
    SavedBulk(Workspace, &'static str),
    SavedRelay(Workspace, &'static str),
    RefreshedCache(Workspace, &'static str),
    SavedOutput(Workspace, &'static str),
    Preview(Box<Preview>),
    ReviewedSource(ReviewedSource),
    SourceChanged,
    Loaded(Zeroizing<String>, bool),
    Copied(Zeroizing<String>),
    Notice(&'static str),
    Published(Box<RelayResult>, Option<output::Receipt>),
    FormatChecked(output::FormatCheck),
    RelayFailed(relay::RelayError),
    Cancelled,
}

pub(super) struct Subscriptions {
    language: Language,
    showing_relay: bool,
    storage_exists: bool,
    workspace: Workspace,
    library: session::Library,
    session_password_form: bool,
    confirm_session_discard: bool,
    session_quit: bool,
    editor: Option<Item>,
    bulk_sources: Option<bulk_sources::Form>,
    deleting: Option<Item>,
    members: Vec<u64>,
    added_content: Option<Zeroizing<String>>,
    password: Entity<TextInput>,
    confirmation: Entity<TextInput>,
    query: Entity<TextInput>,
    name: Entity<TextInput>,
    entry_url: Entity<TextInput>,
    entry_file: Entity<TextInput>,
    relay_url: Entity<TextInput>,
    relay_file: Entity<TextInput>,
    relay_save_name: Entity<TextInput>,
    output_url: Entity<TextInput>,
    output_name: Entity<TextInput>,
    output_title: Entity<TextInput>,
    source_token: Entity<TextInput>,
    token: Entity<TextInput>,
    library_pipeline: Entity<PipelineEditor>,
    relay_pipeline: Entity<PipelineEditor>,
    node_editor: nodes::Editor,
    migration: migration::Migration,
    health: health::Controls,
    output: output::Controls,
    reviewed_source: Option<ReviewedSource>,
    allow_resolution: bool,
    schedule: schedule::Controls,
    pending: bool,
    running: Option<Run>,
    source: Source,
    preview: Option<Preview>,
    reviewing_relay_save: bool,
    resume_relay_save: bool,
    format: OutputFormat,
    rename: bool,
    sort: bool,
    allow_plaintext: bool,
    reviewing_publish: bool,
    last_publication: Option<RelayResult>,
    notice: Option<String>,
    error: bool,
    activation: Option<UiSubscription>,
}

pub(super) struct OpenRelay;
pub(super) struct OpenSubscriptions;
pub(super) struct QuitAfterLibraryReview;

impl EventEmitter<OpenRelay> for Subscriptions {}
impl EventEmitter<OpenSubscriptions> for Subscriptions {}
impl EventEmitter<QuitAfterLibraryReview> for Subscriptions {}

impl Subscriptions {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let storage = Store::exists();
        let schedule = schedule::Controls::new(cx);
        let health = health::Controls::new(cx);
        let output = output::Controls::new(cx);
        let mut input = |hint, private| cx.new(|cx| TextInput::new(hint, private, cx));
        let state = Self {
            language: Language::En,
            showing_relay: false,
            storage_exists: storage.as_ref().is_ok_and(|exists| *exists),
            workspace: Workspace::default(),
            library: session::Library::default(),
            session_password_form: false,
            confirm_session_discard: false,
            session_quit: false,
            editor: None,
            bulk_sources: None,
            deleting: None,
            members: Vec::new(),
            added_content: None,
            password: input("Library password", true),
            confirmation: input("Repeat library password", true),
            query: input("Search by name", false),
            name: input("Name", false),
            entry_url: input("https://…", true),
            entry_file: input("Optional source filename", false),
            relay_url: input("https://…", true),
            relay_file: input("Optional source filename", false),
            relay_save_name: input("", false),
            output_url: input("Existing Gist URL, or leave empty", true),
            output_name: input("nodes.list", false),
            output_title: input("Script Hub Relay", false),
            source_token: input("Optional GitHub token", true),
            token: input("GitHub token", true),
            library_pipeline: cx.new(PipelineEditor::new),
            relay_pipeline: cx.new(PipelineEditor::new),
            node_editor: nodes::Editor::new(cx),
            migration: migration::Migration::default(),
            health,
            output,
            reviewed_source: None,
            allow_resolution: false,
            schedule,
            pending: false,
            running: None,
            source: Source::Link,
            preview: None,
            reviewing_relay_save: false,
            resume_relay_save: false,
            format: OutputFormat::Original,
            rename: false,
            sort: false,
            allow_plaintext: false,
            reviewing_publish: false,
            last_publication: None,
            error: storage.is_err(),
            notice: storage.err().map(|error| error.to_string()),
            activation: None,
        };
        cx.observe(&state.query, |_, _, cx| cx.notify()).detach();
        cx.subscribe(&state.relay_pipeline, |this, _, _: &PipelineChanged, cx| {
            if this.schedule_active() {
                this.stop_schedule(cx);
            }
            this.invalidate_preview(cx);
        })
        .detach();
        cx.observe(&state.library_pipeline, |_, _, cx| cx.notify())
            .detach();
        cx.observe(&state.relay_pipeline, |_, _, cx| cx.notify())
            .detach();
        for field in [&state.relay_url, &state.relay_file] {
            cx.observe(field, |this, _, cx| {
                if this.node_editor.binding.as_ref().is_some_and(|binding| {
                    !binding.revision.matches(
                        this.relay_url.read(cx).value(),
                        this.relay_file.read(cx).value(),
                    )
                }) {
                    this.node_editor.clear(cx);
                }
                if matches!(this.source, Source::Link)
                    && this.reviewed_source.as_ref().is_some_and(|review| {
                        !review.revision.matches(
                            this.relay_url.read(cx).value(),
                            this.relay_file.read(cx).value(),
                        )
                    })
                {
                    this.reviewed_source = None;
                }
                if matches!(this.source, Source::Link) && this.current_preview(cx).is_none() {
                    if this.schedule_active() {
                        this.stop_schedule(cx);
                    }
                    this.invalidate_preview(cx);
                }
                cx.notify();
            })
            .detach();
        }
        for field in [&state.output_url, &state.output_name] {
            cx.observe(field, |this, _, cx| {
                this.revoke_public_gist_if_changed(cx);
                cx.notify();
            })
            .detach();
        }
        state
    }

    pub(super) fn show_relay(&mut self, showing: bool, cx: &mut Context<Self>) {
        if self.showing_relay != showing {
            self.showing_relay = showing;
            self.clear_library_passwords(cx);
            self.clear_relay_save(cx);
            self.conceal_preview(cx);
            if showing && self.library.is_open() && std::mem::take(&mut self.resume_relay_save) {
                self.reviewing_relay_save = self.current_preview(cx).is_some();
            }
            cx.notify();
        }
    }

    pub(super) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        let language = language.resolved();
        if self.language != language {
            self.language = language;
            for pipeline in [&self.library_pipeline, &self.relay_pipeline] {
                pipeline.update(cx, |editor, cx| editor.set_language(language, cx));
            }
            cx.notify();
        }
    }

    pub(super) fn can_close(&self, cx: &App) -> bool {
        !self.pending
            && !self.schedule_running()
            && self.editor.is_none()
            && self.relay_pipeline.read(cx).can_submit()
    }

    fn conceal_preview(&mut self, cx: &mut Context<Self>) {
        self.node_editor.hide(cx);
        self.health.clear_transient();
        self.schedule.clear_probe_consent();
        self.clear_backup_recipient(cx);
        self.allow_resolution = false;
        self.allow_plaintext = false;
        if let Some(preview) = self.preview.as_mut() {
            preview.details = preview_details::Details::Hidden;
            preview.public_gist_consent.clear();
        }
        self.reviewing_publish = false;
        clear(&self.token, cx);
    }

    fn invalidate_preview(&mut self, cx: &mut Context<Self>) {
        self.preview = None;
        self.resume_relay_save = false;
        self.clear_relay_save(cx);
        self.last_publication = None;
        self.output.clear_checks();
        self.output.clear_receipt(cx);
        self.conceal_preview(cx);
        cx.notify();
    }

    fn tr(&self, english: &'static str, chinese: &'static str) -> &'static str {
        if self.language == Language::ZhCn {
            chinese
        } else {
            english
        }
    }

    fn feedback(&mut self, ok: bool, message: impl Into<String>, cx: &mut Context<Self>) {
        self.error = !ok;
        self.notice = Some(message.into());
        cx.notify();
    }

    fn start(
        &mut self,
        work: impl FnOnce() -> Result<Completed, String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.pending || self.schedule_active() {
            return;
        }
        self.pending = true;
        self.set_pipeline_busy(true, cx);
        self.notice = None;
        let task = cx.background_executor().spawn(async move { work() });
        cx.spawn(async move |weak, cx| {
            let result = task.await;
            let _ = weak.update(cx, |this, cx| this.complete(result, cx));
        })
        .detach();
        if let Some(run) = &self.running {
            let progress = run.progress.clone();
            let executor = cx.background_executor().clone();
            cx.spawn(async move |weak, cx| {
                let mut previous = None;
                loop {
                    executor.timer(Duration::from_millis(150)).await;
                    let stage = progress
                        .lock()
                        .ok()
                        .and_then(|value| *value)
                        .map(|value| (value.stage, value.checked, value.total));
                    let active = weak
                        .update(cx, |this, cx| {
                            let active = this
                                .running
                                .as_ref()
                                .is_some_and(|run| Arc::ptr_eq(&progress, &run.progress));
                            if active && stage != previous {
                                cx.notify();
                            }
                            active
                        })
                        .unwrap_or(false);
                    if !active {
                        break;
                    }
                    previous = stage;
                }
            })
            .detach();
        }
        cx.notify();
    }

    fn complete(&mut self, result: Result<Completed, String>, cx: &mut Context<Self>) {
        self.pending = false;
        self.set_pipeline_busy(false, cx);
        self.running = None;
        match result {
            Err(error) => self.feedback(false, error, cx),
            Ok(Completed::RelayFailed(error)) => self.relay_failure(error, cx),
            Ok(Completed::Unlocked(store, data)) => {
                let notice = if data.is_some() {
                    Text::WorkspaceUnlocked
                } else {
                    Text::WorkspaceSaved
                };
                if let Some(data) = data {
                    self.workspace = data;
                }
                self.storage_exists = true;
                self.library = session::Library::Encrypted(store);
                self.session_password_form = false;
                self.confirm_session_discard = false;
                self.feedback(true, text(self.language, notice), cx);
                self.finish_session_quit(cx);
            }
            Ok(Completed::Saved(data, notice)) => {
                self.workspace = data;
                self.reset_editor(cx);
                if let Source::Saved(item) = self.source {
                    let exists = match item {
                        Item::Entry(id) => self.entry(id).is_some(),
                        Item::Collection(id) => self.collection(id).is_some(),
                    };
                    let source = if exists {
                        Source::Saved(item)
                    } else {
                        Source::Link
                    };
                    self.set_source(source, cx);
                }
                self.feedback(true, notice, cx);
            }
            Ok(Completed::SavedBulk(data, notice)) => {
                self.workspace = data;
                self.clear_bulk_sources(cx);
                self.feedback(true, notice, cx);
            }
            Ok(Completed::SavedRelay(data, notice)) => {
                self.workspace = data;
                self.clear_relay_save(cx);
                self.feedback(true, notice, cx);
                if self.showing_relay {
                    cx.emit(OpenSubscriptions);
                }
            }
            Ok(Completed::RefreshedCache(data, notice)) => {
                self.finish_cache_refresh(data, notice, cx)
            }
            Ok(Completed::SavedOutput(data, notice)) => self.finish_output_save(data, notice, cx),
            Ok(Completed::Preview(preview)) => {
                self.preview = Some(*preview);
                if self.current_preview(cx).is_some() {
                    self.feedback(true, self.tr("Preview ready", "预览已就绪"), cx);
                } else {
                    self.preview = None;
                    self.feedback(
                        false,
                        self.tr(
                            "The source changed. Run the preview again.",
                            "来源已更改，请重新预览。",
                        ),
                        cx,
                    );
                }
            }
            Ok(Completed::Loaded(content, library)) => {
                if library {
                    self.added_content = Some(content);
                } else {
                    self.set_source(Source::Local(content), cx);
                }
                self.feedback(true, self.tr("Local nodes loaded", "本地节点已加载"), cx);
            }
            Ok(Completed::ReviewedSource(review)) => {
                if review.revision.matches(
                    self.relay_url.read(cx).value(),
                    self.relay_file.read(cx).value(),
                ) {
                    self.reviewed_source = Some(review);
                    self.notice = None;
                    cx.notify();
                } else {
                    self.feedback(
                        false,
                        self.tr(
                            "The source changed. Review it again.",
                            "来源已更改，请重新检查。",
                        ),
                        cx,
                    );
                }
            }
            Ok(Completed::SourceChanged) => {
                self.reviewed_source = None;
                self.node_editor.clear(cx);
                self.feedback(
                    false,
                    self.tr(
                        "The remote source changed. Review it again.",
                        "远程来源已更改，请重新检查。",
                    ),
                    cx,
                );
            }
            Ok(Completed::Copied(output)) if self.allow_plaintext => {
                self.allow_plaintext = false;
                cx.write_to_clipboard(ClipboardItem::new_string(output.to_string()));
                self.feedback(
                    true,
                    self.tr("Results copied to clipboard", "结果已复制到剪贴板"),
                    cx,
                );
            }
            Ok(Completed::Copied(_)) => cx.notify(),
            Ok(Completed::Notice(notice)) => self.feedback(true, notice, cx),
            Ok(Completed::FormatChecked(check)) => self.finish_format_check(check, cx),
            Ok(Completed::Published(result, receipt)) => {
                let verification = if result.unchanged {
                    Text::RelayUnchanged
                } else {
                    Text::RelayPublished
                };
                self.preview = None;
                self.remember_publication(receipt, &result, cx);
                self.last_publication = Some(*result);
                self.feedback(true, text(self.language, verification), cx);
            }
            Ok(Completed::Cancelled) => cx.notify(),
        }
    }

    fn open_store(&mut self, cx: &mut Context<Self>) {
        if matches!(self.library, session::Library::Encrypted(_))
            || (matches!(self.library, session::Library::Temporary)
                && (self.storage_exists || !self.session_password_form))
        {
            return;
        }
        let password = Zeroizing::new(self.password.read(cx).value().to_owned());
        if !self.storage_exists && password.as_str() != self.confirmation.read(cx).value() {
            self.feedback(
                false,
                text(self.language, Text::WorkspacePasswordMismatch),
                cx,
            );
            return;
        }
        self.clear_library_passwords(cx);
        let exists = self.storage_exists;
        let data = (!exists).then(|| self.workspace.clone());
        self.start(
            move || {
                if let Some(data) = data {
                    let store = Store::create(&data, &password).map_err(message)?;
                    Ok(Completed::Unlocked(Arc::new(store), None))
                } else {
                    let (store, data) = Store::open(&password).map_err(message)?;
                    Ok(Completed::Unlocked(Arc::new(store), Some(data)))
                }
            },
            cx,
        );
    }

    fn lock(&mut self, cx: &mut Context<Self>) {
        self.clear_bulk_sources(cx);
        self.migration.clear();
        self.library = session::Library::Locked;
        self.session_password_form = false;
        self.confirm_session_discard = false;
        self.clear_library_passwords(cx);
        self.workspace = Workspace::default();
        self.reset_editor(cx);
        self.set_source(Source::Link, cx);
        self.last_publication = None;
        for field in [
            &self.query,
            &self.relay_url,
            &self.relay_file,
            &self.output_url,
            &self.output_name,
            &self.output_title,
            &self.source_token,
            &self.token,
        ] {
            clear(field, cx);
        }
        self.feedback(true, self.tr("Library locked", "资料库已锁定"), cx);
        self.finish_session_quit(cx);
    }

    fn reset_editor(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        self.deleting = None;
        self.added_content = None;
        self.members.clear();
        self.library_pipeline
            .update(cx, |editor, cx| editor.replace(Vec::new(), cx));
        for field in [&self.name, &self.entry_url, &self.entry_file] {
            clear(field, cx);
        }
    }

    fn edit(&mut self, item: Item, cx: &mut Context<Self>) {
        self.reset_editor(cx);
        self.notice = None;
        let mut steps = Vec::new();
        match item {
            Item::Entry(id) => {
                if let Some(entry) = self.entry(id) {
                    set(&self.name, &entry.name, cx);
                    set(&self.entry_url, &entry.source, cx);
                    set(&self.entry_file, &entry.source_file, cx);
                    steps = entry.processing.pipeline.clone();
                }
            }
            Item::Collection(id) => {
                if let Some(collection) = self.collection(id) {
                    set(&self.name, &collection.name, cx);
                    steps = collection.processing.pipeline.clone();
                    self.members = collection.members.clone();
                }
            }
        }
        self.library_pipeline
            .update(cx, |editor, cx| editor.replace(steps, cx));
        self.editor = Some(item);
        cx.notify();
    }

    fn entry(&self, id: u64) -> Option<&Entry> {
        self.workspace
            .subscriptions
            .iter()
            .find(|entry| entry.id == id)
    }

    fn collection(&self, id: u64) -> Option<&Collection> {
        self.workspace
            .collections
            .iter()
            .find(|collection| collection.id == id)
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let (Some(library), Some(item)) = (self.library.writer(), self.editor) else {
            return;
        };
        let mut workspace = self.workspace.clone();
        let name = self.name.read(cx).value().trim().to_owned();
        let source = Zeroizing::new(self.entry_url.read(cx).value().trim().to_owned());
        let source_file = Zeroizing::new(self.entry_file.read(cx).value().trim().to_owned());
        let content = self.added_content.clone();
        let members = self.members.clone();
        let pipeline = self.library_pipeline.read(cx);
        if !pipeline.can_submit() {
            self.feedback(
                false,
                self.tr(
                    "Finish or discard the step edit first.",
                    "请先完成或放弃步骤编辑。",
                ),
                cx,
            );
            return;
        }
        let steps = pipeline.steps().to_vec();
        let notice = text(
            self.language,
            if matches!(library, session::Library::Temporary) {
                Text::WorkspaceDraftSaved
            } else {
                Text::WorkspaceSaved
            },
        );
        self.start(
            move || {
                match item {
                    Item::Entry(id) => {
                        let last = workspace.subscriptions.iter().find(|entry| entry.id == id);
                        let mut entry = last.cloned().unwrap_or_else(|| Entry {
                            id,
                            name: String::new(),
                            source: String::new(),
                            source_file: String::new(),
                            content: String::new(),
                            processing: Default::default(),
                            publish: Default::default(),
                        });
                        entry.name = name;
                        entry.processing.pipeline = steps;
                        let changed_source = entry.source != source.as_str()
                            || entry.source_file != source_file.as_str();
                        if !entry.processing.nodes.is_empty()
                            && (changed_source
                                || content.as_ref().is_some_and(|replacement| {
                                    !relay::model::semantic_equal(&entry.content, replacement)
                                }))
                        {
                            return Err(
                                "Review existing per-node edits before replacing this source."
                                    .into(),
                            );
                        }
                        if changed_source {
                            entry.content.zeroize();
                        }
                        entry.source.zeroize();
                        entry.source_file.zeroize();
                        entry.source.push_str(&source);
                        entry.source_file.push_str(&source_file);
                        if let Some(content) = content {
                            validate_nodes(&content)?;
                            entry.content.zeroize();
                            entry.content.push_str(&content);
                        }
                        if entry.source.is_empty() && entry.content.is_empty() {
                            return Err("Enter a URL or load local nodes before saving.".into());
                        }
                        if id == 0 {
                            workspace.add_entry(entry).map_err(message)?;
                        } else {
                            workspace.update_entry(entry).map_err(message)?;
                        }
                    }
                    Item::Collection(id) => {
                        let mut collection = workspace
                            .collections
                            .iter()
                            .find(|collection| collection.id == id)
                            .cloned()
                            .unwrap_or_else(|| Collection {
                                id,
                                name: String::new(),
                                members: Vec::new(),
                                processing: Default::default(),
                                publish: Default::default(),
                            });
                        collection.name = name;
                        collection.processing.pipeline = steps;
                        if collection.members != members {
                            collection.processing.nodes.clear();
                        }
                        collection.members = members;
                        if id == 0 {
                            workspace.add_collection(collection).map_err(message)?;
                        } else {
                            workspace.update_collection(collection).map_err(message)?;
                        }
                    }
                }
                library.save(&workspace)?;
                Ok(Completed::Saved(workspace, notice))
            },
            cx,
        );
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        let (Some(library), Some(item)) = (self.library.writer(), self.deleting.take()) else {
            return;
        };
        let mut workspace = self.workspace.clone();
        let notice = self.tr("Item removed from library", "已从资料库移除该项目");
        self.start(
            move || {
                match item {
                    Item::Entry(id) => workspace.remove_entry(id),
                    Item::Collection(id) => workspace.remove_collection(id),
                }
                .map_err(message)?;
                library.save(&workspace)?;
                Ok(Completed::Saved(workspace, notice))
            },
            cx,
        );
    }

    fn set_source(&mut self, source: Source, cx: &mut Context<Self>) {
        self.reset_schedule(cx);
        self.node_editor.clear(cx);
        let steps = match &source {
            Source::Saved(Item::Entry(id)) => {
                self.entry(*id).map(|entry| &entry.processing.pipeline)
            }
            Source::Saved(Item::Collection(id)) => self
                .collection(*id)
                .map(|collection| &collection.processing.pipeline),
            _ => None,
        }
        .cloned()
        .unwrap_or_default();
        self.relay_pipeline
            .update(cx, |editor, cx| editor.replace(steps, cx));
        self.source = source;
        self.reviewed_source = None;
        self.invalidate_preview(cx);
    }

    fn set_pipeline_busy(&self, busy: bool, cx: &mut Context<Self>) {
        for pipeline in [&self.library_pipeline, &self.relay_pipeline] {
            pipeline.update(cx, |editor, cx| editor.set_busy(busy, cx));
        }
    }

    fn selecting_plan_file(&self, cx: &App) -> bool {
        self.library_pipeline.read(cx).selecting_file()
            || self.relay_pipeline.read(cx).selecting_file()
    }

    fn load_clipboard(&mut self, library: bool, cx: &mut Context<Self>) {
        let Some(content) = cx
            .read_from_clipboard()
            .and_then(|clipboard| clipboard.text())
        else {
            self.feedback(
                false,
                self.tr("The clipboard has no text", "剪贴板里没有文本"),
                cx,
            );
            return;
        };
        let content = Zeroizing::new(content);
        if content.trim().is_empty() || content.len() > MAX_INPUT {
            self.feedback(
                false,
                self.tr(
                    "Clipboard text exceeds the supported size or is empty",
                    "剪贴板内容为空或超出大小限制",
                ),
                cx,
            );
            return;
        }
        if library {
            self.added_content = Some(content);
        } else {
            self.set_source(Source::Local(content), cx);
        }
        self.notice = None;
        cx.notify();
    }

    fn select_file(&mut self, mode: FileMode, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        self.pending = true;
        self.set_pipeline_busy(true, cx);
        self.notice = None;
        let pick = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(self.tr("Open subscription file", "打开订阅文件").into()),
        });
        let executor = cx.background_executor().clone();
        cx.spawn(async move |weak, cx| {
            let result = match pick.await {
                Ok(Ok(Some(files))) => {
                    executor
                        .spawn(async move {
                            let path = files.first().ok_or("Choose a regular file.")?;
                            let bytes = read_limited(path, MAX_INPUT)?;
                            let content = Zeroizing::new(
                                std::str::from_utf8(&bytes)
                                    .map_err(|_| "The subscription is not UTF-8 text.")?
                                    .to_owned(),
                            );
                            validate_nodes(&content)?;
                            Ok(Completed::Loaded(
                                content,
                                matches!(mode, FileMode::LibraryNodes),
                            ))
                        })
                        .await
                }
                Ok(Ok(None)) => Ok(Completed::Cancelled),
                _ => Err("Could not open the selected file.".into()),
            };
            let _ = weak.update(cx, |this, cx| this.complete(result, cx));
        })
        .detach();
        cx.notify();
    }

    fn is_remote_source(&self) -> bool {
        match self.source {
            Source::Link => true,
            Source::Saved(Item::Entry(id)) => {
                self.entry(id).is_some_and(|entry| entry.content.is_empty())
            }
            _ => false,
        }
    }

    fn needs_source_review(&self, cx: &App) -> bool {
        self.is_remote_source()
            && self.reviewed_source.is_none()
            && self.node_editor.binding.is_none()
            && (!self.relay_pipeline.read(cx).steps().is_empty()
                || matches!(self.source, Source::Saved(Item::Entry(id))
                    if self.entry(id).is_some_and(|entry| entry.processing.is_active())))
    }

    fn needs_resolution(&self, cx: &App) -> bool {
        !self.needs_source_review(cx)
            && relay::model::pipeline::has_resolution(self.relay_pipeline.read(cx).steps())
    }

    fn preview(&mut self, cx: &mut Context<Self>) {
        self.preview_inner(false, None, cx);
    }

    fn preview_inner(
        &mut self,
        review_only: bool,
        probe: Option<relay::engine::ProbeOptions>,
        cx: &mut Context<Self>,
    ) {
        if self.check_schedule_cooldown(cx) {
            return;
        }
        let pipeline = self.relay_pipeline.read(cx);
        if !pipeline.can_submit() {
            self.feedback(
                false,
                self.tr(
                    "Finish or discard the step edit first.",
                    "请先完成或放弃步骤编辑。",
                ),
                cx,
            );
            return;
        }
        let steps = pipeline.steps().to_vec();
        let review_source = review_only || self.needs_source_review(cx);
        let allow_resolution = self.allow_resolution;
        if !review_source && self.needs_resolution(cx) && !allow_resolution {
            self.feedback(
                false,
                self.tr(
                    "Approve the network DNS lookups for this preview first.",
                    "请先允许本次预览进行网络 DNS 查询。",
                ),
                cx,
            );
            return;
        }
        self.invalidate_preview(cx);
        let format = self.format;
        let mut config = PrivateConfig(RelayConfig {
            output_format: format,
            allow_resolution,
            rename: self.rename,
            sort: self.sort,
            ..Default::default()
        });
        if let Some(options) = probe {
            config.0.test_nodes = true;
            config.0.engine = options.engine;
            config.0.concurrency = options.concurrency;
            config.0.timeout_seconds = options.timeout.as_secs();
            config.0.compatibility = options.compatibility;
            config.0.doh = options.doh;
        }
        let mut collection = None;
        let source_revision = match &self.source {
            Source::Link => {
                config.0.source = self.relay_url.read(cx).value().trim().to_owned();
                config.0.source_file = self.relay_file.read(cx).value().trim().to_owned();
                SourceRevision::Link(
                    Zeroizing::new(config.0.source.clone()),
                    Zeroizing::new(config.0.source_file.clone()),
                )
            }
            Source::Local(content) => {
                config.0.local_content = Some(content.clone());
                config.0.expected_source = Some(content.clone());
                SourceRevision::Bound
            }
            Source::Saved(Item::Entry(id)) => {
                let Some(entry) = self.entry(*id) else { return };
                config.0.source.clone_from(&entry.source);
                config.0.source_file.clone_from(&entry.source_file);
                config.0.processing = entry.processing.clone();
                if !entry.content.is_empty() {
                    let content = Zeroizing::new(entry.content.clone());
                    config.0.local_content = Some(content.clone());
                    config.0.expected_source = Some(content);
                }
                config.0.script_source.label = RelayConfig::script_source_label(&entry.name);
                SourceRevision::Bound
            }
            Source::Saved(Item::Collection(id)) => {
                collection = Some((self.workspace.clone(), *id));
                SourceRevision::Bound
            }
        };
        config.0.processing.pipeline = steps;
        let node_edits = self
            .node_editor
            .binding
            .as_ref()
            .filter(|binding| {
                binding.revision.matches(
                    self.relay_url.read(cx).value(),
                    self.relay_file.read(cx).value(),
                )
            })
            .map(|binding| (binding.source.clone(), binding.edits()));
        if config.0.local_content.is_none()
            && (config.0.processing.is_active() || config.0.test_nodes)
        {
            config.0.expected_source = self
                .reviewed_source
                .as_ref()
                .map(|review| review.content.clone());
        }
        let token = Zeroizing::new(self.source_token.read(cx).value().to_owned());
        clear(&self.source_token, cx);
        let (cancel, progress) = self.begin_run();
        self.start(
            move || {
                if let Some((workspace, id)) = collection {
                    let collection = workspace
                        .collections
                        .iter()
                        .find(|item| item.id == id)
                        .ok_or("The collection no longer exists.")?;
                    let merged = workspace.merge(id).map_err(message)?;
                    config.0.local_content = Some(merged.clone());
                    config.0.expected_source = Some(merged);
                    let steps = std::mem::take(&mut config.0.processing.pipeline);
                    config.0.processing = collection.processing.clone();
                    config.0.processing.pipeline = steps;
                    config.0.script_source.kind = "collection";
                    config.0.script_source.label =
                        RelayConfig::script_source_label(&collection.name);
                }
                if review_source {
                    let result =
                        match relay::run(&config.0, &token, RunMode::Import, &cancel, &progress) {
                            Ok(result) => result,
                            Err(error) => return Ok(Completed::RelayFailed(error)),
                        };
                    return Ok(Completed::ReviewedSource(ReviewedSource {
                        revision: source_revision,
                        content: result.content,
                        nodes: result.total,
                    }));
                }
                if let Some((source, edits)) = node_edits {
                    config.0.expected_source = Some(source);
                    config.0.processing.nodes = edits;
                }
                if config.0.test_nodes && config.0.expected_source.is_none() {
                    return Ok(Completed::RelayFailed(relay::RelayError::InvalidInput));
                }
                let result =
                    match relay::run(&config.0, &token, RunMode::Preview, &cancel, &progress) {
                        Ok(result) => result,
                        Err(relay::RelayError::SourceChanged) => {
                            return Ok(Completed::SourceChanged);
                        }
                        Err(error) => return Ok(Completed::RelayFailed(error)),
                    };
                Ok(Completed::Preview(Box::new(Preview {
                    config,
                    result,
                    source_revision,
                    details: preview_details::Details::Hidden,
                    public_gist_consent: output::PublicGistConsent::default(),
                })))
            },
            cx,
        );
    }

    fn begin_run(&mut self) -> (Arc<AtomicBool>, impl Fn(Progress) + Sync + use<>) {
        let cancel = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new(None));
        self.running = Some(Run {
            cancel: cancel.clone(),
            progress: progress.clone(),
        });
        (cancel, move |value| {
            if let Ok(mut update) = progress.lock() {
                *update = Some(value);
            }
        })
    }

    fn current_preview(&self, cx: &App) -> Option<&Preview> {
        if self.node_editor.open || !self.relay_pipeline.read(cx).can_submit() {
            return None;
        }
        let preview = self.preview.as_ref()?;
        preview
            .source_revision
            .matches(
                self.relay_url.read(cx).value(),
                self.relay_file.read(cx).value(),
            )
            .then_some(preview)
    }

    fn with_preview(&mut self, cx: &mut Context<Self>) -> Option<&Preview> {
        if self
            .current_preview(cx)
            .is_none_or(|preview| preview.result.content.is_empty())
        {
            self.feedback(
                false,
                self.tr("Preview the current source first", "请先预览当前来源"),
                cx,
            );
            return None;
        }
        self.preview.as_ref()
    }

    fn cancel_run(&mut self, cx: &mut Context<Self>) {
        if let Some(run) = &self.running {
            run.cancel.store(true, Ordering::Relaxed);
            cx.notify();
        }
    }

    fn open_saved(&mut self, item: Item, cx: &mut Context<Self>) {
        self.set_source(Source::Saved(item), cx);
        self.showing_relay = true;
        self.notice = None;
        if let Some(settings) = match item {
            Item::Entry(id) => self.entry(id).map(|entry| &entry.publish),
            Item::Collection(id) => self.collection(id).map(|entry| &entry.publish),
        } {
            let settings = settings.clone();
            set(&self.output_url, &settings.output, cx);
            set(&self.output_name, &settings.output_file, cx);
            set(&self.output_title, &settings.title, cx);
            self.format = settings.format;
            self.set_schedule_interval(settings.interval_minutes, cx);
        }
        cx.emit(OpenRelay);
        cx.notify();
    }

    fn copy_result(&mut self, cx: &mut Context<Self>) {
        if !self.allow_plaintext || !self.output.local_format_valid() {
            return;
        }
        let Some(preview) = self.with_preview(cx) else {
            return;
        };
        let content = preview.result.content.clone();
        let format = self.output.local_format();
        self.start(
            move || {
                export_subscription(&content, format)
                    .map(Completed::Copied)
                    .map_err(message)
            },
            cx,
        );
    }

    fn export_result(&mut self, cx: &mut Context<Self>) {
        if !self.allow_plaintext || !self.output.local_format_valid() {
            return;
        }
        let Some(preview) = self.with_preview(cx) else {
            return;
        };
        let content = preview.result.content.clone();
        self.allow_plaintext = false;
        let format = self.output.local_format();
        let directory = documents_directory();
        let suggested_name = format!("scripthub-nodes.{}", format.extension());
        let prompt = cx.prompt_for_new_path(&directory, Some(&suggested_name));
        let executor = cx.background_executor().clone();
        let notice = self.tr("Subscription exported", "订阅已导出");
        self.pending = true;
        self.set_pipeline_busy(true, cx);
        self.notice = None;
        cx.spawn(async move |weak, cx| {
            let result = match prompt.await {
                Ok(Ok(Some(path))) => {
                    executor
                        .spawn(async move {
                            if !crate::desktop::template_paths::safe_export_destination(&path, None) {
                                return Err("Choose an export file outside Git working trees and vault folders.".into());
                            }
                            let output = export_subscription(&content, format).map_err(message)?;
                            crate::vault::write_private(&path, output.as_bytes(), false)
                                .map_err(message)?;
                            Ok(Completed::Notice(notice))
                        })
                        .await
                }
                Ok(Ok(None)) => Ok(Completed::Cancelled),
                _ => Err("Could not choose an export destination.".into()),
            };
            let _ = weak.update(cx, |this, cx| this.complete(result, cx));
        })
        .detach();
        cx.notify();
    }

    fn publish(&mut self, cx: &mut Context<Self>) {
        if !self.reviewing_publish
            || !self.output.gist_format_valid()
            || self.check_schedule_cooldown(cx)
        {
            return;
        }
        let Some(preview) = self.current_preview(cx).filter(|preview| {
            !preview.result.content.is_empty()
                && (!preview.result.tested || preview.result.healthy > 0)
        }) else {
            self.feedback(
                false,
                self.tr("Preview the current source first", "请先预览当前来源"),
                cx,
            );
            return;
        };
        let mut config = self.publication_config(preview, cx);
        config.0.allow_public_output = self.allows_public_gist(preview, cx);
        let result = preview.result.clone();
        let token = Zeroizing::new(self.token.read(cx).value().to_owned());
        if token.trim().is_empty() {
            self.feedback(
                false,
                self.tr("Enter a GitHub token", "请输入 GitHub 令牌"),
                cx,
            );
            return;
        }
        self.cancel_publication_review(cx);
        let (cancel, progress) = self.begin_run();
        self.start(
            move || {
                Ok(
                    match relay::upload_prepared(&config.0, &token, result, &cancel, &progress) {
                        Ok(result) => {
                            let receipt = output::Receipt::from_result(&config.0, &result);
                            Completed::Published(Box::new(result), receipt)
                        }
                        Err(error) => Completed::RelayFailed(error),
                    },
                )
            },
            cx,
        );
    }

    fn cancel_publication_review(&mut self, cx: &mut Context<Self>) {
        if let Some(preview) = self.preview.as_mut() {
            preview.public_gist_consent.clear();
        }
        clear(&self.token, cx);
        self.schedule.clear_probe_consent();
        self.reviewing_publish = false;
        cx.notify();
    }

    fn publication_config(&self, preview: &Preview, cx: &App) -> PrivateConfig {
        let mut config = PrivateConfig(preview.config.0.clone());
        config.0.output_format = self.format;
        config.0.output = self.output_url.read(cx).value().trim().to_owned();
        config.0.output_file =
            gist_filename(self.output_name.read(cx).value(), self.format).into_owned();
        config.0.allow_public_output = false;
        let title = self.output_title.read(cx).value().trim();
        if !title.is_empty() {
            config.0.title = title.to_owned();
        }
        config
    }
}

fn gist_filename(filename: &str, format: OutputFormat) -> Cow<'_, str> {
    let filename = filename.trim();
    if filename.is_empty() {
        Cow::Owned(format!("nodes.{}", format.extension()))
    } else {
        Cow::Borrowed(filename)
    }
}

fn documents_directory() -> PathBuf {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .map(|home| home.join("Documents"))
        .filter(|path| path.is_dir())
        .unwrap_or_else(std::env::temp_dir)
}

fn clear(field: &Entity<TextInput>, cx: &mut Context<Subscriptions>) {
    field.update(cx, |input, cx| input.clear(cx));
}

fn set(field: &Entity<TextInput>, value: &str, cx: &mut Context<Subscriptions>) {
    field.update(cx, |input, cx| input.set_value(value, cx));
}

fn message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn validate_nodes(content: &str) -> Result<(), String> {
    let subscription = Subscription::parse(content).map_err(message)?;
    if subscription.nodes().is_empty() {
        return Err("The subscription contains no supported nodes.".into());
    }
    Ok(())
}

fn read_limited(path: &Path, limit: usize) -> Result<Zeroizing<Vec<u8>>, String> {
    let contents = Zeroizing::new(crate::vault::read_bounded(path).map_err(
        |error| match error {
            crate::vault::VaultError::TooLarge => {
                "The selected file exceeds the supported size limit."
            }
            _ => "Could not safely read the selected file.",
        },
    )?);
    if contents.len() > limit {
        return Err("The selected file exceeds the supported size limit.".into());
    }
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_preview_cannot_be_reused_for_another_url_or_source_file() {
        let revision = SourceRevision::Link(
            Zeroizing::new("https://example.invalid/subscription".into()),
            Zeroizing::new("source.list".into()),
        );
        assert!(revision.matches("  https://example.invalid/subscription ", " source.list "));
        assert!(!revision.matches("https://example.invalid/other", "source.list"));
        assert!(!revision.matches("https://example.invalid/subscription", "changed.list"));
        assert!(SourceRevision::Bound.matches("", ""));
    }
}
