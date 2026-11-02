use super::{card, primary_button};
use crate::gui::app::HubApp;
use crate::gui::file_dialog::FileAction;
use crate::gui::i18n::{Text, text};
use crate::gui::relay::SECRET_IDS;
use crate::relay::{RelayStage, RunMode};
use eframe::egui;
use std::path::Path;
use zeroize::Zeroize;

impl HubApp {
    pub(crate) fn relay_page(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        let colors = self.colors;
        let admitted = !self.active() && self.pending.is_none();
        let mut action = None;
        let mut file_action = None;
        ui.heading(tr(Text::Relay));
        ui.label(tr(Text::RelayIntro));
        ui.weak(tr(Text::RelayFormats));
        ui.add_space(12.0);
        egui::ScrollArea::vertical()
            .id_salt("relay-page")
            .show(ui, |ui| {
                let state = &mut self.relay;
                if let Some(error) = &state.error {
                    ui.colored_label(
                        colors.error,
                        crate::gui::relay::failure_message(language, error),
                    );
                }
                if let Some(notice) = state.notice {
                    ui.label(tr(notice));
                }
                if state.busy() {
                    card(ui, colors, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spinner();
                            ui.label(tr(Text::RelayWorking));
                        });
                        if let Some(progress) = state.progress {
                            ui.label(tr(match progress.stage {
                                RelayStage::Read => Text::RelayRead,
                                RelayStage::Check => Text::RelayCheck,
                                RelayStage::Render => Text::RelayRender,
                                RelayStage::Publish => Text::RelayUpload,
                                RelayStage::Complete => Text::Succeeded,
                            }));
                            if progress.total > 0 {
                                ui.add(
                                    egui::ProgressBar::new(
                                        progress.checked as f32 / progress.total as f32,
                                    )
                                    .text(format!("{} / {}", progress.checked, progress.total)),
                                );
                            }
                        }
                        if state.file_dialog_active() {
                            ui.weak(tr(Text::RelayDialogHelp));
                        } else if ui.button(tr(Text::RelayCancel)).clicked() {
                            state.request_cancel();
                        }
                    });
                    return;
                }
                ui.weak(tr(Text::RelayPrivacy));
                ui.add_space(12.0);
                ui.add_enabled_ui(admitted, |ui| {
                    card(ui, colors, |ui| {
                        ui.strong(tr(Text::RelayStepImport));
                        if let Some((count, format)) = state.local_summary {
                            ui.label(format!("{format} · {count}"));
                            if ui.button(tr(Text::RelayUseGist)).clicked() {
                                state.local_content = None;
                                state.local_summary = None;
                                state.result = None;
                            }
                        } else {
                            if secret_field(
                                ui,
                                &mut state.source,
                                SECRET_IDS[1],
                                tr(Text::RelaySource),
                            ) {
                                state.source_file.zeroize();
                                state.error = None;
                            }
                            ui.weak(tr(Text::RelaySourceHelp));
                            if let Some(crate::gui::relay::Failure::Operation(
                                crate::relay::RelayError::Gist(
                                    crate::relay::gist::GistError::SelectFile(names),
                                ),
                            )) = &state.error
                            {
                                let names = names.clone();
                                ui.label(tr(Text::RelaySelectFile));
                                for name in names {
                                    if ui
                                        .selectable_label(state.source_file.as_str() == name, &name)
                                        .clicked()
                                    {
                                        state.source_file = zeroize::Zeroizing::new(name);
                                        state.error = None;
                                    }
                                }
                            }
                            ui.collapsing(tr(Text::RelaySourceFile), |ui| {
                                visible_field(
                                    ui,
                                    &mut state.source_file,
                                    SECRET_IDS[2],
                                    tr(Text::RelaySourceFile),
                                );
                            });
                        }
                        if ui.button(tr(Text::RelayImportFile)).clicked() {
                            file_action = Some(FileAction::Import);
                        }
                    });
                    ui.add_space(12.0);
                    card(ui, colors, |ui| {
                        ui.strong(tr(Text::RelayStepClean));
                        ui.checkbox(&mut state.test_nodes, tr(Text::RelayTestNodes));
                        ui.weak(tr(Text::RelayCleanHelp));
                        if state.test_nodes {
                            ui.weak(tr(Text::RelayEngineHelp));
                        }
                        ui.collapsing(tr(Text::RelayAdvanced), |ui| {
                            ui.checkbox(&mut state.rename, tr(Text::RelayRename));
                            ui.checkbox(&mut state.sort, tr(Text::RelaySort));
                            if state.test_nodes {
                                let label = ui.label(tr(Text::SingboxPath));
                                let output = egui::TextEdit::singleline(&mut self.draft.singbox)
                                    .id_salt("relay-singbox-path")
                                    .desired_width(f32::INFINITY)
                                    .show(ui);
                                let _ = (*output.response).clone().labelled_by(label.id);
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(tr(Text::RelayConcurrency));
                                    ui.add(
                                        egui::DragValue::new(&mut state.concurrency).range(1..=8),
                                    );
                                    ui.label(tr(Text::RelayTimeout));
                                    ui.add(
                                        egui::DragValue::new(&mut state.timeout_seconds)
                                            .range(1..=20),
                                    );
                                });
                                ui.checkbox(&mut state.compatibility, tr(Text::RelayCompatibility));
                                ui.checkbox(&mut state.doh, tr(Text::RelayDoh));
                            }
                        });
                        if !state.test_nodes {
                            ui.weak(tr(Text::RelayFormatOnly));
                        }
                        let engine = relay_engine(&self.draft.singbox);
                        let preview = state.validate(RunMode::Preview, engine);
                        if let Err(error) = preview {
                            ui.colored_label(colors.warning, tr(error));
                        }
                        if ui
                            .add_enabled(
                                preview.is_ok(),
                                primary_button(colors, tr(Text::RelayPreview)),
                            )
                            .clicked()
                        {
                            action = Some(RunMode::Preview);
                        }
                    });
                    ui.add_space(12.0);
                    card(ui, colors, |ui| {
                        ui.strong(tr(Text::RelayStepUpload));
                        secret_field(ui, &mut state.token, SECRET_IDS[0], tr(Text::RelayToken));
                        ui.weak(tr(Text::RelayTokenHelp));
                        ui.weak(tr(Text::RelayCreateHelp));
                        visible_field(
                            ui,
                            &mut state.output_file,
                            SECRET_IDS[4],
                            tr(Text::RelayOutputFile),
                        );
                        length_counter(ui, state.output_file.chars().count(), colors.error);
                        visible_field(ui, &mut state.title, SECRET_IDS[5], tr(Text::RelayTitle));
                        length_counter(ui, state.title.chars().count(), colors.error);
                        ui.collapsing(tr(Text::RelayExistingOutput), |ui| {
                            secret_field(
                                ui,
                                &mut state.output,
                                SECRET_IDS[3],
                                tr(Text::RelayOutput),
                            );
                            ui.checkbox(
                                &mut state.allow_public_output,
                                tr(Text::RelayPublicConsent),
                            );
                        });
                        if !state.output.is_empty() {
                            ui.weak(tr(Text::RelayReuseOutput));
                            if ui.button(tr(Text::RelayCreateAnother)).clicked() {
                                state.output.zeroize();
                                state.allow_public_output = false;
                                state.acknowledge_publish = false;
                            }
                        }
                        ui.checkbox(
                            &mut state.acknowledge_publish,
                            tr(Text::RelayPublishConsent),
                        );
                        ui.weak(tr(Text::RelayPublishWarning));
                        let engine = relay_engine(&self.draft.singbox);
                        let preview = state.validate(RunMode::Preview, engine);
                        let publish = state.validate(RunMode::Publish, engine);
                        if let Err(error) = preview {
                            ui.colored_label(colors.warning, tr(error));
                        } else if let Err(error) = publish {
                            ui.weak(tr(error));
                        }
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(
                                    publish.is_ok(),
                                    egui::Button::new(tr(Text::RelayPublish)),
                                )
                                .clicked()
                            {
                                action = Some(RunMode::Publish);
                            }
                        });
                    });
                });
                if !admitted {
                    ui.weak(tr(Text::RelayWait));
                }
                ui.add_space(12.0);
                if let Some(result) = &state.result {
                    let tested = result.tested;
                    let created_url = result.created.as_ref().map(|created| created.url.clone());
                    let counts = [
                        (Text::RelayTotal, result.total),
                        (Text::RelayHealthy, result.healthy),
                        (Text::RelayFailed, result.failed),
                        (Text::RelayUnknown, result.unknown),
                        (Text::RelaySecurityBlocked, result.security_blocked),
                        (Text::RelayRemoved, result.removed),
                        (Text::RelayBaselines, result.baseline_valid),
                    ];
                    card(ui, colors, |ui| {
                        ui.label(tr(Text::RelayLastResult));
                        ui.label(tr(if tested {
                            Text::RelayTested
                        } else {
                            Text::RelayFormatOnly
                        }));
                        if let Some(format) = state.result_format {
                            ui.weak(format);
                        }
                        if let Some(created_url) = &created_url {
                            ui.label(tr(Text::RelayCreated));
                            ui.hyperlink_to(created_url, created_url);
                        }
                        ui.horizontal_wrapped(|ui| {
                            for (key, value) in counts {
                                ui.label(format!("{}: {value}", tr(key)));
                            }
                        });
                        ui.checkbox(&mut state.allow_plaintext, tr(Text::VaultPlaintextConsent));
                        ui.weak(tr(Text::RelayBackupWarning));
                        if ui
                            .add_enabled(
                                state.allow_plaintext,
                                egui::Button::new(tr(Text::RelayCopy)),
                            )
                            .clicked()
                        {
                            state.copy(ui.ctx());
                        }
                        ui.horizontal_wrapped(|ui| {
                            for (key, export) in [
                                (Text::RelayExportNodes, FileAction::ExportNodes),
                                (Text::RelayExportBackup, FileAction::ExportBackup),
                            ] {
                                if ui
                                    .add_enabled(
                                        state.allow_plaintext && admitted,
                                        egui::Button::new(tr(key)),
                                    )
                                    .clicked()
                                {
                                    file_action = Some(export);
                                }
                            }
                        });
                    });
                }
                if ui.button(tr(Text::RelayClear)).clicked() {
                    state.clear(ui.ctx());
                }
            });
        if let Some(mode) = action {
            self.start_relay(mode);
        } else if let Some(action) = file_action {
            self.relay
                .file_action(action, None, self.loaded_root.as_deref());
        }
    }
}

fn relay_engine(value: &str) -> &Path {
    Path::new(if value.trim().is_empty() {
        "sing-box"
    } else {
        value.trim()
    })
}

fn visible_field(ui: &mut egui::Ui, value: &mut String, id: &str, label: &str) {
    let label = ui.label(label);
    let mut output = egui::TextEdit::singleline(value)
        .id(egui::Id::unique(id))
        .desired_width(f32::INFINITY)
        .show(ui);
    let _ = (*output.response).clone().labelled_by(label.id);
    output.state.clear_undoer();
    output.state.store(ui.ctx(), output.response.id);
}

fn secret_field(ui: &mut egui::Ui, value: &mut String, id: &str, label: &str) -> bool {
    let label = ui.label(label);
    let mut output = egui::TextEdit::singleline(value)
        .id(egui::Id::unique(id))
        .password(true)
        .desired_width(f32::INFINITY)
        .show(ui);
    let _ = (*output.response).clone().labelled_by(label.id);
    output.state.clear_undoer();
    output.state.store(ui.ctx(), output.response.id);
    output.response.changed()
}

fn length_counter(ui: &mut egui::Ui, count: usize, error: egui::Color32) {
    if count > 64 {
        ui.colored_label(error, format!("{count} / 64"));
    } else {
        ui.weak(format!("{count} / 64"));
    }
}
