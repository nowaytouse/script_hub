use crate::gui::i18n::{Text, text};
use crate::runtime::settings::Language;
use eframe::egui;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

const MAX_ENTRIES: usize = 4096;
static LISTING_ACTIVE: AtomicUsize = AtomicUsize::new(0);

struct ListingPermit;

impl Drop for ListingPermit {
    fn drop(&mut self) {
        LISTING_ACTIVE.fetch_sub(1, Ordering::Release);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileAction {
    Import,
    ImportPlan,
    ExportPlan,
    ExportBackup,
    ExportEncrypted,
    ExportNodes(crate::relay::export::OutputFormat),
    ChooseEngine,
}

impl FileAction {
    pub(super) fn is_save(self) -> bool {
        matches!(
            self,
            Self::ExportBackup | Self::ExportEncrypted | Self::ExportNodes(_) | Self::ExportPlan
        )
    }
}

pub enum PickerResponse {
    Pending,
    Cancelled,
    Selected(PathBuf),
}

struct Entry {
    name: OsString,
    display: String,
    lowercase: String,
    path: PathBuf,
    directory: bool,
    size: u64,
}

struct Listing {
    entries: Vec<Entry>,
    truncated: bool,
}

struct Worker {
    path: PathBuf,
    receiver: Receiver<Result<Listing, ()>>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

pub struct Picker {
    pub action: FileAction,
    directory: Option<PathBuf>,
    path_text: String,
    search: String,
    filename: String,
    selected: Option<PathBuf>,
    listing: Option<Listing>,
    filtered: Vec<usize>,
    worker: Option<Worker>,
    pending: Option<PathBuf>,
    error: Option<Text>,
    history: Vec<PathBuf>,
    history_index: usize,
    show_hidden: bool,
    all_files: bool,
    editing_path: bool,
    focus_initialized: bool,
    return_focus: Option<egui::Id>,
}

fn home() -> Option<PathBuf> {
    [std::env::var_os("HOME"), std::env::var_os("USERPROFILE")]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .find(|path| path.is_absolute())
}

fn read_directory(path: &Path, action: FileAction, cancel: &AtomicBool) -> Result<Listing, ()> {
    let mut prefix = PathBuf::new();
    for component in path.components() {
        if cancel.load(Ordering::Acquire) {
            return Err(());
        }
        prefix.push(component.as_os_str());
        if prefix.is_absolute() {
            let metadata = std::fs::symlink_metadata(&prefix).map_err(|_| ())?;
            if crate::vault::is_link(&metadata) {
                return Err(());
            }
        }
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| ())?;
    if !metadata.is_dir() || crate::vault::is_link(&metadata) {
        return Err(());
    }
    let mut entries = Vec::new();
    let mut truncated = false;
    for (count, entry) in std::fs::read_dir(path).map_err(|_| ())?.enumerate() {
        if cancel.load(Ordering::Acquire) {
            return Err(());
        }
        if count == MAX_ENTRIES {
            truncated = true;
            break;
        }
        let entry = entry.map_err(|_| ())?;
        let metadata = std::fs::symlink_metadata(entry.path()).map_err(|_| ())?;
        let linked = crate::vault::is_link(&metadata);
        let linked_target = if linked && action == FileAction::ChooseEngine {
            std::fs::metadata(entry.path())
                .ok()
                .filter(|target| target.is_file())
        } else {
            None
        };
        if (linked && linked_target.is_none())
            || (!linked && !(metadata.is_dir() || metadata.is_file()))
        {
            continue;
        }
        let name = entry.file_name();
        let display = name.to_string_lossy().into_owned();
        let lowercase = display.to_lowercase();
        entries.push(Entry {
            name,
            display,
            lowercase,
            path: entry.path(),
            directory: metadata.is_dir(),
            size: linked_target
                .as_ref()
                .map_or_else(|| metadata.len(), std::fs::Metadata::len),
        });
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then_with(|| a.lowercase.cmp(&b.lowercase))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(Listing { entries, truncated })
}

fn valid_filename(name: &str) -> bool {
    !name.is_empty()
        && !name.chars().any(char::is_control)
        && matches!(
            (
                Path::new(name).components().next(),
                Path::new(name).components().nth(1)
            ),
            (Some(Component::Normal(_)), None)
        )
        && !name.contains(['/', '\\'])
}

fn format_size(bytes: u64) -> String {
    let (unit, label) = if bytes >= 1024 * 1024 {
        (1024 * 1024, "MiB")
    } else if bytes >= 1024 {
        (1024, "KiB")
    } else {
        return format!("{bytes} B");
    };
    format!("{}.{:01} {label}", bytes / unit, bytes % unit * 10 / unit)
}

pub(super) fn validate_engine(path: PathBuf) -> Result<PathBuf, ()> {
    let path = path.canonicalize().map_err(|_| ())?;
    if !path.is_file() {
        return Err(());
    }
    #[cfg(windows)]
    if !path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
    {
        return Err(());
    }
    Ok(path)
}

impl Picker {
    pub fn new(action: FileAction, initial: Option<&Path>) -> Self {
        let directory = initial
            .filter(|path| path.is_absolute())
            .map(Path::to_owned)
            .or_else(home);
        let mut picker = Self {
            action,
            path_text: directory
                .as_ref()
                .map_or_else(String::new, |path| path.display().to_string()),
            directory: None,
            search: String::new(),
            filename: if let FileAction::ExportNodes(format) = action {
                format!("nodes.{}", format.extension())
            } else {
                match action {
                    FileAction::ExportBackup => "relay-backup.json",
                    FileAction::ExportEncrypted => "relay-backup.age",
                    FileAction::ExportPlan => "operations.json",
                    _ => "",
                }
                .to_owned()
            },
            selected: None,
            listing: None,
            filtered: Vec::new(),
            worker: None,
            pending: None,
            error: None,
            history: directory.iter().cloned().collect(),
            history_index: 0,
            show_hidden: false,
            all_files: false,
            editing_path: false,
            focus_initialized: false,
            return_focus: None,
        };
        if let Some(path) = directory {
            picker.directory = Some(path.clone());
            picker.pending = Some(path);
        } else {
            picker.error = Some(Text::FilePickerError);
        }
        picker
    }

    fn request(&mut self, path: PathBuf) {
        if !path.is_absolute() {
            self.error = Some(Text::FilePickerInvalid);
            return;
        }
        if self.directory.as_ref() != Some(&path) {
            self.search.clear();
        }
        self.selected = None;
        self.listing = None;
        self.filtered.clear();
        self.error = None;
        self.path_text = path.display().to_string();
        self.directory = Some(path.clone());
        if let Some(worker) = &self.worker {
            worker.cancel.store(true, Ordering::Release);
            self.pending = Some(path);
        } else {
            self.pending = None;
            self.start(path);
        }
    }

    pub fn directory(&self) -> Option<&Path> {
        if self.listing.is_some() && self.worker.is_none() && self.pending.is_none() {
            self.directory.as_deref()
        } else {
            None
        }
    }

    fn navigate(&mut self, path: PathBuf) {
        if self.directory.as_ref() != Some(&path) && path.is_absolute() {
            self.history.truncate(self.history_index + 1);
            self.history.push(path.clone());
            self.history_index = self.history.len() - 1;
        }
        self.request(path);
    }

    fn start(&mut self, path: PathBuf) {
        if LISTING_ACTIVE
            .try_update(Ordering::Acquire, Ordering::Relaxed, |count| {
                (count < 2).then_some(count + 1)
            })
            .is_err()
        {
            self.pending = Some(path);
            return;
        }
        let permit = ListingPermit;
        let (sender, receiver) = mpsc::sync_channel(1);
        let target = path.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let action = self.action;
        match std::thread::Builder::new()
            .name("script-hub-file-list".into())
            .spawn(move || {
                let _permit = permit;
                let _ = sender.send(read_directory(&target, action, &worker_cancel));
            }) {
            Ok(_) => {
                self.worker = Some(Worker {
                    path,
                    receiver,
                    cancel,
                })
            }
            Err(_) => self.error = Some(Text::FilePickerError),
        }
    }

    fn poll(&mut self) {
        let Some(worker) = &self.worker else {
            if let Some(next) = self.pending.take() {
                self.start(next);
            }
            return;
        };
        match worker.receiver.try_recv() {
            Ok(result) => {
                let path = worker.path.clone();
                self.worker = None;
                if let Some(next) = self.pending.take() {
                    self.start(next);
                } else if self.directory.as_ref() == Some(&path) {
                    match result {
                        Ok(listing) => {
                            self.listing = Some(listing);
                            self.filter();
                        }
                        Err(()) => self.error = Some(Text::FilePickerError),
                    }
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.worker = None;
                if let Some(next) = self.pending.take() {
                    self.start(next);
                } else {
                    self.error = Some(Text::FilePickerError);
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    fn filter(&mut self) {
        let query = self.search.to_lowercase();
        self.filtered = self.listing.as_ref().map_or_else(Vec::new, |listing| {
            listing
                .entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| {
                    let visible = (self.show_hidden || !entry.display.starts_with('.'))
                        && (entry.directory
                            || !matches!(self.action, FileAction::Import | FileAction::ImportPlan)
                            || self.all_files
                            || entry.path.extension().is_none()
                            || entry
                                .path
                                .extension()
                                .and_then(|ext| ext.to_str())
                                .is_some_and(|ext| {
                                    if self.action == FileAction::ImportPlan {
                                        ext.eq_ignore_ascii_case("json")
                                    } else {
                                        ["json", "yaml", "yml", "conf", "list", "txt", "age"]
                                            .iter()
                                            .any(|allowed| ext.eq_ignore_ascii_case(allowed))
                                    }
                                }))
                        && entry.lowercase.contains(&query);
                    visible.then_some(index)
                })
                .collect()
        });
        if self.selected.as_ref().is_some_and(|selected| {
            !self.listing.as_ref().is_some_and(|listing| {
                self.filtered
                    .iter()
                    .any(|&index| listing.entries[index].path == *selected)
            })
        }) {
            self.selected = None;
        }
    }

    fn selection(&self) -> Result<PathBuf, Text> {
        match self.action {
            FileAction::Import | FileAction::ImportPlan | FileAction::ChooseEngine => {
                let path = self.selected.as_ref().ok_or(Text::FilePickerInvalid)?;
                Ok(path.clone())
            }
            FileAction::ExportBackup
            | FileAction::ExportEncrypted
            | FileAction::ExportNodes(_)
            | FileAction::ExportPlan => {
                if !valid_filename(&self.filename) {
                    return Err(Text::FilePickerInvalid);
                }
                Ok(self
                    .directory
                    .as_ref()
                    .ok_or(Text::FilePickerInvalid)?
                    .join(&self.filename))
            }
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, language: Language) -> PickerResponse {
        self.poll();
        if self.worker.is_some() || self.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        let tr = |key| text(language, key);
        let mut response = PickerResponse::Pending;
        let mut navigate = None;
        let mut confirm = false;
        let mut history_move = None;
        if !self.focus_initialized {
            self.return_focus = ctx.memory(|m| m.focused());
        }
        egui::Modal::new(egui::Id::unique("builtin-file-picker")).show(ctx, |ui| {
            let available = ctx.content_rect().size();
            let compact = available.y < 480.0;
            if compact {
                let spacing = ui.spacing_mut();
                spacing.item_spacing = egui::vec2(6.0, 6.0);
                spacing.button_padding = egui::vec2(8.0, 4.0);
                spacing.interact_size.y = 28.0;
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Heading, egui::FontId::proportional(20.0));
            }
            let search_id = ui.make_persistent_id("builtin-file-picker-search");
            let filename_id = ui.make_persistent_id("builtin-file-picker-filename");
            let path_id = ui.make_persistent_id("builtin-file-picker-path");
            if !self.focus_initialized {
                ui.memory_mut(|m| m.request_focus(search_id));
                self.focus_initialized = true;
            }
            let editing_text = ui.memory(|m| {
                m.has_focus(search_id) || m.has_focus(filename_id) || m.has_focus(path_id)
            });
            ui.set_width((available.x - 40.0).clamp(100.0, 850.0));
            ui.horizontal(|ui| {
                ui.heading(tr(Text::FilePickerTitle));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr(Text::FilePickerPath)).clicked() {
                        self.editing_path = true;
                        ui.memory_mut(|m| m.request_focus(path_id));
                    }
                });
            });
            ui.horizontal(|ui| {
                let back = ui
                    .add_enabled(self.history_index > 0, egui::Button::new("←"))
                    .on_hover_text(tr(Text::FilePickerBack));
                back.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        self.history_index > 0,
                        tr(Text::FilePickerBack),
                    )
                });
                if back.clicked()
                    || (!editing_text
                        && self.history_index > 0
                        && ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft)))
                {
                    history_move = Some(self.history_index - 1);
                }
                let forward = ui
                    .add_enabled(
                        self.history_index + 1 < self.history.len(),
                        egui::Button::new("→"),
                    )
                    .on_hover_text(tr(Text::FilePickerForward));
                forward.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::Role::Button,
                        self.history_index + 1 < self.history.len(),
                        tr(Text::FilePickerForward),
                    )
                });
                if forward.clicked()
                    || (!editing_text
                        && self.history_index + 1 < self.history.len()
                        && ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowRight)))
                {
                    history_move = Some(self.history_index + 1);
                }
                let up = ui.button("↑").on_hover_text(tr(Text::FilePickerUp));
                up.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, true, tr(Text::FilePickerUp))
                });
                if up.clicked() {
                    navigate = self
                        .directory
                        .as_ref()
                        .and_then(|p| p.parent())
                        .map(Path::to_owned);
                }
                let refresh = ui.button("↻").on_hover_text(tr(Text::FilePickerRefresh));
                refresh.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, true, tr(Text::FilePickerRefresh))
                });
                if refresh.clicked() {
                    navigate = self.directory.clone();
                }
                if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::L)) {
                    self.editing_path = true;
                    ui.memory_mut(|m| m.request_focus(path_id));
                }
                if self.editing_path {
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut self.path_text)
                            .id(path_id)
                            .char_limit(32768)
                            .desired_width(ui.available_width()),
                    );
                    field.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::TextInput,
                            true,
                            tr(Text::FilePickerPath),
                        )
                    });
                    if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        navigate = Some(PathBuf::from(&self.path_text));
                        self.editing_path = false;
                    }
                } else if let Some(directory) = &self.directory {
                    let mut crumbs: Vec<_> = directory.ancestors().take(3).collect();
                    crumbs.reverse();
                    let width = (ui.available_width() / crumbs.len().max(1) as f32 - 4.0).max(20.0);
                    for path in crumbs {
                        let name = path
                            .file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy();
                        let button = ui
                            .add_sized([width, 22.0], egui::Button::new(name.as_ref()).truncate())
                            .on_hover_text(path.display().to_string());
                        button.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Button, true, path.display())
                        });
                        if button.clicked() {
                            navigate = Some(path.to_owned());
                        }
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                let label = ui.label(tr(Text::FilePickerSearch));
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .id(search_id)
                        .char_limit(512)
                        .desired_width(180.0),
                );
                if field.changed() {
                    self.filter();
                }
                let _ = field.labelled_by(label.id);
                let mut changed = ui
                    .checkbox(&mut self.show_hidden, tr(Text::FilePickerHidden))
                    .changed();
                if matches!(self.action, FileAction::Import | FileAction::ImportPlan) {
                    changed |= ui
                        .checkbox(&mut self.all_files, tr(Text::FilePickerAllFiles))
                        .changed();
                }
                if changed {
                    self.filter();
                }
            });
            let extra_status = self.error.is_some()
                || self.pending.is_some()
                || self
                    .listing
                    .as_ref()
                    .is_some_and(|listing| listing.truncated);
            let list_height = (available.y - 280.0 - if extra_status { 40.0 } else { 0.0 })
                .clamp(if compact { 60.0 } else { 80.0 }, 450.0);
            ui.horizontal_top(|ui| {
                if available.x >= 500.0 {
                    ui.vertical(|ui| {
                        ui.set_width(120.0);
                        if let Some(base) = home() {
                            for (label, path) in [
                                (Text::FilePickerHome, base.clone()),
                                (Text::FilePickerDownloads, base.join("Downloads")),
                                (Text::FilePickerDocuments, base.join("Documents")),
                                (Text::FilePickerDesktop, base.join("Desktop")),
                            ] {
                                if ui
                                    .selectable_label(
                                        self.directory.as_ref() == Some(&path),
                                        tr(label),
                                    )
                                    .clicked()
                                {
                                    navigate = Some(path);
                                }
                            }
                        }
                        if let Some(root) =
                            self.directory.as_ref().and_then(|p| p.ancestors().last())
                            && ui.button(tr(Text::FilePickerRoot)).clicked()
                        {
                            navigate = Some(root.to_owned());
                        }
                    });
                    let (divider, _) =
                        ui.allocate_exact_size(egui::vec2(1.0, list_height), egui::Sense::hover());
                    ui.painter().line_segment(
                        [divider.center_top(), divider.center_bottom()],
                        ui.visuals().widgets.noninteractive.bg_stroke,
                    );
                }
                ui.vertical(|ui| {
                    ui.set_min_height(list_height);
                    let columns = ui.available_width() >= 360.0;
                    let (header, _) = ui.allocate_exact_size(
                        egui::vec2(
                            ui.available_width(),
                            ui.text_style_height(&egui::TextStyle::Body),
                        ),
                        egui::Sense::hover(),
                    );
                    let name_right = if columns { 160.0 } else { 6.0 };
                    ui.place(
                        egui::Rect::from_min_max(
                            header.min + egui::vec2(6.0, 0.0),
                            header.max - egui::vec2(name_right, 0.0),
                        ),
                        egui::Label::new(tr(Text::FilePickerName)).halign(egui::Align::Min),
                    );
                    if columns {
                        ui.place(
                            egui::Rect::from_min_max(
                                egui::pos2(header.right() - 155.0, header.top()),
                                egui::pos2(header.right() - 80.0, header.bottom()),
                            ),
                            egui::Label::new(tr(Text::FilePickerKind)).halign(egui::Align::Min),
                        );
                        ui.place(
                            egui::Rect::from_min_max(
                                egui::pos2(header.right() - 80.0, header.top()),
                                header.max - egui::vec2(6.0, 0.0),
                            ),
                            egui::Label::new(tr(Text::FilePickerSize)).halign(egui::Align::Max),
                        );
                    }
                    let row_height = ui.spacing().interact_size.y.max(26.0);
                    let list_id = ui.make_persistent_id("builtin-file-list-focus");
                    let list_rect = egui::Rect::from_min_size(
                        ui.next_widget_position(),
                        egui::vec2(ui.available_width(), list_height - 24.0),
                    );
                    let list_focus =
                        ui.interact(list_rect, list_id, egui::Sense::focusable_noninteractive());
                    list_focus.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::Role::List, true, tr(Text::FilePickerName))
                    });
                    let focused = list_focus.has_focus();
                    if focused {
                        ui.memory_mut(|m| {
                            m.set_focus_lock_filter(
                                list_id,
                                egui::EventFilter {
                                    vertical_arrows: true,
                                    ..Default::default()
                                },
                            );
                        });
                        ui.painter().rect_stroke(
                            list_rect,
                            2.0,
                            ui.visuals().selection.stroke,
                            egui::StrokeKind::Inside,
                        );
                    }
                    let mut scroll_to = None;
                    if focused && !self.filtered.is_empty() {
                        let current = self.listing.as_ref().and_then(|listing| {
                            self.filtered.iter().position(|&i| {
                                self.selected.as_ref() == Some(&listing.entries[i].path)
                            })
                        });
                        let next = ui.input(|i| {
                            if i.key_pressed(egui::Key::ArrowDown) {
                                Some(current.map_or(0, |n| (n + 1).min(self.filtered.len() - 1)))
                            } else if i.key_pressed(egui::Key::ArrowUp) {
                                Some(current.map_or(0, |n| n.saturating_sub(1)))
                            } else if i.key_pressed(egui::Key::Home) {
                                Some(0)
                            } else if i.key_pressed(egui::Key::End) {
                                Some(self.filtered.len() - 1)
                            } else {
                                None
                            }
                        });
                        if let Some(next) = next {
                            if let Some(listing) = &self.listing {
                                let entry = &listing.entries[self.filtered[next]];
                                self.selected = Some(entry.path.clone());
                                if !entry.directory
                                    && self.action.is_save()
                                    && let Some(name) = entry.name.to_str()
                                {
                                    self.filename = name.to_owned();
                                }
                            }
                            scroll_to = Some(next);
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && let Some(entry) = self.listing.as_ref().and_then(|l| {
                                l.entries
                                    .iter()
                                    .find(|e| self.selected.as_ref() == Some(&e.path))
                            })
                        {
                            if entry.directory {
                                navigate = Some(entry.path.clone());
                            } else {
                                confirm = true;
                            }
                        }
                    }
                    let mut area = egui::ScrollArea::vertical()
                        .id_salt("builtin-file-list")
                        .min_scrolled_height(list_height - 24.0)
                        .max_height(list_height - 24.0);
                    if let Some(row) = scroll_to {
                        area = area.vertical_scroll_offset(
                            row as f32 * (row_height + ui.spacing().item_spacing.y),
                        );
                    }
                    area.show_rows(ui, row_height, self.filtered.len(), |ui, range| {
                        if let Some(listing) = &self.listing {
                            for row in range {
                                let entry = &listing.entries[self.filtered[row]];
                                let kind = tr(if entry.directory {
                                    Text::FilePickerFolder
                                } else {
                                    Text::FilePickerFile
                                });
                                let size = if entry.directory {
                                    String::new()
                                } else {
                                    format_size(entry.size)
                                };
                                let r = ui.add_sized(
                                    [ui.available_width(), row_height],
                                    egui::Button::selectable(
                                        self.selected.as_ref() == Some(&entry.path),
                                        "",
                                    )
                                    .sense(egui::Sense::CLICK),
                                );
                                r.widget_info(|| {
                                    egui::WidgetInfo::selected(
                                        egui::Role::ListItem,
                                        true,
                                        self.selected.as_ref() == Some(&entry.path),
                                        &entry.display,
                                    )
                                });
                                let rect = r.rect;
                                let text_color = ui.visuals().text_color();
                                let font = egui::TextStyle::Body.resolve(ui.style());
                                let name_right = if columns {
                                    rect.right() - 160.0
                                } else {
                                    rect.right() - 4.0
                                };
                                ui.painter()
                                    .with_clip_rect(egui::Rect::from_min_max(
                                        rect.min,
                                        egui::pos2(name_right, rect.bottom()),
                                    ))
                                    .text(
                                        egui::pos2(rect.left() + 6.0, rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        &entry.display,
                                        font.clone(),
                                        text_color,
                                    );
                                if columns {
                                    ui.painter().text(
                                        egui::pos2(rect.right() - 155.0, rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        kind,
                                        font.clone(),
                                        text_color,
                                    );
                                    ui.painter().text(
                                        egui::pos2(rect.right() - 6.0, rect.center().y),
                                        egui::Align2::RIGHT_CENTER,
                                        size,
                                        font,
                                        text_color,
                                    );
                                }
                                if scroll_to == Some(row) {
                                    r.scroll_to_me(None);
                                }
                                if r.clicked() {
                                    ui.memory_mut(|m| m.request_focus(list_id));
                                    ui.ctx().request_repaint();
                                    self.selected = Some(entry.path.clone());
                                    if !entry.directory
                                        && self.action.is_save()
                                        && let Some(name) = entry.name.to_str()
                                    {
                                        self.filename = name.to_owned();
                                    }
                                }
                                if r.double_clicked() {
                                    if entry.directory {
                                        navigate = Some(entry.path.clone());
                                    } else {
                                        confirm = true;
                                    }
                                }
                            }
                        }
                    });
                });
            });
            let status = if let Some(error) = self.error {
                tr(error)
            } else if self.pending.is_some() {
                tr(Text::FilePickerBusy)
            } else if self.worker.is_some() {
                tr(Text::FilePickerLoading)
            } else if self.listing.as_ref().is_some_and(|l| l.truncated) {
                tr(Text::FilePickerTruncated)
            } else if self.filtered.is_empty() {
                tr(Text::FilePickerEmpty)
            } else {
                ""
            };
            ui.small(status);
            ui.horizontal(|ui| {
                let label = ui.label(tr(Text::FilePickerName));
                if self.action.is_save() {
                    let field = ui.add(
                        egui::TextEdit::singleline(&mut self.filename)
                            .id(filename_id)
                            .char_limit(255)
                            .desired_width(ui.available_width().min(200.0)),
                    );
                    if field.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && self.listing.is_some()
                        && self.worker.is_none()
                        && self.pending.is_none()
                    {
                        confirm = true;
                    }
                    let _ = field.labelled_by(label.id);
                } else {
                    let name = self
                        .selected
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
                    let item = ui
                        .add_sized(
                            [ui.available_width().max(0.0), 20.0],
                            egui::Label::new(&name).truncate(),
                        )
                        .on_hover_text(&name);
                    item.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let ready = self.worker.is_none()
                    && self.pending.is_none()
                    && self.listing.is_some()
                    && (self.action.is_save()
                        || self.listing.as_ref().is_some_and(|l| {
                            l.entries
                                .iter()
                                .any(|e| !e.directory && self.selected.as_ref() == Some(&e.path))
                        }));
                if ui
                    .add_enabled(
                        ready,
                        egui::Button::new(tr(if !self.action.is_save() {
                            Text::FilePickerOpen
                        } else {
                            Text::FilePickerSave
                        })),
                    )
                    .clicked()
                {
                    confirm = true;
                }
                if ui.button(tr(Text::Cancel)).clicked() {
                    response = PickerResponse::Cancelled;
                }
            });
        });
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            if let Some(id) = self.return_focus {
                ctx.memory_mut(|m| m.request_focus(id));
            }
            return PickerResponse::Cancelled;
        }
        if matches!(response, PickerResponse::Cancelled) {
            if let Some(id) = self.return_focus {
                ctx.memory_mut(|m| m.request_focus(id));
            }
            return response;
        }
        if let Some(index) = history_move {
            self.history_index = index;
            self.request(self.history[index].clone());
        } else if let Some(path) = navigate {
            self.navigate(path);
        }
        // Navigation starts a worker after the repaint check at the start of this frame.
        // Ensure its result (or a queued request waiting for a permit) is polled.
        if self.worker.is_some() || self.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if confirm {
            match self.selection() {
                Ok(path) => {
                    if let Some(id) = self.return_focus {
                        ctx.memory_mut(|m| m.request_focus(id));
                    }
                    return PickerResponse::Selected(path);
                }
                Err(error) => self.error = Some(error),
            }
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_does_not_start_a_directory_read() {
        let path = std::env::temp_dir().join("synthetic-picker-start");
        let picker = Picker::new(FileAction::Import, Some(&path));
        assert!(picker.worker.is_none() && picker.listing.is_none());
        assert_eq!(picker.pending.as_ref(), Some(&path));
        assert!(picker.directory().is_none());
    }

    #[test]
    fn path_navigation_schedules_poll_after_loaded_frame() {
        fn draw(
            ctx: &egui::Context,
            picker: &mut Picker,
            mut events: Vec<egui::Event>,
        ) -> egui::FullOutput {
            let modifiers = events
                .iter()
                .find_map(|event| match event {
                    egui::Event::Key { modifiers, .. } => Some(*modifiers),
                    _ => None,
                })
                .unwrap_or_default();
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let _ = picker.show(ui.ctx(), Language::En);
                },
            );
            output.textures_delta.clear();
            output
        }
        let ctx = egui::Context::default();
        let initial = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let target = initial.join("src");
        let mut picker = Picker::new(FileAction::Import, Some(&initial));
        picker.pending = None;
        picker.listing = Some(Listing {
            entries: Vec::new(),
            truncated: false,
        });
        let _ = draw(&ctx, &mut picker, vec![]);
        let _ = draw(
            &ctx,
            &mut picker,
            vec![egui::Event::Key {
                key: egui::Key::L,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    command: true,
                    ..Default::default()
                },
            }],
        );
        picker.path_text = target.display().to_string();
        let output = draw(
            &ctx,
            &mut picker,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            }],
        );
        assert_eq!(picker.directory.as_ref(), Some(&target));
        assert!(picker.worker.is_some() || picker.pending.is_some());
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT].repaint_delay
                <= Duration::from_millis(100)
        );
        // Deliver a synthetic completion without relying on filesystem timing or input events.
        let (sender, receiver) = mpsc::sync_channel(1);
        sender
            .send(Ok(Listing {
                entries: vec![Entry {
                    name: "nodes.conf".into(),
                    display: "nodes.conf".into(),
                    lowercase: "nodes.conf".into(),
                    path: target.join("nodes.conf"),
                    directory: false,
                    size: 1,
                }],
                truncated: false,
            }))
            .unwrap();
        picker.worker = Some(Worker {
            path: target.clone(),
            receiver,
            cancel: Arc::new(AtomicBool::new(false)),
        });
        picker.pending = None;
        let _ = draw(&ctx, &mut picker, vec![]);
        assert_eq!(picker.directory(), Some(target.as_path()));
        assert!(
            picker
                .listing
                .as_ref()
                .is_some_and(|listing| !listing.entries.is_empty())
        );
    }

    #[test]
    fn save_name_is_one_safe_component() {
        for name in ["", ".", "..", "../x", "a/b", "a\\b", "a\nb"] {
            assert!(!valid_filename(name), "{name:?}");
        }
        assert!(valid_filename("nodes.list"));
    }

    #[test]
    fn file_size_uses_readable_binary_units() {
        assert_eq!(
            [format_size(999), format_size(1536), format_size(1048576)],
            ["999 B", "1.5 KiB", "1.0 MiB"]
        );
    }

    #[test]
    fn directory_change_clears_search_but_refresh_keeps_it() {
        let dir = std::env::temp_dir().join("synthetic-picker-filter");
        let mut picker = Picker::new(FileAction::Import, Some(&dir));
        let (_sender, receiver) = mpsc::sync_channel(1);
        picker.worker = Some(Worker {
            path: dir.clone(),
            receiver,
            cancel: Arc::new(AtomicBool::new(false)),
        });
        picker.search = "folder".into();
        picker.request(dir.clone());
        assert_eq!(picker.search, "folder");
        picker.navigate(dir.join("folder"));
        assert!(picker.search.is_empty());
        assert!(picker.pending.is_some());
    }

    #[test]
    fn import_filter_keeps_folders_and_respects_toggles() {
        let mut picker = Picker::new(FileAction::Import, None);
        picker.listing = Some(Listing {
            truncated: false,
            entries: [
                ("folder", true),
                ("backup.JSON", false),
                ("notes.txt", false),
                ("source", false),
                ("surge.conf", false),
                ("nodes.list", false),
                ("mihomo.yaml", false),
                ("mihomo.yml", false),
                ("other.bin", false),
                (".secret.json", false),
            ]
            .into_iter()
            .map(|(name, directory)| Entry {
                name: name.into(),
                display: name.into(),
                lowercase: name.to_lowercase(),
                path: PathBuf::from(name),
                directory,
                size: 0,
            })
            .collect(),
        });
        picker.filter();
        assert_eq!(picker.filtered, [0, 1, 2, 3, 4, 5, 6, 7]);
        picker.all_files = true;
        picker.show_hidden = true;
        picker.filter();
        assert_eq!(picker.filtered, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        picker.selected = Some(PathBuf::from("backup.JSON"));
        picker.search = "backup".into();
        picker.filter();
        assert_eq!(picker.selected, Some(PathBuf::from("backup.JSON")));
        picker.search = "notes".into();
        picker.filter();
        assert!(picker.selected.is_none());
        assert!(picker.selection().is_err());
        picker.search.clear();
        picker.selected = Some(PathBuf::from(".secret.json"));
        picker.show_hidden = false;
        picker.filter();
        assert!(picker.selected.is_none());
        picker.selected = Some(PathBuf::from("other.bin"));
        picker.all_files = false;
        picker.filter();
        assert!(picker.selected.is_none());
    }

    #[test]
    fn compact_picker_bounds_and_keyboard_selection() {
        fn draw(
            ctx: &egui::Context,
            picker: &mut Picker,
            size: egui::Vec2,
            events: Vec<egui::Event>,
        ) -> egui::FullOutput {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let _ = picker.show(ui.ctx(), Language::En);
                },
            );
            output.textures_delta.clear();
            output
        }
        for size in [
            egui::vec2(640.0, 480.0) / 1.5,
            egui::vec2(740.0, 540.0) / 1.5,
            egui::vec2(800.0, 600.0),
            egui::vec2(1160.0, 760.0),
        ] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            crate::gui::theme::apply(&ctx, &crate::runtime::settings::Preferences::default());
            let dir = std::env::temp_dir().join("synthetic-picker-layout");
            let mut picker = Picker::new(FileAction::Import, Some(&dir));
            picker.pending = None;
            picker.listing = Some(Listing {
                truncated: false,
                entries: ["first.json", "second.json"]
                    .into_iter()
                    .map(|name| Entry {
                        name: name.into(),
                        display: name.into(),
                        lowercase: name.into(),
                        path: dir.join(name),
                        directory: false,
                        size: 1,
                    })
                    .collect(),
            });
            picker.filter();
            let mut nodes = std::collections::HashMap::new();
            for _ in 0..3 {
                let output = draw(&ctx, &mut picker, size, Vec::new());
                if let Some(update) = output.platform_output.accesskit_update.as_ref() {
                    nodes.extend(update.nodes.iter().map(|(id, node)| (*id, node.clone())));
                }
            }
            for label in [
                text(Language::En, Text::FilePickerTitle),
                text(Language::En, Text::Cancel),
                text(Language::En, Text::FilePickerOpen),
                "first.json",
            ] {
                let node = nodes
                    .iter()
                    .find(|(_, node)| {
                        node.label() == Some(label)
                            || (node.role() == egui::Role::Label && node.value() == Some(label))
                    })
                    .unwrap_or_else(|| panic!("missing {label}"));
                let bounds = node.1.bounds().expect("control bounds");
                assert!(
                    bounds.x0 >= 0.0
                        && bounds.y0 >= 0.0
                        && bounds.x1 <= size.x as f64
                        && bounds.y1 <= size.y as f64,
                    "{label}: {bounds:?} in {size:?}"
                );
            }
            let bounds = nodes
                .iter()
                .find(|(_, node)| node.label() == Some("first.json"))
                .unwrap()
                .1
                .bounds()
                .unwrap();
            let pos = egui::pos2(
                ((bounds.x0 + bounds.x1) / 2.0) as f32,
                ((bounds.y0 + bounds.y1) / 2.0) as f32,
            );
            let pointer = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            };
            let _ = draw(
                &ctx,
                &mut picker,
                size,
                vec![egui::Event::PointerMoved(pos), pointer(true)],
            );
            let _ = draw(&ctx, &mut picker, size, vec![pointer(false)]);
            let _ = draw(&ctx, &mut picker, size, Vec::new());
            let list_focus = ctx.memory(|m| m.focused());
            let _ = draw(
                &ctx,
                &mut picker,
                size,
                vec![egui::Event::Key {
                    key: egui::Key::ArrowDown,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::default(),
                }],
            );
            assert_eq!(
                picker.selected.as_deref(),
                Some(dir.join("second.json").as_path())
            );
            assert!(list_focus.is_some());
            assert_eq!(
                ctx.memory(|m| m.focused()),
                list_focus,
                "list must retain arrow-key focus"
            );
            picker
                .listing
                .as_mut()
                .unwrap()
                .entries
                .extend((2..200).map(|n| {
                    let name = format!("row-{n:03}.json");
                    Entry {
                        name: name.clone().into(),
                        display: name.clone(),
                        lowercase: name.clone(),
                        path: dir.join(name),
                        directory: false,
                        size: 1,
                    }
                }));
            picker.filter();
            for events in [
                vec![egui::Event::Key {
                    key: egui::Key::End,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::default(),
                }],
                Vec::new(),
                Vec::new(),
            ] {
                let output = draw(&ctx, &mut picker, size, events);
                if let Some(update) = output.platform_output.accesskit_update.as_ref() {
                    nodes.extend(update.nodes.iter().map(|(id, node)| (*id, node.clone())));
                }
            }
            assert_eq!(picker.selected.as_ref(), Some(&dir.join("row-199.json")));
            let last = nodes
                .values()
                .find(|node| node.label() == Some("row-199.json"))
                .expect("last row must be rendered after End")
                .bounds()
                .unwrap();
            assert!(last.y0 >= 0.0 && last.y1 <= f64::from(size.y));
            assert!(picker.worker.is_none() && picker.pending.is_none());
        }
    }

    #[test]
    fn missing_directory_is_not_an_empty_listing() {
        let path =
            std::env::temp_dir().join(format!("script-hub-no-picker-dir-{}", std::process::id()));
        assert!(read_directory(&path, FileAction::Import, &AtomicBool::new(false)).is_err());
    }

    #[test]
    fn engine_selection_requires_an_existing_file() {
        assert!(validate_engine(std::env::temp_dir()).is_err());
    }

    #[test]
    fn selection_is_structural_and_does_not_require_existing_destination() {
        let dir = std::env::temp_dir().join("synthetic-picker-no-io");
        let mut picker = Picker::new(
            FileAction::ExportNodes(crate::relay::export::OutputFormat::Original),
            Some(&dir),
        );
        picker.directory = Some(dir.clone());
        assert_eq!(picker.selection().unwrap(), dir.join("nodes.txt"));
        picker.filename = "../escape".into();
        assert!(picker.selection().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn engine_file_link_is_visible_only_for_engine_selection() {
        use std::os::unix::fs::symlink;
        let base = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "script-hub-picker-link-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&base).unwrap();
        let target = base.join("engine-target");
        let link = base.join("engine-link");
        std::fs::write(&target, b"synthetic").unwrap();
        symlink(&target, &link).unwrap();
        let cancel = AtomicBool::new(false);
        let engine = read_directory(&base, FileAction::ChooseEngine, &cancel).unwrap();
        let import = read_directory(&base, FileAction::Import, &cancel).unwrap();
        assert!(engine.entries.iter().any(|entry| entry.path == link));
        assert_eq!(
            engine
                .entries
                .iter()
                .find(|entry| entry.path == link)
                .unwrap()
                .size,
            9
        );
        assert!(!import.entries.iter().any(|entry| entry.path == link));
        assert_eq!(validate_engine(link.clone()).unwrap(), target);
        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(target).unwrap();
        std::fs::remove_dir(base).unwrap();
    }

    #[test]
    fn repeated_navigation_retains_only_latest_pending_request() {
        let (_sender, receiver) = mpsc::sync_channel(1);
        let mut picker = Picker {
            action: FileAction::Import,
            directory: None,
            path_text: String::new(),
            search: String::new(),
            filename: String::new(),
            selected: None,
            listing: None,
            filtered: Vec::new(),
            worker: Some(Worker {
                path: std::env::temp_dir(),
                receiver,
                cancel: Arc::new(AtomicBool::new(false)),
            }),
            pending: None,
            error: None,
            history: Vec::new(),
            history_index: 0,
            show_hidden: false,
            all_files: false,
            editing_path: false,
            focus_initialized: false,
            return_focus: None,
        };
        let first = std::env::temp_dir().join("first");
        let last = std::env::temp_dir().join("last");
        picker.request(first);
        picker.request(last.clone());
        assert_eq!(picker.pending.as_ref(), Some(&last));
        assert!(picker.worker.is_some());
    }

    #[test]
    fn search_cache_uses_display_text_without_changing_original_path() {
        let (_sender, receiver) = mpsc::sync_channel(1);
        let path = std::env::temp_dir().join("Node.conf");
        let mut picker = Picker {
            action: FileAction::Import,
            directory: None,
            path_text: String::new(),
            search: "node".into(),
            filename: String::new(),
            selected: None,
            listing: Some(Listing {
                entries: vec![Entry {
                    name: OsString::from("Node.conf"),
                    display: "Node.conf".into(),
                    lowercase: "node.conf".into(),
                    path: path.clone(),
                    directory: false,
                    size: 0,
                }],
                truncated: false,
            }),
            filtered: Vec::new(),
            worker: Some(Worker {
                path: std::env::temp_dir(),
                receiver,
                cancel: Arc::new(AtomicBool::new(false)),
            }),
            pending: None,
            error: None,
            history: Vec::new(),
            history_index: 0,
            show_hidden: false,
            all_files: false,
            editing_path: false,
            focus_initialized: false,
            return_focus: None,
        };
        picker.filter();
        assert_eq!(picker.filtered, [0]);
        assert_eq!(picker.listing.as_ref().unwrap().entries[0].path, path);
        picker.search = "missing".into();
        picker.filter();
        assert!(picker.filtered.is_empty());
    }
}
