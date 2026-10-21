use super::card;
use crate::desktop::app::HubApp;
use crate::desktop::i18n::Text;
use crate::desktop::job::JobKind;
use eframe::egui::{self, RichText};

impl HubApp {
    pub(crate) fn tasks_page(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("tasks-page")
            .show(ui, |ui| self.tasks_content(ui));
    }

    fn tasks_content(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.tr(Text::Tasks));
        ui.label(self.tr(Text::NoAutomaticGit));
        ui.add_space(18.0);
        let ready = self.workspace_ready() && !self.active();
        card(ui, self.colors, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(self.tr(Text::TaskMode));
                egui::ComboBox::from_id_salt("task-kind")
                    .selected_text(self.task_label(self.task_kind))
                    .show_ui(ui, |ui| {
                        for kind in [
                            JobKind::Validate,
                            JobKind::RefreshCatalog,
                            JobKind::Update,
                            JobKind::QuickUpdate,
                            JobKind::GamingOnly,
                        ] {
                            let label = self.task_label(kind);
                            ui.selectable_value(&mut self.task_kind, kind, label);
                        }
                    });
                if ui
                    .add_enabled(ready, egui::Button::new(self.tr(Text::RunTask)))
                    .clicked()
                {
                    self.queue_job(self.task_kind);
                }
            });
            ui.add_space(8.0);
            ui.weak(self.tr(match self.task_kind {
                JobKind::Update | JobKind::QuickUpdate | JobKind::GitPull | JobKind::GitPush => {
                    Text::NetworkRequired
                }
                _ => Text::LocalOnly,
            }));
            if self.draft.singbox.trim().is_empty() {
                ui.weak(self.tr(Text::SingboxOptional));
            }
        });
        ui.add_space(18.0);
        let Some(snapshot) = self.job.as_ref().map(|job| job.snapshot()) else {
            ui.label(self.tr(Text::NoTask));
            return;
        };
        card(ui, self.colors, |ui| {
            let (label, color) = self.outcome(&snapshot);
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(color, RichText::new(label).size(22.0).strong());
                ui.weak(format!(
                    "{} {:02}:{:02}",
                    self.tr(Text::Elapsed),
                    snapshot.elapsed.as_secs() / 60,
                    snapshot.elapsed.as_secs() % 60
                ));
            });
            ui.add_space(10.0);
            ui.label(format!(
                "{} · {}",
                self.tr(Text::CurrentStage),
                snapshot.stage.map_or(
                    self.tr(if snapshot.running {
                        Text::StagePrepare
                    } else {
                        Text::Ready
                    }),
                    |stage| self.stage_label(stage)
                )
            ));
            if let Some(progress) = snapshot.progress {
                if let (Some(completed), Some(total)) = (progress.completed, progress.total) {
                    if total > 0 {
                        ui.add(
                            egui::ProgressBar::new(completed as f32 / total as f32)
                                .text(format!("{completed} / {total}")),
                        );
                    }
                } else if snapshot.running {
                    ui.weak(self.tr(Text::ProgressUnknown));
                }
            } else if snapshot.running {
                ui.weak(self.tr(Text::ProgressUnknown));
            }
            if let Some(code) = snapshot.exit_code {
                ui.weak(format!("Exit {code}"));
            }
            if let Some(line) = snapshot.logs.back() {
                ui.add_space(8.0);
                ui.label(line);
            }
        });
        ui.add_space(14.0);
        ui.horizontal_wrapped(|ui| {
            let label = self.tr(if self.show_logs {
                Text::HideLogs
            } else {
                Text::ShowLogs
            });
            if ui.button(label).clicked() {
                self.show_logs = !self.show_logs;
            }
            if ui.button(self.tr(Text::CopyLogs)).clicked() {
                self.copy(
                    ui.ctx(),
                    &snapshot.logs.iter().cloned().collect::<Vec<_>>().join("\n"),
                );
            }
            if ui.button(self.tr(Text::ClearLogs)).clicked()
                && let Some(job) = &self.job
            {
                job.clear_logs();
            }
        });
        if self.show_logs {
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .id_salt("task-log")
                .max_height(300.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for line in &snapshot.logs {
                        let color = if line.contains("[ERROR]") || line.contains("[GUI ERROR]") {
                            self.colors.error
                        } else if line.contains("[WARN]") {
                            self.colors.warning
                        } else {
                            self.colors.muted
                        };
                        ui.add(
                            egui::Label::new(RichText::new(line).monospace().color(color)).wrap(),
                        );
                    }
                });
        }
    }
}
