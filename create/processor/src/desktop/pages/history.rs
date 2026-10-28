use super::card;
use crate::desktop::app::{HubApp, Pending};
use crate::desktop::i18n::{Text, text};
use eframe::egui::{self, RichText};

impl HubApp {
    pub(crate) fn history_page(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        let has_completed = self
            .history_records
            .iter()
            .any(|record| record.outcome != "running");
        let has_unfinished = self
            .history_records
            .iter()
            .any(|record| record.outcome == "running");
        ui.label(tr(Text::HistoryIntro));
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.history_search)
                    .hint_text(tr(Text::HistorySearch)),
            );
            if ui
                .add_enabled(
                    self.history_loader.is_none(),
                    egui::Button::new(tr(Text::RefreshStatus)),
                )
                .clicked()
            {
                self.refresh_history(false);
            }
            if ui
                .add_enabled(
                    !self.active() && self.history_loader.is_none() && has_completed,
                    egui::Button::new(tr(Text::ClearHistory)),
                )
                .clicked()
                && let Some(path) = self.history_path()
            {
                self.pending = Some(Pending::ClearHistory(path));
            }
            if ui
                .add_enabled(
                    !self.active() && self.history_loader.is_none() && has_unfinished,
                    egui::Button::new(tr(Text::RemoveUnfinishedHistory)),
                )
                .clicked()
                && let Some(path) = self.history_path()
            {
                self.pending = Some(Pending::RemoveUnfinishedHistory(path));
            }
        });
        if self.history_loader.is_some() {
            ui.label(tr(Text::HistoryLoading));
        }
        if let Some(error) = &self.history_error {
            ui.colored_label(self.colors.error, error);
        }
        ui.add_space(12.0);
        let search = self.history_search.to_lowercase();
        let mut copied = None;
        egui::ScrollArea::vertical()
            .id_salt("task-history")
            .show(ui, |ui| {
                if self.history_records.is_empty() {
                    ui.label(tr(Text::HistoryEmpty));
                }
                for record in &self.history_records {
                    let key = match record.kind.as_str() {
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
                    };
                    let (outcome, color) = match record.outcome.as_str() {
                        "success" => (Text::Succeeded, self.colors.success),
                        "warnings" => (Text::Warnings, self.colors.warning),
                        "failed" => (Text::Failed, self.colors.error),
                        _ => (Text::Unfinished, self.colors.muted),
                    };
                    if !search.is_empty()
                        && !format!("{} {} {}", tr(key), tr(outcome), record.kind)
                            .to_lowercase()
                            .contains(&search)
                    {
                        continue;
                    }
                    card(ui, self.colors, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(tr(key)).strong());
                            ui.colored_label(color, tr(outcome));
                        });
                        let started = chrono::DateTime::from_timestamp(record.started_at, 0)
                            .map(|date| {
                                date.with_timezone(&chrono::Local)
                                    .format("%Y-%m-%d %H:%M:%S")
                                    .to_string()
                            })
                            .unwrap_or_else(|| record.started_at.to_string());
                        let details = format!(
                            "{} · {}\n{} · {:.1}s\n{} · {}",
                            tr(Text::StartedAt),
                            started,
                            tr(Text::Elapsed),
                            record.elapsed_ms as f64 / 1000.0,
                            tr(Text::ExitCode),
                            record
                                .exit_code
                                .map_or_else(|| "—".into(), |code| code.to_string())
                        );
                        ui.weak(&details);
                        if ui.small_button(tr(Text::CopySummary)).clicked() {
                            copied = Some(format!("{} · {}\n{}", tr(key), tr(outcome), details));
                        }
                        egui::CollapsingHeader::new(tr(Text::Logs))
                            .id_salt(record.id)
                            .show(ui, |ui| {
                                if record.logs.is_empty() {
                                    ui.weak(tr(Text::NoSavedLogs));
                                }
                                for line in &record.logs {
                                    ui.add(
                                        egui::Label::new(RichText::new(line).monospace()).wrap(),
                                    );
                                }
                            });
                    });
                    ui.add_space(10.0);
                }
            });
        if let Some(summary) = copied {
            self.copy(ui.ctx(), &summary);
        }
    }
}
