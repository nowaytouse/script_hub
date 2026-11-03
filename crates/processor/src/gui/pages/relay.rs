use super::{card, primary_button};
use crate::gui::app::HubApp;
use crate::gui::file_dialog::FileAction;
use crate::gui::i18n::{Text, text};
use crate::gui::relay::{InputMode, RelayStep, SECRET_IDS};
use crate::relay::{RelayStage, RunMode};
use eframe::egui;
use zeroize::Zeroize;

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
        ui.vertical_centered(|ui| {
            ui.set_max_width(880.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                let state = &mut self.relay;
                ui.horizontal_wrapped(|ui| {
                    for (step, label, available) in [
                        (RelayStep::Source, Text::WorkspaceSourceTab, true),
                        (
                            RelayStep::Review,
                            Text::WorkspaceOperationsTab,
                            state.result.is_some() || state.local_content.is_some(),
                        ),
                        (
                            RelayStep::Publish,
                            Text::WorkspaceOutputTab,
                            state.result.is_some() || state.workspace.selected.is_some(),
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
                if state.schedule.armed() || state.schedule.running() {
                    card(ui, colors, |ui| {
                        ui.strong(tr(if state.schedule.armed() { Text::ScheduleActive } else { Text::RelayCancelling }));
                        if let Some(seconds) = state.schedule.seconds_until_due() {
                            ui.label(format!("{} {:02}:{:02}:{:02}", tr(Text::ScheduleNext), seconds / 3600, seconds / 60 % 60, seconds % 60));
                        } else if state.schedule.running() {
                            ui.label(tr(Text::ScheduleRunning));
                        }
                        if let Some(result) = state.schedule.result {
                            ui.label(format!("{} · {} {} · {} {}", tr(if result.unchanged { Text::RelayUnchanged } else { Text::RelayPublished }), tr(Text::RelayTotal), result.total, tr(Text::RelayRemoved), result.removed));
                        }
                        ui.weak(tr(Text::ScheduleSessionOnly));
                        if ui.button(tr(Text::ScheduleStop)).clicked() {
                            state.schedule.stop();
                        }
                    });
                    ui.add_space(8.0);
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
                        ui.add_enabled_ui(admitted && !state.schedule.armed(), |ui| {
                            card(ui, colors, |ui| match state.step {
                                RelayStep::Source => {
                                    ui.label(
                                        egui::RichText::new(tr(Text::RelayImportTitle))
                                            .size(20.0)
                                            .strong(),
                                    );
                                    ui.add_space(8.0);
                                    ui.horizontal_wrapped(|ui| {
                                        for (mode, key) in [(InputMode::Link, Text::InputLink), (InputMode::Paste, Text::InputPaste), (InputMode::File, Text::RelayLocalSource)] {
                                            if ui.selectable_label(state.input_mode == mode, tr(key)).clicked() {
                                                state.input_mode = mode;
                                                state.clear_request_error();
                                            }
                                        }
                                    });
                                    ui.add_space(8.0);
                                    if state.input_mode == InputMode::Link {
                                    ui.strong(tr(Text::RelayGistSource));
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
                                                .selectable_label(
                                                    state.source_file.as_str() == name,
                                                    &name,
                                                )
                                                .clicked()
                                            {
                                                state.source_file = zeroize::Zeroizing::new(name);
                                                state.local_content = None;
                                                state.local_summary = None;
                                                state.clear_editor_widgets(ui.ctx());
                                                state.input_changed();
                                                state.error = None;
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
                                        }
                                        ui.weak(tr(Text::RelayTokenHelp));
                                    });
                                    ui.add_space(8.0);
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
                                        if !state.source.trim().is_empty() { state.local_content = None; state.local_summary = None; }
                                        action = Some(RunMode::Import);
                                    }
                                    } else if state.input_mode == InputMode::Paste {
                                        ui.label(tr(Text::InputPasteHelp));
                                        ui.checkbox(&mut state.reveal_paste, tr(Text::InputReveal));
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
                                        edit.state.clear_undoer();
                                        edit.state.store(ui.ctx(), edit.response.id);
                                        edit.response.changed()
                                            }).inner;
                                        if changed {
                                            state.clear_editor_widgets(ui.ctx());
                                            state.input_changed();
                                            state.local_content = None;
                                            state.local_summary = None;
                                        }
                                        if ui.add_enabled(!state.paste.trim().is_empty(), primary_button(colors, tr(Text::RelayImportGist))).clicked() {
                                            state.import_paste(ui.ctx());
                                        }
                                    } else {
                                    ui.strong(tr(Text::RelayLocalSource));
                                    ui.weak(tr(Text::RelayDropFile));
                                    if ui.button(tr(Text::RelayImportFile)).clicked() {
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
                                         ui.label(format!("{} {} · {} {} · {} {}", report.items.len(), tr(Text::MigrationItems), report.collections.len(), tr(Text::MigrationCollections), report.unsupported_count, tr(Text::MigrationUnsupported)));
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
                                    ui.collapsing(tr(Text::OperationPlan), |ui| {
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
                                    });
                                    let needs_resolution = state.needs_resolution();
                                    if needs_resolution {
                                        let providers = state.resolution_providers().into_iter().map(|provider| provider.name()).collect::<Vec<_>>().join(", ");
                                        ui.weak(format!("{}: {providers}", tr(Text::ResolutionProviders)));
                                        ui.checkbox(
                                            &mut state.allow_resolution,
                                            tr(Text::ResolutionConsent),
                                        );
                                    }
                                    ui.collapsing(tr(Text::RelayAdvanced), |ui| {
                                        ui.weak(tr(Text::RelayStrictProbeHelp));
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
                                    ui.add_space(12.0);
                                    ui.horizontal_wrapped(|ui| {
                                        let ready = state.editor.is_some()
                                            && cooldown == 0
                                            && (!needs_resolution || state.allow_resolution);
                                        if ui
                                            .add_enabled(
                                                ready,
                                                primary_button(colors, tr(if needs_resolution {
                                                    Text::ResolutionPrepare
                                                } else {
                                                    Text::RelayPrepareOnly
                                                })),
                                            )
                                            .clicked()
                                        {
                                            state.test_nodes = false;
                                            action = Some(RunMode::Preview);
                                        }
                                        if ui
                                            .add_enabled(
                                                ready && state.selected_engine.is_some(),
                                                egui::Button::new(tr(Text::RelayPreview)),
                                            )
                                            .clicked()
                                        {
                                            state.test_nodes = true;
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
                                    let has_content = state
                                        .result
                                        .as_ref()
                                        .is_some_and(|result| !result.content.is_empty());
                                    ui.horizontal_wrapped(|ui| {
                                        if ui
                                            .add_enabled(
                                                state.allow_plaintext && has_content,
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
                                                    state.allow_plaintext && has_content,
                                                    egui::Button::new(tr(label)),
                                                )
                                                .clicked()
                                            {
                                                file_action = Some(export);
                                            }
                                        }
                                    });
                                    ui.weak(tr(Text::RelayBackupWarning));
                                     ui.collapsing(tr(Text::ArchiveExport), |ui| {
                                         ui.weak(tr(Text::ArchiveExportHelp));
                                         visible_field(ui, &mut state.archive_recipient, SECRET_IDS[7], tr(Text::ArchiveRecipient));
                                         if ui.add_enabled(has_content && !state.archive_recipient.trim().is_empty(), egui::Button::new(tr(Text::ArchiveExport))).clicked() {
                                             file_action = Some(FileAction::ExportEncrypted);
                                         }
                                     });
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
                                    if state.workspace.selected.is_some() {
                                        ui.weak(tr(Text::WorkspaceOutputSavedHelp));
                                    }
                                    ui.collapsing(tr(Text::RelayOutputOptions), |ui| {
                                        let file_changed = visible_field(
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
                                        let title_changed = visible_field(
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
                                        let target_changed = secret_field(
                                            ui,
                                            &mut state.output,
                                            SECRET_IDS[3],
                                            tr(Text::RelayOutput),
                                            &mut state.reveal_output,
                                            tr(Text::RelayShow),
                                        );
                                        if file_changed || title_changed || target_changed {
                                            state.workspace.draft_dirty |= state.workspace.selected.is_some();
                                            state.allow_public_output = false;
                                            state.acknowledge_publish = false;
                                        }
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
                                            state.workspace.draft_dirty |= state.workspace.selected.is_some();
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
                                            publish.is_ok() && cooldown == 0 && has_content,
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
                                    ui.add_space(16.0);
                                    ui.separator();
                                    ui.strong(tr(Text::ScheduleTitle));
                                        if matches!(state.workspace.selected, Some(crate::gui::subscriptions::Selection::Collection(_))) { ui.weak(tr(Text::WorkspaceCollectionSchedule)); }
                                    ui.weak(tr(Text::ScheduleHelp));
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(tr(Text::ScheduleInterval));
                                        if ui.add(egui::DragValue::new(&mut state.schedule.interval_minutes).range(1..=10080)).changed() {
                                            state.workspace.draft_dirty |= state.workspace.selected.is_some();
                                        }
                                    });
                                    ui.weak(tr(Text::ScheduleSessionOnly));
                                        if ui.add_enabled(!matches!(state.workspace.selected, Some(crate::gui::subscriptions::Selection::Collection(_))) && has_content && publish.is_ok() && !state.output.is_empty() && cooldown == 0, egui::Button::new(tr(Text::ScheduleEnable))).clicked() {
                                        state.arm_schedule();
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
                            if ui.button(tr(if state.workspace.selected.is_some() { Text::WorkspaceListLock } else { Text::RelayClear })).clicked() {
                                if state.workspace.selected.is_some() { state.workspace_request(ui.ctx(), crate::gui::subscriptions::Action::Lock); } else { state.clear(ui.ctx()); }
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
        ui.label(format!("{}: {}", tr(Text::RelayExcluded), result.excluded));
        ui.label(format!(
            "{}: {}",
            tr(Text::RelayDuplicates),
            result.duplicates
        ));
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

#[cfg(test)]
mod tests {
    use super::latency_stats;
    use crate::relay::engine::{ProbeDetail, ProbeReason};

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
