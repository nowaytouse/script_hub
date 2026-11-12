//! Transient template-vault sessions, kept separate from preferences and task history.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;
use zeroize::{Zeroize, Zeroizing};

use super::i18n::Text;
use crate::vault::managed::{ManagedStore, valid_library};
use crate::vault::{self, Client, Library, Template, UnlockedVault};

const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const APP_ASSET_ERROR: &str =
    "Application resources are read-only. Save encrypted copies in the managed project storage.";
const STORAGE_ERROR: &str =
    "Managed project storage is unavailable. Open the project once to enable encrypted saving.";
pub(super) const PASSWORD_ID: &str = "template-vault-password";
pub(super) const CONFIRM_ID: &str = "template-vault-confirm";
pub(super) const EDIT_NAME_ID: &str = "template-vault-edit-name";
pub(super) const EDIT_CONTENT_ID: &str = "template-vault-edit-content";

pub(super) struct TemplateState {
    pub path: String,
    pub creating: bool,
    pub prepare_mode: bool,
    pub prepare_sources: String,
    pub prepare_public: String,
    pub prepare_backup: String,
    pub password: Zeroizing<String>,
    pub confirmation: Zeroizing<String>,
    pub import_path: String,
    pub import_client: Client,
    pub selected: usize,
    pub subscriptions: Vec<(String, Zeroizing<String>)>,
    pub preview: Zeroizing<String>,
    pub show_preview: bool,
    pub editing: bool,
    pub edit_name: Zeroizing<String>,
    pub edit_client: Client,
    pub export_path: String,
    pub allow_plaintext: bool,
    pub windows_dns_guard: bool,
    pub overwrite_export: bool,
    pub overwrite_vault: bool,
    pub remove_pending: bool,
    pub lock_confirmation: Option<egui::Id>,
    pub dirty: bool,
    pub error: Option<String>,
    pub notice: Option<Text>,
    pub(super) vault: Option<UnlockedVault>,
    pending: Option<Pending>,
    generation: Arc<AtomicU64>,
    last_activity: Instant,
    sidecar_checked: bool,
    project_checked: Option<PathBuf>,
    legacy_checked: Option<PathBuf>,
    store: Option<ManagedStore>,
    template_path: Option<PathBuf>,
    originals_path: Option<PathBuf>,
    file_stamps: BTreeMap<PathBuf, (u64, SystemTime)>,
    external_vault: bool,
    pub use_originals: bool,
    explicit_prepare: bool,
    imports: Vec<PathBuf>,
}

struct Pending {
    receiver: mpsc::Receiver<Result<Completed, String>>,
    worker: JoinHandle<()>,
    generation: u64,
}

enum Completed {
    Opened(UnlockedVault, bool, PathBuf),
    Imported(Vec<Template>),
    Saved(PathBuf),
    Exported,
    Provisioned(PathBuf),
}

impl Default for TemplateState {
    fn default() -> Self {
        Self {
            path: String::new(),
            creating: false,
            prepare_mode: false,
            prepare_sources: String::new(),
            prepare_public: String::new(),
            prepare_backup: String::new(),
            password: Zeroizing::new(String::new()),
            confirmation: Zeroizing::new(String::new()),
            import_path: String::new(),
            import_client: Client::Other,
            selected: 0,
            subscriptions: Vec::new(),
            preview: Zeroizing::new(String::new()),
            show_preview: false,
            editing: false,
            edit_name: Zeroizing::new(String::new()),
            edit_client: Client::Other,
            export_path: String::new(),
            allow_plaintext: false,
            windows_dns_guard: false,
            overwrite_export: false,
            overwrite_vault: false,
            remove_pending: false,
            lock_confirmation: None,
            dirty: false,
            error: None,
            notice: None,
            vault: None,
            pending: None,
            generation: Arc::new(AtomicU64::new(0)),
            last_activity: Instant::now(),
            sidecar_checked: false,
            project_checked: None,
            legacy_checked: None,
            store: None,
            template_path: None,
            originals_path: None,
            file_stamps: BTreeMap::new(),
            external_vault: false,
            use_originals: false,
            explicit_prepare: false,
            imports: Vec::new(),
        }
    }
}

impl TemplateState {
    pub(super) fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn unlocked(&self) -> bool {
        self.vault.is_some()
    }

    pub(super) fn preparing(&self) -> bool {
        self.prepare_mode
    }

    pub(super) fn queued_imports(&self) -> usize {
        if self.prepare_mode {
            self.prepare_sources
                .lines()
                .filter(|line| !line.is_empty())
                .count()
        } else {
            self.imports.len()
        }
    }

    pub(super) fn visit(&mut self) {
        if self.sidecar_checked {
            return;
        }
        self.sidecar_checked = true;
        if self.path.is_empty()
            && let Ok(executable) = std::env::current_exe()
            && let Some(path) = bundled_vault(&executable)
            && matches!(valid_library(&path), Ok(true))
        {
            self.template_path = Some(path.clone());
            self.path = path.to_string_lossy().into_owned();
        }
    }

    pub(super) fn visit_project(&mut self, project: &Path) {
        #[cfg(not(test))]
        let legacy = match crate::runtime::settings::preferences_path() {
            Ok(path) => path
                .parent()
                .map(|parent| parent.join("vaults/originals.vault")),
            Err(_) => {
                self.error = Some(
                    "The previous encrypted vault location could not be determined safely."
                        .to_owned(),
                );
                None
            }
        };
        // Unit tests must never inspect the user's real application-data directory.
        #[cfg(test)]
        let legacy: Option<PathBuf> = None;
        self.visit_project_with_legacy(project, legacy.as_deref());
    }

    fn visit_project_with_legacy(&mut self, project: &Path, legacy: Option<&Path>) {
        // An unlocked or still-running session keeps its original save scope until it is locked.
        if self.unlocked() || self.busy() {
            self.prune_pinned_records();
            return;
        }
        if self.project_checked.as_deref() == Some(project) && self.store.is_some() {
            let selected = Path::new(&self.path);
            let regular = |path: &Path| {
                std::fs::symlink_metadata(path)
                    .is_ok_and(|metadata| metadata.is_file() && !vault::is_link(&metadata))
            };
            let originals_stale = self
                .originals_path
                .as_deref()
                .is_some_and(|path| !regular(path));
            if (self.external_vault || self.path.is_empty() || regular(selected))
                && !originals_stale
                && !self.cached_files_changed()
            {
                return;
            }
        }
        self.project_checked = Some(project.to_owned());
        self.store = None;
        self.originals_path = None;
        self.template_path = None;
        self.file_stamps.clear();
        if !self.external_vault {
            self.path.clear();
            self.sidecar_checked = false;
        }
        let store = match ManagedStore::new(project) {
            Ok(store) if writable_template_destination(&store.library(false)) => store,
            Ok(_) => {
                self.error = Some(APP_ASSET_ERROR.to_owned());
                return;
            }
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let check_legacy = self.legacy_checked.as_deref() != Some(project);
        self.legacy_checked = Some(project.to_owned());
        let restored = store.restore().and_then(|locations| {
            if check_legacy
                && let Some(source) = legacy
                && store.migrate_legacy_originals(source)?
            {
                return store.restore();
            }
            Ok(locations)
        });
        let locations = match restored {
            Ok(locations) => locations,
            Err(error) => {
                self.store = None;
                self.error = Some(error.to_string());
                self.visit();
                return;
            }
        };
        if !self.explicit_prepare {
            self.prepare_public = store.public().to_string_lossy().into_owned();
            self.prepare_backup = store.library(true).to_string_lossy().into_owned();
        }
        self.originals_path = locations.originals;
        self.template_path = locations.library;
        if self.template_path.is_none() {
            match valid_library(&store.public()) {
                Ok(true) => self.template_path = Some(store.public()),
                Ok(false) => {}
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        self.store = Some(store);
        if !self.external_vault {
            self.use_originals = false;
            self.path = self
                .template_path
                .as_ref()
                .map_or_else(String::new, |path| path.to_string_lossy().into_owned());
            self.sidecar_checked = false;
        }
        self.visit();
        self.refresh_file_stamps();
    }

    pub(super) fn visit_without_project(&mut self) {
        if self.unlocked() || self.busy() {
            self.prune_pinned_records();
            return;
        }
        if self.project_checked.is_some() || self.store.is_some() {
            self.project_checked = None;
            self.legacy_checked = None;
            self.store = None;
            self.template_path = None;
            self.originals_path = None;
            self.file_stamps.clear();
            if !self.external_vault {
                self.path.clear();
                self.use_originals = false;
                self.sidecar_checked = false;
            }
            if !self.explicit_prepare {
                self.prepare_public.clear();
                self.prepare_backup.clear();
            }
        }
        self.visit();
    }

    fn prune_pinned_records(&mut self) {
        if let Some(store) = &self.store {
            let stale = |cached: &Option<PathBuf>, originals| {
                cached.as_deref().is_some_and(|path| {
                    path == store.library(originals)
                        && !std::fs::symlink_metadata(path)
                            .is_ok_and(|metadata| metadata.is_file() && !vault::is_link(&metadata))
                })
            };
            let template_stale = stale(&self.template_path, false);
            let originals_stale = stale(&self.originals_path, true);
            if template_stale || originals_stale {
                match store.prune_missing() {
                    Ok(locations) => {
                        if template_stale {
                            self.template_path = locations.library;
                            self.file_stamps.remove(&store.library(false));
                        }
                        if originals_stale {
                            self.originals_path = locations.originals;
                            self.file_stamps.remove(&store.library(true));
                        }
                    }
                    Err(error) => self.error = Some(error.to_string()),
                }
            }
        }
    }

    fn cached_files_changed(&self) -> bool {
        // shortcut: length/mtime detects ordinary edits; add file events if metadata-preserving changes must be noticed before unlock.
        self.template_path
            .iter()
            .chain(self.originals_path.iter())
            .any(|path| {
                file_stamp(path).is_none_or(|stamp| self.file_stamps.get(path) != Some(&stamp))
            })
    }

    fn refresh_file_stamps(&mut self) {
        self.file_stamps = self
            .template_path
            .iter()
            .chain(self.originals_path.iter())
            .filter_map(|path| file_stamp(path).map(|stamp| (path.clone(), stamp)))
            .collect();
    }

    pub(super) fn storage_available(&self) -> bool {
        self.store.is_some()
    }

    pub(super) fn originals_available(&self) -> bool {
        self.originals_path.is_some()
    }

    pub(super) fn save_destination(&self) -> Option<PathBuf> {
        self.store
            .as_ref()
            .map(|store| store.library(self.use_originals))
    }

    pub(super) fn select_library(&mut self, originals: bool, ctx: &egui::Context) {
        if self.busy() || self.unlocked() {
            return;
        }
        self.clear_passwords(ctx);
        self.use_originals = originals;
        self.external_vault = false;
        self.path = (if originals {
            &self.originals_path
        } else {
            &self.template_path
        })
        .as_ref()
        .map_or_else(String::new, |path| path.to_string_lossy().into_owned());
        self.error = None;
        self.notice = None;
    }

    /// Selects initial inputs without reading or displaying their plaintext.
    pub(super) fn prepare(&mut self, paths: Vec<PathBuf>, public: PathBuf, backup: PathBuf) {
        self.prepare_sources = paths
            .iter()
            .map(|path| path.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");
        self.prepare_public = public.to_string_lossy().into_owned();
        self.prepare_backup = backup.to_string_lossy().into_owned();
        self.explicit_prepare = true;
        self.prepare_mode = true;
        self.creating = false;
        self.notice = None;
        self.error = None;
    }

    pub(super) fn queue_dropped(&mut self, paths: Vec<PathBuf>) {
        if self.busy() || paths.is_empty() {
            return;
        }
        if self.queued_imports().saturating_add(paths.len()) > vault::MAX_TEMPLATES {
            self.error = Some("Select at most 64 configuration files at a time.".to_owned());
            return;
        }
        if self.prepare_mode {
            for path in paths {
                if !self.prepare_sources.is_empty() && !self.prepare_sources.ends_with('\n') {
                    self.prepare_sources.push('\n');
                }
                self.prepare_sources.push_str(&path.to_string_lossy());
            }
            return;
        }
        if !self.unlocked()
            && paths.len() == 1
            && paths[0]
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("vault"))
        {
            self.path = paths[0].to_string_lossy().into_owned();
            self.creating = false;
            self.prepare_mode = false;
            self.external_vault = true;
            self.use_originals = false;
        } else {
            self.imports.extend(paths);
        }
    }

    pub(super) fn tick(&mut self, ctx: &egui::Context) {
        let focused = ctx.input(|input| input.viewport().focused.unwrap_or(true));
        let sensitive = self.unlocked()
            || !self.password.is_empty()
            || !self.confirmation.is_empty()
            || self.pending.as_ref().is_some_and(|pending| {
                pending.generation == self.generation.load(Ordering::Acquire)
            });
        let expired = self.last_activity.elapsed() >= IDLE_TIMEOUT;
        if sensitive && (!focused || expired) {
            let unsaved = self.dirty || self.editing;
            self.lock(ctx);
            self.notice = Some(if unsaved {
                Text::VaultAutoLockedUnsaved
            } else {
                Text::VaultAutoLocked
            });
        } else if focused && ctx.input(|input| !input.events.is_empty() || input.pointer.any_down())
        {
            self.last_activity = Instant::now();
        }
        if sensitive && focused && !expired {
            ctx.request_repaint_after(IDLE_TIMEOUT.saturating_sub(self.last_activity.elapsed()));
        }
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        let result = match pending.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                "The vault operation stopped unexpectedly. Try again.".to_owned(),
            )),
            Err(mpsc::TryRecvError::Empty) => None,
        };
        let Some(mut result) = result else {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        };
        let Some(pending) = self.pending.take() else {
            return;
        };
        if pending.worker.join().is_err() {
            result = Err("The vault operation stopped unexpectedly. Try again.".to_owned());
        }
        if pending.generation != self.generation.load(Ordering::Acquire) {
            return;
        }
        match result {
            Ok(Completed::Opened(vault, dirty, path)) => {
                if let Some(store) = &self.store {
                    if path == store.library(false) {
                        self.template_path = Some(path.clone());
                    }
                    if path == store.library(true) {
                        self.originals_path = Some(path.clone());
                    }
                }
                self.path = path.to_string_lossy().into_owned();
                self.external_vault = false;
                self.vault = Some(vault);
                self.dirty = dirty;
                self.selected = 0;
                self.refresh_selection(ctx);
                self.notice = Some(Text::VaultUnlocked);
                self.refresh_file_stamps();
            }
            Ok(Completed::Imported(templates)) => match self.append_templates(templates) {
                Ok(()) => {
                    self.refresh_selection(ctx);
                    self.notice = Some(Text::VaultImported);
                }
                Err(error) => self.error = Some(error.to_string()),
            },
            Ok(Completed::Saved(path)) => {
                self.path = path.to_string_lossy().into_owned();
                if self.use_originals {
                    self.originals_path = Some(path);
                } else {
                    self.template_path = Some(path);
                }
                self.dirty = false;
                self.notice = Some(Text::VaultSaved);
                self.refresh_file_stamps();
            }
            Ok(Completed::Exported) => self.notice = Some(Text::VaultExported),
            Ok(Completed::Provisioned(path)) => {
                self.template_path = Some(path.clone());
                self.path = path.to_string_lossy().into_owned();
                if let Some(store) = &self.store {
                    self.originals_path = Some(store.library(true));
                }
                self.prepare_mode = false;
                self.prepare_sources.zeroize();
                self.creating = false;
                self.notice = Some(Text::VaultPrepared);
                self.refresh_file_stamps();
            }
            Err(error) => self.error = Some(error),
        }
        self.last_activity = Instant::now();
    }

    pub(super) fn request_lock(&mut self, ctx: &egui::Context, opener: egui::Id) {
        if self.dirty || self.editing {
            self.lock_confirmation = Some(opener);
        } else {
            self.lock(ctx);
        }
    }

    pub(super) fn lock(&mut self, ctx: &egui::Context) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.lock_confirmation = None;
        self.vault = None;
        self.password.zeroize();
        self.confirmation.zeroize();
        self.clear_selection(ctx);
        self.import_path.zeroize();
        self.imports.clear();
        self.dirty = false;
        self.overwrite_vault = false;
        self.error = None;
        self.notice = Some(Text::VaultLocked);
        clear_secret_widget(ctx, PASSWORD_ID);
        clear_secret_widget(ctx, CONFIRM_ID);
    }

    pub(super) fn refresh_selection(&mut self, ctx: &egui::Context) {
        self.clear_selection(ctx);
        let Some(template) = self.selected_template() else {
            return;
        };
        let mut slots = vault::template::subscription_slots(template);
        if slots.is_empty() && template.client == Client::Shadowrocket {
            slots.push(String::new());
        }
        self.subscriptions = slots
            .into_iter()
            .map(|slot| (slot, Zeroizing::new(String::new())))
            .collect();
    }

    fn clear_selection(&mut self, ctx: &egui::Context) {
        for index in 0..self.subscriptions.len() {
            clear_secret_widget(ctx, &format!("template-vault-url-{index}"));
        }
        self.subscriptions.clear();
        self.cancel_edit(ctx);
        self.export_path.zeroize();
        self.allow_plaintext = false;
        self.windows_dns_guard = false;
        self.overwrite_export = false;
        self.remove_pending = false;
    }

    pub(super) fn selected_template(&self) -> Option<&Template> {
        self.vault.as_ref()?.library().templates.get(self.selected)
    }

    fn append_templates(&mut self, templates: Vec<Template>) -> Result<(), vault::VaultError> {
        let vault = self
            .vault
            .as_mut()
            .ok_or(vault::VaultError::Authentication)?;
        let previous_len = vault.library().templates.len();
        vault.library_mut().templates.extend(templates);
        if let Err(error) = vault.validate_for_storage() {
            vault.library_mut().templates.truncate(previous_len);
            return Err(error);
        }
        self.dirty = true;
        Ok(())
    }

    pub(super) fn open(&mut self, ctx: &egui::Context) {
        if self.busy() || self.password.is_empty() {
            return;
        }
        if (self.creating || self.preparing())
            && self.password.as_str() != self.confirmation.as_str()
        {
            self.error = Some("The passwords do not match.".to_owned());
            return;
        }
        let path = PathBuf::from(self.path.trim());
        if self.prepare_mode {
            if !self.explicit_prepare && !self.storage_available() {
                self.error = Some(STORAGE_ERROR.to_owned());
                return;
            }
            let sources = self
                .prepare_sources
                .lines()
                .filter(|line| !line.is_empty())
                .map(PathBuf::from)
                .collect::<Vec<_>>();
            if sources.is_empty()
                || sources.len() > vault::MAX_TEMPLATES
                || self.prepare_public.trim().is_empty()
                || self.prepare_backup.trim().is_empty()
            {
                self.error = Some(
                    "Select source files and their client; managed encrypted storage must be available."
                        .to_owned(),
                );
                return;
            }
            let public = PathBuf::from(self.prepare_public.trim());
            let backup = PathBuf::from(self.prepare_backup.trim());
            if !writable_template_destination(&public) || !writable_template_destination(&backup) {
                self.error = Some(APP_ASSET_ERROR.to_owned());
                return;
            }
            if public == backup
                || sources
                    .iter()
                    .any(|path| path == &public || path == &backup)
            {
                self.error =
                    Some("Source files and encrypted destinations must be different.".to_owned());
                return;
            }
            let password = self.take_password(ctx);
            let managed = (!self.explicit_prepare)
                .then(|| self.store.clone())
                .flatten();
            let fallback_client = self.import_client;
            self.start(move |_, _| {
                // YAML inspection stays on the worker alongside bounded local imports.
                let sources = sources
                    .into_iter()
                    .map(|path| {
                        let detected = inferred_client(&path);
                        let client = if detected == Client::Other {
                            fallback_client
                        } else {
                            detected
                        };
                        (path, client)
                    })
                    .collect::<Vec<_>>();
                if sources.iter().any(|(_, client)| *client == Client::Other) {
                    return Err("Select a client for each unrecognized configuration.".to_owned());
                }
                if let Some(store) = managed {
                    vault::provision::provision_managed(&sources, store.root(), &password)
                        .map_err(|error| error.to_string())?;
                } else {
                    vault::provision::provision(&sources, &public, &backup, &password)
                        .map_err(|error| error.to_string())?;
                }
                Ok(Completed::Provisioned(public))
            });
        } else if self.creating {
            let Some(path) = self.save_destination() else {
                self.error = Some(STORAGE_ERROR.to_owned());
                return;
            };
            let password = self.take_password(ctx);
            let imports = std::mem::take(&mut self.imports);
            let client = self.import_client;
            self.start(move |_, _| {
                let templates = imports
                    .iter()
                    .map(|path| vault::template::import_template(path, client))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| error.to_string())?;
                vault::create(Library { templates }, &password)
                    .map(|vault| Completed::Opened(vault, true, path))
                    .map_err(|error| error.to_string())
            });
        } else {
            if path.as_os_str().is_empty() {
                self.error = Some(
                    "No encrypted library is available. Drop an encrypted library here to open it."
                        .to_owned(),
                );
                return;
            }
            let password = self.take_password(ctx);
            let import = self.external_vault.then(|| self.store.clone()).flatten();
            self.start(move |generation, expected| {
                let bytes = vault::read_bounded(&path).map_err(|error| error.to_string())?;
                let vault = vault::template::unlock_for_display(&bytes, &password)
                    .map_err(|error| error.to_string())?;
                ensure_current(&generation, expected)?;
                let path = if let Some(store) = import {
                    store
                        .import_authenticated(&path, &bytes)
                        .map_err(|error| error.to_string())?
                } else {
                    path
                };
                Ok(Completed::Opened(vault, false, path))
            });
        }
    }

    pub(super) fn clear_passwords(&mut self, ctx: &egui::Context) {
        self.password.zeroize();
        self.confirmation.zeroize();
        clear_secret_widget(ctx, PASSWORD_ID);
        clear_secret_widget(ctx, CONFIRM_ID);
    }

    fn take_password(&mut self, ctx: &egui::Context) -> Zeroizing<String> {
        let password = std::mem::replace(&mut self.password, Zeroizing::new(String::new()));
        self.clear_passwords(ctx);
        password
    }

    pub(super) fn import(&mut self) {
        if self.busy() || self.editing || !self.unlocked() {
            return;
        }
        let selected_count = self.imports.len() + usize::from(!self.import_path.trim().is_empty());
        let existing_count = self
            .vault
            .as_ref()
            .map_or(0, |vault| vault.library().templates.len());
        if existing_count.saturating_add(selected_count) > vault::MAX_TEMPLATES {
            self.error = Some("A library supports at most 64 configuration files.".to_owned());
            return;
        }
        let mut paths = std::mem::take(&mut self.imports);
        if !self.import_path.trim().is_empty() {
            paths.push(PathBuf::from(self.import_path.trim()));
            self.import_path.zeroize();
        }
        if paths.is_empty() {
            return;
        }
        let client = self.import_client;
        self.start(move |_, _| {
            paths
                .iter()
                .map(|path| vault::template::import_template(path, client))
                .collect::<Result<Vec<_>, _>>()
                .map(Completed::Imported)
                .map_err(|error| error.to_string())
        });
    }

    pub(super) fn save(&mut self) {
        if self.busy() || self.editing {
            return;
        }
        let Some(store) = self.store.clone() else {
            self.error = Some(STORAGE_ERROR.to_owned());
            return;
        };
        let originals = self.use_originals;
        let path = store.library(originals);
        if !writable_template_destination(&path) {
            self.error = Some(APP_ASSET_ERROR.to_owned());
            return;
        }
        let Some(vault) = self.vault.as_ref() else {
            return;
        };
        let bytes = match vault.seal() {
            Ok(bytes) => bytes,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let overwrite = self.overwrite_vault;
        self.start(move |generation, expected| {
            ensure_current(&generation, expected)?;
            let path = store
                .publish(&bytes, originals, overwrite)
                .map_err(|error| error.to_string())?;
            Ok(Completed::Saved(path))
        });
    }

    pub(super) fn remove_selected(&mut self, ctx: &egui::Context) {
        if self.busy() || !self.remove_pending {
            return;
        }
        if let Some(vault) = self.vault.as_mut()
            && self.selected < vault.library().templates.len()
        {
            vault.library_mut().templates.remove(self.selected);
            self.selected = self.selected.saturating_sub(1);
            self.dirty = true;
            self.refresh_selection(ctx);
        }
    }

    pub(super) fn show_template(&mut self) {
        if self.editing {
            return;
        }
        self.preview.zeroize();
        if self.show_preview
            && let Some(template) = self.selected_template()
        {
            self.preview = Zeroizing::new(template.content.clone());
        }
    }

    pub(super) fn edit_selected(&mut self, ctx: &egui::Context) {
        if self.busy() {
            return;
        }
        let Some(template) = self.selected_template() else {
            return;
        };
        let name = Zeroizing::new(template.name.clone());
        let client = template.client;
        let content = Zeroizing::new(template.content.clone());
        self.cancel_edit(ctx);
        self.edit_name = name;
        self.edit_client = client;
        self.preview = content;
        self.editing = true;
        self.remove_pending = false;
        self.error = None;
        self.notice = None;
    }

    pub(super) fn apply_edit(&mut self, ctx: &egui::Context) {
        if self.busy() || !self.editing {
            return;
        }
        let Some(vault) = self.vault.as_mut() else {
            return;
        };
        let Some(selected) = vault.library_mut().templates.get_mut(self.selected) else {
            self.error = Some(vault::VaultError::Format.to_string());
            return;
        };
        let previous = std::mem::replace(
            selected,
            Template {
                name: self.edit_name.to_string(),
                client: self.edit_client,
                content: self.preview.to_string(),
            },
        );
        if let Err(error) = vault.validate_for_storage() {
            vault.library_mut().templates[self.selected] = previous;
            self.error = Some(error.to_string());
            return;
        }
        self.dirty = true;
        self.refresh_selection(ctx);
        self.error = None;
        self.notice = None;
    }

    pub(super) fn cancel_edit(&mut self, ctx: &egui::Context) {
        self.editing = false;
        self.edit_name.zeroize();
        self.edit_client = Client::Other;
        self.preview.zeroize();
        self.show_preview = false;
        clear_secret_widget(ctx, EDIT_NAME_ID);
        clear_secret_widget(ctx, EDIT_CONTENT_ID);
    }

    pub(super) fn rendered(&self) -> Result<Zeroizing<String>, String> {
        let template = self.selected_template().ok_or("Unlock a template first.")?;
        let mut bindings = self
            .subscriptions
            .iter()
            .filter(|(slot, _)| !slot.is_empty())
            .map(|(slot, value)| (slot.clone(), value.to_string()))
            .collect::<BTreeMap<_, _>>();
        let result = if self.windows_dns_guard {
            vault::template::render_flclash_windows_dns(template, &bindings).map_err(|error| {
                if matches!(error, vault::VaultError::Format) {
                    "Windows strict DNS requires valid subscription slots, HTTPS-only DNS servers and an HTTPS bootstrap resolver with an IP host.".to_owned()
                } else {
                    error.to_string()
                }
            })
        } else {
            vault::template::render(template, &bindings).map_err(|error| error.to_string())
        };
        for value in bindings.values_mut() {
            value.zeroize();
        }
        result
    }

    pub(super) fn copy(&mut self, ctx: &egui::Context) {
        if !self.allow_plaintext || self.busy() {
            return;
        }
        match self.rendered() {
            Ok(output) => {
                ctx.copy_text(output.to_string());
                self.notice = Some(Text::Copied);
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn copy_subscription(&mut self, ctx: &egui::Context) {
        if !self.allow_plaintext || self.busy() {
            return;
        }
        let Some((_, value)) = self.subscriptions.iter().find(|(slot, _)| slot.is_empty()) else {
            return;
        };
        match vault::template::validate_subscription_url(value) {
            Ok(()) => {
                ctx.copy_text(value.to_string());
                self.notice = Some(Text::Copied);
                self.error = None;
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub(super) fn export(&mut self, workspace: Option<&Path>) {
        if !self.allow_plaintext || self.busy() || self.export_path.trim().is_empty() {
            return;
        }
        let client = self
            .selected_template()
            .map_or(Client::Other, |template| template.client);
        let Some(path) = configuration_export_path(Path::new(self.export_path.trim()), client)
        else {
            self.error = Some("Choose a file path for the FlClash export.".to_owned());
            return;
        };
        let existing_vault = Path::new(self.path.trim()).canonicalize().ok();
        let workspace = workspace.or_else(|| self.store.as_ref().map(ManagedStore::root));
        if !safe_export_destination(&path, workspace)
            || existing_vault.is_some_and(|vault| path.canonicalize().ok().as_ref() == Some(&vault))
        {
            self.error = Some(
                "Choose an absolute export path outside the project, application resources, and vault storage.".to_owned(),
            );
            return;
        }
        let output = match self.rendered() {
            Ok(output) => output,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let overwrite = self.overwrite_export;
        self.start(move |generation, expected| {
            ensure_current(&generation, expected)?;
            vault::write_private(&path, output.as_bytes(), overwrite)
                .map_err(|error| error.to_string())?;
            Ok(Completed::Exported)
        });
    }

    fn start<F>(&mut self, work: F)
    where
        F: FnOnce(Arc<AtomicU64>, u64) -> Result<Completed, String> + Send + 'static,
    {
        if self.busy() {
            return;
        }
        self.error = None;
        self.notice = None;
        let generation = Arc::clone(&self.generation);
        let expected = generation.load(Ordering::Acquire);
        let (sender, receiver) = mpsc::sync_channel(1);
        match thread::Builder::new()
            .name("script-hub-vault".to_owned())
            .spawn(move || {
                let result = work(Arc::clone(&generation), expected);
                if generation.load(Ordering::Acquire) == expected {
                    let _ = sender.send(result);
                }
            }) {
            Ok(worker) => {
                self.pending = Some(Pending {
                    receiver,
                    worker,
                    generation: expected,
                })
            }
            Err(_) => self.error = Some("Cannot start the vault operation. Try again.".to_owned()),
        }
        self.last_activity = Instant::now();
    }
}

impl Drop for TemplateState {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.password.zeroize();
        self.confirmation.zeroize();
        self.preview.zeroize();
        self.edit_name.zeroize();
    }
}

fn file_stamp(path: &Path) -> Option<(u64, SystemTime)> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || vault::is_link(&metadata) {
        return None;
    }
    Some((metadata.len(), metadata.modified().ok()?))
}

fn ensure_current(generation: &AtomicU64, expected: u64) -> Result<(), String> {
    if generation.load(Ordering::Acquire) == expected {
        Ok(())
    } else {
        Err("The operation was cancelled because the vault was locked.".to_owned())
    }
}

#[cfg(any(target_os = "windows", test))]
fn windows_resource_directory(executable: &Path) -> Option<PathBuf> {
    Some(executable.parent()?.join("ScriptHub.resources"))
}

fn bundled_vault(executable: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        Some(
            executable
                .parent()?
                .parent()?
                .join("Resources/templates.vault"),
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        Some(windows_resource_directory(executable)?.join("templates.vault"))
    }
}

fn writable_template_destination(path: &Path) -> bool {
    std::env::current_exe()
        .ok()
        .is_some_and(|executable| outside_application_assets(path, &executable))
}

fn outside_application_assets(path: &Path, executable: &Path) -> bool {
    #[cfg(target_os = "windows")]
    let resources = {
        let Some(resources) = windows_resource_directory(executable) else {
            return false;
        };
        Some(resources)
    };
    #[cfg(not(target_os = "windows"))]
    let resources: Option<PathBuf> = None;
    outside_application_assets_with_resources(path, executable, resources.as_deref())
}

fn outside_application_assets_with_resources(
    path: &Path,
    executable: &Path,
    windows_resources: Option<&Path>,
) -> bool {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let Ok(directory) = std::env::current_dir() else {
            return false;
        };
        directory.join(path)
    };
    let Some(parent) = absolute.parent() else {
        return false;
    };
    let mut existing = parent;
    let resolved = loop {
        match existing.canonicalize() {
            Ok(resolved) => break resolved,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if absolute
                    .components()
                    .any(|part| part == std::path::Component::ParentDir)
                {
                    return false;
                }
                let Some(ancestor) = existing.parent() else {
                    return false;
                };
                existing = ancestor;
            }
            Err(_) => return false,
        }
    };
    if let Some(resources) = windows_resources
        && !matches!(
            protected_windows_resources(&absolute, &resolved, resources),
            Ok(false)
        )
    {
        return false;
    }
    // Inspect resolved aliases and declared bundle ancestors, including missing child directories.
    let declared = (!absolute
        .components()
        .any(|part| part == std::path::Component::ParentDir))
    .then_some(parent);
    for ancestor in resolved
        .ancestors()
        .chain(declared.into_iter().flat_map(Path::ancestors))
    {
        if !ancestor
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
        {
            continue;
        }
        match application_bundle(ancestor) {
            Ok(false) => {}
            Ok(true) | Err(_) => return false,
        }
    }
    let normalized_destination = (existing == parent)
        .then(|| absolute.file_name().map(|name| resolved.join(name)))
        .flatten();
    if let Some(asset) = bundled_vault(executable).and_then(|asset| {
        Some(
            asset
                .parent()?
                .canonicalize()
                .ok()?
                .join(asset.file_name()?),
        )
    }) {
        let protected = normalized_destination.as_ref().is_some_and(|destination| {
            if destination == &asset {
                return true;
            }
            #[cfg(target_os = "windows")]
            {
                destination.parent() == asset.parent()
                    && destination
                        .file_name()
                        .and_then(|name| name.to_str())
                        .zip(asset.file_name().and_then(|name| name.to_str()))
                        .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
            }
            #[cfg(not(target_os = "windows"))]
            {
                false
            }
        });
        if protected {
            return false;
        }
    }
    true
}

fn protected_windows_resources(
    destination: &Path,
    resolved_parent: &Path,
    resources: &Path,
) -> std::io::Result<bool> {
    let Some(resource_parent) = resources.parent() else {
        return Ok(true);
    };
    let Some(resource_name) = resources.file_name() else {
        return Ok(true);
    };
    let resource_parent = resource_parent.canonicalize()?;
    // Protect the declared package namespace even when it is absent or aliases elsewhere.
    for ancestor in destination.ancestors() {
        if !ancestor.file_name().is_some_and(|name| {
            name.as_encoded_bytes()
                .eq_ignore_ascii_case(resource_name.as_encoded_bytes())
        }) {
            continue;
        }
        let Some(parent) = ancestor.parent() else {
            return Ok(true);
        };
        match parent.canonicalize() {
            Ok(parent) if parent == resource_parent => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    let canonical_resources = match resources.canonicalize() {
        Ok(resources) => resources,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if resolved_parent.starts_with(&canonical_resources) {
        return Ok(true);
    }
    match destination.canonicalize() {
        Ok(destination) => Ok(destination.starts_with(canonical_resources)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn application_bundle(path: &Path) -> std::io::Result<bool> {
    let info = match std::fs::metadata(path.join("Contents/Info.plist")) {
        Ok(info) => info,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    let executables = match std::fs::metadata(path.join("Contents/MacOS")) {
        Ok(executables) => executables,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };
    Ok(info.is_file() && executables.is_dir())
}

fn inferred_client(path: &Path) -> Client {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    if filename.contains("shadowrocket") {
        return Client::Shadowrocket;
    }
    if filename.contains("surge") {
        return Client::Surge;
    }
    let Ok(bytes) = vault::read_bounded(path) else {
        return Client::Other;
    };
    let bytes = Zeroizing::new(bytes);
    if bytes.len() > vault::MAX_TEMPLATE_BYTES {
        return Client::Other;
    }
    match std::str::from_utf8(&bytes) {
        Ok(content) if vault::template::is_flclash_config(content) => Client::FlClash,
        _ => Client::Other,
    }
}

fn configuration_export_path(path: &Path, client: Client) -> Option<PathBuf> {
    if client == Client::FlClash {
        let filename = path.file_name()?;
        let raw = path.as_os_str().as_encoded_bytes();
        // A verbatim Windows path may treat a trailing slash as filename data.
        if raw
            .last()
            .is_some_and(|&byte| std::path::is_separator(char::from(byte)))
            || !raw.ends_with(filename.as_encoded_bytes())
            || path.is_dir()
        {
            return None;
        }
        if path.extension().is_none() {
            return Some(path.with_extension("yaml"));
        }
    }
    Some(path.to_owned())
}

pub(super) fn clear_secret_widget(ctx: &egui::Context, id: &str) {
    ctx.data_mut(|data| data.remove::<egui::text_edit::TextEditState>(egui::Id::unique(id)));
}

pub(super) fn safe_export_destination(path: &Path, workspace: Option<&Path>) -> bool {
    if !writable_template_destination(path)
        || !path.is_absolute()
        || path
            .components()
            .any(|component| component == std::path::Component::ParentDir)
    {
        return false;
    }
    let Some(parent) = path.parent().and_then(|parent| parent.canonicalize().ok()) else {
        return false;
    };
    if workspace.is_some_and(|root| parent.starts_with(root)) {
        return false;
    }
    if parent
        .ancestors()
        .any(|ancestor| ancestor.join(".git").exists())
    {
        return false;
    }
    if let Some(storage) = crate::runtime::settings::preferences_path()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("vaults")))
        .and_then(|path| path.canonicalize().ok())
        && parent.starts_with(storage)
    {
        return false;
    }
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("vault"))
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLCLASH_FIXTURE: &str = "proxy-providers:\n  subscription:\n    type: http\n    url: \"{{SUBSCRIPTION_1}}\"\n    path: ./providers/subscription.yaml\n    interval: 3600\nproxy-groups:\n  - name: PROXY\n    type: select\n    use: [subscription]\nrules:\n  - MATCH,PROXY\ndns:\n  nameserver: ['https://resolver.example/dns-query']\n  default-nameserver: ['https://192.0.2.53/dns-query']\n";

    fn flclash_state() -> TemplateState {
        let mut state = TemplateState::default();
        state.vault = Some(
            vault::create(
                Library {
                    templates: vec![Template {
                        name: "FULL".to_owned(),
                        client: Client::FlClash,
                        content: FLCLASH_FIXTURE.to_owned(),
                    }],
                },
                "synthetic FlClash fixture password",
            )
            .unwrap(),
        );
        state
    }

    #[test]
    fn flclash_detection_reads_supported_content_including_extensionless_files() {
        let fixture = ManagedFixture::new();
        for filename in ["profile.yaml", "profile.yml", "FULL"] {
            let source = fixture.0.join(filename);
            std::fs::write(&source, FLCLASH_FIXTURE).unwrap();
            assert_eq!(inferred_client(&source), Client::FlClash);
        }
        // Existing client name hints still work without reading their contents.
        assert_eq!(inferred_client(Path::new("surge.conf")), Client::Surge);
        assert_eq!(
            inferred_client(Path::new("shadowrocket.conf")),
            Client::Shadowrocket
        );
    }

    #[test]
    fn flclash_detection_rejects_unrelated_invalid_and_oversized_yaml() {
        let fixture = ManagedFixture::new();
        let path = fixture.0.join("FlClash.yaml");
        for content in [
            "name: unrelated application\nversion: 1\n",
            "# proxy-providers: {}\n# proxy-groups: []\n# rules: []\n",
            "proxy-providers: [unterminated\n",
        ] {
            std::fs::write(&path, content).unwrap();
            assert_eq!(inferred_client(&path), Client::Other);
        }
        std::fs::write(&path, "x".repeat(vault::MAX_TEMPLATE_BYTES + 1)).unwrap();
        assert_eq!(inferred_client(&path), Client::Other);
        assert_eq!(
            inferred_client(&fixture.0.join("missing.yaml")),
            Client::Other
        );
    }

    #[test]
    fn flclash_edit_retains_the_yaml_client_and_content_in_the_encrypted_library() {
        let ctx = egui::Context::default();
        let mut state = flclash_state();
        state.edit_selected(&ctx);
        assert_eq!(state.edit_client, Client::FlClash);
        state.edit_name = Zeroizing::new("edited.yaml".to_owned());
        state.apply_edit(&ctx);
        assert!(state.error.is_none() && state.dirty && !state.editing);
        let sealed = state.vault.as_ref().unwrap().seal().unwrap();
        let reopened = vault::unlock(&sealed, "synthetic FlClash fixture password").unwrap();
        let template = &reopened.library().templates[0];
        assert_eq!(template.client, Client::FlClash);
        assert_eq!(template.name, "edited.yaml");
        assert_eq!(template.content, FLCLASH_FIXTURE);
    }

    #[test]
    fn flclash_export_uses_yaml_without_changing_explicit_or_existing_client_extensions() {
        for filename in ["profile.yaml", "profile.yml", "profile.YAML", "custom.conf"] {
            assert_eq!(
                configuration_export_path(Path::new(filename), Client::FlClash).as_deref(),
                Some(Path::new(filename))
            );
        }
        for client in [Client::Surge, Client::Shadowrocket, Client::Other] {
            assert_eq!(
                configuration_export_path(Path::new("profile"), client).as_deref(),
                Some(Path::new("profile"))
            );
        }
        let ctx = egui::Context::default();
        let fixture = ManagedFixture::new();
        let directory = fixture.0.join("directory");
        std::fs::create_dir(&directory).unwrap();
        assert!(configuration_export_path(&directory, Client::FlClash).is_none());
        let trailing_separator = PathBuf::from(format!("{}/", fixture.0.join("missing").display()));
        assert!(configuration_export_path(&trailing_separator, Client::FlClash).is_none());
        #[cfg(windows)]
        for path in [r"\\?\C:\missing/", r"\\?\C:\missing\"] {
            assert!(configuration_export_path(Path::new(path), Client::FlClash).is_none());
        }
        let mut state = flclash_state();
        state.refresh_selection(&ctx);
        assert_eq!(state.subscriptions.len(), 1);
        assert!(!state.windows_dns_guard);
        state.subscriptions[0]
            .1
            .push_str("https://subscriptions.example/fixture");
        state.allow_plaintext = true;
        state.windows_dns_guard = true;
        state.export_path = fixture.0.join("profile").to_string_lossy().into_owned();
        state.export(None);
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.busy() && Instant::now() < deadline {
            state.tick(&ctx);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!state.busy() && state.error.is_none(), "{:?}", state.error);
        assert_eq!(state.notice, Some(Text::VaultExported));
        assert!(!fixture.0.join("profile").exists());
        let exported = std::fs::read_to_string(fixture.0.join("profile.yaml")).unwrap();
        assert!(vault::template::is_flclash_config(&exported));
        assert!(exported.contains("https://subscriptions.example/fixture"));
        assert!(exported.contains("strict-route: true") && exported.contains("tcp://any:53"));
        state.refresh_selection(&ctx);
        assert!(!state.windows_dns_guard);
    }

    fn editable_state() -> TemplateState {
        let mut state = TemplateState::default();
        state.vault = Some(
            vault::create(
                Library {
                    templates: vec![Template {
                        name: "original.conf".to_owned(),
                        client: Client::Surge,
                        content: "[Rule]\nDOMAIN,original.example,DIRECT\n".to_owned(),
                    }],
                },
                "synthetic editor fixture password",
            )
            .unwrap(),
        );
        state
    }

    #[test]
    fn template_changes_auto_lock_clears_plaintext_and_reports_unsaved_discard() {
        for apply in [false, true] {
            let ctx = egui::Context::default();
            let mut state = editable_state();
            state.edit_selected(&ctx);
            state.preview = Zeroizing::new("synthetic unsaved template changes".to_owned());
            if apply {
                state.apply_edit(&ctx);
            }
            assert!(state.unlocked() && (state.editing || state.dirty));
            state.request_lock(&ctx, egui::Id::unique("lock-opener"));
            assert!(state.lock_confirmation.is_some());
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
            assert!(!state.unlocked() && !state.editing && !state.dirty);
            assert!(state.lock_confirmation.is_none());
            assert!(state.preview.is_empty() && state.edit_name.is_empty());
            assert!(state.password.is_empty() && state.confirmation.is_empty());
            assert_eq!(state.notice, Some(Text::VaultAutoLockedUnsaved));
        }
    }

    #[test]
    fn template_edit_is_explicit_and_apply_updates_the_encrypted_library() {
        let ctx = egui::Context::default();
        let mut state = editable_state();
        assert!(!state.editing && state.preview.is_empty() && state.edit_name.is_empty());
        state.edit_selected(&ctx);
        assert!(state.editing);
        assert_eq!(state.edit_name.as_str(), "original.conf");
        assert_eq!(state.edit_client, Client::Surge);
        assert_eq!(
            state.preview.as_str(),
            "[Rule]\nDOMAIN,original.example,DIRECT\n"
        );
        state.save();
        assert!(
            !state.busy() && state.error.is_none(),
            "Apply the draft before encrypted saving"
        );
        state.edit_name = Zeroizing::new("edited.conf".to_owned());
        state.edit_client = Client::Shadowrocket;
        state.preview = Zeroizing::new("[Rule]\nDOMAIN,edited.example,DIRECT\n".to_owned());
        state.apply_edit(&ctx);
        assert!(!state.editing && state.dirty && state.error.is_none());
        assert!(state.preview.is_empty() && state.edit_name.is_empty());
        let sealed = state.vault.as_ref().unwrap().seal().unwrap();
        let reopened = vault::unlock(&sealed, "synthetic editor fixture password").unwrap();
        let template = &reopened.library().templates[0];
        assert_eq!(template.name, "edited.conf");
        assert_eq!(template.client, Client::Shadowrocket);
        assert_eq!(template.content, "[Rule]\nDOMAIN,edited.example,DIRECT\n");
    }

    #[test]
    fn template_edit_invalid_draft_preserves_the_original_and_draft() {
        let ctx = egui::Context::default();
        let mut state = editable_state();
        for (name, content) in [("invalid\nname", "valid content"), ("valid.conf", "")] {
            state.editing = true;
            state.edit_name = Zeroizing::new(name.to_owned());
            state.edit_client = Client::Other;
            state.preview = Zeroizing::new(content.to_owned());
            state.apply_edit(&ctx);
            assert!(state.editing && state.error.is_some() && !state.dirty);
            assert_eq!(state.edit_name.as_str(), name);
            assert_eq!(state.preview.as_str(), content);
            let template = state.selected_template().unwrap();
            assert_eq!(template.name, "original.conf");
            assert_eq!(template.client, Client::Surge);
            assert_eq!(template.content, "[Rule]\nDOMAIN,original.example,DIRECT\n");
        }
    }

    #[test]
    fn template_edit_cancel_lock_and_selection_clear_draft_and_widget_history() {
        let ctx = egui::Context::default();
        let mut state = editable_state();
        for action in 0..3 {
            state.editing = true;
            state.edit_name = Zeroizing::new("private draft name".to_owned());
            state.preview = Zeroizing::new("private synthetic draft".to_owned());
            for id in [EDIT_NAME_ID, EDIT_CONTENT_ID] {
                egui::text_edit::TextEditState::default().store(&ctx, egui::Id::unique(id));
            }
            match action {
                0 => state.cancel_edit(&ctx),
                1 => state.refresh_selection(&ctx),
                _ => state.lock(&ctx),
            }
            assert!(!state.editing && state.edit_name.is_empty() && state.preview.is_empty());
            for id in [EDIT_NAME_ID, EDIT_CONTENT_ID] {
                assert!(egui::text_edit::TextEditState::load(&ctx, egui::Id::unique(id)).is_none());
            }
        }
    }

    struct ManagedFixture(PathBuf);

    impl ManagedFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "vault-managed-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(root.join(".cache/vaults")).unwrap();
            Self(root.canonicalize().unwrap())
        }

        fn encrypted_library(&self) -> PathBuf {
            let path = self.0.join(".cache/vaults/templates.vault");
            let encrypted = vault::create(
                Library {
                    templates: Vec::new(),
                },
                "synthetic fixture password",
            )
            .unwrap()
            .seal()
            .unwrap();
            vault::write_private(&path, &encrypted, false).unwrap();
            path
        }

        fn registry(&self) -> PathBuf {
            self.0.join(".cache/vaults/locations.json")
        }
    }

    impl Drop for ManagedFixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn managed_vault_restores_the_project_record_without_path_entry() {
        let fixture = ManagedFixture::new();
        let library = fixture.encrypted_library();
        std::fs::write(
            fixture.registry(),
            br#"{"version":1,"library":"templates.vault","originals":null}"#,
        )
        .unwrap();
        let mut state = TemplateState::default();
        state.visit_project(&fixture.0);
        assert_eq!(Path::new(&state.path), library);
        assert!(state.password.is_empty() && !state.unlocked());
    }

    #[test]
    fn managed_vault_stale_record_cleanup_never_deletes_ciphertext() {
        let fixture = ManagedFixture::new();
        let original = fixture.0.join("original-kept.vault");
        std::fs::write(&original, b"unrelated encrypted fixture").unwrap();
        std::fs::write(
            fixture.registry(),
            br#"{"version":1,"library":"../../original-kept.vault","originals":null}"#,
        )
        .unwrap();
        let mut state = TemplateState::default();
        state.visit_project(&fixture.0);
        let registry: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixture.registry()).unwrap()).unwrap();
        assert!(registry["library"].is_null());
        assert_eq!(
            std::fs::read(original).unwrap(),
            b"unrelated encrypted fixture"
        );
    }

    #[test]
    fn managed_vault_prepare_uses_fixed_project_destinations() {
        let fixture = ManagedFixture::new();
        let mut state = TemplateState::default();
        state.visit_project(&fixture.0);
        assert_eq!(
            Path::new(&state.prepare_public),
            fixture.0.join("crates/processor/assets/templates.vault")
        );
        assert_eq!(
            Path::new(&state.prepare_backup),
            fixture.0.join(".cache/vaults/originals.vault")
        );
        assert!(state.password.is_empty());
    }

    #[test]
    fn managed_vault_project_switch_cannot_retarget_an_unlocked_session() {
        let first = ManagedFixture::new();
        let second = ManagedFixture::new();
        let mut state = TemplateState::default();
        state.visit_project(&first.0);
        state.vault = Some(
            vault::create(
                Library {
                    templates: Vec::new(),
                },
                "synthetic fixture password",
            )
            .unwrap(),
        );
        state.visit_project(&second.0);
        assert_eq!(
            state.save_destination(),
            Some(first.0.join(".cache/vaults/templates.vault"))
        );
        state.lock(&egui::Context::default());
        state.visit_project(&second.0);
        assert_eq!(
            state.save_destination(),
            Some(second.0.join(".cache/vaults/templates.vault"))
        );
    }

    #[test]
    fn managed_vault_missing_selected_file_clears_only_its_record_on_revisit() {
        let fixture = ManagedFixture::new();
        let library = fixture.encrypted_library();
        let mut state = TemplateState::default();
        state.visit_project(&fixture.0);
        let kept = fixture.0.join("kept-encrypted.vault");
        std::fs::rename(library, &kept).unwrap();
        state.visit_project(&fixture.0);
        let registry: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixture.registry()).unwrap()).unwrap();
        assert!(registry["library"].is_null());
        assert!(kept.is_file());
        assert_ne!(
            state.path,
            fixture
                .0
                .join(".cache/vaults/templates.vault")
                .to_string_lossy()
        );
    }

    #[test]
    fn managed_vault_explicit_visit_restores_a_verified_copy_of_known_legacy_originals() {
        let fixture = ManagedFixture::new();
        let legacy = ManagedFixture::new();
        let encrypted = legacy.encrypted_library();
        let source = legacy.0.join("originals.vault");
        std::fs::rename(encrypted, &source).unwrap();
        let before = vault::read_bounded(&source).unwrap();
        let mut state = TemplateState::default();
        state.visit_project_with_legacy(&fixture.0, Some(&source));
        assert!(state.originals_available());
        state.select_library(true, &egui::Context::default());
        assert_eq!(
            Path::new(&state.path),
            fixture.0.join(".cache/vaults/originals.vault")
        );
        assert_eq!(vault::read_bounded(Path::new(&state.path)).unwrap(), before);
        assert_eq!(vault::read_bounded(&source).unwrap(), before);
        assert!(state.password.is_empty() && !state.unlocked());
    }

    #[test]
    fn managed_vault_wrong_password_never_removes_a_valid_record_or_ciphertext() {
        let fixture = ManagedFixture::new();
        let library = fixture.encrypted_library();
        let mut state = TemplateState::default();
        state.visit_project(&fixture.0);
        let record = vault::read_bounded(&fixture.registry()).unwrap();
        let encrypted = vault::read_bounded(&library).unwrap();
        state.password.push_str("synthetic wrong password");
        let ctx = egui::Context::default();
        state.open(&ctx);
        let deadline = Instant::now() + Duration::from_secs(10);
        while state.busy() && Instant::now() < deadline {
            state.tick(&ctx);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!state.busy() && !state.unlocked() && state.error.is_some());
        assert_eq!(vault::read_bounded(&fixture.registry()).unwrap(), record);
        assert_eq!(vault::read_bounded(&library).unwrap(), encrypted);
    }

    #[test]
    fn managed_vault_unlocked_missing_file_clears_record_without_retargeting_session() {
        let first = ManagedFixture::new();
        let second = ManagedFixture::new();
        let library = first.encrypted_library();
        let encrypted = vault::read_bounded(&library).unwrap();
        let mut state = TemplateState::default();
        state.visit_project(&first.0);
        state.vault = Some(vault::unlock(&encrypted, "synthetic fixture password").unwrap());
        state.dirty = true;
        let selected = state.path.clone();
        let kept = first.0.join("kept-original.vault");
        std::fs::rename(&library, &kept).unwrap();
        state.visit_project(&second.0);
        let registry: serde_json::Value =
            serde_json::from_slice(&vault::read_bounded(&first.registry()).unwrap()).unwrap();
        assert!(registry["library"].is_null());
        assert!(state.unlocked() && state.dirty);
        assert_eq!(state.path, selected);
        assert_eq!(state.save_destination(), Some(library));
        assert_eq!(vault::read_bounded(&kept).unwrap(), encrypted);
        assert!(!second.registry().exists());
    }

    #[test]
    fn managed_vault_locked_replacement_revalidates_envelope_without_deleting_ciphertext() {
        let fixture = ManagedFixture::new();
        let library = fixture.encrypted_library();
        let mut state = TemplateState::default();
        state.visit_project(&fixture.0);
        vault::write_private(&library, b"replaced malformed encrypted fixture", true).unwrap();
        state.visit_project(&fixture.0);
        let registry: serde_json::Value =
            serde_json::from_slice(&vault::read_bounded(&fixture.registry()).unwrap()).unwrap();
        assert!(registry["library"].is_null());
        assert_eq!(
            vault::read_bounded(&library).unwrap(),
            b"replaced malformed encrypted fixture"
        );
        assert_ne!(state.path, library.to_string_lossy());
        assert!(!state.unlocked());
    }

    #[test]
    fn dropped_vault_extensions_are_case_insensitive() {
        for name in ["library.vault", "LIBRARY.VAULT", "Library.Vault"] {
            let mut state = TemplateState::default();
            state.queue_dropped(vec![PathBuf::from(name)]);
            assert_eq!(state.path, name);
            assert_eq!(state.queued_imports(), 0);
        }
    }

    #[test]
    fn returning_input_cannot_extend_an_expired_unlocked_session() {
        let ctx = egui::Context::default();
        let mut state = TemplateState::default();
        state.vault = Some(
            vault::create(
                Library {
                    templates: Vec::new(),
                },
                "synthetic password long",
            )
            .unwrap(),
        );
        state.last_activity = Instant::now() - IDLE_TIMEOUT - Duration::from_secs(1);
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Text("x".to_owned()));
        let output = ctx.run_ui(input, |ui| state.tick(ui.ctx()));
        output.drop_without_applying_deltas();
        assert!(
            !state.unlocked(),
            "An expired session must lock before accepting new input"
        );
    }

    #[test]
    fn an_unlocked_session_requests_its_idle_deadline_without_a_worker() {
        let ctx = egui::Context::default();
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed = Arc::clone(&requests);
        ctx.set_repaint_observer(move |request| observed.lock().unwrap().push(request.delay));
        let mut state = TemplateState::default();
        state.vault = Some(
            vault::create(
                Library {
                    templates: Vec::new(),
                },
                "synthetic password long",
            )
            .unwrap(),
        );
        state.tick(&ctx);
        assert!(
            requests
                .lock()
                .unwrap()
                .iter()
                .any(|delay| *delay > Duration::ZERO && *delay <= IDLE_TIMEOUT)
        );
    }

    #[test]
    fn locking_discards_late_unlock_results_and_secret_inputs() {
        let ctx = egui::Context::default();
        let mut state = TemplateState::default();
        state.password.push_str("fixture password");
        state.confirmation.push_str("fixture password");
        state.subscriptions.push((
            "SUBSCRIPTION_1".into(),
            Zeroizing::new("https://example.invalid/secret".into()),
        ));
        state.preview.push_str("private fixture");
        let generation = state.generation.load(Ordering::Acquire);
        state.lock(&ctx);
        assert!(
            state.password.is_empty()
                && state.confirmation.is_empty()
                && state.subscriptions.is_empty()
                && state.preview.is_empty()
        );
        assert!(ensure_current(&state.generation, generation).is_err());
    }

    #[test]
    fn lock_preserves_the_busy_guard_until_the_worker_finishes() {
        let ctx = egui::Context::default();
        let mut state = TemplateState::default();
        let (release, wait) = mpsc::sync_channel(1);
        state.start(move |_, _| {
            wait.recv().unwrap();
            Ok(Completed::Exported)
        });
        state.lock(&ctx);
        assert!(state.busy());
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            state.tick(&ctx);
            if !state.busy() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!state.busy());
        assert!(!state.unlocked());
        assert_ne!(state.notice, Some(Text::VaultExported));
    }

    #[test]
    fn focus_loss_revokes_a_pending_result_before_it_can_be_displayed() {
        let ctx = egui::Context::default();
        let mut state = TemplateState::default();
        let (release, wait) = mpsc::sync_channel(1);
        state.start(move |_, _| {
            wait.recv().unwrap();
            Ok(Completed::Exported)
        });
        let generation = state.generation.load(Ordering::Acquire);
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
        assert!(ensure_current(&state.generation, generation).is_err());
        assert!(state.busy());
        release.send(()).unwrap();
    }

    #[test]
    fn fresh_interaction_starts_a_new_deadline_in_an_empty_locked_view() {
        let ctx = egui::Context::default();
        let mut state = TemplateState::default();
        state.last_activity = Instant::now() - IDLE_TIMEOUT;
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Text("x".to_owned()));
        let output = ctx.run_ui(input, |ui| state.tick(ui.ctx()));
        output.drop_without_applying_deltas();
        assert!(state.password.is_empty());
        assert!(state.last_activity.elapsed() < IDLE_TIMEOUT);
    }

    #[test]
    fn invalid_prepare_inputs_leave_the_password_available_for_correction() {
        let ctx = egui::Context::default();
        let mut state = TemplateState::default();
        state.prepare_mode = true;
        state.password.push_str("synthetic strong passphrase");
        state.confirmation.push_str("synthetic strong passphrase");
        state.open(&ctx);
        assert!(!state.busy() && state.error.is_some());
        assert_eq!(state.password.as_str(), "synthetic strong passphrase");

        let source = std::env::temp_dir().join("synthetic.surge.conf");
        state.prepare_sources = source.to_string_lossy().into_owned();
        state.prepare_public = source.to_string_lossy().into_owned();
        state.prepare_backup = std::env::temp_dir()
            .join("synthetic-backup.vault")
            .to_string_lossy()
            .into_owned();
        state.open(&ctx);
        assert!(!state.busy() && state.error.is_some());
        assert_eq!(state.password.as_str(), "synthetic strong passphrase");
    }

    #[test]
    fn imports_that_exceed_encoded_size_preserve_the_existing_library() {
        let mut state = TemplateState::default();
        state.vault = Some(
            vault::create(
                Library {
                    templates: vec![Template {
                        name: "kept.conf".to_owned(),
                        client: Client::Other,
                        content: "keep this fixture".to_owned(),
                    }],
                },
                "synthetic test passphrase",
            )
            .unwrap(),
        );
        let templates = (0..5)
            .map(|index| Template {
                name: format!("escaped-{index}.conf"),
                client: Client::Other,
                content: "\n".repeat(vault::MAX_TEMPLATE_BYTES),
            })
            .collect();
        assert_eq!(
            state.append_templates(templates).err(),
            Some(vault::VaultError::TooLarge)
        );
        let library = state.vault.as_ref().unwrap().library();
        assert_eq!(library.templates.len(), 1);
        assert_eq!(library.templates[0].content, "keep this fixture");
        assert!(!state.dirty);
    }

    #[test]
    fn plain_exports_reject_relative_and_workspace_paths() {
        let root = std::env::temp_dir().canonicalize().unwrap();
        assert!(!safe_export_destination(Path::new("template.conf"), None));
        assert!(!safe_export_destination(
            &root.join("template.conf"),
            Some(&root)
        ));
        assert!(!safe_export_destination(
            &root.join("templates.vault"),
            None
        ));
        assert!(!safe_export_destination(
            &root.join("templates.VAULT"),
            None
        ));
        assert!(safe_export_destination(&root.join("template.conf"), None));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn plain_exports_reject_application_bundles_but_allow_external_locations() {
        let root = std::env::temp_dir().join(format!(
            "vault-app-assets-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let bundle = root.join("Synthetic.app");
        std::fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        std::fs::create_dir_all(bundle.join("Contents/Resources")).unwrap();
        std::fs::write(bundle.join("Contents/Info.plist"), "synthetic fixture").unwrap();
        let ordinary = root.join("ordinary.app");
        std::fs::create_dir(&ordinary).unwrap();
        let bundle_allowed =
            safe_export_destination(&bundle.join("Contents/Resources/profile.conf"), None);
        let external_allowed = safe_export_destination(&root.join("profile.conf"), None);
        let ordinary_allowed = safe_export_destination(&ordinary.join("profile.conf"), None);
        let executable = bundle.join("Contents/MacOS/script-hub-gui");
        let inside_missing = outside_application_assets(
            &bundle.join("Contents/new/backup/originals.vault"),
            &executable,
        );
        let outside_missing = outside_application_assets(
            &root.join("new-private-backup/originals.vault"),
            &executable,
        );
        let outside_save_as = outside_application_assets(&root.join("editable.vault"), &executable);
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            !bundle_allowed
                && external_allowed
                && ordinary_allowed
                && !inside_missing
                && outside_missing
                && outside_save_as,
            "application resource allowed={bundle_allowed}, external allowed={external_allowed}, ordinary .app directory allowed={ordinary_allowed}"
        );
    }

    #[test]
    fn windows_packaged_vault_uses_the_resource_directory() {
        let root = std::env::temp_dir();
        let executable = root.join("ScriptHub.exe");
        assert_eq!(
            windows_resource_directory(&executable),
            Some(root.join("ScriptHub.resources"))
        );
    }

    #[test]
    fn windows_packaged_resources_protect_the_whole_namespace() {
        let root = std::env::temp_dir().join(format!(
            "vault-windows-resources-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let resources = root.join("ScriptHub.resources");
        let executable = root.join("ScriptHub.exe");
        let protected_paths = [
            resources.clone(),
            resources.join("templates.vault"),
            resources.join("new/backup/profile.conf"),
            root.join("sCrIpThUb.ReSoUrCeS/new/profile.conf"),
        ];
        let allowed_paths = [
            root.join("my-copy.vault"),
            root.join("local-backups/originals.vault"),
            root.join("ScriptHub.resources-copy/profile.conf"),
        ];
        let absent_protected = protected_paths.iter().all(|path| {
            !outside_application_assets_with_resources(path, &executable, Some(&resources))
        });
        std::fs::create_dir(&resources).unwrap();
        let present_protected = protected_paths.iter().all(|path| {
            !outside_application_assets_with_resources(path, &executable, Some(&resources))
        });
        let neighboring_allowed = allowed_paths.iter().all(|path| {
            outside_application_assets_with_resources(path, &executable, Some(&resources))
        });
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            absent_protected,
            "missing packaged resource directories must remain protected"
        );
        assert!(
            present_protected,
            "all packaged resources must remain protected"
        );
        assert!(
            neighboring_allowed,
            "neighboring user destinations must remain writable"
        );
    }

    #[cfg(unix)]
    #[test]
    fn windows_packaged_resources_reject_resolved_and_declared_aliases() {
        let root = std::env::temp_dir().join(format!(
            "vault-windows-aliases-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let resources = root.join("ScriptHub.resources");
        let external = root.join("external");
        std::fs::create_dir_all(&resources).unwrap();
        std::fs::create_dir(&external).unwrap();
        std::fs::write(resources.join("templates.vault"), "synthetic fixture").unwrap();
        std::os::unix::fs::symlink(&resources, root.join("resource-alias")).unwrap();
        std::os::unix::fs::symlink(&external, resources.join("external-alias")).unwrap();
        std::os::unix::fs::symlink(resources.join("templates.vault"), root.join("file-alias"))
            .unwrap();
        let executable = root.join("ScriptHub.exe");
        let protected_paths = [
            root.join("resource-alias/new/profile.conf"),
            resources.join("external-alias/profile.conf"),
            root.join("file-alias"),
        ];
        let protected = protected_paths.iter().all(|path| {
            !outside_application_assets_with_resources(path, &executable, Some(&resources))
        });
        let editable = outside_application_assets_with_resources(
            &external.join("profile.conf"),
            &executable,
            Some(&resources),
        );
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            protected,
            "canonical and declared resource aliases must remain read-only"
        );
        assert!(editable);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_packaged_sidecar_is_read_only_but_a_neighboring_copy_is_editable() {
        let root = std::env::temp_dir().join(format!(
            "vault-sidecar-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let executable = root.join("script-hub-gui.exe");
        let sidecar = bundled_vault(&executable).unwrap();
        assert_eq!(sidecar, root.join("ScriptHub.resources/templates.vault"));
        let protected = outside_application_assets(&sidecar, &executable);
        let protected_uppercase = outside_application_assets(
            &root.join("SCRIPTHUB.RESOURCES/TEMPLATES.VAULT"),
            &executable,
        );
        let protected_missing = outside_application_assets(
            &root.join("ScriptHub.resources/new/backup/profile.conf"),
            &executable,
        );
        let editable = outside_application_assets(&root.join("my-copy.vault"), &executable);
        let missing_backup =
            outside_application_assets(&root.join("local-backups/originals.vault"), &executable);
        std::fs::remove_dir_all(root).unwrap();
        assert!(
            !protected && !protected_uppercase && !protected_missing && editable && missing_backup
        );
    }
}
