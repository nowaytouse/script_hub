use super::{card, primary_button};
use crate::gui::app::HubApp;
use crate::gui::file_dialog::FileAction;
use crate::gui::i18n::{Text, text};
use crate::gui::relay::{InputMode, RelayState, RelayStep, SECRET_IDS};
use crate::gui::relay_schedule::Status as ScheduleStatus;
use crate::gui::theme::Palette;
use crate::relay::{Progress, RelayStage, RunMode};
use eframe::egui;
use zeroize::Zeroize;

#[path = "relay_output.rs"]
mod output;

impl HubApp {
    pub(super) fn relay_editor(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        let colors = self.colors;
        let admitted = !self.active() && self.pending.is_none();
        let mut action = None;
        let mut file_action = None;
        let mut migration_selection = None;
        let mut workspace_import = false;
        let width = ui.available_width().min(880.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(width);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                let state = &mut self.relay;
                let busy = state.busy();
                super::subscriptions::relay_tabs(ui, state, &tr);
                ui.add_space(12.0);
                if state.step != RelayStep::Source && !state.paste.trim().is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(colors.warning, tr(Text::RelayPastedTextPending));
                        if ui.add_enabled(admitted && !state.schedule.armed(), egui::Button::new(tr(Text::RelayOutputGoSource))).clicked() {
                            state.step = RelayStep::Source;
                        }
                    });
                }
                if let Some(error) = &state.error {
                    ui.colored_label(
                        colors.error,
                        crate::gui::relay::failure_message(language, error),
                    );
                } else if let Some(notice) = state.notice
                    && !(busy && notice == Text::RelayCancelling)
                {
                    ui.label(tr(notice));
                }
                let cooldown = state.cooldown_seconds();
                if !state.schedule.armed()
                    && let Some(message) = state.cooldown_message(language)
                {
                    ui.weak(message);
                }
                let schedule_status = state.schedule.status();
                if schedule_status != ScheduleStatus::Idle {
                    card(ui, colors, |ui| {
                        match schedule_status {
                            ScheduleStatus::Idle => {}
                            ScheduleStatus::Stopping => pending_progress(ui, &tr, None, Text::RelayCancelling),
                            ScheduleStatus::Running => {
                                pending_progress(ui, &tr, state.schedule.progress, Text::ScheduleRunning);
                                if state.schedule.progress.is_some_and(|progress| progress.stage == RelayStage::Publish) {
                                    ui.weak(tr(Text::RelayCancelWriteWarning));
                                }
                            }
                            ScheduleStatus::Waiting { seconds, retry } => {
                                if !retry {
                                    ui.strong(tr(Text::ScheduleActive));
                                }
                                if seconds == 0 {
                                    ui.strong(tr(Text::ScheduleReadyWhenIdle));
                                } else {
                                    let label = tr(if retry { Text::ScheduleRetryIn } else { Text::ScheduleNext });
                                    ui.label(format!("{label} {:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60));
                                }
                            }
                            ScheduleStatus::Paused => {
                                ui.strong(tr(Text::SchedulePaused));
                            }
                        }
                        if matches!(schedule_status, ScheduleStatus::Paused | ScheduleStatus::Waiting { retry: true, .. })
                            && let Some(error) = &state.schedule.last_error
                        {
                            ui.colored_label(colors.warning, crate::gui::relay::failure_message(language, error));
                        }
                        if let Some(result) = state.schedule.result {
                            ui.label(format!("{}: {} · {} {} · {} {}", tr(Text::ScheduleLastConfirmed), tr(if result.unchanged { Text::RelayUnchanged } else { Text::RelayPublished }), tr(Text::RelayTotal), result.total, tr(Text::RelayRemoved), result.removed));
                        }
                        if state.schedule.armed() {
                            output::show_surge_handoff(ui, state, language, colors);
                        }
                        if schedule_status == ScheduleStatus::Paused {
                            ui.weak(tr(Text::ScheduleRecovery));
                            ui.horizontal_wrapped(|ui| {
                                if ui.add_enabled(admitted, egui::Button::new(tr(Text::ScheduleReviewSettings))).clicked() {
                                    state.step = RelayStep::Publish;
                                    output::open_gist(ui.ctx());
                                }
                                if ui.button(tr(Text::Close)).clicked() {
                                    state.schedule.clear_feedback();
                                }
                            });
                        } else {
                            ui.weak(tr(Text::ScheduleSessionOnly));
                            if state.schedule.armed() {
                                ui.horizontal_wrapped(|ui| {
                                    if ui
                                        .add_enabled(
                                            admitted && state.schedule_can_run_now(),
                                            egui::Button::new(tr(Text::ScheduleRunNow)),
                                        )
                                        .clicked()
                                    {
                                        state.run_schedule_now();
                                        ui.ctx().request_repaint();
                                    }
                                    if ui.button(tr(Text::ScheduleStop)).clicked() {
                                        state.schedule.stop();
                                        if state.workspace.error == Some(Text::ScheduleStopBeforeImport) {
                                            state.workspace.error = None;
                                        }
                                    }
                                });
                            }
                        }
                    });
                    ui.add_space(8.0);
                }
                if busy {
                    if !state.schedule.running() {
                        card(ui, colors, |ui| {
                            let local = state.file_dialog_active();
                            let cancelling = state.notice == Some(Text::RelayCancelling);
                            let progress = if local || cancelling { None } else { state.progress };
                            pending_progress(ui, &tr, progress, if local {
                                Text::RelayLocalProcessing
                            } else if cancelling {
                                Text::RelayCancelling
                            } else {
                                Text::RelayWorking
                            });
                            if let Some(result) = state.result.as_ref().filter(|result| !result.content.is_empty()) {
                                ui.weak(format!("{} · {}: {}", tr(Text::RelayPreviousPreview), tr(Text::RelayOutputNodeCount), result.total.saturating_sub(result.removed)));
                            }
                            if progress.is_some_and(|progress| progress.stage == RelayStage::Publish) {
                                ui.weak(tr(Text::RelayCancelWriteWarning));
                            }
                            if !local && ui.add_enabled(!cancelling, egui::Button::new(tr(Text::RelayCancel))).clicked() {
                                state.request_cancel();
                            }
                        });
                    }
                    return;
                }
                if state.step == RelayStep::Review && state.editor.is_some() {
                    ui.add_enabled_ui(admitted && !state.schedule.armed(), |ui| {
                        (action, file_action) = review_actions(ui, state, colors, cooldown, &tr);
                    });
                    ui.add_space(8.0);
                }
                egui::ScrollArea::vertical()
                    .id_salt(("relay-step", state.step as u8))
                    .show(ui, |ui| {
                        ui.add_enabled_ui(admitted && !state.schedule.armed(), |ui| {
                            card(ui, colors, |ui| match state.step {
                                RelayStep::Source if state.demo_mode => {
                                    ui.label(tr(Text::RelaySandboxOffline));
                                }
                                RelayStep::Source => {
                                    let mut focus_paste = false;
                                    ui.label(
                                        egui::RichText::new(tr(Text::RelayImportTitle))
                                            .size(20.0)
                                            .strong(),
                                    );
                                    ui.add_space(8.0);
                                    ui.horizontal_wrapped(|ui| {
                                        for (mode, key) in [(InputMode::Link, Text::InputLink), (InputMode::Paste, Text::InputPaste), (InputMode::File, Text::RelayLocalSource)] {
                                            if ui.selectable_label(state.input_mode == mode, tr(key)).clicked() && state.input_mode != mode {
                                                state.input_mode = mode;
                                                state.reveal_source = false;
                                                state.reveal_paste = false;
                                                state.reveal_token = false;
                                                state.clear_request_error();
                                                focus_paste = mode == InputMode::Paste;
                                            }
                                        }
                                    });
                                    ui.add_space(8.0);
                                    if state.input_mode == InputMode::Link {
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
                                        state.clear_editor_widgets(ui.ctx());
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
                                                .add_enabled(cooldown == 0, egui::Button::new(&name))
                                                .clicked()
                                            {
                                                state.source_file = zeroize::Zeroizing::new(name);
                                                state.local_content = None;
                                                state.local_summary = None;
                                                action = Some(RunMode::Import);
                                            }
                                        }
                                    }
                                    ui.collapsing(tr(Text::RelaySourceOptions), |ui| {
                                        let previous_file = state.source_file.clone();
                                        visible_field(
                                            ui,
                                            &mut state.source_file,
                                            SECRET_IDS[2],
                                            tr(Text::RelaySourceFile),
                                        );
                                        if state.source_file != previous_file {
                                            state.local_content = None;
                                            state.local_summary = None;
                                            state.clear_editor_widgets(ui.ctx());
                                            state.input_changed();
                                        }
                                        if secret_field(
                                            ui,
                                            &mut state.token,
                                            SECRET_IDS[0],
                                            tr(Text::RelayToken),
                                            &mut state.reveal_token,
                                            tr(Text::RelayShow),
                                        ) {
                                            state.clear_request_error();
                                            state.acknowledge_publish = false;
                                            state.allow_public_output = false;
                                        }
                                        ui.weak(tr(Text::RelayTokenHelp));
                                    });
                                    ui.add_space(8.0);
                                    if ui
                                        .add_enabled(
                                            !state.source.trim().is_empty() && state
                                                .validate(RunMode::Import, state.engine_path())
                                                .is_ok()
                                                && cooldown == 0,
                                            primary_button(colors, tr(Text::RelayImportGist)),
                                        )
                                        .clicked()
                                    {
                                        if !state.source.trim().is_empty() { state.local_content = None; state.local_summary = None; }
                                        action = Some(RunMode::Import);
                                    }
                                    if !state.source.trim().is_empty() && state.validate(RunMode::Import, state.engine_path()).is_err() {
                                        ui.weak(tr(Text::RelayInputInvalid));
                                    }
                                    } else if state.input_mode == InputMode::Paste {
                                        ui.label(tr(Text::InputPasteHelp));
                                        ui.horizontal_wrapped(|ui| {
                                            if ui.add_enabled(!state.paste.trim().is_empty(), primary_button(colors, tr(Text::RelayImportGist))).clicked() {
                                                state.import_paste(ui.ctx());
                                            }
                                            ui.checkbox(&mut state.reveal_paste, tr(Text::InputReveal));
                                            if !state.paste.is_empty() && ui.button(tr(Text::RelayClearPastedText)).clicked() {
                                                state.clear_pasted_text(ui.ctx());
                                                focus_paste = true;
                                            }
                                        });
                                        let label = ui.label(tr(Text::RelayPasteLabel));
                                        let changed = egui::ScrollArea::vertical()
                                            .id_salt("relay-paste-scroll")
                                            .max_height(180.0)
                                            .show(ui, |ui| {
                                        let mut edit = egui::TextEdit::multiline(&mut *state.paste)
                                            .id(egui::Id::unique("relay-paste"))
                                            .desired_rows(6)
                                            .desired_width(f32::INFINITY)
                                            .password(!state.reveal_paste)
                                            .char_limit(4 * 1024 * 1024)
                                            .show(ui);
                                        let _ = (*edit.response).clone().labelled_by(label.id);
                                        if focus_paste {
                                            edit.response.request_focus();
                                        }
                                        edit.state.clear_undoer();
                                        edit.state.store(ui.ctx(), edit.response.id);
                                        edit.response.changed()
                                            }).inner;
                                        if changed {
                                            state.clear_request_error();
                                            state.workspace.draft_dirty |= state.workspace.selected.is_some();
                                        }
                                        if !state.paste.trim().is_empty() && state.workspace.selected.is_some() {
                                            ui.weak(tr(Text::RelayLoadPastedBeforeSave));
                                        }
                                    } else {
                                    ui.strong(tr(Text::RelayLocalSource));
                                    ui.weak(tr(Text::RelayDropFile));
                                    if ui.add(primary_button(colors, tr(Text::RelayImportFile))).clicked() {
                                        file_action = Some(FileAction::Import);
                                    }
                                     ui.weak(tr(Text::MigrationFormats));
                                     if state.encrypted_import.is_some() {
                                         ui.separator();
                                         ui.strong(tr(Text::ArchiveUnlock));
                                         ui.weak(tr(Text::ArchiveHelp));
                                         secret_field(ui, &mut state.archive_identity, SECRET_IDS[6], tr(Text::ArchiveIdentity), &mut state.reveal_identity, tr(Text::RelayShow));
                                         ui.horizontal_wrapped(|ui| {
                                             if ui.add_enabled(!state.archive_identity.is_empty(), primary_button(colors, tr(Text::ArchiveUnlock))).clicked() {
                                                 state.unlock_import(ui.ctx());
                                             }
                                             if ui.button(tr(Text::RelayCancel)).clicked() {
                                                 state.encrypted_import = None;
                                                 state.archive_identity.zeroize();
                                                 crate::gui::templates::clear_secret_widget(ui.ctx(), SECRET_IDS[6]);
                                             }
                                         });
                                     }
                                     if let Some(report) = &state.migration {
                                         ui.separator();
                                         ui.strong(tr(Text::MigrationPreview));
                                         let operations = report.items.iter().map(|item| item.operations.len()).sum::<usize>()
                                             + report.collections.iter().map(|item| item.operations.len()).sum::<usize>();
                                         ui.label(format!("{} {} · {} {} · {} {} · {} {}", report.items.len(), tr(Text::MigrationItems), report.collections.len(), tr(Text::MigrationCollections), operations, tr(Text::WorkspaceOperationsTab), report.unsupported_count, tr(Text::MigrationUnsupported)));
                                         ui.weak(tr(Text::MigrationHelp));
                                         ui.checkbox(&mut state.migration_ack, tr(Text::MigrationConsent));
                                         if ui.add_enabled(state.migration_ack, egui::Button::new(tr(Text::WorkspaceImportAll))).clicked() { workspace_import = true; }
                                         egui::ScrollArea::vertical().id_salt("substore-import-preview").max_height(240.0).show_rows(ui, 32.0, report.items.len(), |ui, rows| {
                                             for index in rows {
                                                 let item = &report.items[index];
                                                 ui.horizontal(|ui| {
                                                     let name_width = (ui.available_width() - 280.0).max(60.0);
                                                     ui.add_sized([name_width, 28.0], egui::Label::new(item.name.as_str()).truncate());
                                                     ui.weak(tr(if item.source.is_some() { Text::InputLink } else { Text::RelayLocalSource }));
                                                     if item.unsupported > 0 { ui.colored_label(colors.warning, format!("{} {}", item.unsupported, tr(Text::MigrationUnsupported))); }
                                                     if ui.add_enabled(state.migration_ack && (item.content.is_some() || item.source.is_some()), egui::Button::new(tr(Text::MigrationChoose))).clicked() { migration_selection = Some(index); }
                                                 });
                                             }
                                         });
                                         if !report.collections.is_empty() {
                                             ui.collapsing(tr(Text::MigrationCollections), |ui| {
                                                 egui::ScrollArea::vertical().id_salt("substore-collections-preview").max_height(120.0).show_rows(ui, 24.0, report.collections.len(), |ui, rows| {
                                                     for index in rows {
                                                         let collection = &report.collections[index];
                                                         ui.add(egui::Label::new(format!("{} · {} {}", collection.name.as_str(), collection.subscriptions.len(), tr(Text::MigrationItems))).truncate());
                                                     }
                                                 });
                                             });
                                         }
                                         if ui.button(tr(Text::RelayCancel)).clicked() { state.migration = None; }
                                     }
                                    }
                                }
                                RelayStep::Review => {
                                    ui.label(
                                        egui::RichText::new(tr(Text::RelayReviewTitle))
                                            .size(20.0)
                                            .strong(),
                                    );
                                    if state.editor.is_none() {
                                        ui.label(tr(Text::RelayReviewNoSource));
                                        if ui.button(tr(Text::RelayOutputGoSource)).clicked() {
                                            state.step = RelayStep::Source;
                                        }
                                        return;
                                    }
                                    if let Some(result) = &state.result {
                                        summary(ui, result, state.result_format, &tr);
                                    } else if let Some((count, format)) = state.local_summary {
                                        ui.label(format!(
                                            "{format} · {} {count}",
                                            tr(Text::RelayTotal)
                                        ));
                                    }
                                    ui.add_space(12.0);
                                    ui.collapsing(tr(Text::NativePreset), |ui| {
                                        ui.weak(tr(Text::NativePresetHelp));
                                        let mut selected = None;
                                        ui.horizontal_wrapped(|ui| {
                                            if ui.button(tr(Text::NativeSafe)).clicked() { selected = Some(crate::relay::model::entrance::Preset::Safe); }
                                            if ui.button(tr(Text::NativeCompatible)).clicked() { selected = Some(crate::relay::model::entrance::Preset::Compatible); }
                                        });
                                        if let Some(preset) = selected { state.add_preset(ui.ctx(), preset); }
                                    });
                                    let mut reset_editor_widgets = false;
                                    if let Some(editor) = &mut state.editor {
                                        let mut changed = super::node_operations::show(
                                            ui,
                                            language,
                                            colors,
                                            &mut editor.options.pipeline,
                                            state
                                                .result
                                                .as_ref()
                                                .map_or(&[], |result| result.stages.as_slice()),
                                        );
                                        if state.rename || state.sort {
                                            ui.horizontal_wrapped(|ui| {
                                                changed |= ui
                                                    .checkbox(
                                                        &mut state.rename,
                                                        tr(Text::RelayRename),
                                                    )
                                                    .changed();
                                                changed |= ui
                                                    .checkbox(&mut state.sort, tr(Text::RelaySort))
                                                    .changed();
                                            });
                                        }
                                        ui.checkbox(&mut editor.reveal, tr(Text::RelayRevealNames));
                                        if editor.reveal {
                                            egui::ScrollArea::vertical()
                                                .id_salt("relay-node-list")
                                                .max_height(260.0)
                                                .show_rows(
                                                    ui,
                                                    30.0,
                                                    editor.rows.len(),
                                                    |ui, range| {
                                                        for row in &mut editor.rows[range] {
                                                            ui.horizontal(|ui| {
                                                                let label = ui.label(format!(
                                                                    "#{} · {}",
                                                                    row.index + 1,
                                                                    row.protocol.as_str()
                                                                ));
                                                                changed |= ui
                                                                    .checkbox(
                                                                        &mut row.included,
                                                                        tr(Text::RelayNodeEnabled),
                                                                    )
                                                                    .changed();
                                                                let mut output =
                                                                    egui::TextEdit::singleline(
                                                                        &mut *row.name,
                                                                    )
                                                                    .id(egui::Id::unique((
                                                                        "relay-node-name",
                                                                        row.index,
                                                                    )))
                                                                    .char_limit(512)
                                                                    .desired_width(ui.available_width())
                                                                    .show(ui);
                                                                let _ = (*output.response)
                                                                    .clone()
                                                                    .labelled_by(label.id);
                                                                changed |=
                                                                    output.response.changed();
                                                                output.state.clear_undoer();
                                                                output.state.store(
                                                                    ui.ctx(),
                                                                    output.response.id,
                                                                );
                                                            });
                                                        }
                                                    },
                                                );
                                            if state.result.is_some() {
                                                ui.collapsing(
                                                    tr(Text::RelayProcessedNames),
                                                    |ui| {
                                                        egui::ScrollArea::vertical()
                                                            .id_salt("relay-processed-names")
                                                            .max_height(180.0)
                                                            .show_rows(
                                                                ui,
                                                                52.0,
                                                                state.preview_names.len(),
                                                                |ui, range| {
                                                                    for index in range {
                                                                        let name = state.preview_names[index].as_str();
                                                                        ui.add(egui::Label::new(name).truncate()).on_hover_text(name);
                                                                        if let Some(endpoint) = state.preview_endpoints.get(index) {
                                                                            ui.add(egui::Label::new(egui::RichText::new(endpoint.as_str()).monospace().weak()).truncate())
                                                                                .on_hover_text(endpoint.as_str());
                                                                        }
                                                                    }
                                                                },
                                                            );
                                                    },
                                                );
                                            }
                                        }
                                        if ui.button(tr(Text::RelayResetOriginal)).clicked() {
                                            super::node_operations::clear_widgets(
                                                ui.ctx(),
                                                &editor.options.pipeline,
                                            );
                                            editor.reset();
                                            reset_editor_widgets = true;
                                            state.rename = false;
                                            state.sort = false;
                                            changed = true;
                                        }
                                        if changed {
                                            state.invalidate_result();
                                        }
                                    }
                                    if reset_editor_widgets {
                                        state.clear_editor_widgets(ui.ctx());
                                    }
                                    if !state.demo_mode { ui.collapsing(tr(Text::OperationPlan), |ui| {
                                        ui.weak(tr(Text::OperationPlanHelp));
                                        if ui
                                            .add_enabled(
                                                state.editor.is_some(),
                                                egui::Button::new(tr(Text::OperationPlanImport)),
                                            )
                                            .clicked()
                                        {
                                            file_action = Some(FileAction::ImportPlan);
                                        }
                                        ui.checkbox(
                                            &mut state.allow_plaintext,
                                            tr(Text::VaultPlaintextConsent),
                                        );
                                        if ui
                                            .add_enabled(
                                                state.editor.is_some() && state.allow_plaintext,
                                                egui::Button::new(tr(Text::OperationPlanExport)),
                                            )
                                            .clicked()
                                        {
                                            file_action = Some(FileAction::ExportPlan);
                                        }
                                    }); }
                                }
                                RelayStep::Publish => {
                                    (action, file_action) = output::show(ui, state, language, colors, cooldown);
                                }
                            });
                        });
                        ui.add_space(12.0);
                        ui.collapsing(tr(Text::RelayDetails), |ui| {
                            ui.label(tr(Text::RelayFormats));
                            ui.label(tr(Text::RelayPrivacy));
                            if state.workspace.selected.is_none()
                                && ui.button(tr(Text::RelayClear)).clicked()
                            {
                                state.clear(ui.ctx());
                            }
                        });
                        if !admitted {
                            ui.weak(tr(Text::RelayWait));
                        }
                    });
            });
        });
        if workspace_import {
            self.relay.import_workspace_report();
        } else if let Some(mode) = action {
            if mode == RunMode::Import {
                self.relay.clear_editor_widgets(ui.ctx());
                self.relay.input_changed();
            }
            self.start_relay(mode);
        } else if let Some(index) = migration_selection {
            self.relay.select_migration(ui.ctx(), index);
        } else if let Some(action) = file_action {
            self.relay
                .file_action(action, None, self.loaded_root.as_deref());
        }
    }
}

fn pending_progress(
    ui: &mut egui::Ui,
    tr: &impl Fn(Text) -> &'static str,
    progress: Option<Progress>,
    fallback: Text,
) {
    let progress = progress.filter(|progress| progress.stage != RelayStage::Complete);
    let label = progress.map_or(fallback, |progress| match progress.stage {
        RelayStage::Read => Text::RelayReadingInput,
        RelayStage::Check => Text::RelayCheck,
        RelayStage::Render => Text::RelayRender,
        RelayStage::Publish => Text::RelayUpload,
        RelayStage::Complete => fallback,
    });
    ui.horizontal_wrapped(|ui| {
        ui.spinner();
        ui.strong(tr(label));
    });
    if let Some(progress) =
        progress.filter(|progress| progress.stage == RelayStage::Check && progress.total > 0)
    {
        let checked = progress.checked.min(progress.total);
        ui.add(
            egui::ProgressBar::new(checked as f32 / progress.total as f32)
                .text(format!("{checked} / {}", progress.total)),
        );
    }
}

fn review_actions(
    ui: &mut egui::Ui,
    state: &mut RelayState,
    colors: Palette,
    cooldown: u64,
    tr: &impl Fn(Text) -> &'static str,
) -> (Option<RunMode>, Option<FileAction>) {
    let mut action = None;
    let mut file = None;
    let has_content = state
        .result
        .as_ref()
        .is_some_and(|result| !result.content.is_empty());
    let needs_resolution = state.needs_resolution();
    card(ui, colors, |ui| {
        ui.strong(tr(match (state.result.is_some(), has_content) {
            (_, true) => Text::RelayReviewReady,
            (true, false) => Text::RelayReviewEmptyResult,
            _ => Text::RelayReviewNeedsPreview,
        }));
        if needs_resolution {
            let providers = state
                .resolution_providers()
                .into_iter()
                .map(|provider| provider.name())
                .collect::<Vec<_>>()
                .join(", ");
            ui.label(format!("{}: {providers}", tr(Text::ResolutionProviders)));
            if state.demo_mode {
                ui.weak(tr(Text::RelaySandboxOffline));
            } else {
                ui.checkbox(&mut state.allow_resolution, tr(Text::ResolutionConsent));
            }
        }
        ui.horizontal_wrapped(|ui| {
            let ready = !needs_resolution || (!state.demo_mode && state.allow_resolution);
            let label = tr(if needs_resolution {
                Text::ResolutionPrepare
            } else {
                Text::RelayPrepareOnly
            });
            let preview = if has_content {
                egui::Button::new(label).min_size(egui::vec2(0.0, 36.0))
            } else {
                primary_button(colors, label)
            };
            if ui
                .add_enabled(ready && (!needs_resolution || cooldown == 0), preview)
                .clicked()
            {
                state.test_nodes = false;
                action = Some(RunMode::Preview);
            }
            if state.selected_engine.is_some()
                && ui
                    .add_enabled(
                        ready && cooldown == 0,
                        egui::Button::new(tr(Text::RelayPreview)),
                    )
                    .clicked()
            {
                state.test_nodes = true;
                action = Some(RunMode::Preview);
            }
            if !state.demo_mode {
                ui.menu_button(
                    tr(if state.selected_engine.is_some() {
                        Text::RelayAdvanced
                    } else {
                        Text::RelayTestSetup
                    }),
                    |ui| {
                        ui.set_min_width(240.0);
                        ui.label(tr(if state.selected_engine.is_some() {
                            Text::RelayEngineReady
                        } else {
                            Text::RelayEngineMissing
                        }));
                        ui.label(tr(Text::RelayEnginePurpose));
                        if ui.button(tr(Text::RelayDetectEngine)).clicked() {
                            state.selected_engine = crate::relay::engine::discover_engine();
                        }
                        if ui.button(tr(Text::RelayChooseEngine)).clicked() {
                            file = Some(FileAction::ChooseEngine);
                            ui.close();
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            let label = ui.label(tr(Text::RelayConcurrency));
                            ui.add(egui::DragValue::new(&mut state.concurrency).range(1..=8))
                                .labelled_by(label.id);
                        });
                        ui.horizontal(|ui| {
                            let label = ui.label(tr(Text::RelayTimeout));
                            ui.add(egui::DragValue::new(&mut state.timeout_seconds).range(1..=20))
                                .labelled_by(label.id);
                        });
                        ui.checkbox(&mut state.compatibility, tr(Text::RelayCompatibility));
                        ui.checkbox(&mut state.doh, tr(Text::RelayDoh));
                        ui.weak(tr(Text::RelayStrictProbeHelp));
                    },
                );
            }
            if has_content
                && ui
                    .add(primary_button(colors, tr(Text::RelayContinue)))
                    .clicked()
            {
                state.step = RelayStep::Publish;
            }
        });
    });
    (action, file)
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
        ui.label(format!(
            "{}: {}",
            tr(Text::RelayProcessed),
            result.processed
        ));
        if result.excluded > 0 {
            ui.label(format!("{}: {}", tr(Text::RelayExcluded), result.excluded));
        }
        if result.duplicates > 0 {
            ui.label(format!(
                "{}: {}",
                tr(Text::RelayDuplicates),
                result.duplicates
            ));
        }
        if result.duplicate_skipped > 0 {
            ui.label(format!(
                "{}: {}",
                tr(Text::RelayDuplicateSkipped),
                result.duplicate_skipped
            ));
        }
        if result.tested {
            ui.label(format!("{}: {}", tr(Text::RelayHealthy), result.healthy));
            ui.label(format!("{}: {}", tr(Text::RelayRemoved), result.removed));
        }
    });
    if result.tested {
        if result.healthy == 0 {
            ui.label(tr(Text::RelayNoHealthyHelp));
        }
        ui.collapsing(tr(Text::RelayResultDetails), |ui| {
            for (key, count) in [
                (Text::RelayFailed, result.failed),
                (Text::RelayUnknown, result.unknown),
                (Text::RelaySecurityBlocked, result.security_blocked),
                (Text::RelayBaselines, result.baseline_valid),
            ] {
                ui.label(format!("{}: {count}", tr(key)));
            }
            use crate::relay::engine::ProbeReason;
            let strict_probed = result
                .details
                .iter()
                .filter(|detail| detail.strict_tls)
                .count();
            if strict_probed > 0 {
                let verified = result
                    .details
                    .iter()
                    .filter(|detail| detail.strict_tls && detail.reason == ProbeReason::Verified)
                    .count();
                ui.label(format!(
                    "{}: {strict_probed} · {}: {verified}",
                    tr(Text::RelayStrictProbed),
                    tr(Text::RelayStrictVerified)
                ));
            }
            for (reason, key) in [
                (ProbeReason::Verified, Text::RelayReasonVerified),
                (ProbeReason::Unsupported, Text::RelayReasonUnsupported),
                (ProbeReason::UnsafeAddress, Text::RelayReasonUnsafeAddress),
                (
                    ProbeReason::AddressUnverified,
                    Text::RelayReasonAddressUnverified,
                ),
                (
                    ProbeReason::ExternalResource,
                    Text::RelayReasonExternalResource,
                ),
                (
                    ProbeReason::EngineConfigRejected,
                    Text::RelayReasonEngineConfigRejected,
                ),
                (
                    ProbeReason::EngineConfigTimeout,
                    Text::RelayReasonEngineConfigTimeout,
                ),
                (ProbeReason::EngineNotReady, Text::RelayReasonEngineNotReady),
                (ProbeReason::EngineExited, Text::RelayReasonEngineExited),
                (
                    ProbeReason::ConnectionFailed,
                    Text::RelayReasonConnectionFailed,
                ),
                (ProbeReason::Timeout, Text::RelayReasonTimeout),
                (ProbeReason::TunnelRejected, Text::RelayReasonTunnelRejected),
                (ProbeReason::HttpResponse, Text::RelayReasonHttpResponse),
                (ProbeReason::TlsFailure, Text::RelayReasonTlsFailure),
                (ProbeReason::Inconclusive, Text::RelayReasonInconclusive),
                (
                    ProbeReason::StrictTlsUnverified,
                    Text::RelayReasonStrictTlsUnverified,
                ),
                (
                    ProbeReason::BaselineUnstable,
                    Text::RelayReasonBaselineUnstable,
                ),
                (ProbeReason::NotTested, Text::RelayReasonNotTested),
            ] {
                let count = result
                    .details
                    .iter()
                    .filter(|detail| detail.reason == reason)
                    .count();
                if count > 0 {
                    ui.label(format!("{}: {count}", tr(key)));
                }
            }
            ui.weak(tr(Text::RelayLatencyHelp));
            for (target, name) in [(0, "Google 204"), (1, "Cloudflare 204")] {
                if let Some((minimum, median)) = latency_stats(&result.details, target) {
                    ui.label(format!(
                        "{name}: {} {minimum} ms · {} {median} ms",
                        tr(Text::RelayMinimum),
                        tr(Text::RelayMedian)
                    ));
                }
            }
        });
    }
}

fn latency_stats(
    details: &[crate::relay::engine::ProbeDetail],
    target: usize,
) -> Option<(u64, u64)> {
    let mut values: Vec<_> = details
        .iter()
        .filter(|detail| {
            detail.reason == crate::relay::engine::ProbeReason::Verified
                && detail.target == Some(target)
        })
        .filter_map(|detail| detail.latency_ms)
        .collect();
    values.sort_unstable();
    let minimum = *values.first()?;
    let middle = values.len() / 2;
    let median = if values.len() % 2 == 0 {
        values[middle - 1] + (values[middle] - values[middle - 1]) / 2
    } else {
        values[middle]
    };
    Some((minimum, median))
}

fn visible_field(ui: &mut egui::Ui, value: &mut String, id: &str, label: &str) -> bool {
    let label = ui.label(label);
    let mut output = egui::TextEdit::singleline(value)
        .id(egui::Id::unique(id))
        .desired_width(f32::INFINITY)
        .show(ui);
    let _ = (*output.response).clone().labelled_by(label.id);
    output.state.clear_undoer();
    output.state.store(ui.ctx(), output.response.id);
    output.response.changed()
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
    let toggle_width = ui
        .painter()
        .layout_no_wrap(
            show_label.to_owned(),
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().text_color(),
        )
        .size()
        .x
        + 2.0 * ui.spacing().interact_size.y
        + ui.spacing().item_spacing.x;
    let inline = ui.available_width() >= toggle_width + 160.0;
    let password = !*reveal;
    let mut edit = |ui: &mut egui::Ui, width| {
        let mut output = egui::TextEdit::singleline(value)
            .id(egui::Id::unique(id))
            .password(password)
            .desired_width(width)
            .show(ui);
        let _ = (*output.response).clone().labelled_by(label.id);
        output.state.clear_undoer();
        output.state.store(ui.ctx(), output.response.id);
        output.response.changed()
    };
    if inline {
        ui.horizontal(|ui| {
            let changed = edit(ui, ui.available_width() - toggle_width);
            ui.checkbox(reveal, show_label);
            changed
        })
        .inner
    } else {
        let changed = edit(ui, ui.available_width());
        ui.checkbox(reveal, show_label);
        changed
    }
}

fn length_counter(ui: &mut egui::Ui, count: usize, error: egui::Color32) {
    if count > 64 {
        ui.colored_label(error, format!("{count} / 64"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::engine::{ProbeDetail, ProbeReason};
    use crate::runtime::settings::{Language, Preferences};
    use std::path::Path;
    use std::sync::mpsc;

    fn render(app: &mut HubApp, ctx: &egui::Context) -> Vec<(String, bool)> {
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(420.0, 580.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| app.relay_editor(ui));
            },
        );
        let entries = output
            .platform_output
            .accesskit_update
            .as_ref()
            .expect("accessibility tree")
            .nodes
            .iter()
            .filter_map(|(_, node)| {
                node.label()
                    .or_else(|| node.value())
                    .map(|label| (label.to_owned(), node.is_disabled()))
            })
            .collect();
        output.drop_without_applying_deltas();
        entries
    }

    #[test]
    fn upload_progress_explains_cancel_and_keeps_navigation_locked() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = crate::gui::app::test_app(Preferences {
            language: Language::En,
            ..Preferences::default()
        });
        app.relay.step = RelayStep::Publish;
        app.relay.local_content = Some(zeroize::Zeroizing::new(
            "Example = socks5, node.example, 1080\n".to_owned(),
        ));
        app.relay.paste.push_str("new text");
        let (_finish, wait) = mpsc::channel::<()>();
        app.relay
            .start_with(RunMode::Import, Path::new(""), move |_, _, _, _, _| {
                let _ = wait.recv();
                Err(crate::relay::RelayError::Cancelled)
            });
        assert!(app.relay.busy());
        app.relay.progress = Some(Progress {
            stage: RelayStage::Publish,
            checked: 1,
            total: 2,
        });

        let entry = |key, disabled| (text(Language::En, key).to_owned(), disabled);
        let running = render(&mut app, &ctx);
        assert!(running.contains(&entry(Text::RelayUpload, false)));
        assert!(running.contains(&entry(Text::RelayCancelWriteWarning, false)));
        assert!(running.contains(&entry(Text::RelayCancel, false)));
        assert!(running.contains(&entry(Text::RelayOutputGoSource, true)));

        app.relay.request_cancel();
        let cancelling = render(&mut app, &ctx);
        assert!(cancelling.contains(&entry(Text::RelayCancel, true)));
        assert_eq!(
            cancelling
                .iter()
                .filter(|(label, _)| label == text(Language::En, Text::RelayCancelling))
                .count(),
            1
        );
    }

    #[test]
    fn armed_schedule_shows_sync_now_and_disables_it_after_queueing() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = crate::gui::app::test_app(Preferences {
            language: Language::En,
            ..Preferences::default()
        });
        app.relay.step = RelayStep::Publish;
        app.relay
            .schedule
            .arm(
                crate::relay::RelayConfig {
                    source: "https://example.invalid/nodes".into(),
                    output: "fedcba9876543210fedcba9876543210".into(),
                    output_file: "nodes.conf".into(),
                    test_nodes: false,
                    ..Default::default()
                },
                "synthetic-token",
            )
            .unwrap();
        let entry = |key, disabled| (text(Language::En, key).to_owned(), disabled);
        let ready = render(&mut app, &ctx);
        assert!(ready.contains(&entry(Text::ScheduleRunNow, false)));
        assert!(ready.contains(&entry(Text::ScheduleStop, false)));

        // No worker ticks in this test, so queueing never makes a network request.
        app.relay.run_schedule_now();
        let queued = render(&mut app, &ctx);
        assert!(queued.contains(&entry(Text::ScheduleRunNow, true)));
        assert!(queued.contains(&entry(Text::ScheduleStop, false)));
    }

    #[test]
    fn latency_statistics_keep_targets_and_unverified_results_separate() {
        let detail = |reason, target, latency_ms| ProbeDetail {
            reason,
            target,
            latency_ms,
            strict_tls: true,
        };
        let details = [
            detail(ProbeReason::Verified, Some(0), Some(10)),
            detail(ProbeReason::Verified, Some(0), Some(30)),
            detail(ProbeReason::Verified, Some(1), Some(100)),
            detail(ProbeReason::Timeout, Some(0), Some(1)),
            detail(ProbeReason::Verified, Some(0), None),
        ];
        assert_eq!(latency_stats(&details, 0), Some((10, 20)));
        assert_eq!(latency_stats(&details, 1), Some((100, 100)));
        assert_eq!(latency_stats(&details, 2), None);
    }
}
