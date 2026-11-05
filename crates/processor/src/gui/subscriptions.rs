//! Subscription drafts and encrypted storage are separate from upload credentials.
use super::relay::Failure;
use super::{
    i18n::Text,
    relay::{RelayState, RelayStep},
};
use crate::relay::{
    RelayConfig,
    backup::RelayBackup,
    library::{Collection, Entry, PublishSettings, Workspace},
    library_store::{Store, StoreError},
};
use eframe::egui;
use std::sync::{Arc, mpsc};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Selection {
    Subscription(u64),
    Collection(u64),
}
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ListKind {
    #[default]
    Subscriptions,
    Collections,
}
#[derive(Default)]
pub(super) struct ListState {
    pub kind: ListKind,
    pub query: String,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    NewSubscription,
    NewCollection,
    Open(Selection),
    Duplicate(Selection, &'static str),
    Import,
    SaveLibrary,
    Lock,
    Back,
    Close,
    Delete,
    Prepared,
    Migrated,
}

#[derive(Default)]
pub(super) struct State {
    pub data: Workspace,
    pub list: ListState,
    pub selected: Option<Selection>,
    pub show_editor: bool,
    pub name: Zeroizing<String>,
    pub members: Vec<u64>,
    pub members_changed: bool,
    pub collection_options: crate::relay::model::editor::Options,
    pub import_summary: Option<crate::relay::library_import::Summary>,
    pub draft_dirty: bool,
    pub dirty: bool,
    pub checked: bool,
    pub stored: bool,
    pub locked: bool,
    pub password_form: bool,
    pub password: Zeroizing<String>,
    pub confirmation: Zeroizing<String>,
    pub confirm: Option<Action>,
    pub error: Option<Text>,
    failure: Option<Failure>,
    pub notice: Option<Text>,
    store: Option<Arc<Store>>,
    pending: Option<Pending>,
    after_save: Option<Action>,
    pub cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Option<crate::relay::Progress>>>,
    prepared: Option<Prepared>,
}
struct Prepared {
    data: Workspace,
    changed: bool,
    content: Zeroizing<String>,
    processing: crate::relay::model::editor::Options,
}
enum Completed {
    Checked(bool),
    Opened(Arc<Store>, Workspace),
    Saved(Arc<Store>),
    Prepared(Box<Prepared>),
    Migrated(Workspace, crate::relay::library_import::Summary),
}
struct Pending {
    worker: JoinHandle<()>,
    receiver: mpsc::Receiver<Result<Completed, Failure>>,
}

fn storage_error(error: StoreError) -> Failure {
    use crate::vault::VaultError;
    Failure::Validation(match error {
        StoreError::Conflict => Text::WorkspaceConflict,
        StoreError::Vault(VaultError::Password) => Text::WorkspacePasswordInvalid,
        StoreError::Vault(VaultError::Authentication) => Text::WorkspaceUnlockFailed,
        _ => Text::WorkspaceStorageFailed,
    })
}

fn duplicate_name<'a>(
    name: &str,
    suffix: &str,
    existing: impl Iterator<Item = &'a str> + Clone,
) -> Zeroizing<String> {
    let mut number = 1;
    loop {
        let ending = if number == 1 {
            suffix.to_owned()
        } else {
            format!("{suffix} {number}")
        };
        let available = 128usize.saturating_sub(ending.chars().count());
        let mut candidate = Zeroizing::new(name.trim().chars().take(available).collect::<String>());
        candidate.push_str(&ending);
        if !existing.clone().any(|name| name == candidate.trim()) {
            return candidate;
        }
        number += 1;
    }
}

impl State {
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn unsaved(&self) -> bool {
        self.dirty || self.draft_dirty
    }
    pub fn progress(&self) -> Option<crate::relay::Progress> {
        self.progress.lock().ok().and_then(|p| *p)
    }
    pub fn take_failure(&mut self) -> Option<Failure> {
        self.failure.take()
    }
    pub fn cancel_after_save(&mut self) {
        self.after_save = None;
    }
    fn start(&mut self, work: impl FnOnce() -> Result<Completed, Failure> + Send + 'static) {
        if self.busy() {
            return;
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        self.error = None;
        self.notice = None;
        match thread::Builder::new()
            .name("script-hub-subscriptions".into())
            .spawn(move || {
                let _ = sender.send(work());
            }) {
            Ok(worker) => self.pending = Some(Pending { worker, receiver }),
            Err(_) => self.error = Some(Text::WorkspaceStorageFailed),
        }
    }
    pub fn check_storage(&mut self) {
        if self.checked || self.busy() {
            return;
        }
        self.checked = true;
        self.start(|| {
            Store::exists()
                .map(Completed::Checked)
                .map_err(storage_error)
        });
    }
    pub fn unlock(&mut self, ctx: &egui::Context) {
        if self.busy() {
            return;
        }
        let password = Zeroizing::new(std::mem::take(&mut *self.password));
        super::templates::clear_secret_widget(ctx, "workspace-password");
        self.start(move || {
            Store::open(&password)
                .map(|(store, data)| Completed::Opened(Arc::new(store), data))
                .map_err(storage_error)
        });
    }
    pub fn save_storage(&mut self, ctx: &egui::Context) {
        if self.busy() {
            return;
        }
        let data = self.data.clone();
        if let Some(store) = self.store.clone() {
            self.start(move || {
                store.save(&data).map_err(storage_error)?;
                Ok(Completed::Saved(store))
            });
        } else {
            self.password_form = true;
            if self.password.is_empty() {
                return;
            }
            if self.password.as_str() != self.confirmation.as_str() {
                self.error = Some(Text::WorkspacePasswordMismatch);
                return;
            }
            let password = Zeroizing::new(std::mem::take(&mut *self.password));
            self.confirmation.zeroize();
            for id in ["workspace-password", "workspace-password-confirm"] {
                super::templates::clear_secret_widget(ctx, id);
            }
            self.start(move || {
                Store::create(&data, &password)
                    .map(|store| Completed::Saved(Arc::new(store)))
                    .map_err(storage_error)
            });
        }
    }
    pub fn tick(&mut self, ctx: &egui::Context) -> Option<Action> {
        let pending = self.pending.as_ref()?;
        if !pending.worker.is_finished() {
            ctx.request_repaint_after(Duration::from_millis(100));
            return None;
        }
        let pending = self.pending.take()?;
        let result = pending.receiver.try_recv();
        match (pending.worker.join(), result) {
            (Ok(()), Ok(Ok(Completed::Checked(stored)))) => {
                self.stored = stored;
                self.locked = stored;
            }
            (Ok(()), Ok(Ok(Completed::Opened(store, data)))) => {
                self.data = data;
                self.store = Some(store);
                self.locked = false;
                self.stored = true;
                self.dirty = false;
                self.error = None;
                self.notice = Some(Text::WorkspaceUnlocked);
            }
            (Ok(()), Ok(Ok(Completed::Saved(store)))) => {
                self.store = Some(store);
                self.stored = true;
                self.dirty = false;
                self.password_form = false;
                self.notice = Some(Text::WorkspaceSaved);
                return self.after_save.take();
            }
            (Ok(()), Ok(Ok(Completed::Prepared(prepared)))) => {
                self.prepared = Some(*prepared);
                return Some(Action::Prepared);
            }
            (Ok(()), Ok(Ok(Completed::Migrated(data, summary)))) => {
                self.data = data;
                self.import_summary = Some(summary);
                if summary.subscriptions + summary.collections > 0 {
                    return Some(Action::Migrated);
                }
            }
            (Ok(()), Ok(Err(Failure::Validation(error)))) => {
                self.error = Some(error);
                self.after_save = None;
            }
            (Ok(()), Ok(Err(error))) => {
                self.failure = Some(error);
                self.after_save = None;
            }
            _ => {
                self.error = Some(Text::WorkspaceStorageFailed);
                self.after_save = None;
            }
        }
        None
    }
}

impl RelayState {
    fn publish_settings(&self) -> PublishSettings {
        PublishSettings {
            output: self.output.trim().to_owned(),
            output_file: self.output_file.to_string(),
            format: self.publish_format,
            title: self.title.to_string(),
            interval_minutes: self.schedule.interval_minutes,
        }
    }

    fn restore_publish_settings(&mut self, settings: &PublishSettings) {
        self.output = Zeroizing::new(settings.output.clone());
        self.output_file = Zeroizing::new(settings.output_file.clone());
        self.publish_format = settings.format;
        self.title = Zeroizing::new(settings.title.clone());
        self.schedule.interval_minutes = settings.interval_minutes;
    }

    pub(super) fn workspace_request(&mut self, ctx: &egui::Context, action: Action) {
        if self.busy() || self.schedule.armed() {
            return;
        }
        if action == Action::Delete
            || self.workspace.draft_dirty
            || matches!(action, Action::Lock | Action::Close) && self.workspace.dirty
        {
            self.workspace.error = None;
            self.workspace.confirm = Some(action);
        } else {
            self.execute_workspace(ctx, action);
        }
    }
    pub(super) fn save_workspace_draft(&mut self) -> bool {
        if !self.workspace.draft_dirty {
            return true;
        }
        let Some(selection) = self.workspace.selected else {
            return true;
        };
        if matches!(selection, Selection::Subscription(_)) && !self.paste.trim().is_empty() {
            self.workspace.error = Some(Text::RelayLoadPastedBeforeSave);
            return false;
        }
        let config = self.private_config(self.engine_path());
        let result = match selection {
            Selection::Subscription(id) => {
                let content = self
                    .editor
                    .as_ref()
                    .map(|e| e.source.as_str())
                    .or(self.local_content.as_ref().map(|c| c.as_str()))
                    .or_else(|| {
                        self.workspace
                            .data
                            .subscriptions
                            .iter()
                            .find(|entry| {
                                entry.id == id
                                    && entry.source == config.0.source
                                    && entry.source_file == config.0.source_file
                            })
                            .map(|entry| entry.content.as_str())
                    })
                    .unwrap_or("");
                if !content.is_empty() && crate::relay::model::Subscription::parse(content).is_err()
                {
                    self.workspace.error = Some(Text::WorkspaceDraftInvalid);
                    return false;
                }
                let entry = Entry {
                    id,
                    name: self.workspace.name.trim().to_owned(),
                    source: config.0.source.clone(),
                    source_file: config.0.source_file.clone(),
                    content: content.to_owned(),
                    processing: config.0.processing.clone(),
                    publish: self.publish_settings(),
                };
                if id == 0 {
                    self.workspace
                        .data
                        .add_entry(entry)
                        .map(Selection::Subscription)
                } else {
                    self.workspace.data.update_entry(entry).map(|()| selection)
                }
            }
            Selection::Collection(id) => {
                let collection = self.collection_draft(id);
                if id == 0 {
                    self.workspace
                        .data
                        .add_collection(collection)
                        .map(Selection::Collection)
                } else {
                    self.workspace
                        .data
                        .update_collection(collection)
                        .map(|()| selection)
                }
            }
        };
        match result {
            Ok(selected) => {
                self.workspace.selected = Some(selected);
                self.workspace.draft_dirty = false;
                self.workspace.dirty = true;
                self.workspace.error = None;
                self.workspace.notice = Some(if self.demo_mode {
                    Text::RelaySandboxDraftSaved
                } else {
                    Text::WorkspaceDraftSaved
                });
                true
            }
            Err(error) => {
                self.workspace.error = Some(match error {
                    crate::relay::library::Error::TooLarge => Text::RelayTooLarge,
                    crate::relay::library::Error::Referenced => Text::WorkspaceBoundEdits,
                    _ => Text::WorkspaceDraftInvalid,
                });
                false
            }
        }
    }

    fn collection_draft(&self, id: u64) -> Collection {
        let mut processing = self
            .editor
            .as_ref()
            .map(|editor| editor.processing())
            .unwrap_or_else(|| self.workspace.collection_options.clone());
        if self.workspace.members_changed {
            processing.nodes.clear();
        }
        Collection {
            id,
            name: self.workspace.name.trim().to_owned(),
            members: self.workspace.members.clone(),
            processing,
            publish: self.publish_settings(),
        }
    }

    pub(super) fn collection_schedule_snapshot(
        &self,
        id: u64,
    ) -> Result<(Workspace, u64), crate::relay::library::Error> {
        let mut snapshot = self.workspace.data.clone();
        let id = if !self.workspace.draft_dirty {
            id
        } else if id == 0 {
            snapshot.add_collection(self.collection_draft(id))?
        } else {
            snapshot.update_collection(self.collection_draft(id))?;
            id
        };
        Ok((snapshot, id))
    }
    pub(super) fn workspace_save_and_continue(&mut self, ctx: &egui::Context) {
        let Some(action) = self.workspace.confirm else {
            return;
        };
        if !self.save_workspace_draft() {
            return;
        }
        self.workspace.confirm = None;
        if matches!(action, Action::Lock | Action::Close) {
            self.workspace.after_save = Some(action);
            self.workspace.save_storage(ctx);
        } else {
            self.execute_workspace(ctx, action);
        }
    }
    pub(super) fn execute_workspace(&mut self, ctx: &egui::Context, action: Action) {
        self.workspace.confirm = None;
        if self.demo_mode && matches!(action, Action::Import | Action::SaveLibrary | Action::Lock) {
            return;
        }
        if self.workspace.notice == Some(Text::WorkspaceDuplicateReady)
            && matches!(
                action,
                Action::NewSubscription
                    | Action::NewCollection
                    | Action::Open(_)
                    | Action::Lock
                    | Action::Close
                    | Action::Delete
                    | Action::Migrated
            )
        {
            self.workspace.notice = None;
        }
        let token = (!matches!(action, Action::Lock | Action::Close | Action::Duplicate(..)))
            .then(|| self.token.clone());
        match action {
            Action::Import => self.start_workspace_import(None, None),
            Action::NewSubscription | Action::NewCollection => {
                self.clear(ctx);
                let collection = action == Action::NewCollection;
                self.workspace.selected = Some(if collection {
                    Selection::Collection(0)
                } else {
                    Selection::Subscription(0)
                });
                self.workspace.name = Zeroizing::new(format!(
                    "{} {}",
                    if collection {
                        "Collection"
                    } else {
                        "Subscription"
                    },
                    self.workspace.data.next_id
                ));
                self.workspace.members.clear();
                self.workspace.show_editor = true;
                self.workspace.draft_dirty = true;
                self.workspace.members_changed = false;
                self.workspace.collection_options = Default::default();
            }
            Action::Open(selection) | Action::Duplicate(selection, _) => {
                let duplicate = match action {
                    Action::Duplicate(_, suffix) => Some(suffix),
                    _ => None,
                };
                match selection {
                    Selection::Subscription(id) => {
                        let Some(entry) = self
                            .workspace
                            .data
                            .subscriptions
                            .iter()
                            .find(|e| e.id == id)
                            .cloned()
                        else {
                            return;
                        };
                        self.clear(ctx);
                        self.workspace.name = if let Some(suffix) = duplicate {
                            duplicate_name(
                                &entry.name,
                                suffix,
                                self.workspace
                                    .data
                                    .subscriptions
                                    .iter()
                                    .map(|item| item.name.as_str()),
                            )
                        } else {
                            Zeroizing::new(entry.name.clone())
                        };
                        if entry.content.is_empty() {
                            self.rename = false;
                            self.sort = false;
                            self.source = Zeroizing::new(entry.source.clone());
                            self.source_file = Zeroizing::new(entry.source_file.clone());
                            self.imported_processing = Some(entry.processing.clone());
                        } else {
                            self.apply_backup(
                                ctx,
                                Box::new(RelayBackup {
                                    config: RelayConfig {
                                        source: entry.source.clone(),
                                        source_file: entry.source_file.clone(),
                                        processing: entry.processing.clone(),
                                        rename: false,
                                        sort: false,
                                        ..Default::default()
                                    },
                                    content: Zeroizing::new(entry.content.clone()),
                                }),
                            );
                        }
                        if duplicate.is_none() {
                            self.restore_publish_settings(&entry.publish);
                        }
                    }
                    Selection::Collection(id) => {
                        let Some(collection) = self
                            .workspace
                            .data
                            .collections
                            .iter()
                            .find(|c| c.id == id)
                            .cloned()
                        else {
                            return;
                        };
                        self.clear(ctx);
                        self.workspace.name = if let Some(suffix) = duplicate {
                            duplicate_name(
                                &collection.name,
                                suffix,
                                self.workspace
                                    .data
                                    .collections
                                    .iter()
                                    .map(|item| item.name.as_str()),
                            )
                        } else {
                            Zeroizing::new(collection.name.clone())
                        };
                        self.workspace.members = collection.members.clone();
                        self.workspace.collection_options = collection.processing.clone();
                        if duplicate.is_none() {
                            self.restore_publish_settings(&collection.publish);
                        }
                    }
                }
                self.workspace.selected = Some(if duplicate.is_some() {
                    self.workspace.notice = Some(Text::WorkspaceDuplicateReady);
                    match selection {
                        Selection::Subscription(_) => Selection::Subscription(0),
                        Selection::Collection(_) => Selection::Collection(0),
                    }
                } else {
                    selection
                });
                self.workspace.show_editor = true;
                self.workspace.draft_dirty = duplicate.is_some();
                self.workspace.members_changed = false;
            }
            Action::Back => {
                self.workspace.show_editor = false;
            }
            Action::SaveLibrary => {
                if self.save_workspace_draft() && (self.workspace.dirty || !self.workspace.stored) {
                    self.workspace.save_storage(ctx);
                }
            }
            Action::Lock | Action::Close => {
                self.clear(ctx);
                self.workspace.data = Workspace::default();
                self.workspace.store = None;
                self.workspace.selected = None;
                self.workspace.name.zeroize();
                self.workspace.members.clear();
                self.workspace.list.query.zeroize();
                self.workspace.collection_options = Default::default();
                self.workspace.import_summary = None;
                self.workspace.password.zeroize();
                self.workspace.confirmation.zeroize();
                self.workspace.locked = self.workspace.stored;
                self.workspace.show_editor = false;
                self.workspace.draft_dirty = false;
                self.workspace.dirty = false;
                self.workspace.password_form = false;
                self.workspace.after_save = None;
                for id in [
                    "workspace-item-name",
                    "workspace-password",
                    "workspace-password-confirm",
                ] {
                    super::templates::clear_secret_widget(ctx, id);
                }
                if action == Action::Close {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Action::Delete => {
                let result = match self.workspace.selected {
                    Some(Selection::Subscription(id)) => self.workspace.data.remove_entry(id),
                    Some(Selection::Collection(id)) => self.workspace.data.remove_collection(id),
                    None => return,
                };
                match result {
                    Ok(()) => {
                        self.clear(ctx);
                        self.workspace.selected = None;
                        self.workspace.show_editor = false;
                        self.workspace.draft_dirty = false;
                        self.workspace.dirty = true;
                    }
                    Err(_) => self.workspace.error = Some(Text::WorkspaceReferenced),
                }
            }
            Action::Prepared => {
                if let Some(prepared) = self.workspace.prepared.take() {
                    let publish = self.publish_settings();
                    self.workspace.data = prepared.data;
                    self.workspace.dirty |= prepared.changed;
                    self.apply_backup(
                        ctx,
                        Box::new(RelayBackup {
                            config: RelayConfig {
                                processing: prepared.processing,
                                rename: false,
                                sort: false,
                                output: publish.output.clone(),
                                output_file: publish.output_file.clone(),
                                output_format: publish.format,
                                title: publish.title.clone(),
                                ..Default::default()
                            },
                            content: prepared.content,
                        }),
                    );
                    self.schedule.interval_minutes = publish.interval_minutes;
                    self.step = RelayStep::Review;
                }
            }
            Action::Migrated => {
                self.clear(ctx);
                self.workspace.selected = None;
                self.workspace.show_editor = false;
                self.workspace.draft_dirty = false;
                self.workspace.dirty |= self
                    .workspace
                    .import_summary
                    .is_some_and(|summary| summary.subscriptions + summary.collections > 0);
            }
        }
        if let Some(token) = token {
            self.token = token;
        }
        if self.notice == Some(Text::RelayCleared)
            && matches!(
                action,
                Action::NewSubscription
                    | Action::NewCollection
                    | Action::Open(_)
                    | Action::Duplicate(..)
            )
        {
            self.notice = None;
        }
    }
    pub(super) fn prepare_collection(&mut self, refresh: bool) {
        if refresh && self.demo_mode {
            return;
        }
        if refresh && self.cooldown_seconds() > 0 || self.busy() {
            return;
        }
        if !self.save_workspace_draft() {
            return;
        }
        let Some(Selection::Collection(id)) = self.workspace.selected else {
            return;
        };
        let data = self.workspace.data.clone();
        let token = self.token.clone();
        self.workspace.cancel = Arc::new(AtomicBool::new(false));
        self.workspace.progress = Arc::new(Mutex::new(None));
        let cancel = self.workspace.cancel.clone();
        let progress = self.workspace.progress.clone();
        self.workspace.start(move || {
            let (data, changed) = if refresh {
                let updated =
                    crate::relay::library_refresh::refresh(&data, id, &token, &cancel, &|p| {
                        if let Ok(mut slot) = progress.lock() {
                            *slot = Some(p);
                        }
                    })
                    .map_err(Failure::Operation)?;
                let changed = updated
                    .subscriptions
                    .iter()
                    .zip(&data.subscriptions)
                    .any(|(new, old)| new.content != old.content);
                (updated, changed)
            } else {
                (data, false)
            };
            if cancel.load(Ordering::Acquire) {
                return Err(Failure::Validation(Text::RelayCancelled));
            }
            let content = data
                .merge(id)
                .map_err(|_| Failure::Validation(Text::WorkspaceMergeFailed))?;
            if cancel.load(Ordering::Acquire) {
                return Err(Failure::Validation(Text::RelayCancelled));
            }
            let processing = data
                .collections
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.processing.clone())
                .unwrap_or_default();
            Ok(Completed::Prepared(Box::new(Prepared {
                data,
                changed,
                content,
                processing,
            })))
        });
    }

    pub(super) fn import_workspace_report(&mut self) {
        let Some(report) = self.migration.clone() else {
            return;
        };
        let data = self.workspace.data.clone();
        let acknowledged = self.migration_ack;
        self.workspace.start(move || {
            crate::relay::library_import::import(&data, &report, acknowledged)
                .map(|(data, summary)| Completed::Migrated(data, summary))
                .map_err(|_| Failure::Validation(Text::WorkspaceDraftInvalid))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_a_saved_subscription_does_not_add_implicit_name_edits() {
        let ctx = egui::Context::default();
        let mut relay = RelayState::new(true);
        relay.workspace.checked = true;
        relay.workspace.data = crate::relay::demo::workspace().unwrap();
        let id = relay.workspace.data.subscriptions[0].id;
        relay.execute_workspace(&ctx, Action::Open(Selection::Subscription(id)));
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while relay.busy() && std::time::Instant::now() < deadline {
            relay.tick(&ctx);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!relay.busy() && relay.error.is_none());
        assert!(relay.editor.is_some());
        assert!(!relay.rename && !relay.sort);
    }

    #[test]
    fn building_a_saved_collection_keeps_implicit_name_edits_off() {
        let ctx = egui::Context::default();
        let mut relay = RelayState::new(true);
        relay.workspace.checked = true;
        relay.workspace.data = crate::relay::demo::workspace().unwrap();
        let id = relay.workspace.data.collections[0].id;
        relay.execute_workspace(&ctx, Action::Open(Selection::Collection(id)));
        relay.prepare_collection(false);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while relay.busy() && std::time::Instant::now() < deadline {
            relay.tick(&ctx);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(!relay.busy() && relay.error.is_none());
        assert!(relay.editor.is_some());
        assert!(!relay.rename && !relay.sort);
    }

    #[test]
    fn cancelling_import_picker_leaves_the_original_dirty_draft_active() {
        let ctx = egui::Context::default();
        let mut relay = RelayState::default();
        relay.execute_workspace(&ctx, Action::NewSubscription);
        relay.workspace.name = Zeroizing::new("Original draft".into());
        relay.paste.push_str("unloaded text");
        relay.workspace_request(&ctx, Action::Import);
        assert!(relay.workspace.confirm == Some(Action::Import));
        relay.execute_workspace(&ctx, Action::Import);
        assert!(relay.selecting_file());
        relay.dismiss_file_picker();
        assert!(relay.workspace.selected == Some(Selection::Subscription(0)));
        assert!(relay.workspace.draft_dirty && relay.workspace.show_editor);
        assert_eq!(relay.workspace.name.as_str(), "Original draft");
        assert_eq!(relay.paste.as_str(), "unloaded text");
        assert!(!relay.selecting_file());
    }

    #[test]
    fn unimported_paste_blocks_saving_even_when_the_link_tab_is_open() {
        let ctx = egui::Context::default();
        let mut relay = RelayState::default();
        relay.execute_workspace(&ctx, Action::NewSubscription);
        relay.source.push_str("https://example.com/nodes");
        relay
            .paste
            .push_str("Example = socks5, node.example, 1080\n");
        assert!(!relay.save_workspace_draft());
        assert!(relay.workspace.data.subscriptions.is_empty());
        assert_eq!(relay.workspace.error, Some(Text::RelayLoadPastedBeforeSave));
        relay.clear_pasted_text(&ctx);
        assert!(relay.save_workspace_draft());
        assert_eq!(relay.workspace.data.subscriptions.len(), 1);
    }

    #[test]
    fn switching_preserves_invalid_draft_until_it_can_be_saved() {
        let ctx = egui::Context::default();
        let mut relay = RelayState::default();
        relay.execute_workspace(&ctx, Action::NewSubscription);
        relay.workspace.name.clear();
        relay.source.push_str("https://example.com/nodes");
        relay.workspace_request(&ctx, Action::NewCollection);
        assert!(matches!(
            relay.workspace.confirm,
            Some(Action::NewCollection)
        ));
        relay.workspace_save_and_continue(&ctx);
        assert!(relay.workspace.data.subscriptions.is_empty());
        assert!(relay.workspace.draft_dirty);
        assert!(matches!(
            relay.workspace.selected,
            Some(Selection::Subscription(0))
        ));
        relay.workspace.name.push_str("Example source");
        relay.workspace_save_and_continue(&ctx);
        assert_eq!(relay.workspace.data.subscriptions.len(), 1);
        assert!(matches!(
            relay.workspace.selected,
            Some(Selection::Collection(0))
        ));
    }

    #[test]
    fn cancelling_password_form_disarms_queued_close() {
        let ctx = egui::Context::default();
        let mut relay = RelayState::default();
        relay.execute_workspace(&ctx, Action::NewSubscription);
        relay.source.push_str("https://example.com/nodes");
        relay.workspace_request(&ctx, Action::Close);
        relay.workspace_save_and_continue(&ctx);
        assert!(relay.workspace.password_form && relay.workspace.dirty);
        assert!(matches!(relay.workspace.after_save, Some(Action::Close)));
        relay.workspace.cancel_after_save();
        assert!(relay.workspace.after_save.is_none());
        assert_eq!(relay.workspace.data.subscriptions.len(), 1);
    }

    #[test]
    fn saved_output_settings_follow_the_item_without_restoring_consent() {
        let ctx = egui::Context::default();
        let mut relay = RelayState::default();
        relay.execute_workspace(&ctx, Action::NewSubscription);
        relay.source.push_str("https://example.com/nodes");
        relay.output.push_str("11111111111111111111111111111111");
        relay.output_file = Zeroizing::new("my-nodes.list".into());
        relay.title = Zeroizing::new("My subscription".into());
        relay.schedule.interval_minutes = 180;
        relay.acknowledge_publish = true;
        relay.allow_public_output = true;
        assert!(relay.save_workspace_draft());
        let subscription = relay.workspace.selected.unwrap();

        relay.execute_workspace(&ctx, Action::NewCollection);
        assert!(relay.output.is_empty());
        assert_eq!(relay.schedule.interval_minutes, 60);
        relay.output.push_str("22222222222222222222222222222222");
        assert!(relay.save_workspace_draft());
        let collection = relay.workspace.selected.unwrap();

        relay.execute_workspace(&ctx, Action::Open(subscription));
        assert_eq!(relay.output.as_str(), "11111111111111111111111111111111");
        assert_eq!(relay.output_file.as_str(), "my-nodes.list");
        assert_eq!(relay.title.as_str(), "My subscription");
        assert_eq!(relay.schedule.interval_minutes, 180);
        assert!(!relay.acknowledge_publish && !relay.allow_public_output);
        assert!(!relay.schedule.armed());

        relay.execute_workspace(&ctx, Action::Open(collection));
        assert_eq!(relay.output.as_str(), "22222222222222222222222222222222");
        assert_eq!(relay.schedule.interval_minutes, 60);
        assert!(!relay.workspace.draft_dirty);
    }

    #[test]
    fn duplicate_subscription_preserves_cached_nodes_and_processing_without_publication_state() {
        use crate::relay::model::{
            editor::NodeEdit,
            pipeline::{Operation, Step},
        };

        let ctx = egui::Context::default();
        let mut relay = RelayState::default();
        let node = "A = socks5, node.example, 1080\n";
        let mut processing = crate::relay::model::editor::Options::default();
        processing.nodes.push(NodeEdit {
            index: 0,
            included: true,
            name: Some("Edited".into()),
        });
        processing.pipeline.push(Step {
            id: 1,
            enabled: true,
            operation: Operation::Deduplicate,
        });
        let mut publish = PublishSettings::default();
        publish.output.push_str("22222222222222222222222222222222");
        publish.interval_minutes = 180;
        let id = relay
            .workspace
            .data
            .add_entry(Entry {
                id: 0,
                name: "Original".into(),
                source: "11111111111111111111111111111111".into(),
                source_file: "input.list".into(),
                content: node.into(),
                processing,
                publish,
            })
            .unwrap();
        relay.workspace.selected = Some(Selection::Subscription(id));
        relay.token.push_str("token-never-copied");
        relay.acknowledge_publish = true;
        relay.allow_public_output = true;
        relay.allow_plaintext = true;

        relay.workspace_request(
            &ctx,
            Action::Duplicate(Selection::Subscription(id), " (copy)"),
        );
        assert!(relay.workspace.selected == Some(Selection::Subscription(0)));
        assert!(relay.workspace.draft_dirty && !relay.workspace.dirty);
        assert_eq!(relay.workspace.data.subscriptions.len(), 1);
        assert_eq!(relay.workspace.name.as_str(), "Original (copy)");
        assert!(relay.publish_settings() == PublishSettings::default());
        assert!(relay.token.is_empty());
        assert!(!relay.acknowledge_publish && !relay.allow_public_output && !relay.allow_plaintext);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while relay.busy() && std::time::Instant::now() < deadline {
            relay.tick(&ctx);
            thread::sleep(Duration::from_millis(2));
        }
        assert!(!relay.busy());
        assert!(relay.save_workspace_draft());
        let [original, copy] = relay.workspace.data.subscriptions.as_slice() else {
            panic!("expected original and copy")
        };
        assert_ne!(original.id, copy.id);
        assert_eq!(original.source, copy.source);
        assert_eq!(original.source_file, copy.source_file);
        assert_eq!(copy.content, node);
        assert_eq!(copy.processing.nodes[0].name.as_deref(), Some("Edited"));
        assert!(matches!(
            copy.processing.pipeline[0].operation,
            Operation::Deduplicate
        ));
        assert!(copy.publish == PublishSettings::default());
    }

    #[test]
    fn duplicate_collection_waits_for_draft_validation_and_keeps_member_order() {
        let ctx = egui::Context::default();
        let mut relay = RelayState::default();
        let members: Vec<_> = ["One", "Two"]
            .into_iter()
            .map(|name| {
                relay
                    .workspace
                    .data
                    .add_entry(Entry {
                        id: 0,
                        name: name.into(),
                        source: String::new(),
                        source_file: String::new(),
                        content: "A = socks5, node.example, 1080\n".into(),
                        processing: Default::default(),
                        publish: Default::default(),
                    })
                    .unwrap()
            })
            .collect();
        let mut publish = PublishSettings::default();
        publish.output.push_str("22222222222222222222222222222222");
        let id = relay
            .workspace
            .data
            .add_collection(Collection {
                id: 0,
                name: "Collection".into(),
                members,
                processing: Default::default(),
                publish,
            })
            .unwrap();
        let original = Selection::Collection(id);
        relay.execute_workspace(&ctx, Action::Open(original));
        relay.workspace.members.reverse();
        relay.workspace.members_changed = true;
        relay.workspace.name.clear();
        relay.workspace.draft_dirty = true;
        let action = Action::Duplicate(original, " (copy)");
        relay.workspace_request(&ctx, action);
        relay.workspace_save_and_continue(&ctx);
        assert!(relay.workspace.confirm == Some(action));
        assert!(relay.workspace.selected == Some(original));
        relay.workspace.name.push_str("Updated");
        relay.workspace_save_and_continue(&ctx);
        assert!(relay.workspace.selected == Some(Selection::Collection(0)));
        assert_eq!(relay.workspace.name.as_str(), "Updated (copy)");
        assert_eq!(relay.workspace.data.collections.len(), 1);
        assert_eq!(
            relay.workspace.members,
            relay.workspace.data.collections[0].members
        );
        assert!(relay.publish_settings() == PublishSettings::default());
    }

    #[test]
    fn duplicate_name_stays_editable_and_resolves_collisions() {
        let name = "🧭".repeat(128);
        let copy = duplicate_name(&name, "（副本）", std::iter::empty());
        let next = duplicate_name(&name, "（副本）", [copy.as_str()].into_iter());
        assert_eq!(copy.chars().count(), 128);
        assert_eq!(next.chars().count(), 128);
        assert!(next.ends_with("（副本） 2"));
    }

    #[test]
    fn offline_collection_build_keeps_a_saved_library_clean() {
        let ctx = egui::Context::default();
        let mut relay = RelayState::default();
        let id = relay
            .workspace
            .data
            .add_entry(Entry {
                id: 0,
                name: "Local".into(),
                source: String::new(),
                source_file: String::new(),
                content: "A = socks5, node.example, 1080\n".into(),
                processing: Default::default(),
                publish: Default::default(),
            })
            .unwrap();
        let collection = relay
            .workspace
            .data
            .add_collection(Collection {
                id: 0,
                name: "Combined".into(),
                members: vec![id],
                processing: Default::default(),
                publish: Default::default(),
            })
            .unwrap();
        relay.execute_workspace(&ctx, Action::Open(Selection::Collection(collection)));
        relay.prepare_collection(false);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while relay.busy() && std::time::Instant::now() < deadline {
            relay.tick(&ctx);
            thread::sleep(Duration::from_millis(2));
        }
        assert!(!relay.busy());
        assert_eq!(relay.editor.as_ref().unwrap().rows.len(), 1);
        assert!(!relay.workspace.unsaved());
    }
}
