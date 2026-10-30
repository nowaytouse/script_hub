use super::app::HubApp;
use super::history::{HistoryRecord, HistoryStore};
use super::i18n::Text;
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(super) struct HistoryLoader {
    receiver: Receiver<Result<Vec<HistoryRecord>, String>>,
    worker: JoinHandle<()>,
    completion_notice: Option<Text>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HistoryLoadAction {
    Read,
    ClearCompleted,
    RemoveUnfinished,
}

impl HubApp {
    pub(super) fn history_path(&self) -> Option<PathBuf> {
        self.settings_path
            .as_ref()
            .map(|path| path.with_file_name("history.sqlite3"))
    }

    pub(super) fn refresh_history(&mut self, clear: bool) {
        self.load_history(if clear {
            HistoryLoadAction::ClearCompleted
        } else {
            HistoryLoadAction::Read
        });
    }

    pub(super) fn remove_unfinished_history(&mut self) {
        self.load_history(HistoryLoadAction::RemoveUnfinished);
    }

    fn load_history(&mut self, action: HistoryLoadAction) {
        if self.history_loader.is_some() || (action != HistoryLoadAction::Read && self.active()) {
            return;
        }
        let Some(path) = self.history_path() else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        match thread::Builder::new()
            .name("script-hub-history".into())
            .spawn(move || {
                let result = (|| {
                    if action == HistoryLoadAction::Read {
                        match std::fs::symlink_metadata(&path) {
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                                return Ok(Vec::new());
                            }
                            Err(error) => {
                                return Err(format!("cannot inspect local history: {error}"));
                            }
                            Ok(_) => {}
                        }
                    }
                    let store = HistoryStore::open(&path)?;
                    match action {
                        HistoryLoadAction::Read => {}
                        HistoryLoadAction::ClearCompleted => store.clear()?,
                        HistoryLoadAction::RemoveUnfinished => store.clear_unfinished()?,
                    }
                    store.recent(100)
                })();
                let _ = sender.send(result);
            }) {
            Ok(worker) => {
                self.history_loader = Some(HistoryLoader {
                    receiver,
                    worker,
                    completion_notice: match action {
                        HistoryLoadAction::Read => None,
                        HistoryLoadAction::ClearCompleted => Some(Text::HistoryCleared),
                        HistoryLoadAction::RemoveUnfinished => Some(Text::UnfinishedHistoryRemoved),
                    },
                })
            }
            Err(error) => self.history_error = Some(format!("cannot read task history: {error}")),
        }
    }

    pub(super) fn poll_history(&mut self, ctx: &egui::Context) {
        let Some(loader) = self.history_loader.as_ref() else {
            return;
        };
        let result = match loader.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Disconnected) => {
                Some(Err("history reader stopped unexpectedly".into()))
            }
            Err(TryRecvError::Empty) => None,
        };
        if let Some(mut result) = result {
            let Some(loader) = self.history_loader.take() else {
                return;
            };
            if loader.worker.join().is_err() {
                result = Err("history reader stopped unexpectedly".into());
            }
            match result {
                Ok(records) => {
                    self.history_records = records;
                    self.history_error = None;
                    if let Some(notice) = loader.completion_notice {
                        self.notice = Some(notice);
                    }
                }
                Err(error) => self.history_error = Some(error),
            }
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
