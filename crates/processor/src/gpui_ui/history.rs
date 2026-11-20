//! Local task history, backed by the same store as the maintenance worker.

use super::{Palette, button, heading, input::TextInput, panel};
use crate::desktop::i18n::{Text, text};
use crate::runtime::history::{HistoryRecord, HistoryStore};
use crate::runtime::settings::Language;
use gpui::{
    AppContext as _, ClipboardItem, Context, Entity, FontWeight, Window, div, prelude::*, px,
};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Read,
    RemoveFinished,
    RemoveUnfinished,
}

impl Action {
    fn label(self) -> Text {
        match self {
            Self::Read => Text::RefreshStatus,
            Self::RemoveFinished => Text::ClearHistory,
            Self::RemoveUnfinished => Text::RemoveUnfinishedHistory,
        }
    }

    fn notice(self) -> Option<Text> {
        match self {
            Self::Read => None,
            Self::RemoveFinished => Some(Text::HistoryCleared),
            Self::RemoveUnfinished => Some(Text::UnfinishedHistoryRemoved),
        }
    }
}

fn read_or_change(path: &Path, action: Action) -> Result<Vec<HistoryRecord>, String> {
    match path.symlink_metadata() {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
        Ok(_) => {}
    }
    let store = HistoryStore::open(path)?;
    match action {
        Action::Read => store.recent(100),
        Action::RemoveFinished => store.clear(),
        Action::RemoveUnfinished => store.clear_unfinished(),
    }
}

pub(super) struct TaskHistory {
    path: Option<PathBuf>,
    language: Language,
    search: Entity<TextInput>,
    records: Vec<HistoryRecord>,
    selected: Option<i64>,
    busy: Option<Action>,
    pending: Option<Action>,
    refresh_pending: bool,
    task_running: bool,
    notice: Option<Text>,
    error: Option<String>,
}

impl TaskHistory {
    pub(super) fn new(path: Option<PathBuf>, language: Language, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new(text(language, Text::HistorySearch), false, cx));
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        Self {
            path,
            language,
            search,
            records: Vec::new(),
            selected: None,
            busy: None,
            pending: None,
            refresh_pending: false,
            task_running: false,
            notice: None,
            error: None,
        }
    }

    pub(super) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        self.language = language;
        self.search.update(cx, |input, cx| {
            input.set_placeholder(text(language, Text::HistorySearch), cx);
        });
        cx.notify();
    }

    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            self.refresh_pending = true;
        } else {
            self.start(Action::Read, cx);
        }
    }

    pub(super) fn task_running(&mut self, running: bool, cx: &mut Context<Self>) {
        self.task_running = running;
        if running {
            self.pending = None;
        }
        cx.notify();
    }

    pub(super) fn dismiss_confirmation(&mut self) {
        self.pending = None;
    }

    pub(super) fn is_writing(&self) -> bool {
        self.busy.is_some_and(|action| action != Action::Read)
    }

    fn tr(&self, key: Text) -> &'static str {
        text(self.language, key)
    }

    fn start(&mut self, action: Action, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        if self.busy.is_some()
            || (action != Action::Read && (self.task_running || self.error.is_some()))
        {
            return;
        }
        self.busy = Some(action);
        self.pending = None;
        self.notice = None;
        self.error = None;
        let task = cx
            .background_executor()
            .spawn(async move { read_or_change(&path, action) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |model, cx| {
                model.busy = None;
                match result {
                    Ok(records) => {
                        model.selected = model
                            .selected
                            .filter(|id| records.iter().any(|entry| entry.id == *id));
                        model.records = records;
                        model.notice = action.notice();
                    }
                    Err(error) => model.error = Some(error),
                }
                if std::mem::take(&mut model.refresh_pending) {
                    model.refresh(cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

fn task_label(kind: &str) -> Text {
    match kind {
        "validate" => Text::Validate,
        "refresh_catalog" => Text::RefreshCatalog,
        "update" => Text::FullUpdate,
        "quick_update" => Text::QuickUpdate,
        "gaming_only" => Text::GamingUpdate,
        "adblock" => Text::AdBlockUpdate,
        "srs" => Text::RebuildSrs,
        "git_pull" => Text::Pull,
        "git_commit" => Text::Commit,
        "git_push" => Text::Push,
        _ => Text::Tasks,
    }
}

fn outcome_label(outcome: &str) -> Text {
    match outcome {
        "success" => Text::Succeeded,
        "warnings" => Text::Warnings,
        "failed" => Text::Failed,
        _ => Text::Unfinished,
    }
}

impl Render for TaskHistory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::for_window(window, cx);
        let page = div().flex().flex_col().gap_4().child(heading(
            self.tr(Text::History),
            self.tr(Text::HistoryIntro),
            p,
        ));
        if self.path.is_none() {
            return page.child(
                panel(p)
                    .p_5()
                    .child(self.tr(Text::HistorySettingsUnavailable)),
            );
        }
        let query = self.search.read(cx).value().trim().to_lowercase();
        let filtered = self
            .records
            .iter()
            .filter(|record| {
                format!(
                    "{} {} {}",
                    self.tr(task_label(&record.kind)),
                    self.tr(outcome_label(&record.outcome)),
                    record.kind
                )
                .to_lowercase()
                .contains(&query)
            })
            .collect::<Vec<_>>();
        let controls = if self.busy.is_none() {
            let can_remove = !self.task_running && self.error.is_none();
            let actions = [
                (
                    Action::RemoveFinished,
                    self.records
                        .iter()
                        .any(|record| record.outcome != "running"),
                ),
                (
                    Action::RemoveUnfinished,
                    self.records
                        .iter()
                        .any(|record| record.outcome == "running"),
                ),
            ];
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_wrap()
                .child(
                    button("history-refresh", self.tr(Text::RefreshStatus), p, false)
                        .on_click(cx.listener(|view, _, _, cx| view.refresh(cx))),
                )
                .children(
                    actions
                        .into_iter()
                        .filter(|&(_, applicable)| can_remove && applicable)
                        .map(|(action, _)| {
                            button(
                                ("history-remove", action as usize),
                                self.tr(action.label()),
                                p,
                                false,
                            )
                            .on_click(cx.listener(
                                move |view, _, _, cx| {
                                    view.pending = Some(action);
                                    cx.notify();
                                },
                            ))
                        }),
                )
        } else {
            div()
        };
        let mut page = page
            .child(self.search.clone())
            .child(controls)
            .when(self.busy.is_some(), |page| {
                page.child(self.tr(Text::HistoryLoading))
            })
            .when_some(self.error.as_ref(), |page, error| {
                page.child(
                    panel(p)
                        .p_4()
                        .child(self.tr(Text::HistoryRefreshFailed))
                        .child(div().text_sm().text_color(p.muted).child(error.clone())),
                )
            })
            .when_some(self.notice, |page, notice| {
                page.child(div().text_color(p.accent).child(self.tr(notice)))
            });
        if let Some(action) = self.pending {
            page = page.child(
                panel(p)
                    .p_4()
                    .gap_3()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.tr(action.label())),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(p.muted)
                            .child(self.tr(match action {
                                Action::RemoveFinished => Text::ConfirmClearHistory,
                                Action::RemoveUnfinished => Text::ConfirmRemoveUnfinishedHistory,
                                Action::Read => Text::HistoryIntro,
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                button("history-cancel", self.tr(Text::Cancel), p, false).on_click(
                                    cx.listener(|view, _, _, cx| {
                                        view.pending = None;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                button("history-confirm", self.tr(action.label()), p, true)
                                    .on_click(
                                        cx.listener(move |view, _, _, cx| view.start(action, cx)),
                                    ),
                            ),
                    ),
            );
        }
        if filtered.is_empty() && self.error.is_none() && self.busy.is_none() {
            page = page.child(panel(p).p_5().text_color(p.muted).child(self.tr(
                if self.records.is_empty() {
                    Text::HistoryEmpty
                } else {
                    Text::HistoryNoMatches
                },
            )));
        }
        page.children(filtered.into_iter().map(|record| {
            let id = record.id;
            let task = self.tr(task_label(&record.kind));
            let outcome = self.tr(outcome_label(&record.outcome));
            let time = chrono::DateTime::from_timestamp(record.started_at, 0)
                .map(|date| {
                    date.with_timezone(&chrono::Local)
                        .format("%Y-%m-%d %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_else(|| record.started_at.to_string());
            let details = format!(
                "{}  ·  {:.1}s  ·  {} {}",
                time,
                record.elapsed_ms as f64 / 1000.0,
                self.tr(Text::ExitCode),
                record
                    .exit_code
                    .map_or_else(|| "—".to_owned(), |code| code.to_string())
            );
            let summary = format!("{task} · {outcome}\n{details}");
            let selected = self.selected == Some(id);
            panel(p)
                .p_4()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(task))
                        .child(div().text_sm().text_color(p.accent).child(outcome)),
                )
                .child(div().text_sm().text_color(p.muted).child(details))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .flex_wrap()
                        .child(
                            button(
                                ("history-summary", id as usize),
                                self.tr(Text::CopySummary),
                                p,
                                false,
                            )
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(summary.clone()))
                            }),
                        )
                        .when(!record.logs.is_empty(), |row| {
                            row.child(
                                button(
                                    ("history-logs", id as usize),
                                    self.tr(Text::Logs),
                                    p,
                                    selected,
                                )
                                .on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        view.selected = if selected { None } else { Some(id) };
                                        cx.notify();
                                    },
                                )),
                            )
                        }),
                )
                .when(selected && !record.logs.is_empty(), |panel| {
                    panel.child(
                        div()
                            .id(("history-lines", id as usize))
                            .overflow_y_scroll()
                            .max_h(px(300.))
                            .p_3()
                            .bg(p.subtle)
                            .rounded_md()
                            .text_xs()
                            .children(
                                record
                                    .logs
                                    .iter()
                                    .map(|line| div().py_1().child(line.clone())),
                            ),
                    )
                })
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visiting_empty_history_does_not_create_a_database() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("script-hub-history-{}-{nonce}", std::process::id()));
        let path = directory.join("history.sqlite3");
        assert!(read_or_change(&path, Action::Read).unwrap().is_empty());
        assert!(!directory.exists());
    }
}
