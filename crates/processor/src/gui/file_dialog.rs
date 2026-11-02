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
    ExportBackup,
    ExportNodes,
    ChooseEngine,
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
        let linked_file = linked
            && action == FileAction::ChooseEngine
            && std::fs::metadata(entry.path()).is_ok_and(|target| target.is_file());
        if (linked && !linked_file) || (!linked && !(metadata.is_dir() || metadata.is_file())) {
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
        });
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
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
            filename: match action {
                FileAction::ExportBackup => "relay-backup.json",
                FileAction::ExportNodes => "nodes.list",
                _ => "",
            }
            .to_owned(),
            selected: None,
            listing: None,
            filtered: Vec::new(),
            worker: None,
            pending: None,
            error: None,
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
                .filter_map(|(index, entry)| entry.lowercase.contains(&query).then_some(index))
                .collect()
        });
    }

    fn selection(&self) -> Result<PathBuf, Text> {
        match self.action {
            FileAction::Import | FileAction::ChooseEngine => {
                let path = self.selected.as_ref().ok_or(Text::FilePickerInvalid)?;
                Ok(path.clone())
            }
            FileAction::ExportBackup | FileAction::ExportNodes => {
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
        egui::Modal::new(egui::Id::unique("builtin-file-picker")).show(ctx, |ui| {
            ui.set_width(600.0);
            ui.heading(tr(Text::FilePickerTitle));
            ui.weak(tr(Text::FilePickerPrivacy));
            ui.horizontal(|ui| {
                let label = ui.label(tr(Text::FilePickerPath));
                let field = ui.add(
                    egui::TextEdit::singleline(&mut self.path_text)
                        .char_limit(32768)
                        .desired_width(380.0),
                );
                let _ = field.clone().labelled_by(label.id);
                if ui.button(tr(Text::FilePickerGo)).clicked()
                    || (field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                {
                    navigate = Some(PathBuf::from(&self.path_text));
                }
            });
            ui.horizontal(|ui| {
                if ui.button(tr(Text::FilePickerUp)).clicked() {
                    navigate = self
                        .directory
                        .as_ref()
                        .and_then(|path| path.parent())
                        .map(Path::to_owned);
                }
                if ui.button(tr(Text::FilePickerHome)).clicked() {
                    navigate = home();
                    if navigate.is_none() {
                        self.error = Some(Text::FilePickerError);
                    }
                }
                if ui.button(tr(Text::FilePickerRefresh)).clicked() {
                    navigate = self.directory.clone();
                }
            });
            ui.horizontal(|ui| {
                let label = ui.label(tr(Text::FilePickerSearch));
                let field = ui.add(egui::TextEdit::singleline(&mut self.search).char_limit(512));
                if field.changed() {
                    self.filter();
                }
                let _ = field.labelled_by(label.id);
            });
            if self.pending.is_some() {
                ui.label(tr(Text::FilePickerBusy));
            } else if self.worker.is_some() {
                ui.label(tr(Text::FilePickerLoading));
            } else if let Some(listing) = &self.listing {
                if listing.truncated {
                    ui.weak(tr(Text::FilePickerTruncated));
                }
                if self.filtered.is_empty() {
                    ui.label(tr(Text::FilePickerEmpty));
                }
                egui::ScrollArea::vertical()
                    .id_salt("builtin-file-list")
                    .max_height(300.0)
                    .show_rows(ui, 26.0, self.filtered.len(), |ui, range| {
                        for &index in &self.filtered[range] {
                            let entry = &listing.entries[index];
                            let label = format!(
                                "{}{}",
                                if entry.directory { "📁 " } else { "" },
                                entry.display
                            );
                            let clicked = ui.selectable_label(
                                self.selected.as_ref() == Some(&entry.path),
                                label,
                            );
                            if clicked.clicked() {
                                if entry.directory {
                                    navigate = Some(entry.path.clone());
                                } else {
                                    self.selected = Some(entry.path.clone());
                                }
                            }
                        }
                    });
            }
            if let Some(error) = self.error {
                ui.colored_label(ui.visuals().error_fg_color, tr(error));
            }
            if matches!(
                self.action,
                FileAction::ExportBackup | FileAction::ExportNodes
            ) {
                ui.horizontal(|ui| {
                    let label = ui.label(tr(Text::FilePickerName));
                    let field =
                        ui.add(egui::TextEdit::singleline(&mut self.filename).char_limit(255));
                    if field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                        confirm = self.worker.is_none()
                            && self.pending.is_none()
                            && self.listing.is_some();
                    }
                    let _ = field.labelled_by(label.id);
                });
            }
            ui.horizontal(|ui| {
                if ui.button(tr(Text::Cancel)).clicked() {
                    response = PickerResponse::Cancelled;
                }
                let ready = self.worker.is_none()
                    && self.pending.is_none()
                    && self.listing.is_some()
                    && (!matches!(self.action, FileAction::Import | FileAction::ChooseEngine)
                        || self.selected.is_some());
                if ui
                    .add_enabled(
                        ready,
                        egui::Button::new(tr(
                            if matches!(self.action, FileAction::Import | FileAction::ChooseEngine)
                            {
                                Text::FilePickerOpen
                            } else {
                                Text::FilePickerSave
                            },
                        )),
                    )
                    .clicked()
                {
                    confirm = true;
                }
            });
        });
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            return PickerResponse::Cancelled;
        }
        if matches!(response, PickerResponse::Cancelled) {
            return response;
        }
        if let Some(path) = navigate {
            self.request(path);
        }
        if confirm {
            match self.selection() {
                Ok(path) => return PickerResponse::Selected(path),
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
    }

    #[test]
    fn save_name_is_one_safe_component() {
        for name in ["", ".", "..", "../x", "a/b", "a\\b", "a\nb"] {
            assert!(!valid_filename(name), "{name:?}");
        }
        assert!(valid_filename("nodes.list"));
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
        let mut picker = Picker::new(FileAction::ExportNodes, Some(&dir));
        picker.directory = Some(dir.clone());
        assert_eq!(picker.selection().unwrap(), dir.join("nodes.list"));
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
        };
        picker.filter();
        assert_eq!(picker.filtered, [0]);
        assert_eq!(picker.listing.as_ref().unwrap().entries[0].path, path);
        picker.search = "missing".into();
        picker.filter();
        assert!(picker.filtered.is_empty());
    }
}
