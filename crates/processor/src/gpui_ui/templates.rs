use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, ClipboardItem, Context, Div, Entity, ExternalPaths, FontWeight,
    InteractiveElement, IntoElement, ListHorizontalSizingBehavior, ParentElement,
    PathPromptOptions, Render, SharedString, Stateful, StatefulInteractiveElement, Styled,
    Subscription, Task, Window, div, px, rgb, uniform_list,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use zeroize::{Zeroize, Zeroizing};

use super::{Palette, button, heading, input::TextInput, panel};
use crate::gui::i18n::{Text, text};
use crate::gui::templates::{
    configuration_export_path, inferred_client, safe_export_destination,
    writable_template_destination,
};
use crate::runtime::settings::Language;
use crate::vault::{
    self, Client, Library, Template, UnlockedVault, VaultError, managed::ManagedStore,
};

const BUNDLED: &[u8] = include_bytes!("../../assets/templates.vault");
const IDLE_LIMIT: Duration = Duration::from_secs(5 * 60);
const DIALOG_FOCUS_SETTLE: Duration = Duration::from_millis(80);
const CLIENTS: [Client; 5] = [
    Client::Surge,
    Client::Shadowrocket,
    Client::FlClash,
    Client::SingBox,
    Client::Other,
];

#[derive(Clone)]
enum Source {
    Bundled,
    Disk(PathBuf),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Unlock,
    Create,
    Prepare,
}

struct Preparation {
    inputs: Vec<PathBuf>,
    outputs: Option<(PathBuf, PathBuf)>,
}

impl Drop for Templates {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

enum Completed {
    Opened(UnlockedVault, Option<PathBuf>),
    Imported(Vec<Template>),
    Saved(PathBuf),
    Exported,
    Prepared(PathBuf),
}

pub(super) struct Templates {
    language: Language,
    workspace: Option<PathBuf>,
    next_workspace: Option<Option<PathBuf>>,
    store: Option<ManagedStore>,
    managed: Option<PathBuf>,
    source: Source,
    mode: Mode,
    preparation: Preparation,
    password: Entity<TextInput>,
    confirmation: Entity<TextInput>,
    vault: Option<UnlockedVault>,
    selected: usize,
    client: Client,
    slots: Vec<(String, Entity<TextInput>)>,
    edit_name: Entity<TextInput>,
    edit_content: Entity<TextInput>,
    edit_client: Client,
    editing: bool,
    preview: bool,
    dirty: bool,
    confirm_lock: bool,
    confirm_remove: bool,
    replace_vault: bool,
    replace_export: bool,
    plaintext: bool,
    strict_dns: bool,
    error: Option<String>,
    notice: Option<Text>,
    busy: bool,
    picking: bool,
    generation: Arc<AtomicU64>,
    last_active: Instant,
    activation: Option<Subscription>,
    _clock: Task<()>,
}

impl Templates {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let clock = cx.spawn(async |view, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                if view.update(cx, Self::lock_if_idle).is_err() {
                    break;
                }
            }
        });
        Self {
            language: Language::System.resolved(),
            workspace: None,
            next_workspace: None,
            store: None,
            managed: None,
            source: Source::Bundled,
            mode: Mode::Unlock,
            preparation: Preparation {
                inputs: Vec::new(),
                outputs: None,
            },
            password: cx.new(|cx| TextInput::new("", true, cx)),
            confirmation: cx.new(|cx| TextInput::new("", true, cx)),
            vault: None,
            selected: 0,
            client: Client::Other,
            slots: Vec::new(),
            edit_name: cx.new(|cx| TextInput::new("", false, cx)),
            edit_content: cx.new(|cx| TextInput::multiline("", 12, cx)),
            edit_client: Client::Other,
            editing: false,
            preview: false,
            dirty: false,
            confirm_lock: false,
            confirm_remove: false,
            replace_vault: false,
            replace_export: false,
            plaintext: false,
            strict_dns: false,
            error: None,
            notice: None,
            busy: false,
            picking: false,
            generation: Arc::new(AtomicU64::new(0)),
            last_active: Instant::now(),
            activation: None,
            _clock: clock,
        }
    }

    pub(super) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        let language = language.resolved();
        if self.language != language {
            self.language = language;
            cx.notify();
        }
    }

    pub(super) fn set_workspace(&mut self, root: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.vault.is_some() || self.busy || self.picking {
            self.next_workspace = (self.workspace != root).then_some(root);
            return;
        }
        if self.workspace != root {
            self.clear_passwords(cx);
            self.change_workspace(root, cx);
        }
    }

    fn change_workspace(&mut self, root: Option<PathBuf>, cx: &mut Context<Self>) {
        self.workspace = root;
        self.store = self.workspace.as_deref().and_then(|path| {
            ManagedStore::new(path)
                .ok()
                .filter(|store| writable_template_destination(&store.library(false)))
        });
        self.refresh_managed();
        self.source = Source::Bundled;
        if self.preparation.outputs.is_none() {
            self.mode = Mode::Unlock;
        }
        cx.notify();
    }

    pub(super) fn can_close(&self) -> bool {
        !(self.editing || self.dirty || self.busy || self.picking)
    }

    pub(super) fn prepare(
        &mut self,
        inputs: Vec<PathBuf>,
        public: PathBuf,
        backup: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.set_mode(Mode::Prepare, cx);
        self.preparation = Preparation {
            inputs,
            outputs: Some((public, backup)),
        };
    }

    fn refresh_managed(&mut self) {
        self.managed = self
            .store
            .as_ref()
            .and_then(|store| store.restore().ok().flatten());
    }

    fn t(&self, key: Text) -> &'static str {
        text(self.language, key)
    }

    fn client_name(&self, client: Client) -> &'static str {
        match client {
            Client::Surge => "Surge",
            Client::Shadowrocket => "Shadowrocket",
            Client::FlClash => "FlClash",
            Client::SingBox => "sing-box",
            Client::Other => self.t(Text::VaultOther),
        }
    }

    fn display_error(&mut self, error: impl ToString, cx: &mut Context<Self>) {
        self.error = Some(error.to_string());
        self.notice = None;
        cx.notify();
    }

    fn clear_passwords(&self, cx: &mut App) {
        self.password.update(cx, TextInput::clear);
        self.confirmation.update(cx, TextInput::clear);
    }

    fn lock(&mut self, automatic: bool, cx: &mut Context<Self>) {
        let discard = self.dirty || self.editing;
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.vault = None;
        self.clear_passwords(cx);
        self.select(0, cx);
        self.dirty = false;
        self.confirm_lock = false;
        self.replace_vault = false;
        self.error = None;
        self.notice = automatic.then_some(if discard {
            Text::VaultAutoLockedUnsaved
        } else {
            Text::VaultAutoLocked
        });
        if let Some(workspace) = self.next_workspace.take() {
            self.change_workspace(workspace, cx);
        }
        cx.notify();
    }

    fn has_session_data(&self, cx: &App) -> bool {
        self.vault.is_some()
            || self.busy
            || !self.password.read(cx).value().is_empty()
            || !self.confirmation.read(cx).value().is_empty()
    }

    fn lock_if_idle(&mut self, cx: &mut Context<Self>) {
        if self.last_active.elapsed() >= IDLE_LIMIT && self.has_session_data(cx) {
            self.lock(true, cx);
        }
    }

    fn activity(&mut self, cx: &mut Context<Self>) {
        self.lock_if_idle(cx);
        self.last_active = Instant::now();
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected = index;
        self.cancel_edit(cx);
        for (_, field) in &self.slots {
            field.update(cx, TextInput::clear);
        }
        self.slots.clear();
        let mut keys = self
            .selected()
            .map_or_else(Vec::new, vault::template::subscription_slots);
        if keys.is_empty()
            && self
                .selected()
                .is_some_and(|template| template.client == Client::Shadowrocket)
        {
            keys.push(String::new());
        }
        self.slots = keys
            .into_iter()
            .map(|key| (key, cx.new(|cx| TextInput::new("", true, cx))))
            .collect();
        self.confirm_remove = false;
        self.plaintext = false;
        self.replace_export = false;
        self.strict_dns = false;
        cx.notify();
    }

    fn selected(&self) -> Option<&Template> {
        self.vault.as_ref()?.library().templates.get(self.selected)
    }

    fn start(
        &mut self,
        cx: &mut Context<Self>,
        operation: impl FnOnce(&AtomicU64, u64) -> Result<Completed, VaultError> + Send + 'static,
    ) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        self.notice = None;
        let generation = self.generation.clone();
        let expected = generation.load(Ordering::Acquire);
        let work = cx
            .background_executor()
            .spawn(async move { operation(&generation, expected) });
        cx.spawn(async move |view, cx| {
            let result = work.await;
            let _ = view.update(cx, |view, cx| {
                view.busy = false;
                if view.generation.load(Ordering::Acquire) == expected {
                    match result {
                        Ok(result) => view.completed(result, cx),
                        Err(error) => view.display_error(error, cx),
                    }
                }
                if view.vault.is_none()
                    && let Some(workspace) = view.next_workspace.take()
                {
                    view.change_workspace(workspace, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn completed(&mut self, result: Completed, cx: &mut Context<Self>) {
        match result {
            Completed::Opened(vault, saved_path) => {
                self.vault = Some(vault);
                self.mode = Mode::Unlock;
                self.dirty = false;
                if let Some(path) = saved_path {
                    self.source = Source::Disk(path.clone());
                    self.managed = Some(path);
                    self.notice = Some(Text::VaultSaved);
                }
                self.select(0, cx);
                self.activity(cx);
            }
            Completed::Imported(templates) => {
                if let Some(vault) = &mut self.vault {
                    let index = vault.library().templates.len();
                    vault.library_mut().templates.extend(templates);
                    if let Err(error) = vault.validate_for_storage() {
                        vault.library_mut().templates.truncate(index);
                        self.display_error(error, cx);
                        return;
                    }
                    self.dirty = true;
                    self.select(index, cx);
                    self.notice = Some(Text::VaultImported);
                }
            }
            Completed::Saved(path) => {
                self.source = Source::Disk(path.clone());
                self.managed = Some(path);
                self.dirty = false;
                self.replace_vault = false;
                self.notice = Some(Text::VaultSaved);
            }
            Completed::Exported => {
                self.plaintext = false;
                self.replace_export = false;
                self.notice = Some(Text::VaultExported);
            }
            Completed::Prepared(path) => {
                self.mode = Mode::Unlock;
                self.source = Source::Disk(path);
                self.preparation.inputs.clear();
                self.preparation.outputs = None;
                self.notice = Some(Text::VaultPrepared);
            }
        }
    }

    fn choose_source(&mut self, source: Source, cx: &mut Context<Self>) {
        self.source = source;
        self.set_mode(Mode::Unlock, cx);
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        self.error = None;
        self.notice = None;
        self.clear_passwords(cx);
        cx.notify();
    }

    fn open(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.picking || self.vault.is_some() {
            return;
        }
        let password = self.password.read(cx).value();
        if password.is_empty() {
            self.display_error(VaultError::Password, cx);
            return;
        }
        if self.mode != Mode::Unlock && password != self.confirmation.read(cx).value() {
            self.display_error(
                match self.language {
                    Language::ZhCn => "两次输入的密码不一致。",
                    Language::Ja => "入力したパスワードが一致しません。",
                    _ => "The passwords do not match.",
                },
                cx,
            );
            return;
        }
        let password = Zeroizing::new(password.to_owned());
        self.clear_passwords(cx);
        let source = self.source.clone();
        let mode = self.mode;
        let store = self.store.clone();
        let inputs = self.preparation.inputs.clone();
        let outputs = self.preparation.outputs.clone();
        let client = self.client;
        self.start(cx, move |generation, expected| match mode {
            Mode::Create => {
                let store = store.ok_or(VaultError::Io)?;
                let vault = vault::create(
                    Library {
                        templates: Vec::new(),
                    },
                    &password,
                )?;
                let bytes = vault.seal()?;
                require_current(generation, expected)?;
                let saved_path = store.publish(&bytes, false)?;
                Ok(Completed::Opened(vault, Some(saved_path)))
            }
            Mode::Unlock => {
                let vault = match &source {
                    Source::Bundled => vault::template::unlock_for_display(BUNDLED, &password)?,
                    Source::Disk(path) => {
                        let bytes = vault::read_bounded(path)?;
                        vault::template::unlock_for_display(&bytes, &password)?
                    }
                };
                Ok(Completed::Opened(vault, None))
            }
            Mode::Prepare => {
                let sources = inputs
                    .into_iter()
                    .map(|path| {
                        let detected = inferred_client(&path);
                        (
                            path,
                            if detected == Client::Other {
                                client
                            } else {
                                detected
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                if sources.is_empty()
                    || sources.len() > vault::MAX_TEMPLATES
                    || sources.iter().any(|(_, client)| *client == Client::Other)
                {
                    return Err(VaultError::Format);
                }
                require_current(generation, expected)?;
                let path = if let Some((public, backup)) = outputs {
                    if !writable_template_destination(&public)
                        || !writable_template_destination(&backup)
                    {
                        return Err(VaultError::Io);
                    }
                    vault::provision::provision(&sources, &public, &backup, &password)?;
                    public
                } else {
                    let store = store.ok_or(VaultError::Io)?;
                    vault::provision::provision_managed(&sources, store.root(), &password)?;
                    store.public()
                };
                Ok(Completed::Prepared(path))
            }
        });
    }

    fn choose_files(&mut self, multiple: bool, window: &Window, cx: &mut Context<Self>) {
        if self.picking || self.busy {
            return;
        }
        self.picking = true;
        let expected = self.generation.load(Ordering::Acquire);
        let response = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple,
            prompt: None,
        });
        cx.spawn_in(window, async move |view, cx| {
            let response = response.await;
            cx.background_executor().timer(DIALOG_FOCUS_SETTLE).await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.picking = false;
                if !window.is_window_active() && view.has_session_data(cx) {
                    view.lock(true, cx);
                } else if view.generation.load(Ordering::Acquire) == expected {
                    match response {
                        Ok(Ok(Some(paths))) => view.receive_paths(paths, cx),
                        Ok(Ok(None)) => {}
                        _ => view.display_error(VaultError::Io, cx),
                    }
                }
                if view.vault.is_none()
                    && let Some(workspace) = view.next_workspace.take()
                {
                    view.change_workspace(workspace, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn receive_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.is_empty() || self.busy || self.editing {
            return;
        }
        if self.vault.is_some() {
            self.import_paths(paths, cx);
        } else if self.mode == Mode::Prepare {
            if paths.len() > vault::MAX_TEMPLATES {
                self.display_error(VaultError::TooLarge, cx);
            } else {
                self.preparation.inputs = paths;
                self.error = None;
                cx.notify();
            }
        } else if let [path] = paths.as_slice() {
            self.choose_source(Source::Disk(path.clone()), cx);
        }
    }

    fn import_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(vault) = &self.vault else {
            return;
        };
        if paths.len().saturating_add(vault.library().templates.len()) > vault::MAX_TEMPLATES {
            self.display_error(VaultError::TooLarge, cx);
            return;
        }
        let client = self.client;
        self.start(cx, move |_, _| {
            paths
                .iter()
                .map(|path| {
                    let client = if client == Client::Other {
                        inferred_client(path)
                    } else {
                        client
                    };
                    vault::template::import_template(path, client)
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Completed::Imported)
        });
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let (Some(store), Some(vault)) = (self.store.clone(), self.vault.as_ref()) else {
            return;
        };
        let bytes = match vault.seal() {
            Ok(bytes) => bytes,
            Err(error) => {
                self.display_error(error, cx);
                return;
            }
        };
        let overwrite = self.replace_vault;
        self.start(cx, move |generation, expected| {
            require_current(generation, expected)?;
            store.publish(&bytes, overwrite).map(Completed::Saved)
        });
    }

    fn edit(&mut self, cx: &mut Context<Self>) {
        let Some(template) = self.selected() else {
            return;
        };
        let client = template.client;
        let name = template.name.clone();
        let content = template.content.clone();
        self.edit_name
            .update(cx, |field, cx| field.set_value(name, cx));
        self.edit_content
            .update(cx, |field, cx| field.set_value(content, cx));
        self.edit_client = client;
        self.editing = true;
        self.preview = false;
        self.confirm_remove = false;
        self.plaintext = false;
        self.error = None;
        self.notice = None;
        cx.notify();
    }

    fn apply_edit(&mut self, cx: &mut Context<Self>) {
        let (Some(vault), true) = (&mut self.vault, self.editing) else {
            return;
        };
        let Some(template) = vault.library_mut().templates.get_mut(self.selected) else {
            return;
        };
        let previous = std::mem::replace(
            template,
            Template {
                name: self.edit_name.read(cx).value().to_owned(),
                client: self.edit_client,
                content: self.edit_content.read(cx).value().to_owned(),
            },
        );
        if let Err(error) = vault.validate_for_storage() {
            vault.library_mut().templates[self.selected] = previous;
            self.display_error(error, cx);
            return;
        }
        self.dirty = true;
        self.select(self.selected, cx);
    }

    fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        self.editing = false;
        self.preview = false;
        self.edit_name.update(cx, TextInput::clear);
        self.edit_content.update(cx, TextInput::clear);
        cx.notify();
    }

    fn remove(&mut self, cx: &mut Context<Self>) {
        if self.confirm_remove
            && let Some(vault) = &mut self.vault
            && self.selected < vault.library().templates.len()
        {
            vault.library_mut().templates.remove(self.selected);
            self.dirty = true;
            self.select(self.selected.saturating_sub(1), cx);
        }
    }

    fn rendered(&self, cx: &App) -> Result<Zeroizing<String>, VaultError> {
        let template = self.selected().ok_or(VaultError::Format)?;
        let mut slots = self
            .slots
            .iter()
            .filter(|(key, _)| !key.is_empty())
            .map(|(key, field)| (key.clone(), field.read(cx).value().to_owned()))
            .collect::<BTreeMap<_, _>>();
        let result = if self.strict_dns {
            vault::template::render_flclash_windows_dns(template, &slots)
        } else {
            vault::template::render(template, &slots)
        };
        slots.values_mut().for_each(Zeroize::zeroize);
        result
    }

    fn copy(&mut self, subscription: bool, cx: &mut Context<Self>) {
        if !self.plaintext {
            return;
        }
        let output = if subscription {
            self.slots
                .iter()
                .find(|(key, _)| key.is_empty())
                .ok_or(VaultError::Format)
                .and_then(|(_, field)| {
                    let value = field.read(cx).value();
                    vault::template::validate_subscription_url(value)?;
                    Ok(Zeroizing::new(value.to_owned()))
                })
        } else {
            self.rendered(cx)
        };
        match output {
            Ok(output) => {
                cx.write_to_clipboard(ClipboardItem::new_string(output.to_string()));
                self.notice = Some(Text::Copied);
                self.error = None;
                self.plaintext = false;
                cx.notify();
            }
            Err(error) => self.display_error(error, cx),
        }
    }

    fn choose_export(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.plaintext || self.picking || self.busy {
            return;
        }
        let Some(template) = self.selected() else {
            return;
        };
        let client = template.client;
        let suffix = match client {
            Client::FlClash => "yaml",
            Client::SingBox => "json",
            _ => "conf",
        };
        let directory = ["USERPROFILE", "HOME"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(|home| PathBuf::from(home).join("Downloads"))
            .find(|path| path.is_dir())
            .unwrap_or_else(std::env::temp_dir);
        let name = format!("ScriptHub.{suffix}");
        let expected = self.generation.load(Ordering::Acquire);
        self.picking = true;
        let response = cx.prompt_for_new_path(&directory, Some(&name));
        cx.spawn_in(window, async move |view, cx| {
            let response = response.await;
            cx.background_executor().timer(DIALOG_FOCUS_SETTLE).await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.picking = false;
                if !window.is_window_active() && view.has_session_data(cx) {
                    view.lock(true, cx);
                } else if view.generation.load(Ordering::Acquire) == expected {
                    match response {
                        Ok(Ok(Some(path))) => view.export(&path, client, cx),
                        Ok(Ok(None)) => {}
                        _ => view.display_error(VaultError::Io, cx),
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn export(&mut self, path: &Path, client: Client, cx: &mut Context<Self>) {
        if !self.plaintext {
            return;
        }
        let Some(path) = configuration_export_path(path, client) else {
            self.display_error(VaultError::Format, cx);
            return;
        };
        let same_vault = match &self.source {
            Source::Bundled => false,
            Source::Disk(source) => source
                .canonicalize()
                .ok()
                .is_some_and(|source| path.canonicalize().ok().as_ref() == Some(&source)),
        };
        if same_vault || !safe_export_destination(&path, self.workspace.as_deref()) {
            self.display_error(self.t(Text::VaultExportHint), cx);
            return;
        }
        let output = match self.rendered(cx) {
            Ok(output) => output,
            Err(error) => {
                self.display_error(error, cx);
                return;
            }
        };
        let overwrite = self.replace_export;
        self.start(cx, move |generation, expected| {
            require_current(generation, expected)?;
            vault::write_private(&path, output.as_bytes(), overwrite)?;
            Ok(Completed::Exported)
        });
    }
}

fn require_current(generation: &AtomicU64, expected: u64) -> Result<(), VaultError> {
    if generation.load(Ordering::Acquire) == expected {
        Ok(())
    } else {
        Err(VaultError::Io)
    }
}

fn action(
    control: Stateful<Div>,
    enabled: bool,
    cx: &Context<Templates>,
    handler: impl Fn(&mut Templates, &mut Window, &mut Context<Templates>) + 'static,
) -> Stateful<Div> {
    if enabled {
        control.on_click(cx.listener(move |view, _, window, cx| handler(view, window, cx)))
    } else {
        control.opacity(0.42).cursor_default().tab_stop(false)
    }
}

fn actions() -> Div {
    div().flex().items_center().flex_wrap().gap_2()
}

fn card(p: Palette, label: &'static str) -> Div {
    panel(p)
        .p_4()
        .gap_3()
        .child(div().font_weight(FontWeight::SEMIBOLD).child(label))
}

fn detail(p: Palette, value: impl Into<SharedString>) -> Div {
    div()
        .text_sm()
        .line_height(px(20.))
        .text_color(p.muted)
        .child(value.into())
}

fn field(p: Palette, label: impl Into<SharedString>, field: &Entity<TextInput>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(detail(p, label))
        .child(field.clone())
}

fn toggle_label(checked: bool, label: &str) -> String {
    format!("{}  {label}", if checked { "☑" } else { "☐" })
}

impl Templates {
    fn clients(&self, editing: bool, p: Palette, cx: &Context<Self>) -> Div {
        let current = if editing {
            self.edit_client
        } else {
            self.client
        };
        actions()
            .child(detail(p, self.t(Text::Platform)).mr_1())
            .children(CLIENTS.into_iter().enumerate().map(|(index, client)| {
                let label = if !editing && client == Client::Other {
                    self.t(Text::VaultDetectClient)
                } else {
                    self.client_name(client)
                };
                button(("vault-client", index), label, p, client == current).on_click(cx.listener(
                    move |view, _, _, cx| {
                        if editing {
                            view.edit_client = client;
                        } else {
                            view.client = client;
                        }
                        cx.notify();
                    },
                ))
            }))
    }

    fn locked(&self, p: Palette, cx: &Context<Self>) -> Div {
        let enabled = !self.busy && !self.picking;
        let unlocking = self.mode == Mode::Unlock;
        let preparing = self.mode == Mode::Prepare;
        let creating = self.mode == Mode::Create;
        let method = match self.mode {
            Mode::Unlock => Text::VaultUnlock,
            Mode::Create => Text::VaultCreateNew,
            Mode::Prepare => Text::VaultPrepare,
        };
        let source_row = if preparing {
            let queued = if self.preparation.inputs.is_empty() {
                self.t(Text::VaultPrepareSourcesHelp).to_owned()
            } else {
                format!(
                    "{} · {}",
                    self.t(Text::VaultQueuedImports),
                    self.preparation.inputs.len()
                )
            };
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(detail(p, self.t(Text::VaultPrepareHelp)))
                .child(actions().child(action(
                    button(
                        "choose-prepare-inputs",
                        self.t(Text::VaultPrepareSources),
                        p,
                        false,
                    ),
                    enabled,
                    cx,
                    |view, window, cx| view.choose_files(true, window, cx),
                )))
                .child(detail(p, queued))
                .child(self.clients(false, p, cx))
        } else if unlocking {
            let source = match &self.source {
                Source::Bundled => self.t(Text::VaultLibraryBundled).to_owned(),
                Source::Disk(path) => path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            };
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(detail(p, source))
                .child(
                    actions()
                        .child(action(
                            button(
                                "vault-open-bundled",
                                self.t(Text::VaultLibraryBundled),
                                p,
                                false,
                            ),
                            enabled,
                            cx,
                            |view, _, cx| view.choose_source(Source::Bundled, cx),
                        ))
                        .children(self.managed.as_ref().map(|path| {
                            let path = path.clone();
                            action(
                                button(
                                    "vault-open-private",
                                    self.t(Text::VaultLibraryTemplates),
                                    p,
                                    false,
                                ),
                                enabled,
                                cx,
                                move |view, _, cx| {
                                    view.choose_source(Source::Disk(path.clone()), cx)
                                },
                            )
                        }))
                        .child(action(
                            button("vault-open-file", self.t(Text::VaultOpenExisting), p, false),
                            enabled,
                            cx,
                            |view, window, cx| view.choose_files(false, window, cx),
                        )),
                )
                .when(matches!(self.source, Source::Bundled), |node| {
                    node.child(detail(p, self.t(Text::VaultPublicGateHelp)))
                })
        } else {
            detail(p, self.t(Text::VaultPasswordHelp))
        };
        card(p, self.t(Text::VaultLocked))
            .child(
                actions()
                    .child(action(
                        button(
                            "vault-tab-open",
                            self.t(Text::VaultOpenExisting),
                            p,
                            unlocking,
                        ),
                        enabled,
                        cx,
                        |view, _, cx| view.set_mode(Mode::Unlock, cx),
                    ))
                    .child(action(
                        button(
                            "vault-tab-create",
                            self.t(Text::VaultCreateNew),
                            p,
                            creating,
                        ),
                        enabled && self.store.is_some() && self.managed.is_none(),
                        cx,
                        |view, _, cx| view.set_mode(Mode::Create, cx),
                    ))
                    .child(action(
                        button(
                            "vault-tab-prepare",
                            self.t(Text::VaultPrepare),
                            p,
                            preparing,
                        ),
                        enabled && (self.store.is_some() || self.preparation.outputs.is_some()),
                        cx,
                        |view, _, cx| view.set_mode(Mode::Prepare, cx),
                    )),
            )
            .child(source_row)
            .when(!enabled, |node| {
                node.child(detail(p, self.t(Text::VaultWorking)))
            })
            .when(enabled, |node| {
                node.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .max_w(px(420.))
                        .child(field(p, self.t(Text::VaultPassword), &self.password))
                        .when(!unlocking, |node| {
                            node.child(field(
                                p,
                                self.t(Text::VaultConfirmPassword),
                                &self.confirmation,
                            ))
                        }),
                )
                .child(actions().child(action(
                    button("vault-unlock", self.t(method), p, true),
                    unlocking
                        || (creating && self.store.is_some())
                        || (preparing && !self.preparation.inputs.is_empty()),
                    cx,
                    |view, _, cx| view.open(cx),
                )))
            })
            .when(
                self.store.is_none() && self.preparation.outputs.is_none(),
                |node| node.child(detail(p, self.t(Text::VaultStorageUnavailable))),
            )
    }

    fn header(&self, p: Palette, cx: &Context<Self>) -> Div {
        card(p, self.t(Text::VaultUnlocked))
            .when(self.dirty, |node| {
                node.child(detail(p, self.t(Text::UnsavedChanges)))
            })
            .child(
                actions()
                    .child(action(
                        button("vault-encrypted-save", self.t(Text::VaultSave), p, true),
                        self.store.is_some() && !self.editing && !self.busy && !self.picking,
                        cx,
                        |view, _, cx| view.save(cx),
                    ))
                    .child(
                        button("vault-lock", self.t(Text::VaultLock), p, false).on_click(
                            cx.listener(|view, _, _, cx| {
                                if view.dirty || view.editing {
                                    view.confirm_lock = true;
                                    cx.notify();
                                } else {
                                    view.lock(false, cx);
                                }
                            }),
                        ),
                    ),
            )
            .when(self.store.is_some(), |node| {
                node.child(actions().child(action(
                    button(
                        "vault-replace",
                        toggle_label(self.replace_vault, self.t(Text::VaultOverwrite)),
                        p,
                        false,
                    ),
                    !self.busy && !self.editing && !self.picking,
                    cx,
                    |view, _, cx| {
                        view.replace_vault = !view.replace_vault;
                        cx.notify();
                    },
                )))
            })
            .child(detail(
                p,
                self.t(if self.store.is_some() {
                    Text::VaultSessionHelp
                } else {
                    Text::VaultStorageUnavailable
                }),
            ))
    }

    fn library(&self, p: Palette, cx: &Context<Self>) -> Div {
        let Some(vault) = &self.vault else {
            return div();
        };
        let templates = &vault.library().templates;
        card(p, self.t(Text::VaultChooseTemplate))
            .when(templates.is_empty(), |node| {
                node.child(detail(p, self.t(Text::VaultEmpty)))
            })
            .children(templates.iter().enumerate().map(|(index, template)| {
                let selected = index == self.selected;
                let control = div()
                    .id(("choose-template", index))
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(if selected { p.accent } else { p.border })
                    .cursor_pointer()
                    .hover(move |style| style.border_color(p.accent))
                    .child(div().truncate().child(template.name.clone()))
                    .child(detail(p, self.client_name(template.client)).flex_shrink_0());
                action(control, !self.editing, cx, move |view, _, cx| {
                    view.select(index, cx)
                })
            }))
    }

    fn editor(&self, p: Palette, cx: &Context<Self>) -> Div {
        card(p, self.t(Text::VaultEditTemplate))
            .child(detail(p, self.t(Text::VaultEditHelp)))
            .child(field(p, self.t(Text::VaultEditName), &self.edit_name))
            .child(self.clients(true, p, cx))
            .child(field(p, self.t(Text::VaultEditContent), &self.edit_content))
            .child(
                actions()
                    .child(
                        button("vault-apply-edit", self.t(Text::VaultApplyEdit), p, true)
                            .on_click(cx.listener(|view, _, _, cx| view.apply_edit(cx))),
                    )
                    .child(
                        button("vault-cancel-edit", self.t(Text::Cancel), p, false)
                            .on_click(cx.listener(|view, _, _, cx| view.cancel_edit(cx))),
                    ),
            )
    }

    fn use_template(&self, p: Palette, cx: &Context<Self>) -> Div {
        let Some(template) = self.selected() else {
            return div();
        };
        let client = template.client;
        let mut config = card(p, self.t(Text::VaultConfigure)).child(
            actions()
                .child(
                    button("vault-edit", self.t(Text::VaultEditTemplate), p, false)
                        .on_click(cx.listener(|view, _, _, cx| view.edit(cx))),
                )
                .child(
                    button(
                        "vault-view-content",
                        toggle_label(self.preview, self.t(Text::VaultShowTemplate)),
                        p,
                        false,
                    )
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.preview = !view.preview;
                        cx.notify();
                    })),
                ),
        );
        if self.preview {
            let view = cx.entity().downgrade();
            let widest = template
                .content
                .lines()
                .enumerate()
                .max_by_key(|(_, line)| line.len())
                .map(|(index, _)| index);
            config = config
                .child(detail(p, self.t(Text::VaultTemplatePreviewHelp)))
                .child(
                    uniform_list(
                        "vault-preview",
                        template.content.lines().count(),
                        move |range, _, cx| {
                            view.upgrade()
                                .and_then(|view| view.read(cx).selected())
                                .map(|template| {
                                    template
                                        .content
                                        .lines()
                                        .skip(range.start)
                                        .take(range.len())
                                        .map(|line| {
                                            div()
                                                .text_sm()
                                                .line_height(px(19.))
                                                .font_family(if cfg!(windows) {
                                                    "Consolas"
                                                } else {
                                                    "Menlo"
                                                })
                                                .whitespace_nowrap()
                                                .px_2()
                                                .child(line.to_owned())
                                        })
                                        .collect()
                                })
                                .unwrap_or_default()
                        },
                    )
                    .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
                    .with_width_from_item(widest)
                    .h(px(235.))
                    .border_1()
                    .border_color(p.border)
                    .rounded_md(),
                );
        }
        config
            .children(match client {
                Client::FlClash => Some(detail(p, self.t(Text::VaultFlClashHelp))),
                Client::Shadowrocket => Some(detail(p, self.t(Text::VaultShadowrocketHelp))),
                Client::SingBox => Some(detail(p, self.t(Text::VaultSingBoxHelp))),
                _ => None,
            })
            .children(self.slots.iter().enumerate().map(|(index, (key, input))| {
                let label = if key.is_empty() {
                    self.t(Text::VaultSeparateSubscription).to_owned()
                } else {
                    format!("{} {}", self.t(Text::VaultSubscription), index + 1)
                };
                field(p, label, input)
            }))
            .when(client == Client::FlClash, |node| {
                node.child(
                    actions().child(
                        button(
                            "vault-strict-dns",
                            toggle_label(self.strict_dns, self.t(Text::VaultWindowsStrictDns)),
                            p,
                            false,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.strict_dns = !view.strict_dns;
                            cx.notify();
                        })),
                    ),
                )
                .when(self.strict_dns, |node| {
                    node.child(detail(p, self.t(Text::VaultWindowsStrictDnsHelp)))
                })
            })
            .child(detail(p, self.t(Text::VaultClipboardWarning)))
            .when(self.plaintext, |node| {
                node.child(
                    actions().child(
                        button(
                            "vault-overwrite-export",
                            toggle_label(self.replace_export, self.t(Text::VaultOverwrite)),
                            p,
                            false,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.replace_export = !view.replace_export;
                            cx.notify();
                        })),
                    ),
                )
            })
            .child(
                actions().child(
                    button(
                        "vault-accept-plaintext",
                        toggle_label(self.plaintext, self.t(Text::VaultPlaintextConsent)),
                        p,
                        false,
                    )
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.plaintext = !view.plaintext;
                        cx.notify();
                    })),
                ),
            )
            .child(
                actions()
                    .child(action(
                        button(
                            "vault-copy-config",
                            self.t(Text::VaultCopyConfiguration),
                            p,
                            false,
                        ),
                        self.plaintext,
                        cx,
                        |view, _, cx| view.copy(false, cx),
                    ))
                    .child(action(
                        button("vault-export-config", self.t(Text::VaultExport), p, true),
                        self.plaintext,
                        cx,
                        |view, window, cx| view.choose_export(window, cx),
                    ))
                    .when(self.slots.iter().any(|(key, _)| key.is_empty()), |node| {
                        node.child(action(
                            button(
                                "vault-copy-sub",
                                self.t(Text::VaultCopySubscription),
                                p,
                                false,
                            ),
                            self.plaintext,
                            cx,
                            |view, _, cx| view.copy(true, cx),
                        ))
                    }),
            )
            .when(!self.confirm_remove, |node| {
                node.child(actions().child(
                    button("vault-remove", self.t(Text::VaultRemove), p, false).on_click(
                        cx.listener(|view, _, _, cx| {
                            view.confirm_remove = true;
                            cx.notify();
                        }),
                    ),
                ))
            })
            .when(self.confirm_remove, |node| {
                node.child(detail(p, self.t(Text::VaultRemoveConfirm)))
                    .child(
                        actions()
                            .child(
                                button("vault-confirm-remove", self.t(Text::VaultRemove), p, false)
                                    .on_click(cx.listener(|view, _, _, cx| view.remove(cx))),
                            )
                            .child(
                                button("vault-cancel-remove", self.t(Text::Cancel), p, false)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.confirm_remove = false;
                                        cx.notify();
                                    })),
                            ),
                    )
            })
    }
}

impl Render for Templates {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.activation.is_none() {
            self.activation = Some(cx.observe_window_activation(window, |view, window, cx| {
                if !window.is_window_active() && !view.picking && view.has_session_data(cx) {
                    view.lock(true, cx);
                }
            }));
        }
        let p = Palette::for_window(window, cx);
        let mut body = div()
            .id("template-vault")
            .flex()
            .flex_col()
            .gap_4()
            .min_w_0()
            .capture_any_mouse_down(cx.listener(|view, _, _, cx| view.activity(cx)))
            .capture_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, _, cx| {
                view.activity(cx);
                if view.vault.is_none()
                    && event.keystroke.key == "enter"
                    && !event.is_held
                    && !view.password.read(cx).value().is_empty()
                {
                    view.open(cx);
                    cx.stop_propagation();
                }
            }))
            .on_drop(cx.listener(|view, paths: &ExternalPaths, _, cx| {
                view.receive_paths(paths.paths().to_vec(), cx);
            }))
            .child(heading(
                self.t(Text::Templates),
                self.t(Text::VaultIntro),
                p,
            ))
            .when_some(self.error.as_ref(), |node, error| {
                node.child(
                    panel(p)
                        .p_3()
                        .text_color(rgb(0xc85448))
                        .child(error.clone()),
                )
            })
            .when_some(self.notice, |node, notice| {
                node.child(panel(p).p_3().text_color(p.accent).child(self.t(notice)))
            });
        if self.picking {
            return body.child(panel(p).p_4().child(detail(p, self.t(Text::VaultWorking))));
        }
        if self.vault.is_none() {
            return body.child(self.locked(p, cx));
        }
        body = body.child(self.header(p, cx));
        if self.confirm_lock {
            return body.child(
                card(p, self.t(Text::UnsavedChanges))
                    .child(detail(
                        p,
                        self.t(if self.editing {
                            Text::VaultLockEditHelp
                        } else {
                            Text::VaultLockSaveHelp
                        }),
                    ))
                    .child(
                        actions()
                            .child(
                                button("vault-cancel-lock", self.t(Text::Cancel), p, true)
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.confirm_lock = false;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                button(
                                    "vault-discard-lock",
                                    self.t(Text::VaultDiscardAndLock),
                                    p,
                                    false,
                                )
                                .on_click(cx.listener(|view, _, _, cx| view.lock(false, cx))),
                            ),
                    ),
            );
        }
        if self.busy {
            return body.child(panel(p).p_4().child(detail(p, self.t(Text::VaultWorking))));
        }
        if self.editing {
            return body.child(self.editor(p, cx));
        }
        body.child(
            card(p, self.t(Text::VaultAddTemplate))
                .child(detail(p, self.t(Text::VaultImportHelp)))
                .child(self.clients(false, p, cx))
                .child(actions().child(
                    button("vault-import-files", self.t(Text::VaultImport), p, false).on_click(
                        cx.listener(|view, _, window, cx| view.choose_files(true, window, cx)),
                    ),
                )),
        )
        .child(self.library(p, cx))
        .child(self.use_template(p, cx))
    }
}
