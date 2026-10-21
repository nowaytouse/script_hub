use super::card;
use crate::desktop::app::{CatalogTab, HubApp, Page};
use crate::desktop::i18n::Text;
use crate::desktop::job::JobKind;
use eframe::egui::{self, RichText};

impl HubApp {
    pub(crate) fn overview_page(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.tr(Text::Overview));
        ui.label(self.tr(Text::OverviewDescription));
        ui.add_space(24.0);
        egui::ScrollArea::vertical()
            .id_salt("overview")
            .show(ui, |ui| {
                card(ui, self.colors, |ui| {
                    ui.label(RichText::new(self.tr(Text::Workspace)).strong());
                    ui.add_space(8.0);
                    ui.label(&self.root_input);
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button(self.tr(Text::LoadWorkspace)).clicked() {
                            if self.root_input.trim().is_empty() {
                                self.page = Page::Settings;
                            } else {
                                self.reload();
                            }
                        }
                        if ui
                            .add_enabled(
                                self.workspace_ready() && !self.active(),
                                egui::Button::new(self.tr(Text::Validate)),
                            )
                            .clicked()
                        {
                            self.queue_job(JobKind::Validate);
                        }
                    });
                    ui.weak(self.tr(Text::LocalOnly));
                    if !self.workspace_ready() {
                        ui.weak(self.tr(Text::OpenWorkspaceHint));
                    }
                });
                ui.add_space(16.0);
                ui.horizontal_wrapped(|ui| {
                    for (count, key, tab) in [
                        (
                            self.catalog.modules.len(),
                            Text::ModuleCount,
                            CatalogTab::Modules,
                        ),
                        (self.catalog.rules.len(), Text::RuleCount, CatalogTab::Rules),
                        (self.sources.len(), Text::Sources, CatalogTab::Sources),
                    ] {
                        let value = if self.workspace_ready() {
                            count.to_string()
                        } else {
                            "—".to_string()
                        };
                        let label = format!("{value}  {}", self.tr(key));
                        if ui
                            .add(
                                egui::Button::new(RichText::new(label).size(18.0))
                                    .min_size(egui::vec2(180.0, 72.0))
                                    .corner_radius(10),
                            )
                            .clicked()
                        {
                            self.page = Page::Catalog;
                            self.catalog_tab = tab;
                        }
                    }
                });
                ui.add_space(24.0);
                ui.label(
                    RichText::new(self.tr(Text::LatestActivity))
                        .size(18.0)
                        .strong(),
                );
                ui.add_space(10.0);
                card(ui, self.colors, |ui| {
                    if let Some(job) = &self.job {
                        let snapshot = job.snapshot();
                        let (label, color) = self.outcome(&snapshot);
                        ui.colored_label(color, RichText::new(label).strong());
                        if let Some(stage) = snapshot.stage {
                            ui.label(self.stage_label(stage));
                        }
                        ui.weak(format!(
                            "{} {:02}:{:02}",
                            self.tr(Text::Elapsed),
                            snapshot.elapsed.as_secs() / 60,
                            snapshot.elapsed.as_secs() % 60
                        ));
                        if let Some(last) = snapshot.logs.back() {
                            ui.label(last);
                        }
                    } else {
                        ui.label(self.tr(Text::NoTask));
                    }
                    ui.add_space(8.0);
                    if ui.button(self.tr(Text::ViewTasks)).clicked() {
                        self.page = Page::Tasks;
                    }
                });
                ui.add_space(16.0);
                ui.weak(self.tr(Text::NoAutomaticGit));
            });
    }
}
