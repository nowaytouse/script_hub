mod view;

use super::input::TextInput;
use crate::relay::{
    self, Progress, RelayConfig, RelayResult, RunMode,
    export::{OutputFormat, export_subscription},
    library::{Collection, Entry, PublishSettings, Workspace},
    library_store::Store,
    model::Subscription,
};
use crate::runtime::settings::Language;
use gpui::{
    App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, PathPromptOptions,
    Subscription as UiSubscription,
};
use std::{
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
    Backup,
}

struct Preview {
    config: PrivateConfig,
    result: RelayResult,
    source_revision: SourceRevision,
    names: Vec<Zeroizing<String>>,
}

enum SourceRevision {
    Link(Zeroizing<String>, Zeroizing<String>),
    Bound,
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

struct PrivateConfig(RelayConfig);

impl Drop for PrivateConfig {
    fn drop(&mut self) {
        self.0.source.zeroize();
        self.0.source_file.zeroize();
        self.0.output.zeroize();
        self.0.output_file.zeroize();
        self.0.title.zeroize();
    }
}

struct Run {
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Option<Progress>>>,
}

enum Completed {
    Unlocked(Arc<Store>, Option<Workspace>),
    Saved(Workspace, &'static str),
    Preview(Box<Preview>),
    Loaded(Zeroizing<String>, bool),
    Copied(Zeroizing<String>),
    Notice(&'static str),
    Published(Box<RelayResult>),
    Cancelled,
}

pub(super) struct Subscriptions {
    language: Language,
    showing_relay: bool,
    storage_exists: bool,
    workspace: Workspace,
    store: Option<Arc<Store>>,
    editor: Option<Item>,
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
    output_url: Entity<TextInput>,
    output_name: Entity<TextInput>,
    output_title: Entity<TextInput>,
    source_token: Entity<TextInput>,
    token: Entity<TextInput>,
    pending: bool,
    running: Option<Run>,
    source: Source,
    preview: Option<Preview>,
    format: OutputFormat,
    allow_plaintext: bool,
    reviewing_publish: bool,
    showing_node_names: bool,
    last_publication: Option<RelayResult>,
    notice: Option<String>,
    error: bool,
    activation: Option<UiSubscription>,
}

pub(super) struct OpenRelay;

impl EventEmitter<OpenRelay> for Subscriptions {}

impl Subscriptions {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let storage = Store::exists();
        let mut input = |hint, private| cx.new(|cx| TextInput::new(hint, private, cx));
        let state = Self {
            language: Language::En,
            showing_relay: false,
            storage_exists: storage.as_ref().is_ok_and(|exists| *exists),
            workspace: Workspace::default(),
            store: None,
            editor: None,
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
            output_url: input("Existing secret Gist URL, or leave empty", true),
            output_name: input("nodes.list", false),
            output_title: input("Script Hub Relay", false),
            source_token: input("Optional GitHub token", true),
            token: input("GitHub token", true),
            pending: false,
            running: None,
            source: Source::Link,
            preview: None,
            format: OutputFormat::Original,
            allow_plaintext: false,
            reviewing_publish: false,
            showing_node_names: false,
            last_publication: None,
            error: storage.is_err(),
            notice: storage.err().map(|error| error.to_string()),
            activation: None,
        };
        cx.observe(&state.query, |_, _, cx| cx.notify()).detach();
        for field in [&state.relay_url, &state.relay_file] {
            cx.observe(field, |this, _, cx| {
                if matches!(this.source, Source::Link) && this.current_preview(cx).is_none() {
                    this.preview = None;
                    this.last_publication = None;
                    this.conceal_preview(cx);
                }
                cx.notify();
            })
            .detach();
        }
        state
    }

    pub(super) fn show_relay(&mut self, showing: bool, cx: &mut Context<Self>) {
        if self.showing_relay != showing {
            self.showing_relay = showing;
            self.conceal_preview(cx);
            cx.notify();
        }
    }

    pub(super) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        let language = language.resolved();
        if self.language != language {
            self.language = language;
            cx.notify();
        }
    }

    pub(super) fn can_close(&self) -> bool {
        !self.pending && self.editor.is_none()
    }

    fn conceal_preview(&mut self, cx: &mut Context<Self>) {
        self.allow_plaintext = false;
        self.showing_node_names = false;
        self.reviewing_publish = false;
        clear(&self.token, cx);
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
        if self.pending {
            return;
        }
        self.pending = true;
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
        self.running = None;
        match result {
            Err(error) => self.feedback(false, error, cx),
            Ok(Completed::Unlocked(store, data)) => {
                if let Some(data) = data {
                    self.workspace = data;
                }
                self.storage_exists = true;
                self.store = Some(store);
                self.feedback(true, self.tr("Library unlocked", "资料库已解锁"), cx);
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
            Ok(Completed::Published(result)) => {
                self.preview = None;
                self.last_publication = Some(*result);
                self.feedback(
                    true,
                    self.tr("Secret Gist verified", "私密 Gist 已核验"),
                    cx,
                );
            }
            Ok(Completed::Cancelled) => cx.notify(),
        }
    }

    fn open_store(&mut self, cx: &mut Context<Self>) {
        let password = Zeroizing::new(self.password.read(cx).value().to_owned());
        if !self.storage_exists && password.as_str() != self.confirmation.read(cx).value() {
            self.feedback(false, self.tr("Passwords differ", "两次输入的密码不同"), cx);
            return;
        }
        clear(&self.password, cx);
        clear(&self.confirmation, cx);
        let exists = self.storage_exists;
        self.start(
            move || {
                if exists {
                    let (store, data) = Store::open(&password).map_err(message)?;
                    Ok(Completed::Unlocked(Arc::new(store), Some(data)))
                } else {
                    let store = Store::create(&Workspace::default(), &password).map_err(message)?;
                    Ok(Completed::Unlocked(Arc::new(store), None))
                }
            },
            cx,
        );
    }

    fn lock(&mut self, cx: &mut Context<Self>) {
        self.store = None;
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
    }

    fn reset_editor(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        self.deleting = None;
        self.added_content = None;
        self.members.clear();
        for field in [&self.name, &self.entry_url, &self.entry_file] {
            clear(field, cx);
        }
    }

    fn edit(&mut self, item: Item, cx: &mut Context<Self>) {
        self.reset_editor(cx);
        self.notice = None;
        match item {
            Item::Entry(id) => {
                if let Some(entry) = self.entry(id) {
                    set(&self.name, &entry.name, cx);
                    set(&self.entry_url, &entry.source, cx);
                    set(&self.entry_file, &entry.source_file, cx);
                }
            }
            Item::Collection(id) => {
                if let Some(collection) = self.collection(id) {
                    set(&self.name, &collection.name, cx);
                    self.members = collection.members.clone();
                }
            }
        }
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
        let (Some(store), Some(item)) = (self.store.clone(), self.editor) else {
            return;
        };
        let mut workspace = self.workspace.clone();
        let name = self.name.read(cx).value().trim().to_owned();
        let source = Zeroizing::new(self.entry_url.read(cx).value().trim().to_owned());
        let source_file = Zeroizing::new(self.entry_file.read(cx).value().trim().to_owned());
        let content = self.added_content.clone();
        let members = self.members.clone();
        let notice = self.tr("Library saved", "资料库已保存");
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
                        collection.members = members;
                        if id == 0 {
                            workspace.add_collection(collection).map_err(message)?;
                        } else {
                            workspace.update_collection(collection).map_err(message)?;
                        }
                    }
                }
                store.save(&workspace).map_err(message)?;
                Ok(Completed::Saved(workspace, notice))
            },
            cx,
        );
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        let (Some(store), Some(item)) = (self.store.clone(), self.deleting.take()) else {
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
                store.save(&workspace).map_err(message)?;
                Ok(Completed::Saved(workspace, notice))
            },
            cx,
        );
    }

    fn set_source(&mut self, source: Source, cx: &mut Context<Self>) {
        self.source = source;
        self.preview = None;
        self.last_publication = None;
        self.conceal_preview(cx);
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
        let store = self.store.clone();
        let mut workspace = self.workspace.clone();
        self.pending = true;
        self.notice = None;
        let pick = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(self.tr("Open subscription file", "打开订阅文件").into()),
        });
        let executor = cx.background_executor().clone();
        let notice = self.tr(
            "Backup stored in the encrypted library",
            "备份已存入加密资料库",
        );
        cx.spawn(async move |weak, cx| {
            let result = match pick.await {
                Ok(Ok(Some(files))) => {
                    executor
                        .spawn(async move {
                            let path = files.first().ok_or("Choose a regular file.")?;
                            match mode {
                                FileMode::Nodes | FileMode::LibraryNodes => {
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
                                }
                                FileMode::Backup => {
                                    let store = store.ok_or("Unlock the library first.")?;
                                    let bytes = read_limited(path, MAX_BACKUP)?;
                                    let backup = relay::backup::decode(&bytes).map_err(message)?;
                                    let cfg = &backup.config;
                                    let name = cfg.title.trim();
                                    let name =
                                        if workspace.subscriptions.iter().any(|e| e.name == name) {
                                            format!("Backup {}", workspace.next_id)
                                        } else {
                                            name.to_owned()
                                        };
                                    workspace
                                        .add_entry(Entry {
                                            id: 0,
                                            name,
                                            source: cfg.source.clone(),
                                            source_file: cfg.source_file.clone(),
                                            content: backup.content.to_string(),
                                            processing: cfg.processing.clone(),
                                            publish: PublishSettings {
                                                output: cfg.output.clone(),
                                                output_file: cfg.output_file.clone(),
                                                format: cfg.output_format,
                                                title: cfg.title.clone(),
                                                ..Default::default()
                                            },
                                        })
                                        .map_err(message)?;
                                    store.save(&workspace).map_err(message)?;
                                    Ok(Completed::Saved(workspace, notice))
                                }
                            }
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

    fn preview(&mut self, cx: &mut Context<Self>) {
        self.preview = None;
        self.last_publication = None;
        self.conceal_preview(cx);
        let format = self.format;
        let mut config = PrivateConfig(RelayConfig {
            output_format: format,
            ..Default::default()
        });
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
                    config.0.processing = collection.processing.clone();
                    config.0.script_source.kind = "collection";
                    config.0.script_source.label =
                        RelayConfig::script_source_label(&collection.name);
                }
                let result = relay::run(&config.0, &token, RunMode::Preview, &cancel, &progress)
                    .map_err(message)?;
                let parsed = Subscription::parse(&result.content).map_err(message)?;
                let names = parsed
                    .nodes()
                    .iter()
                    .take(24)
                    .map(|node| Zeroizing::new(node.name().chars().take(120).collect()))
                    .collect();
                Ok(Completed::Preview(Box::new(Preview {
                    config,
                    result,
                    source_revision,
                    names,
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
        if self.current_preview(cx).is_none() {
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
        }
        cx.emit(OpenRelay);
        cx.notify();
    }

    fn copy_result(&mut self, cx: &mut Context<Self>) {
        if !self.allow_plaintext {
            return;
        }
        let Some(preview) = self.with_preview(cx) else {
            return;
        };
        let content = preview.result.content.clone();
        let format = self.format;
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
        if !self.allow_plaintext {
            return;
        }
        let Some(preview) = self.with_preview(cx) else {
            return;
        };
        let content = preview.result.content.clone();
        self.allow_plaintext = false;
        let format = self.format;
        let directory = documents_directory();
        let suggested_name = format!("scripthub-nodes.{}", format.extension());
        let prompt = cx.prompt_for_new_path(&directory, Some(&suggested_name));
        let executor = cx.background_executor().clone();
        let notice = self.tr("Subscription exported", "订阅已导出");
        self.pending = true;
        self.notice = None;
        cx.spawn(async move |weak, cx| {
            let result = match prompt.await {
                Ok(Ok(Some(path))) => {
                    executor
                        .spawn(async move {
                            if !crate::gui::templates::safe_export_destination(&path, None) {
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
        if !self.reviewing_publish {
            return;
        }
        let Some(preview) = self.with_preview(cx) else {
            return;
        };
        let mut config = PrivateConfig(preview.config.0.clone());
        let result = preview.result.clone();
        config.0.output = self.output_url.read(cx).value().trim().to_owned();
        let filename = self.output_name.read(cx).value().trim();
        config.0.output_file = if filename.is_empty() {
            format!("nodes.{}", self.format.extension())
        } else {
            filename.to_owned()
        };
        let title = self.output_title.read(cx).value().trim();
        if !title.is_empty() {
            config.0.title = title.to_owned();
        }
        let token = Zeroizing::new(self.token.read(cx).value().to_owned());
        if token.trim().is_empty() {
            self.feedback(
                false,
                self.tr("Enter a GitHub token", "请输入 GitHub 令牌"),
                cx,
            );
            return;
        }
        clear(&self.token, cx);
        self.reviewing_publish = false;
        let (cancel, progress) = self.begin_run();
        self.start(
            move || {
                relay::upload_prepared(&config.0, &token, result, &cancel, &progress)
                    .map(|result| Completed::Published(Box::new(result)))
                    .map_err(message)
            },
            cx,
        );
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
