use super::{card, primary_button};
use crate::desktop::app::HubApp;
use crate::desktop::i18n::{Text, text};
use crate::desktop::settings::{self, Accent, Density, Language, Theme, WindowSize};
use eframe::egui::{self, RichText};

fn choice<T: Copy + PartialEq>(ui: &mut egui::Ui, id: &str, value: &mut T, choices: &[(T, &str)]) {
    let current = choices
        .iter()
        .find(|(candidate, _)| candidate == value)
        .map_or("", |(_, label)| *label);
    egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .width(180.0)
        .show_ui(ui, |ui| {
            for (candidate, label) in choices {
                ui.selectable_value(value, *candidate, *label);
            }
        });
}

impl HubApp {
    pub(crate) fn settings_page(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        ui.heading(tr(Text::Settings));
        ui.add_space(18.0);
        egui::ScrollArea::vertical()
            .id_salt("settings")
            .show(ui, |ui| {
                card(ui, self.colors, |ui| {
                    ui.label(RichText::new(tr(Text::Appearance)).size(18.0).strong());
                    ui.add_space(12.0);
                    egui::Grid::new("appearance-grid")
                        .num_columns(2)
                        .spacing([24.0, 14.0])
                        .show(ui, |ui| {
                            ui.label(tr(Text::Theme));
                            choice(
                                ui,
                                "theme",
                                &mut self.draft.theme,
                                &[
                                    (Theme::System, tr(Text::System)),
                                    (Theme::Light, tr(Text::Light)),
                                    (Theme::Dark, tr(Text::Dark)),
                                ],
                            );
                            ui.end_row();
                            ui.label(tr(Text::Accent));
                            choice(
                                ui,
                                "accent",
                                &mut self.draft.accent,
                                &[
                                    (Accent::Blue, tr(Text::Blue)),
                                    (Accent::Teal, tr(Text::Teal)),
                                    (Accent::Amber, tr(Text::Amber)),
                                ],
                            );
                            ui.end_row();
                            ui.label(tr(Text::Language));
                            choice(
                                ui,
                                "language",
                                &mut self.draft.language,
                                &[
                                    (Language::System, tr(Text::System)),
                                    (Language::ZhCn, "简体中文"),
                                    (Language::En, "English"),
                                    (Language::Ja, "日本語"),
                                ],
                            );
                            ui.end_row();
                            ui.label(tr(Text::Scale));
                            ui.add(
                                egui::Slider::new(&mut self.draft.scale_percent, 80..=150)
                                    .suffix("%"),
                            );
                            ui.end_row();
                            ui.label(tr(Text::WindowSize));
                            let previous = self.draft.window;
                            choice(
                                ui,
                                "window",
                                &mut self.draft.window,
                                &[
                                    (WindowSize::Compact, tr(Text::Compact)),
                                    (WindowSize::Standard, tr(Text::Standard)),
                                    (WindowSize::Wide, tr(Text::Wide)),
                                ],
                            );
                            if previous != self.draft.window {
                                ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(
                                    self.draft.window.dimensions().into(),
                                ));
                            }
                            ui.end_row();
                            ui.label(tr(Text::Density));
                            choice(
                                ui,
                                "density",
                                &mut self.draft.density,
                                &[
                                    (Density::Comfortable, tr(Text::Comfortable)),
                                    (Density::Compact, tr(Text::Compact)),
                                ],
                            );
                            ui.end_row();
                        });
                });
                ui.add_space(16.0);
                card(ui, self.colors, |ui| {
                    ui.label(RichText::new(tr(Text::Workspace)).size(18.0).strong());
                    ui.add_space(10.0);
                    ui.label(tr(Text::ProjectPath));
                    ui.add_enabled(
                        !self.active(),
                        egui::TextEdit::singleline(&mut self.root_input)
                            .desired_width(f32::INFINITY),
                    );
                    if ui
                        .add_enabled(!self.active(), egui::Button::new(tr(Text::LoadWorkspace)))
                        .clicked()
                    {
                        self.reload();
                    }
                    ui.weak(tr(Text::HelpWorkspace));
                    ui.checkbox(&mut self.draft.reopen_workspace, tr(Text::ReopenWorkspace));
                    egui::CollapsingHeader::new(tr(Text::FolderAccess)).show(ui, |ui| {
                        ui.weak(tr(Text::WorkspacePermissionHelp));
                    });
                    if !self.workspace_ready() {
                        ui.colored_label(self.colors.warning, tr(Text::WorkspaceNotApplied));
                    }
                    ui.add_space(10.0);
                    ui.label(tr(Text::SingboxPath));
                    ui.add_enabled(
                        !self.active(),
                        egui::TextEdit::singleline(&mut self.draft.singbox)
                            .desired_width(f32::INFINITY),
                    );
                    ui.weak(tr(Text::SingboxOptional));
                });
                ui.add_space(16.0);
                card(ui, self.colors, |ui| {
                    ui.label(
                        RichText::new(tr(Text::DataAndDiagnostics))
                            .size(18.0)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    ui.add_enabled_ui(!self.active(), |ui| {
                        ui.checkbox(&mut self.draft.record_history, tr(Text::RecordHistory));
                        ui.weak(tr(Text::RecordHistoryHelp));
                        ui.add_space(12.0);
                        if ui
                            .checkbox(&mut self.draft.developer_mode, tr(Text::DeveloperMode))
                            .changed()
                            && !self.draft.developer_mode
                        {
                            self.draft.diagnostic_logs = false;
                        }
                        ui.weak(tr(Text::DeveloperModeHelp));
                        if self.draft.developer_mode {
                            ui.add_space(10.0);
                            ui.add_enabled(
                                self.draft.record_history,
                                egui::Checkbox::new(
                                    &mut self.draft.diagnostic_logs,
                                    tr(Text::DiagnosticLogs),
                                ),
                            );
                            ui.weak(tr(Text::DiagnosticLogsHelp));
                            if let Some(path) = self.history_path() {
                                ui.label(tr(Text::DatabaseLocation));
                                ui.add(egui::Label::new(path.display().to_string()).wrap());
                            }
                        }
                    });
                });
                ui.add_space(16.0);
                if self.draft != self.preferences {
                    ui.colored_label(self.colors.warning, tr(Text::UnsavedChanges));
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !self.active(),
                            primary_button(self.colors, tr(Text::SaveSettings)),
                        )
                        .clicked()
                    {
                        let result = self
                            .settings_path
                            .as_ref()
                            .ok_or_else(|| tr(Text::SettingsError).to_string())
                            .and_then(|path| settings::save(path, &self.draft));
                        match result {
                            Ok(()) => {
                                self.preferences = self.draft.clone();
                                self.notice = Some(Text::SettingsSaved);
                                self.error = None;
                            }
                            Err(error) => self.error = Some(error),
                        }
                    }
                    if ui
                        .add_enabled(!self.active(), egui::Button::new(tr(Text::Cancel)))
                        .clicked()
                    {
                        self.draft = self.preferences.clone();
                        self.root_input = self.loaded_root.as_deref().map_or_else(
                            || self.preferences.root.clone(),
                            |root| root.display().to_string(),
                        );
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(
                            self.draft.window.dimensions().into(),
                        ));
                    }
                    if ui.button(tr(Text::ResetSettings)).clicked() {
                        let root = self.draft.root.clone();
                        let singbox = self.draft.singbox.clone();
                        self.draft = settings::Preferences {
                            root,
                            singbox,
                            ..Default::default()
                        };
                    }
                });
                if let Some(path) = &self.settings_path {
                    ui.add_space(12.0);
                    ui.weak(path.display().to_string());
                }
            });
    }
}
