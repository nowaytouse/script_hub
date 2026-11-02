use super::{card, primary_button};
use crate::gui::app::HubApp;
use crate::gui::i18n::{Text, text};
use crate::runtime::settings::{self, Accent, Density, Language, Theme, WindowSize};
use eframe::egui::{self, RichText};

fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut T,
    choices: &[(T, &str)],
) {
    ui.vertical(|ui| {
        let label = ui.label(label);
        let current = choices
            .iter()
            .find(|(candidate, _)| candidate == value)
            .map_or("", |(_, label)| *label);
        egui::ComboBox::from_id_salt(id)
            .selected_text(current)
            .width(ui.available_width().min(240.0))
            .show_ui(ui, |ui| {
                for (candidate, label) in choices {
                    ui.selectable_value(value, *candidate, *label);
                }
            })
            .response
            .labelled_by(label.id);
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
                    let wide = ui.available_width() >= 620.0;
                    let field_width = ui.available_width().min(240.0);
                    egui::Grid::new("appearance-grid")
                        .min_col_width(field_width)
                        .spacing([24.0, 14.0])
                        .show(ui, |ui| {
                            choice(
                                ui,
                                "theme",
                                tr(Text::Theme),
                                &mut self.draft.theme,
                                &[
                                    (Theme::System, tr(Text::System)),
                                    (Theme::Light, tr(Text::Light)),
                                    (Theme::Dark, tr(Text::Dark)),
                                ],
                            );
                            if !wide {
                                ui.end_row();
                            }
                            choice(
                                ui,
                                "accent",
                                tr(Text::Accent),
                                &mut self.draft.accent,
                                &[
                                    (Accent::Purple, tr(Text::Purple)),
                                    (Accent::Blue, tr(Text::Blue)),
                                    (Accent::Teal, tr(Text::Teal)),
                                    (Accent::Amber, tr(Text::Amber)),
                                ],
                            );
                            ui.end_row();
                            choice(
                                ui,
                                "language",
                                tr(Text::Language),
                                &mut self.draft.language,
                                &[
                                    (Language::En, "English"),
                                    (Language::ZhCn, "简体中文"),
                                    (Language::System, tr(Text::System)),
                                    (Language::Ja, "日本語"),
                                ],
                            );
                            if !wide {
                                ui.end_row();
                            }
                            ui.vertical(|ui| {
                                let scale_label = ui.label(tr(Text::Scale));
                                ui.add_sized(
                                    [
                                        ui.available_width().min(240.0),
                                        ui.spacing().interact_size.y,
                                    ],
                                    egui::Slider::new(&mut self.draft.scale_percent, 80..=150)
                                        .suffix("%"),
                                )
                                .labelled_by(scale_label.id);
                            });
                            ui.end_row();
                            let previous = self.draft.window;
                            choice(
                                ui,
                                "window",
                                tr(Text::WindowSize),
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
                            if !wide {
                                ui.end_row();
                            }
                            choice(
                                ui,
                                "density",
                                tr(Text::Density),
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
                            .desired_width(ui.available_width()),
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
                    ui.label(tr(Text::RelayEnginePurpose));
                    ui.label(tr(if self.relay.selected_engine.is_some() {
                        Text::RelayEngineReady
                    } else {
                        Text::RelayEngineMissing
                    }));
                    if ui
                        .add_enabled(
                            !self.active(),
                            egui::Button::new(tr(Text::RelayChooseEngine)),
                        )
                        .clicked()
                    {
                        self.relay.file_action(
                            crate::gui::file_dialog::FileAction::ChooseEngine,
                            None,
                            None,
                        );
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::{app::test_app, theme};
    use crate::runtime::settings::Preferences;

    #[test]
    fn settings_controls_fit_narrow_viewport_and_have_accessible_names() {
        for (language, width) in [
            (Language::En, 740.0 / 1.5),
            (Language::ZhCn, 740.0 / 1.5),
            (Language::En, 1160.0),
            (Language::ZhCn, 1160.0),
        ] {
            let preferences = Preferences {
                language,
                scale_percent: 150,
                ..Default::default()
            };
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            theme::apply(&ctx, &preferences);
            let mut app = test_app(preferences);
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 540.0 / 1.5),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| app.settings_page(ui));
                },
            );
            let (bounds, names) = output
                .platform_output
                .accesskit_update
                .as_ref()
                .map_or_else(
                    || (Vec::new(), Vec::new()),
                    |update| {
                        let controls: Vec<_> = update
                            .nodes
                            .iter()
                            .filter(|(_, node)| {
                                matches!(
                                    node.role(),
                                    egui::Role::Button
                                        | egui::Role::CheckBox
                                        | egui::Role::TextInput
                                        | egui::Role::ComboBox
                                        | egui::Role::Slider
                                )
                            })
                            .collect();
                        let bounds = controls
                            .iter()
                            .filter_map(|(_, node)| {
                                node.bounds().map(|bounds| {
                                    (
                                        bounds,
                                        node.role(),
                                        format!(
                                            "role={:?} label={:?} value={:?}",
                                            node.role(),
                                            node.label(),
                                            node.value()
                                        ),
                                    )
                                })
                            })
                            .collect::<Vec<_>>();
                        let names = controls
                            .iter()
                            .filter(|(_, node)| {
                                matches!(node.role(), egui::Role::ComboBox | egui::Role::Slider)
                            })
                            .map(|(_, node)| {
                                node.labelled_by()
                                    .iter()
                                    .find_map(|id| {
                                        update
                                            .nodes
                                            .iter()
                                            .find(|(candidate, _)| candidate == id)
                                            .and_then(|(_, label)| {
                                                label.label().or_else(|| label.value())
                                            })
                                            .map(str::to_owned)
                                    })
                                    .or_else(|| node.label().map(str::to_owned))
                            })
                            .collect::<Vec<_>>();
                        (bounds, names)
                    },
                );
            let viewport = ctx.content_rect();
            output.drop_without_applying_deltas();
            assert!(!bounds.is_empty(), "missing controls for {language:?}");
            for (rect, role, description) in bounds {
                assert!(
                    rect.x0 >= f64::from(viewport.min.x) && rect.x1 <= f64::from(viewport.max.x),
                    "control outside {viewport:?}: {rect:?} {description}"
                );
                if role == egui::Role::ComboBox {
                    assert!(
                        rect.width() >= 140.0,
                        "collapsed choice: {rect:?} {description}"
                    );
                }
            }
            for key in [
                Text::Theme,
                Text::Accent,
                Text::Language,
                Text::Scale,
                Text::WindowSize,
                Text::Density,
            ] {
                let expected = text(language, key);
                assert!(
                    names.iter().any(|name| name.as_deref() == Some(expected)),
                    "missing accessible name {expected}"
                );
            }
        }
    }
}
