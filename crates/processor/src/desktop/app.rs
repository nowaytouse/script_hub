use super::app_history::HistoryLoader;
use super::catalog::Catalog;
use super::history::{HistoryOptions, HistoryRecord};
use super::i18n::{Text, text};
use super::job::{Job, JobKind, JobOptions, JobOutcome, Snapshot, validate_root};
use super::progress::Stage;
use super::repository::{self, RepositoryStatus};
use super::settings::{self, Preferences};
use super::theme::{self, Palette};
use crate::pipeline::custom_sources::{self, CustomSource, SourceCategory};
use eframe::egui::{self, RichText};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Page {
    Overview,
    Catalog,
    Tasks,
    Repository,
    Settings,
    Support,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum CatalogTab {
    Modules,
    Rules,
    Sources,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SupportTab {
    Help,
    About,
    Licenses,
    Privacy,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TaskTab {
    Current,
    History,
}

pub(super) enum Pending {
    Job(JobKind, JobOptions),
    AddSource(PathBuf, CustomSource),
    RemoveSource(PathBuf, SourceCategory, String),
    ClearHistory(PathBuf),
    RemoveUnfinishedHistory(PathBuf),
}

struct WorkspaceLoader {
    receiver: Receiver<Result<LoadedWorkspace, String>>,
    worker: JoinHandle<()>,
    requested_root: Option<PathBuf>,
    shadowrocket: bool,
}

struct LoadedWorkspace {
    root: PathBuf,
    catalog: Catalog,
    sources: Result<Vec<CustomSource>, String>,
}

pub(super) struct RepositoryLoader {
    receiver: Receiver<Result<RepositoryStatus, String>>,
    worker: JoinHandle<()>,
    requested_root: PathBuf,
}

fn load_workspace(
    requested_root: Option<PathBuf>,
    shadowrocket: bool,
) -> Result<LoadedWorkspace, String> {
    let candidate = requested_root.ok_or_else(|| "choose a project folder first".to_string())?;
    let root = validate_root(&candidate)?;
    let catalog = Catalog::load(&root, shadowrocket)?;
    // A damaged optional source manifest must not hide the valid local catalog.
    // Its write/update paths still reject the malformed manifest independently.
    let sources = custom_sources::load(&root);
    Ok(LoadedWorkspace {
        root,
        catalog,
        sources,
    })
}

pub struct HubApp {
    pub(super) preferences: Preferences,
    pub(super) draft: Preferences,
    pub(super) settings_path: Option<PathBuf>,
    pub(super) loaded_root: Option<PathBuf>,
    pub(super) root_input: String,
    pub(super) page: Page,
    pub(super) catalog_tab: CatalogTab,
    pub(super) shadowrocket: bool,
    pub(super) catalog: Catalog,
    pub(super) module_search: String,
    pub(super) rule_search: String,
    pub(super) category: String,
    pub(super) selected_module: Option<usize>,
    pub(super) selected_rule: Option<usize>,
    pub(super) selected_modules: BTreeSet<usize>,
    pub(super) selected_rules: BTreeSet<usize>,
    pub(super) copy_policy: String,
    pub(super) sources: Vec<CustomSource>,
    pub(super) source_name: String,
    pub(super) source_url: String,
    pub(super) source_category: SourceCategory,
    pub(super) repository: Option<RepositoryStatus>,
    pub(super) repository_error: Option<String>,
    pub(super) repository_loader: Option<RepositoryLoader>,
    pub(super) selected_files: BTreeSet<String>,
    pub(super) commit_message: String,
    pub(super) task_kind: JobKind,
    pub(super) job: Option<Job>,
    workspace_loader: Option<WorkspaceLoader>,
    initial_load: bool,
    startup_error: Option<String>,
    refresh_repository_after_load: bool,
    pub(super) job_finished_handled: bool,
    pub(super) show_logs: bool,
    pub(super) support_tab: SupportTab,
    pub(super) task_tab: TaskTab,
    pub(super) log_search: String,
    pub(super) problems_only: bool,
    pub(super) history_records: Vec<HistoryRecord>,
    pub(super) history_loader: Option<HistoryLoader>,
    pub(super) history_error: Option<String>,
    pub(super) history_search: String,
    pub(super) pending: Option<Pending>,
    pub(super) error: Option<String>,
    pub(super) notice: Option<Text>,
    pub(super) colors: Palette,
    appearance: Option<(Preferences, bool)>,
}

impl HubApp {
    fn new(
        explicit: Option<PathBuf>,
        preferences: Preferences,
        settings_path: Option<PathBuf>,
        startup_error: Option<String>,
    ) -> Self {
        Self::new_with_loader(
            explicit,
            preferences,
            settings_path,
            startup_error,
            load_workspace,
        )
    }

    fn new_with_loader<F>(
        explicit: Option<PathBuf>,
        preferences: Preferences,
        settings_path: Option<PathBuf>,
        startup_error: Option<String>,
        loader: F,
    ) -> Self
    where
        F: FnOnce(Option<PathBuf>, bool) -> Result<LoadedWorkspace, String> + Send + 'static,
    {
        let open_on_startup = explicit.is_some() || preferences.reopen_workspace;
        let root = explicit.or_else(|| {
            let saved = preferences.root.trim();
            (!saved.is_empty()).then(|| PathBuf::from(saved))
        });
        let mut app = Self {
            root_input: root
                .as_deref()
                .map_or_else(String::new, |path| path.to_string_lossy().into_owned()),
            loaded_root: None,
            draft: preferences.clone(),
            colors: theme::palette(false, preferences.accent),
            preferences,
            settings_path,
            page: Page::Overview,
            catalog_tab: CatalogTab::Modules,
            shadowrocket: false,
            catalog: Catalog::default(),
            module_search: String::new(),
            rule_search: String::new(),
            category: String::new(),
            selected_module: None,
            selected_rule: None,
            selected_modules: BTreeSet::new(),
            selected_rules: BTreeSet::new(),
            copy_policy: String::new(),
            sources: Vec::new(),
            source_name: String::new(),
            source_url: String::new(),
            source_category: SourceCategory::Wool,
            repository: None,
            repository_error: None,
            repository_loader: None,
            selected_files: BTreeSet::new(),
            commit_message: String::new(),
            task_kind: JobKind::Validate,
            job: None,
            workspace_loader: None,
            initial_load: false,
            startup_error: startup_error.clone(),
            refresh_repository_after_load: false,
            job_finished_handled: true,
            show_logs: false,
            support_tab: SupportTab::Help,
            task_tab: TaskTab::Current,
            log_search: String::new(),
            problems_only: false,
            history_records: Vec::new(),
            history_loader: None,
            history_error: None,
            history_search: String::new(),
            pending: None,
            error: None,
            notice: None,
            appearance: None,
        };
        if let Some(error) = startup_error {
            app.error = Some(error);
        }
        if open_on_startup && root.is_some() {
            app.start_workspace_load_with(root, loader);
            app.initial_load = true;
        }
        app.refresh_history(false);
        app
    }

    pub(super) fn tr(&self, key: Text) -> &'static str {
        text(self.draft.language, key)
    }
    pub(super) fn active(&self) -> bool {
        self.job.as_ref().is_some_and(Job::is_running)
            || self.workspace_loader.is_some()
            || self.repository_loader.is_some()
    }
    fn job_active(&self) -> bool {
        self.job.as_ref().is_some_and(Job::is_running)
    }
    pub(super) fn workspace_ready(&self) -> bool {
        self.loaded_root.as_deref() == Some(Path::new(self.root_input.trim()))
    }

    pub(super) fn reload(&mut self) {
        if self.active() {
            return;
        }
        let root =
            (!self.root_input.trim().is_empty()).then(|| PathBuf::from(self.root_input.trim()));
        self.start_workspace_load_with(root, load_workspace);
    }

    fn start_workspace_load_with<F>(&mut self, requested_root: Option<PathBuf>, loader_fn: F)
    where
        F: FnOnce(Option<PathBuf>, bool) -> Result<LoadedWorkspace, String> + Send + 'static,
    {
        if self.workspace_loader.is_some() || self.job_active() {
            return;
        }
        self.loaded_root = None;
        self.catalog = Catalog::default();
        self.sources.clear();
        self.repository = None;
        self.repository_error = None;
        self.repository_loader = None;
        self.selected_module = None;
        self.selected_rule = None;
        self.selected_modules.clear();
        self.selected_rules.clear();
        self.copy_policy.clear();
        self.selected_files.clear();
        let shadowrocket = self.shadowrocket;
        self.error = self.startup_error.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let loader_root = requested_root.clone();
        let worker = match thread::Builder::new()
            .name("script-hub-workspace-loader".to_owned())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    loader_fn(loader_root, shadowrocket)
                }))
                .unwrap_or_else(|_| Err("workspace loading failed unexpectedly".to_owned()));
                let _ = sender.send(result);
            }) {
            Ok(worker) => worker,
            Err(error) => {
                self.error = Some(format!("cannot start workspace loader: {error}"));
                return;
            }
        };
        self.workspace_loader = Some(WorkspaceLoader {
            receiver,
            worker,
            requested_root,
            shadowrocket,
        });
    }

    fn poll_workspace_loader(&mut self, ctx: &egui::Context) {
        let Some(loader) = self.workspace_loader.as_ref() else {
            return;
        };
        let result = match loader.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err("workspace loader stopped without a result".to_owned()))
            }
        };
        let Some(result) = result else {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        };
        let Some(loader) = self.workspace_loader.take() else {
            return;
        };
        self.finish_workspace_loader(loader, result);
    }

    fn finish_workspace_loader(
        &mut self,
        loader: WorkspaceLoader,
        mut result: Result<LoadedWorkspace, String>,
    ) {
        let initial_load = std::mem::take(&mut self.initial_load);
        if loader.worker.join().is_err() {
            result = Err("workspace loader stopped unexpectedly".to_owned());
        }
        // A platform change or root edit must never apply a stale snapshot.
        if loader.shadowrocket != self.shadowrocket
            || loader.requested_root.as_deref().map_or_else(
                || !self.root_input.trim().is_empty(),
                |root| root.to_string_lossy() != self.root_input,
            )
        {
            self.refresh_repository_after_load = false;
            return;
        }
        match result {
            Ok(loaded) => {
                self.root_input = loaded.root.to_string_lossy().into_owned();
                self.draft.root = self.root_input.clone();
                if initial_load {
                    self.preferences.root = self.draft.root.clone();
                }
                self.catalog = loaded.catalog;
                self.loaded_root = Some(loaded.root);
                match loaded.sources {
                    Ok(sources) => {
                        self.sources = sources;
                        self.error = self.startup_error.clone();
                    }
                    Err(error) => {
                        self.sources.clear();
                        self.error = self.startup_error.clone().or(Some(error));
                    }
                }
                if self.refresh_repository_after_load {
                    self.refresh_repository_after_load = false;
                    self.refresh_repository();
                }
            }
            Err(error) => {
                self.loaded_root = None;
                self.catalog = Catalog::default();
                self.sources.clear();
                self.repository = None;
                self.error = self.startup_error.clone().or(Some(error));
                self.refresh_repository_after_load = false;
            }
        }
    }

    pub(super) fn refresh_repository(&mut self) {
        self.refresh_repository_with(repository::snapshot);
    }

    fn refresh_repository_with<F>(&mut self, reader: F)
    where
        F: FnOnce(&Path) -> Result<RepositoryStatus, String> + Send + 'static,
    {
        if self.repository_loader.is_some() || self.workspace_loader.is_some() || self.job_active()
        {
            return;
        }
        let Some(root) = self.loaded_root.clone().filter(|_| self.workspace_ready()) else {
            return;
        };
        self.repository = None;
        self.repository_error = None;
        let worker_root = root.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = match thread::Builder::new()
            .name("script-hub-repository-reader".to_owned())
            .spawn(move || {
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| reader(&worker_root)))
                        .unwrap_or_else(|_| {
                            Err("repository reader stopped unexpectedly".to_owned())
                        });
                let _ = sender.send(result);
            }) {
            Ok(worker) => worker,
            Err(error) => {
                self.repository_error = Some(format!("cannot start repository reader: {error}"));
                return;
            }
        };
        self.repository_loader = Some(RepositoryLoader {
            receiver,
            worker,
            requested_root: root,
        });
    }

    fn poll_repository_loader(&mut self, ctx: &egui::Context) {
        let Some(loader) = self.repository_loader.as_ref() else {
            return;
        };
        let result = match loader.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err("repository reader stopped without a result".to_owned()))
            }
        };
        let Some(result) = result else {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        };
        let Some(loader) = self.repository_loader.take() else {
            return;
        };
        self.finish_repository_loader(loader, result);
    }

    fn finish_repository_loader(
        &mut self,
        loader: RepositoryLoader,
        mut result: Result<RepositoryStatus, String>,
    ) {
        if loader.worker.join().is_err() {
            result = Err("repository reader stopped unexpectedly".to_owned());
        }
        if self.loaded_root.as_ref() != Some(&loader.requested_root) || !self.workspace_ready() {
            return;
        }
        match result {
            Ok(status) => {
                self.selected_files
                    .retain(|path| status.changes.iter().any(|change| &change.path == path));
                self.repository = Some(status);
                self.repository_error = None;
            }
            Err(error) => {
                self.repository = None;
                self.repository_error = Some(error);
            }
        }
    }

    pub(super) fn queue_job(&mut self, kind: JobKind) {
        if self.active() || !self.workspace_ready() {
            return;
        }
        let Some(root) = self.loaded_root.clone() else {
            return;
        };
        let options = JobOptions {
            root,
            singbox: (!self.draft.singbox.trim().is_empty())
                .then(|| PathBuf::from(self.draft.singbox.trim())),
            commit_message: Some(self.commit_message.clone()),
            paths: self.selected_files.iter().cloned().collect(),
            history: if self.draft.record_history {
                self.history_path().map(|path| HistoryOptions {
                    path,
                    keep_logs: self.draft.developer_mode && self.draft.diagnostic_logs,
                })
            } else {
                None
            },
        };
        if kind == JobKind::Validate {
            self.start(kind, options);
        } else {
            self.pending = Some(Pending::Job(kind, options));
        }
    }

    fn start(&mut self, kind: JobKind, options: JobOptions) {
        if self.active() {
            return;
        }
        match Job::start(kind, options) {
            Ok(job) => {
                self.job = Some(job);
                if !matches!(
                    kind,
                    JobKind::GitPull | JobKind::GitCommit | JobKind::GitPush
                ) {
                    self.task_kind = kind;
                }
                self.job_finished_handled = false;
                self.page = Page::Tasks;
                self.task_tab = TaskTab::Current;
                self.error = None;
                self.notice = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn copy(&mut self, ctx: &egui::Context, value: &str) {
        ctx.copy_text(value.to_string());
        self.notice = Some(Text::Copied);
    }

    pub(super) fn task_label(&self, kind: JobKind) -> &'static str {
        self.tr(match kind {
            JobKind::Validate => Text::Validate,
            JobKind::RefreshCatalog => Text::RefreshCatalog,
            JobKind::Update => Text::FullUpdate,
            JobKind::QuickUpdate => Text::QuickUpdate,
            JobKind::GamingOnly => Text::GamingUpdate,
            JobKind::AdBlockOnly => Text::AdBlockUpdate,
            JobKind::SrsOnly => Text::RebuildSrs,
            JobKind::GitPull => Text::Pull,
            JobKind::GitCommit => Text::Commit,
            JobKind::GitPush => Text::Push,
        })
    }

    pub(super) fn stage_label(&self, stage: Stage) -> &'static str {
        self.tr(match stage {
            Stage::Prepare => Text::StagePrepare,
            Stage::Functional => Text::StageFunctional,
            Stage::General => Text::StageGeneral,
            Stage::Adblock => Text::StageAdblock,
            Stage::Cleanup => Text::StageCleanup,
            Stage::Categories => Text::StageCategories,
            Stage::Gaming => Text::StageGaming,
            Stage::Rewrites => Text::StageRewrites,
            Stage::Mitm => Text::StageMitm,
            Stage::Shadowrocket => Text::StageShadowrocket,
            Stage::Security => Text::StageSecurity,
            Stage::Mirrors => Text::StageMirrors,
            Stage::Catalog => Text::StageCatalog,
            Stage::Srs => Text::StageSrs,
            Stage::Finalize => Text::StageFinalize,
            Stage::Validate => Text::StageValidate,
            Stage::Repository => Text::Repository,
        })
    }

    pub(super) fn outcome(&self, snapshot: &Snapshot) -> (&'static str, egui::Color32) {
        if snapshot.running {
            return (self.tr(Text::Running), self.colors.accent);
        }
        match snapshot.outcome {
            Some(JobOutcome::Success) => (self.tr(Text::Succeeded), self.colors.success),
            Some(JobOutcome::Warnings) => (self.tr(Text::Warnings), self.colors.warning),
            _ => (self.tr(Text::Failed), self.colors.error),
        }
    }

    fn navigation(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.label(
            RichText::new("SCRIPT HUB")
                .size(20.0)
                .strong()
                .color(self.colors.accent),
        );
        ui.weak(self.tr(Text::Workspace));
        ui.add_space(24.0);
        for (page, key, shortcut) in [
            (Page::Overview, Text::Overview, "1"),
            (Page::Catalog, Text::Library, "2"),
            (Page::Tasks, Text::Tasks, "3"),
            (Page::Repository, Text::Repository, "4"),
        ] {
            if ui
                .add(
                    egui::Button::new("")
                        .left_text(self.tr(key))
                        .right_text(shortcut)
                        .selected(self.page == page)
                        .min_size(egui::vec2(ui.available_width(), 39.0))
                        .corner_radius(8),
                )
                .clicked()
            {
                self.page = page;
                if page == Page::Repository && self.repository.is_none() {
                    self.refresh_repository();
                }
            }
        }
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(12.0);
        for (page, key) in [
            (Page::Settings, Text::Settings),
            (Page::Support, Text::Help),
        ] {
            if ui
                .add(
                    egui::Button::new("")
                        .left_text(self.tr(key))
                        .selected(self.page == page)
                        .min_size(egui::vec2(ui.available_width(), 36.0))
                        .frame_when_inactive(false),
                )
                .clicked()
            {
                self.page = page;
            }
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.weak(format!("v{}", env!("CARGO_PKG_VERSION")));
            ui.label(
                RichText::new("Windows · macOS")
                    .size(12.0)
                    .color(self.colors.muted),
            );
        });
    }

    fn confirm(&mut self, ctx: &egui::Context) {
        if self.pending.is_none() {
            return;
        }
        let mut execute = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::unique("confirm-operation")).show(ctx, |ui| {
            ui.set_width(440.0);
            let (title, root) = match self.pending.as_ref() {
                Some(Pending::Job(kind, options)) => (self.task_label(*kind), &options.root),
                Some(Pending::AddSource(root, _)) => (self.tr(Text::AddSource), root),
                Some(Pending::RemoveSource(root, _, _)) => (self.tr(Text::RemoveSource), root),
                Some(Pending::ClearHistory(path)) => (self.tr(Text::ClearHistory), path),
                Some(Pending::RemoveUnfinishedHistory(path)) => {
                    (self.tr(Text::RemoveUnfinishedHistory), path)
                }
                None => return,
            };
            ui.heading(title);
            ui.add_space(8.0);
            ui.label(self.tr(match self.pending.as_ref() {
                Some(Pending::ClearHistory(_)) => Text::ConfirmClearHistory,
                Some(Pending::RemoveUnfinishedHistory(_)) => Text::ConfirmRemoveUnfinishedHistory,
                _ => Text::ConfirmWrite,
            }));
            ui.label(
                RichText::new(root.display().to_string())
                    .small()
                    .color(self.colors.muted),
            );
            match self.pending.as_ref() {
                Some(Pending::Job(kind, options)) => {
                    if matches!(
                        kind,
                        JobKind::Update
                            | JobKind::QuickUpdate
                            | JobKind::AdBlockOnly
                            | JobKind::GitPull
                            | JobKind::GitPush
                    ) {
                        ui.label(self.tr(Text::ConfirmNetwork));
                    }
                    if matches!(
                        kind,
                        JobKind::GitPull | JobKind::GitCommit | JobKind::GitPush
                    ) {
                        ui.label(self.tr(Text::GitSafety));
                        if let Some(repository) = &self.repository {
                            ui.monospace(&repository.branch);
                            ui.label(if *kind == JobKind::GitPush {
                                &repository.push_remote
                            } else {
                                &repository.remote
                            });
                        }
                    }
                    if *kind == JobKind::GitCommit {
                        ui.label(options.commit_message.as_deref().unwrap_or_default());
                        egui::ScrollArea::vertical()
                            .max_height(160.0)
                            .show(ui, |ui| {
                                for path in &options.paths {
                                    ui.monospace(path);
                                }
                            });
                    }
                }
                Some(Pending::AddSource(_, source)) => {
                    ui.label(&source.name);
                    ui.label(&source.url);
                    ui.label(self.tr(Text::SourceTrust));
                }
                Some(Pending::RemoveSource(_, _, name)) => {
                    ui.label(name);
                }
                Some(Pending::ClearHistory(_)) => {}
                Some(Pending::RemoveUnfinishedHistory(_)) => {}
                None => {}
            }
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                execute = ui.button(self.tr(Text::Confirm)).clicked();
                cancel = ui.button(self.tr(Text::Cancel)).clicked();
            });
        });
        if execute {
            if let Some(pending) = self.pending.take() {
                match pending {
                    Pending::Job(kind, options) => self.start(kind, options),
                    Pending::AddSource(root, source) => {
                        let result = super::job::acquire_write_lock(&root)
                            .and_then(|_lock| custom_sources::add(&root, source));
                        self.source_result(result);
                    }
                    Pending::RemoveSource(root, category, name) => {
                        let result = super::job::acquire_write_lock(&root)
                            .and_then(|_lock| custom_sources::remove(&root, category, &name));
                        self.source_result(result);
                    }
                    Pending::ClearHistory(_) => self.refresh_history(true),
                    Pending::RemoveUnfinishedHistory(_) => self.remove_unfinished_history(),
                }
            }
        } else if cancel || response.should_close() {
            self.pending = None;
        }
    }

    fn source_result(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.reload();
                self.notice = Some(Text::SourceSaved);
                self.source_name.clear();
                self.source_url.clear();
            }
            Err(error) => self.error = Some(error),
        }
    }
}

impl eframe::App for HubApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_workspace_loader(ctx);
        self.poll_repository_loader(ctx);
        self.poll_history(ctx);
        let dark = ctx.global_style().visuals.dark_mode;
        if self.appearance.as_ref() != Some(&(self.draft.clone(), dark)) {
            theme::apply(ctx, &self.draft);
            self.colors = theme::palette(ctx.global_style().visuals.dark_mode, self.draft.accent);
            self.appearance = Some((self.draft.clone(), ctx.global_style().visuals.dark_mode));
        }
        if !self.job_finished_handled && !self.job_active() {
            self.job_finished_handled = true;
            self.refresh_repository_after_load = true;
            self.reload();
            self.refresh_history(false);
        }
        if self.active() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if self.job_active() && ctx.input(|input| input.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.notice = Some(Text::Running);
        }
        let dropped = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .first()
                .map(|file| file.path().to_path_buf())
        });
        if !self.active()
            && let Some(path) = dropped
        {
            self.root_input = path.to_string_lossy().into_owned();
            self.reload();
        }
        if !ctx.egui_wants_keyboard_input() && self.pending.is_none() {
            for (key, page) in [
                (egui::Key::Num1, Page::Overview),
                (egui::Key::Num2, Page::Catalog),
                (egui::Key::Num3, Page::Tasks),
                (egui::Key::Num4, Page::Repository),
            ] {
                if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, key)) {
                    self.page = page;
                    if page == Page::Repository && self.repository.is_none() {
                        self.refresh_repository();
                    }
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("status-bar")
            .frame(
                egui::Frame::new()
                    .fill(self.colors.surface)
                    .inner_margin(10),
            )
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if self.workspace_loader.is_some() {
                        ui.colored_label(self.colors.accent, self.tr(Text::LoadingWorkspace));
                    } else if let Some(job) = &self.job {
                        let snapshot = job.summary();
                        let (label, color) = self.outcome(&snapshot);
                        ui.colored_label(color, label);
                        ui.weak(format!(
                            "{} {:02}:{:02}",
                            self.tr(Text::Elapsed),
                            snapshot.elapsed.as_secs() / 60,
                            snapshot.elapsed.as_secs() % 60
                        ));
                    } else {
                        ui.colored_label(
                            if self.workspace_ready() {
                                self.colors.success
                            } else {
                                self.colors.warning
                            },
                            self.tr(if self.workspace_ready() {
                                Text::WorkspaceReady
                            } else {
                                Text::WorkspaceMissing
                            }),
                        );
                    }
                    if let Some(notice) = self.notice {
                        ui.separator();
                        ui.label(self.tr(notice));
                    }
                });
            });
        let compact = ui.available_width() < 880.0;
        if compact {
            egui::Panel::top("compact-navigation").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (page, key) in [
                        (Page::Overview, Text::Overview),
                        (Page::Catalog, Text::Library),
                        (Page::Tasks, Text::Tasks),
                        (Page::Repository, Text::Repository),
                        (Page::Settings, Text::Settings),
                        (Page::Support, Text::Help),
                    ] {
                        if ui
                            .selectable_label(self.page == page, self.tr(key))
                            .clicked()
                        {
                            self.page = page;
                            if page == Page::Repository && self.repository.is_none() {
                                self.refresh_repository();
                            }
                        }
                    }
                });
            });
        } else {
            egui::Panel::left("navigation")
                .exact_size(192.0)
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(self.colors.surface)
                        .inner_margin(18),
                )
                .show(ui, |ui| self.navigation(ui));
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(self.colors.canvas)
                    .inner_margin(if compact { 16 } else { 28 }),
            )
            .show(ui, |ui| {
                if let Some(error) = self.error.clone() {
                    egui::Frame::new()
                        .fill(self.colors.surface)
                        .stroke(egui::Stroke::new(1.0, self.colors.error))
                        .inner_margin(12)
                        .corner_radius(8)
                        .show(ui, |ui| {
                            ui.colored_label(self.colors.error, error);
                            if ui.small_button(self.tr(Text::Close)).clicked() {
                                self.error = None;
                            }
                        });
                    ui.add_space(12.0);
                }
                ui.push_id(("page", self.page as u8), |ui| match self.page {
                    Page::Overview => self.overview_page(ui),
                    Page::Catalog => self.catalog_page(ui),
                    Page::Tasks => self.tasks_page(ui),
                    Page::Repository => self.repository_page(ui),
                    Page::Settings => self.settings_page(ui),
                    Page::Support => self.support_page(ui),
                });
            });
        self.confirm(ui.ctx());
    }
}

pub fn run(mut args: impl Iterator<Item = String>) -> ExitCode {
    let explicit = match args.next().as_deref() {
        Some("--worker") => return super::worker::run(args),
        Some("--root") => match args.next() {
            Some(root) if args.next().is_none() => Some(PathBuf::from(root)),
            _ => return usage(false),
        },
        Some("--help" | "-h") => return usage(true),
        Some(_) => return usage(false),
        None => None,
    };
    let (path, preferences, error) = match settings::preferences_path() {
        Ok(path) => match settings::load(&path) {
            Ok(preferences) => (Some(path), preferences.unwrap_or_default(), None),
            Err(error) => (Some(path), Preferences::default(), Some(error)),
        },
        Err(error) => (None, Preferences::default(), Some(error)),
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(preferences.window.dimensions())
            .with_min_inner_size([740.0, 540.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    match eframe::run_native(
        "Script Hub",
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx, &preferences);
            Ok(Box::new(HubApp::new(explicit, preferences, path, error)))
        }),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Cannot start Script Hub: {error}");
            ExitCode::FAILURE
        }
    }
}

fn usage(success: bool) -> ExitCode {
    println!(
        "Usage: script-hub-gui [--root <repository>]\nNative desktop support: Windows and macOS. Linux GUI development is frozen."
    );
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn catalog_copy_dispatches_the_exact_multiline_clipboard_payload() {
        let ctx = egui::Context::default();
        let mut app = HubApp::new(None, Preferences::default(), None, None);
        let payload = "https://example.test/one.sgmodule\nhttps://example.test/two.sgmodule";
        let mut output = ctx.run_ui(Default::default(), |ui| app.copy(ui.ctx(), payload));
        assert!(output.platform_output.commands.iter().any(|command| {
            matches!(command, egui::OutputCommand::CopyText(value) if value == payload)
        }));
        assert!(app.job.is_none());
        output.textures_delta.clear();
    }

    #[test]
    fn pages_render_in_supported_languages_themes_and_compact_layout() {
        use super::super::settings::{Language, Theme};
        let ctx = egui::Context::default();
        for language in [Language::En, Language::ZhCn, Language::Ja] {
            for theme in [Theme::Light, Theme::Dark] {
                for size in [[740.0, 540.0], [1160.0, 760.0]] {
                    let preferences = Preferences {
                        language,
                        theme,
                        ..Default::default()
                    };
                    let mut app = HubApp::new(None, preferences.clone(), None, None);
                    theme::apply(&ctx, &preferences);
                    app.colors = theme::palette(theme == Theme::Dark, preferences.accent);
                    app.catalog =
                        Catalog::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."), false)
                            .unwrap();
                    for (page, catalog_tab, task_tab, support_tab) in [
                        (
                            Page::Overview,
                            CatalogTab::Modules,
                            TaskTab::Current,
                            SupportTab::Help,
                        ),
                        (
                            Page::Catalog,
                            CatalogTab::Modules,
                            TaskTab::Current,
                            SupportTab::Help,
                        ),
                        (
                            Page::Catalog,
                            CatalogTab::Rules,
                            TaskTab::Current,
                            SupportTab::Help,
                        ),
                        (
                            Page::Catalog,
                            CatalogTab::Sources,
                            TaskTab::Current,
                            SupportTab::Help,
                        ),
                        (
                            Page::Tasks,
                            CatalogTab::Modules,
                            TaskTab::Current,
                            SupportTab::Help,
                        ),
                        (
                            Page::Tasks,
                            CatalogTab::Modules,
                            TaskTab::History,
                            SupportTab::Help,
                        ),
                        (
                            Page::Repository,
                            CatalogTab::Modules,
                            TaskTab::Current,
                            SupportTab::Help,
                        ),
                        (
                            Page::Settings,
                            CatalogTab::Modules,
                            TaskTab::Current,
                            SupportTab::Help,
                        ),
                        (
                            Page::Support,
                            CatalogTab::Modules,
                            TaskTab::Current,
                            SupportTab::Help,
                        ),
                        (
                            Page::Support,
                            CatalogTab::Modules,
                            TaskTab::Current,
                            SupportTab::About,
                        ),
                        (
                            Page::Support,
                            CatalogTab::Modules,
                            TaskTab::Current,
                            SupportTab::Licenses,
                        ),
                        (
                            Page::Support,
                            CatalogTab::Modules,
                            TaskTab::Current,
                            SupportTab::Privacy,
                        ),
                    ] {
                        app.catalog_tab = catalog_tab;
                        app.task_tab = task_tab;
                        app.support_tab = support_tab;
                        let output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    size.into(),
                                )),
                                ..Default::default()
                            },
                            |ui| {
                                egui::CentralPanel::default().show(ui, |ui| match page {
                                    Page::Overview => app.overview_page(ui),
                                    Page::Catalog => app.catalog_page(ui),
                                    Page::Tasks => app.tasks_page(ui),
                                    Page::Repository => app.repository_page(ui),
                                    Page::Settings => app.settings_page(ui),
                                    Page::Support => app.support_page(ui),
                                });
                            },
                        );
                        assert!(!output.shapes.is_empty(), "empty page: {page:?}");
                        output.drop_without_applying_deltas();
                    }
                    assert_eq!(app.preferences, preferences);
                    assert!(app.pending.is_none() && app.job.is_none());
                }
            }
        }
    }

    #[test]
    fn catalog_tabs_keep_accesskit_nodes_across_pointer_navigation() {
        fn draw(
            ctx: &egui::Context,
            app: &mut HubApp,
            frame: &mut eframe::Frame,
            events: Vec<egui::Event>,
        ) -> egui::FullOutput {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1160.0, 760.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| eframe::App::ui(app, ui, frame),
            );
            output.textures_delta.clear();
            output
        }

        fn label_center(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
            let update = output
                .platform_output
                .accesskit_update
                .as_ref()
                .expect("AccessKit update should be produced");
            let (node_id, node) = update
                .nodes
                .iter()
                .find(|(_, node)| node.label() == Some(label))
                .unwrap_or_else(|| panic!("missing AccessKit node {label:?}"));
            let mut reachable = std::collections::HashSet::new();
            let mut pending = vec![update.tree.as_ref().unwrap().root];
            while let Some(id) = pending.pop() {
                if reachable.insert(id)
                    && let Some((_, node)) =
                        update.nodes.iter().find(|(candidate, _)| *candidate == id)
                {
                    pending.extend(node.children());
                }
            }
            assert!(
                reachable.contains(node_id),
                "unreachable AccessKit node {label:?}"
            );
            let bounds = node.bounds().expect("tab node should have bounds");
            egui::pos2(
                ((bounds.x0 + bounds.x1) * 0.5) as f32,
                ((bounds.y0 + bounds.y1) * 0.5) as f32,
            )
        }

        fn click(
            ctx: &egui::Context,
            app: &mut HubApp,
            frame: &mut eframe::Frame,
            position: egui::Pos2,
        ) -> egui::FullOutput {
            let button = |pressed| egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            };
            draw(
                ctx,
                app,
                frame,
                vec![egui::Event::PointerMoved(position), button(true)],
            );
            draw(
                ctx,
                app,
                frame,
                vec![egui::Event::PointerMoved(position), button(false)],
            )
        }

        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut frame = eframe::Frame::_new_kittest();
        let mut app = HubApp::new(None, Preferences::default(), None, None);
        app.catalog =
            Catalog::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."), false).unwrap();
        app.page = Page::Catalog;

        let modules = draw(&ctx, &mut app, &mut frame, Vec::new());
        let rules_position = label_center(&modules, text(app.draft.language, Text::Rules));
        assert!(
            modules
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.label()
                    == Some(text(app.draft.language, Text::CopySelectedCdn)))
        );
        modules.drop_without_applying_deltas();

        app.catalog.modules.truncate(3);
        app.catalog.modules[0].name = "Visible module".into();
        app.catalog.modules[0].merged_into.clear();
        app.catalog.modules[1].name = "Visible retired module".into();
        app.catalog.modules[1].merged_into = "PROMAX".into();
        app.catalog.modules[2].name = "Hidden module".into();
        app.module_search = "Visible".into();
        app.selected_modules.insert(2);
        let filtered = draw(&ctx, &mut app, &mut frame, Vec::new());
        let select = label_center(&filtered, text(app.draft.language, Text::SelectVisible));
        filtered.drop_without_applying_deltas();
        click(&ctx, &mut app, &mut frame, select).drop_without_applying_deltas();
        assert_eq!(app.selected_modules, BTreeSet::from([0, 2]));

        let rules = click(&ctx, &mut app, &mut frame, rules_position);
        assert!(matches!(app.catalog_tab, CatalogTab::Rules));
        label_center(&rules, text(app.draft.language, Text::CopyRuleSetCdn));
        assert!(
            rules
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.label()
                    == Some(text(app.draft.language, Text::CopyRuleSetCdn)))
        );
        let modules_position = label_center(&rules, text(app.draft.language, Text::Modules));
        rules.drop_without_applying_deltas();

        let modules = click(&ctx, &mut app, &mut frame, modules_position);
        assert!(matches!(app.catalog_tab, CatalogTab::Modules));
        assert!(
            modules
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.label()
                    == Some(text(app.draft.language, Text::CopySelectedCdn)))
        );
        modules.drop_without_applying_deltas();

        app.catalog.rules.truncate(3);
        app.catalog.rules[0].name = "Visible domain rules".into();
        app.catalog.rules[1].name = "Visible IP rules".into();
        app.catalog.rules[2].name = "Hidden rules".into();
        app.rule_search = "Visible".into();
        app.selected_rules.insert(2);
        app.catalog_tab = CatalogTab::Rules;
        let filtered = draw(&ctx, &mut app, &mut frame, Vec::new());
        let select = label_center(&filtered, text(app.draft.language, Text::SelectVisible));
        filtered.drop_without_applying_deltas();
        click(&ctx, &mut app, &mut frame, select).drop_without_applying_deltas();
        assert_eq!(app.selected_rules, BTreeSet::from([0, 1, 2]));

        app.rule_search = "no matching rows".into();
        let empty = draw(&ctx, &mut app, &mut frame, Vec::new());
        let select = label_center(&empty, text(app.draft.language, Text::SelectVisible));
        empty.drop_without_applying_deltas();
        let selected = click(&ctx, &mut app, &mut frame, select);
        assert_eq!(app.selected_rules, BTreeSet::from([0, 1, 2]));
        let copy = label_center(&selected, text(app.draft.language, Text::CopySelectedCdn));
        selected.drop_without_applying_deltas();
        let copied = click(&ctx, &mut app, &mut frame, copy);
        let expected = super::super::catalog::selected_rule_urls(
            &app.catalog.rules,
            &app.selected_rules,
            false,
        );
        assert!(copied.platform_output.commands.iter().any(|command| {
            matches!(command, egui::OutputCommand::CopyText(value) if value == &expected)
        }));
        copied.drop_without_applying_deltas();
    }

    fn wait_for_workspace(app: &mut HubApp) -> bool {
        let result = app
            .workspace_loader
            .as_ref()
            .expect("workspace loader should be active")
            .receiver
            .recv_timeout(Duration::from_secs(10));
        let result = match result {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return false,
        };
        if let Some(loader) = app.workspace_loader.take() {
            app.finish_workspace_loader(loader, result);
        }
        true
    }

    fn wait_for_repository(app: &mut HubApp) -> bool {
        let result = app
            .repository_loader
            .as_ref()
            .expect("repository loader should be active")
            .receiver
            .recv_timeout(Duration::from_secs(10));
        let result = match result {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => return false,
        };
        if let Some(loader) = app.repository_loader.take() {
            app.finish_repository_loader(loader, result);
        }
        true
    }

    struct Workspace(PathBuf);

    impl Workspace {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "script-hub-desktop-app-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            for directory in [
                "crates/processor",
                "modules/source",
                "rulesets/Sources",
                "rulesets/AdBlock",
                "rulesets/RULE-SET",
            ] {
                fs::create_dir_all(root.join(directory)).unwrap();
            }
            fs::write(root.join("crates/processor/Cargo.toml"), "").unwrap();
            fs::write(root.join("rulesets/Sources/adblock_whitelist.list"), "").unwrap();
            fs::write(
                root.join("rulesets/AdBlock/catalog.json"),
                r#"{"shards":[]}"#,
            )
            .unwrap();
            fs::write(
                root.join("rulesets/RULE-SET/Example.list"),
                "DOMAIN,example.com\n",
            )
            .unwrap();
            Self(root)
        }
    }

    impl Drop for Workspace {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn invalid_root_clears_catalog_and_surfaces_error() {
        let mut app = HubApp::new(
            Some("nonexistent-script-hub-root".into()),
            Preferences::default(),
            None,
            None,
        );
        assert!(wait_for_workspace(&mut app));
        assert!(app.error.is_some());
        assert!(app.catalog.modules.is_empty() && app.loaded_root.is_none());
    }

    #[test]
    fn malformed_custom_sources_do_not_hide_valid_local_catalog() {
        let workspace = Workspace::new();
        let manifest = workspace.0.join("modules/source/custom_sources.json");
        fs::write(&manifest, "{broken").unwrap();
        let mut app = HubApp::new(
            Some(workspace.0.clone()),
            Preferences::default(),
            None,
            None,
        );
        assert!(wait_for_workspace(&mut app));
        assert!(app.workspace_ready());
        assert_eq!(app.catalog.rules.len(), 1);
        assert!(app.error.is_some());
        assert!(app.sources.is_empty());
        assert_eq!(fs::read_to_string(manifest).unwrap(), "{broken");
    }

    #[test]
    fn initial_explicit_root_is_session_baseline_without_saving_preferences() {
        let workspace = Workspace::new();
        let mut app = HubApp::new(
            Some(workspace.0.clone()),
            Preferences::default(),
            None,
            None,
        );
        assert!(wait_for_workspace(&mut app));
        assert!(app.workspace_ready());
        assert_eq!(app.draft, app.preferences);
        assert_eq!(
            app.loaded_root.unwrap(),
            workspace.0.canonicalize().unwrap()
        );
        assert!(app.settings_path.is_none());
    }

    #[test]
    fn constructor_returns_while_workspace_loader_is_still_blocked() {
        let workspace = Workspace::new();
        let (entered_sender, entered_receiver) = mpsc::sync_channel(1);
        let (release_sender, release_receiver) = mpsc::sync_channel(1);
        let mut app = HubApp::new_with_loader(
            Some(workspace.0.clone()),
            Preferences::default(),
            None,
            None,
            move |root, platform| {
                entered_sender.send(()).map_err(|error| error.to_string())?;
                release_receiver
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(|error| format!("test loader gate failed: {error}"))?;
                load_workspace(root, platform)
            },
        );

        assert!(
            entered_receiver
                .recv_timeout(Duration::from_secs(2))
                .is_ok()
        );
        assert!(matches!(
            app.workspace_loader.as_ref().unwrap().receiver.try_recv(),
            Err(TryRecvError::Empty)
        ));
        release_sender.send(()).unwrap();
        assert!(wait_for_workspace(&mut app));
        assert!(app.workspace_ready());
    }

    #[test]
    fn repository_refresh_is_backgrounded_and_ignores_a_stale_root_result() {
        let workspace = Workspace::new();
        let root = workspace.0.canonicalize().unwrap();
        let mut app = HubApp::new(None, Preferences::default(), None, None);
        app.loaded_root = Some(root.clone());
        app.root_input = root.to_string_lossy().into_owned();
        app.selected_files.insert("selected.list".to_string());
        let (entered_sender, entered_receiver) = mpsc::sync_channel(1);
        let (release_sender, release_receiver) = mpsc::sync_channel(1);

        app.refresh_repository_with(move |_| {
            entered_sender.send(()).map_err(|error| error.to_string())?;
            release_receiver
                .recv_timeout(Duration::from_secs(5))
                .map_err(|error| format!("test reader gate failed: {error}"))?;
            Err("stale repository result".to_string())
        });

        assert!(
            entered_receiver
                .recv_timeout(Duration::from_secs(2))
                .is_ok()
        );
        assert!(matches!(
            app.repository_loader.as_ref().unwrap().receiver.try_recv(),
            Err(TryRecvError::Empty)
        ));
        assert!(app.active());
        assert!(app.repository.is_none());

        app.root_input = "/different/workspace".to_string();
        release_sender.send(()).unwrap();
        assert!(wait_for_repository(&mut app));
        assert!(app.repository.is_none() && app.repository_error.is_none());
        assert!(app.selected_files.contains("selected.list"));
    }

    #[test]
    fn default_launch_does_not_probe_saved_or_ancestor_folders() {
        let preferences = Preferences {
            root: "/unavailable/protected/project".to_string(),
            ..Preferences::default()
        };
        let app = HubApp::new_with_loader(None, preferences, None, None, |_, _| {
            panic!("default launch must not attempt workspace access")
        });
        assert!(app.workspace_loader.is_none());
        assert!(!app.workspace_ready());
        assert_eq!(app.root_input, "/unavailable/protected/project");
    }

    #[test]
    fn opt_in_reopens_saved_workspace_without_overriding_explicit_root() {
        let workspace = Workspace::new();
        let preferences = Preferences {
            root: workspace.0.to_string_lossy().into_owned(),
            reopen_workspace: true,
            ..Preferences::default()
        };
        let mut app = HubApp::new(None, preferences, None, None);
        assert!(wait_for_workspace(&mut app));
        assert!(app.workspace_ready());
    }
}
