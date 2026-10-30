use super::card;
use crate::desktop::app::HubApp;
use crate::desktop::i18n::{Text, text};
use crate::desktop::job::JobKind;
use eframe::egui::{self, RichText};

impl HubApp {
    pub(crate) fn repository_page(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("repository-page")
            .show(ui, |ui| self.repository_content(ui));
    }

    fn repository_content(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        ui.horizontal_wrapped(|ui| {
            ui.heading(tr(Text::Repository));
            if ui
                .add_enabled(!self.active(), egui::Button::new(tr(Text::RefreshStatus)))
                .clicked()
            {
                self.refresh_repository();
            }
        });
        ui.label(tr(Text::NoAutomaticGit));
        ui.add_space(16.0);
        if let Some(error) = &self.repository_error {
            ui.colored_label(self.colors.error, error);
        }
        let Some(repository) = self.repository.clone() else {
            if self.repository_loader.is_some() {
                ui.label(tr(Text::LoadingRepository));
            } else if self.repository_error.is_none() {
                ui.label(tr(Text::GitUnavailable));
            }
            return;
        };
        let ready = self.workspace_ready() && !self.active();
        card(ui, self.colors, |ui| {
            ui.label(
                RichText::new(format!("{} · {}", tr(Text::Branch), repository.branch)).strong(),
            );
            ui.label(format!("{} · {}", tr(Text::Remote), repository.remote));
            if repository.push_remote != repository.remote {
                ui.label(format!("{} · {}", tr(Text::Push), repository.push_remote));
            }
            ui.weak(format!(
                "{} {}  ·  {} {}",
                tr(Text::Ahead),
                repository.ahead,
                tr(Text::Behind),
                repository.behind
            ));
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        ready && repository.changes.is_empty(),
                        egui::Button::new(tr(Text::Pull)),
                    )
                    .clicked()
                {
                    self.queue_job(JobKind::GitPull);
                }
                if ui
                    .add_enabled(
                        ready && repository.changes.is_empty(),
                        egui::Button::new(tr(Text::Push)),
                    )
                    .clicked()
                {
                    self.queue_job(JobKind::GitPush);
                }
            });
            ui.weak(tr(Text::GitSafety));
        });
        ui.add_space(18.0);
        ui.label(
            RichText::new(format!(
                "{} · {}",
                tr(Text::Changes),
                repository.changes.len()
            ))
            .size(18.0)
            .strong(),
        );
        if repository.changes.is_empty() {
            ui.add_space(12.0);
            ui.label(tr(Text::NoChanges));
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(ready, egui::Button::new(tr(Text::SelectAll)))
                .clicked()
            {
                self.selected_files = repository
                    .changes
                    .iter()
                    .map(|change| change.path.clone())
                    .collect();
            }
            if ui
                .add_enabled(ready, egui::Button::new(tr(Text::SelectNone)))
                .clicked()
            {
                self.selected_files.clear();
            }
            ui.weak(format!(
                "{} · {}",
                tr(Text::SelectedFiles),
                self.selected_files.len()
            ));
        });
        egui::ScrollArea::vertical()
            .id_salt("git-changes")
            .max_height(300.0)
            .show(ui, |ui| {
                ui.add_enabled_ui(ready, |ui| {
                    for change in &repository.changes {
                        let mut selected = self.selected_files.contains(&change.path);
                        let label = match &change.previous_path {
                            Some(previous) => {
                                format!("{}  {} → {}", change.status, previous, change.path)
                            }
                            None => format!("{}  {}", change.status, change.path),
                        };
                        if ui.checkbox(&mut selected, label).changed() {
                            if selected {
                                self.selected_files.insert(change.path.clone());
                            } else {
                                self.selected_files.remove(&change.path);
                            }
                        }
                    }
                });
            });
        ui.label(tr(Text::CommitMessage));
        ui.add_enabled(
            ready,
            egui::TextEdit::singleline(&mut self.commit_message).desired_width(f32::INFINITY),
        );
        if ui
            .add_enabled(
                ready && !self.selected_files.is_empty() && !self.commit_message.trim().is_empty(),
                egui::Button::new(tr(Text::Commit)),
            )
            .clicked()
        {
            self.queue_job(JobKind::GitCommit);
        }
    }
}
