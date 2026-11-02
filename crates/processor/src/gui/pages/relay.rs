use super::{card, primary_button};
use crate::gui::app::HubApp;
use crate::gui::file_dialog::FileAction;
use crate::gui::i18n::{Text, text};
use crate::gui::relay::{RelayStep, SECRET_IDS};
use crate::relay::{RelayStage, RunMode};
use eframe::egui;
use zeroize::Zeroize;

impl HubApp {
    pub(crate) fn relay_page(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        let colors = self.colors;
        let admitted = !self.active() && self.pending.is_none();
        let mut action = None;
        let mut file_action = None;
        ui.vertical_centered(|ui| {
            ui.set_max_width(760.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.heading(tr(Text::Relay));
                ui.label(tr(Text::RelayIntro));
                ui.add_space(12.0);
                let state = &mut self.relay;
                ui.horizontal_wrapped(|ui| {
                    for (step, label, available) in [
                        (RelayStep::Source, Text::RelayStepImport, true),
                        (
                            RelayStep::Review,
                            Text::RelayStepClean,
                            state.result.is_some() || state.local_content.is_some(),
                        ),
                        (
                            RelayStep::Publish,
                            Text::RelayStepUpload,
                            state.result.is_some(),
                        ),
                    ] {
                        if ui
                            .add_enabled(
                                !state.busy() && available,
                                egui::Button::new(tr(label)).selected(state.step == step),
                            )
                            .clicked()
                        {
                            state.step = step;
                        }
                    }
                });
                ui.add_space(12.0);
                if let Some(error) = &state.error {
                    ui.colored_label(
                        colors.error,
                        crate::gui::relay::failure_message(language, error),
                    );
                } else if let Some(notice) = state.notice {
                    ui.label(tr(notice));
                }
                let cooldown = state.cooldown_seconds();
                if cooldown > 0 {
                    ui.weak(format!("{} {cooldown} s", tr(Text::RelayRetryWait)));
                }
                if state.busy() {
                    card(ui, colors, |ui| {
                        ui.horizontal(|ui| {
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
                egui::ScrollArea::vertical()
                    .id_salt(("relay-step", state.step as u8))
                    .show(ui, |ui| {
                        ui.add_enabled_ui(admitted, |ui| {
                            card(ui, colors, |ui| match state.step {
                                RelayStep::Source => {
                                    ui.label(
                                        egui::RichText::new(tr(Text::RelayImportTitle))
                                            .size(20.0)
                                            .strong(),
                                    );
                                    ui.add_space(8.0);
                                    if secret_field(
                                        ui,
                                        &mut state.source,
                                        SECRET_IDS[1],
                                        tr(Text::RelaySource),
                                        &mut state.reveal_source,
                                        tr(Text::RelayShow),
                                    ) {
                                        state.local_content = None;
                                        state.local_summary = None;
                                        state.source_file.zeroize();
                                        state.input_changed();
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
                                                .selectable_label(
                                                    state.source_file.as_str() == name,
                                                    &name,
                                                )
                                                .clicked()
                                            {
                                                state.source_file = zeroize::Zeroizing::new(name);
                                                state.error = None;
                                            }
                                        }
                                    }
                                    ui.collapsing(tr(Text::RelaySourceOptions), |ui| {
                                        visible_field(
                                            ui,
                                            &mut state.source_file,
                                            SECRET_IDS[2],
                                            tr(Text::RelaySourceFile),
                                        );
                                        if secret_field(
                                            ui,
                                            &mut state.token,
                                            SECRET_IDS[0],
                                            tr(Text::RelayToken),
                                            &mut state.reveal_token,
                                            tr(Text::RelayShow),
                                        ) {
                                            state.clear_request_error();
                                        }
                                        ui.weak(tr(Text::RelayTokenHelp));
                                    });
                                    ui.add_space(12.0);
                                    ui.horizontal_wrapped(|ui| {
                                        if ui
                                            .add_enabled(
                                                state
                                                    .validate(RunMode::Import, state.engine_path())
                                                    .is_ok()
                                                    && cooldown == 0,
                                                primary_button(colors, tr(Text::RelayImportGist)),
                                            )
                                            .clicked()
                                        {
                                            action = Some(RunMode::Import);
                                        }
                                        if ui.button(tr(Text::RelayImportFile)).clicked() {
                                            file_action = Some(FileAction::Import);
                                        }
                                    });
                                }
                                RelayStep::Review => {
                                    ui.label(
                                        egui::RichText::new(tr(Text::RelayReviewTitle))
                                            .size(20.0)
                                            .strong(),
                                    );
                                    if let Some(result) = &state.result {
                                        summary(ui, result, state.result_format, &tr);
                                    } else if let Some((count, format)) = state.local_summary {
                                        ui.label(format!(
                                            "{format} · {} {count}",
                                            tr(Text::RelayTotal)
                                        ));
                                    }
                                    ui.add_space(12.0);
                                    ui.checkbox(&mut state.test_nodes, tr(Text::RelayTestNodes));
                                    if state.test_nodes {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(tr(if state.selected_engine.is_some() {
                                                Text::RelayEngineReady
                                            } else {
                                                Text::RelayEngineMissing
                                            }));
                                            if ui.button(tr(Text::RelayChooseEngine)).clicked() {
                                                file_action = Some(FileAction::ChooseEngine);
                                            }
                                            if ui.button(tr(Text::RelayDetectEngine)).clicked() {
                                                state.selected_engine =
                                                    crate::relay::engine::discover_engine();
                                            }
                                        });
                                        ui.weak(tr(Text::RelayEnginePurpose));
                                    } else {
                                        ui.weak(tr(Text::RelayFormatOnly));
                                    }
                                    ui.collapsing(tr(Text::RelayAdvanced), |ui| {
                                        ui.checkbox(&mut state.rename, tr(Text::RelayRename));
                                        ui.checkbox(&mut state.sort, tr(Text::RelaySort));
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(tr(Text::RelayConcurrency));
                                            ui.add(
                                                egui::DragValue::new(&mut state.concurrency)
                                                    .range(1..=8),
                                            );
                                            ui.label(tr(Text::RelayTimeout));
                                            ui.add(
                                                egui::DragValue::new(&mut state.timeout_seconds)
                                                    .range(1..=20),
                                            );
                                        });
                                        ui.checkbox(
                                            &mut state.compatibility,
                                            tr(Text::RelayCompatibility),
                                        );
                                        ui.checkbox(&mut state.doh, tr(Text::RelayDoh));
                                    });
                                    ui.weak(tr(Text::RelayCleanHelp));
                                    ui.add_space(12.0);
                                    ui.horizontal_wrapped(|ui| {
                                        let ready = state
                                            .validate(RunMode::Preview, state.engine_path())
                                            .is_ok()
                                            && (!state.test_nodes
                                                || state.selected_engine.is_some())
                                            && cooldown == 0;
                                        if ui
                                            .add_enabled(
                                                ready,
                                                primary_button(
                                                    colors,
                                                    tr(if state.test_nodes {
                                                        Text::RelayPreview
                                                    } else {
                                                        Text::RelayPrepareOnly
                                                    }),
                                                ),
                                            )
                                            .clicked()
                                        {
                                            action = Some(RunMode::Preview);
                                        }
                                        if ui
                                            .add_enabled(
                                                state.result.is_some(),
                                                egui::Button::new(tr(Text::RelayContinue)),
                                            )
                                            .clicked()
                                        {
                                            state.step = RelayStep::Publish;
                                        }
                                    });
                                }
                                RelayStep::Publish => {
                                    ui.label(
                                        egui::RichText::new(tr(Text::RelaySaveTitle))
                                            .size(20.0)
                                            .strong(),
                                    );
                                    if let Some(result) = &state.result {
                                        summary(ui, result, state.result_format, &tr);
                                    }
                                    ui.add_space(12.0);
                                    ui.label(
                                        egui::RichText::new(tr(Text::RelayLocalSave)).strong(),
                                    );
                                    ui.checkbox(
                                        &mut state.allow_plaintext,
                                        tr(Text::VaultPlaintextConsent),
                                    );
                                    ui.horizontal_wrapped(|ui| {
                                        if ui
                                            .add_enabled(
                                                state.allow_plaintext && state.result.is_some(),
                                                egui::Button::new(tr(Text::RelayCopy)),
                                            )
                                            .clicked()
                                        {
                                            state.copy(ui.ctx());
                                        }
                                        for (label, export) in [
                                            (Text::RelayExportNodes, FileAction::ExportNodes),
                                            (Text::RelayExportBackup, FileAction::ExportBackup),
                                        ] {
                                            if ui
                                                .add_enabled(
                                                    state.allow_plaintext && state.result.is_some(),
                                                    egui::Button::new(tr(label)),
                                                )
                                                .clicked()
                                            {
                                                file_action = Some(export);
                                            }
                                        }
                                    });
                                    ui.weak(tr(Text::RelayBackupWarning));
                                    ui.add_space(16.0);
                                    ui.separator();
                                    ui.label(
                                        egui::RichText::new(tr(Text::RelayStepUpload)).strong(),
                                    );
                                    if secret_field(
                                        ui,
                                        &mut state.token,
                                        SECRET_IDS[0],
                                        tr(Text::RelayToken),
                                        &mut state.reveal_token,
                                        tr(Text::RelayShow),
                                    ) {
                                        state.clear_request_error();
                                    }
                                    ui.weak(tr(Text::RelayCreateHelp));
                                    ui.collapsing(tr(Text::RelayOutputOptions), |ui| {
                                        visible_field(
                                            ui,
                                            &mut state.output_file,
                                            SECRET_IDS[4],
                                            tr(Text::RelayOutputFile),
                                        );
                                        length_counter(
                                            ui,
                                            state.output_file.chars().count(),
                                            colors.error,
                                        );
                                        visible_field(
                                            ui,
                                            &mut state.title,
                                            SECRET_IDS[5],
                                            tr(Text::RelayTitle),
                                        );
                                        length_counter(
                                            ui,
                                            state.title.chars().count(),
                                            colors.error,
                                        );
                                        secret_field(
                                            ui,
                                            &mut state.output,
                                            SECRET_IDS[3],
                                            tr(Text::RelayOutput),
                                            &mut state.reveal_output,
                                            tr(Text::RelayShow),
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
                                    let publish =
                                        state.validate(RunMode::Publish, state.engine_path());
                                    if let Err(error) = publish {
                                        ui.weak(tr(error));
                                    }
                                    if ui
                                        .add_enabled(
                                            publish.is_ok()
                                                && cooldown == 0
                                                && state.result.is_some(),
                                            primary_button(colors, tr(Text::RelayPublish)),
                                        )
                                        .clicked()
                                    {
                                        action = Some(RunMode::Publish);
                                    }
                                    if let Some(created) = state
                                        .result
                                        .as_ref()
                                        .and_then(|result| result.created.as_ref())
                                    {
                                        ui.label(tr(Text::RelayCreated));
                                        ui.hyperlink_to(&created.url, &created.url);
                                    }
                                }
                            });
                        });
                        ui.add_space(12.0);
                        ui.horizontal_wrapped(|ui| {
                            if state.step != RelayStep::Source
                                && ui.button(tr(Text::RelayBack)).clicked()
                            {
                                state.step = if state.step == RelayStep::Publish {
                                    RelayStep::Review
                                } else {
                                    RelayStep::Source
                                };
                            }
                            if ui.button(tr(Text::RelayClear)).clicked() {
                                state.clear(ui.ctx());
                            }
                        });
                        ui.collapsing(tr(Text::RelayDetails), |ui| {
                            ui.label(tr(Text::RelayFormats));
                            ui.label(tr(Text::RelayPrivacy));
                            ui.label(tr(Text::RelayPublishWarning));
                        });
                        if !admitted {
                            ui.weak(tr(Text::RelayWait));
                        }
                    });
            });
        });
        if let Some(mode) = action {
            self.start_relay(mode);
        } else if let Some(action) = file_action {
            self.relay
                .file_action(action, None, self.loaded_root.as_deref());
        }
    }
}

fn summary(
    ui: &mut egui::Ui,
    result: &crate::relay::RelayResult,
    format: Option<&str>,
    tr: &impl Fn(Text) -> &'static str,
) {
    ui.label(tr(if result.tested {
        Text::RelayTested
    } else {
        Text::RelayNotTested
    }));
    if let Some(format) = format {
        ui.weak(format);
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("{}: {}", tr(Text::RelayTotal), result.total));
        if result.tested {
            ui.label(format!("{}: {}", tr(Text::RelayHealthy), result.healthy));
            ui.label(format!("{}: {}", tr(Text::RelayRemoved), result.removed));
        }
    });
    if result.tested {
        ui.collapsing(tr(Text::RelayResultDetails), |ui| {
            for (key, count) in [
                (Text::RelayFailed, result.failed),
                (Text::RelayUnknown, result.unknown),
                (Text::RelaySecurityBlocked, result.security_blocked),
                (Text::RelayBaselines, result.baseline_valid),
            ] {
                ui.label(format!("{}: {count}", tr(key)));
            }
        });
    }
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

fn secret_field(
    ui: &mut egui::Ui,
    value: &mut String,
    id: &str,
    label: &str,
    reveal: &mut bool,
    show_label: &str,
) -> bool {
    let label = ui.label(label);
    let mut changed = false;
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 92.0).max(80.0);
        let mut output = egui::TextEdit::singleline(value)
            .id(egui::Id::unique(id))
            .password(!*reveal)
            .desired_width(width)
            .show(ui);
        let _ = (*output.response).clone().labelled_by(label.id);
        changed = output.response.changed();
        output.state.clear_undoer();
        output.state.store(ui.ctx(), output.response.id);
        ui.checkbox(reveal, show_label);
    });
    changed
}

fn length_counter(ui: &mut egui::Ui, count: usize, error: egui::Color32) {
    if count > 64 {
        ui.colored_label(error, format!("{count} / 64"));
    }
}
