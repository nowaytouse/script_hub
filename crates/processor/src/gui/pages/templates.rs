use super::{card, primary_button};
use crate::gui::app::HubApp;
use crate::gui::i18n::{Text, text};
use crate::gui::templates::{
    CONFIRM_ID, EDIT_CONTENT_ID, EDIT_NAME_ID, PASSWORD_ID, TemplateState,
};
use crate::runtime::settings::Language;
use crate::vault::Client;
use eframe::egui::{self, RichText};

impl HubApp {
    pub(crate) fn templates_page(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        let colors = self.colors;
        let workspace = self.loaded_root.clone();
        let project = self.template_project_root();
        let state = &mut self.templates;
        if let Some(project) = project {
            state.visit_project(&project);
        } else {
            state.visit_without_project();
        }
        ui.heading(tr(Text::Templates));
        ui.label(tr(Text::VaultIntro));
        ui.weak(tr(Text::VaultPublicGateHelp));
        ui.hyperlink_to(
            tr(Text::Documentation),
            if language.resolved() == Language::ZhCn {
                "https://github.com/nowaytouse/script_hub/blob/master/docs/TEMPLATES.zh-CN.md"
            } else {
                "https://github.com/nowaytouse/script_hub/blob/master/docs/TEMPLATES.md"
            },
        );
        ui.add_space(14.0);
        if let Some(error) = &state.error {
            ui.colored_label(colors.error, error);
            ui.add_space(8.0);
        }
        if let Some(notice) = state.notice {
            ui.label(tr(notice));
            ui.add_space(8.0);
        }
        if state.busy() {
            card(ui, colors, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(tr(Text::VaultWorking));
                });
                let lock = ui.button(tr(Text::VaultLock));
                if lock.clicked() {
                    state.request_lock(ui.ctx(), lock.id);
                }
            });
            show_lock_confirmation(ui.ctx(), state, colors.error, &tr);
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("templates-page")
            .show(ui, |ui| {
                if !state.unlocked() {
                    card(ui, colors, |ui| {
                        ui.label(
                            RichText::new(tr(if state.preparing() {
                                Text::VaultPrepare
                            } else {
                                Text::VaultLocked
                            }))
                            .size(20.0)
                            .strong(),
                        );
                        ui.add_space(10.0);
                        let previous_mode = if state.preparing() {
                            2
                        } else if state.creating {
                            1
                        } else {
                            0
                        };
                        let mut mode = previous_mode;
                        ui.horizontal_wrapped(|ui| {
                            ui.selectable_value(&mut mode, 0, tr(Text::VaultOpenExisting));
                            ui.selectable_value(&mut mode, 1, tr(Text::VaultCreateNew));
                            ui.selectable_value(&mut mode, 2, tr(Text::VaultPrepare));
                        });
                        if mode != previous_mode {
                            state.prepare_mode = mode == 2;
                            state.creating = mode == 1;
                            if mode != 0 {
                                state.use_originals = false;
                            }
                            state.clear_passwords(ui.ctx());
                            state.error = None;
                            state.notice = None;
                        }
                        ui.add_space(10.0);
                        ui.weak(tr(Text::VaultManagedStorage));
                        if !state.storage_available() {
                            ui.weak(tr(Text::VaultStorageUnavailable));
                        }
                        if state.preparing() {
                            ui.label(tr(Text::VaultPrepareHelp));
                            let label = ui.label(tr(Text::VaultPrepareSources));
                            let field = egui::TextEdit::multiline(&mut state.prepare_sources)
                                .id_salt("vault-prepare-sources")
                                .desired_rows(3)
                                .desired_width(f32::INFINITY)
                                .show(ui);
                            let _ = (*field.response).clone().labelled_by(label.id);
                            ui.weak(tr(Text::VaultPrepareSourcesHelp));
                            ui.horizontal_wrapped(|ui| {
                                ui.label(tr(Text::VaultPrepareClient));
                                ui.selectable_value(
                                    &mut state.import_client,
                                    Client::Other,
                                    tr(Text::VaultDetectClient),
                                );
                                ui.selectable_value(
                                    &mut state.import_client,
                                    Client::Surge,
                                    "Surge",
                                );
                                ui.selectable_value(
                                    &mut state.import_client,
                                    Client::Shadowrocket,
                                    "Shadowrocket",
                                );
                                ui.selectable_value(
                                    &mut state.import_client,
                                    Client::FlClash,
                                    "FlClash",
                                );
                            });
                            ui.label(tr(Text::VaultPreparePublic));
                            ui.monospace(&state.prepare_public);
                            ui.label(tr(Text::VaultPrepareBackup));
                            ui.monospace(&state.prepare_backup);
                        } else {
                            if !state.creating {
                                ui.horizontal_wrapped(|ui| {
                                    if ui
                                        .selectable_label(
                                            !state.use_originals,
                                            tr(Text::VaultLibraryTemplates),
                                        )
                                        .clicked()
                                    {
                                        state.select_library(false, ui.ctx());
                                    }
                                    if state.originals_available()
                                        && ui
                                            .selectable_label(
                                                state.use_originals,
                                                tr(Text::VaultLibraryOriginals),
                                            )
                                            .clicked()
                                    {
                                        state.select_library(true, ui.ctx());
                                    }
                                });
                                ui.weak(tr(Text::VaultImportEncryptedHint));
                            }
                            ui.label(tr(Text::VaultFile));
                            if state.creating {
                                if let Some(path) = state.save_destination() {
                                    ui.monospace(path.display().to_string());
                                }
                            } else {
                                ui.monospace(&state.path);
                            }
                        }
                        ui.add_space(10.0);
                        secret_field(
                            ui,
                            &mut state.password,
                            PASSWORD_ID,
                            tr(Text::VaultPassword),
                            "",
                        );
                        if state.creating || state.preparing() {
                            ui.add_space(8.0);
                            secret_field(
                                ui,
                                &mut state.confirmation,
                                CONFIRM_ID,
                                tr(Text::VaultConfirmPassword),
                                "",
                            );
                            ui.weak(tr(Text::VaultPasswordHelp));
                        }
                        if state.queued_imports() > 0 {
                            ui.label(format!(
                                "{}: {}",
                                tr(Text::VaultQueuedImports),
                                state.queued_imports()
                            ));
                        }
                        ui.add_space(12.0);
                        if ui
                            .add_enabled(
                                !state.password.is_empty()
                                    && (!state.creating || state.storage_available()),
                                primary_button(
                                    colors,
                                    tr(if state.preparing() {
                                        Text::VaultPrepare
                                    } else if state.creating {
                                        Text::VaultCreateNew
                                    } else {
                                        Text::VaultUnlock
                                    }),
                                ),
                            )
                            .clicked()
                        {
                            state.open(ui.ctx());
                        }
                        ui.add_space(12.0);
                        ui.weak(tr(Text::VaultLockedHelp));
                    });
                    return;
                }
                card(ui, colors, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(tr(Text::VaultUnlocked)).size(20.0).strong());
                        if state.dirty {
                            ui.label(tr(Text::UnsavedChanges));
                        }
                        let lock = ui.button(tr(Text::VaultLock));
                        if lock.clicked() {
                            state.request_lock(ui.ctx(), lock.id);
                        }
                    });
                    if !state.unlocked() {
                        return;
                    }
                    ui.add_space(10.0);
                    ui.label(tr(Text::VaultFile));
                    if let Some(path) = state.save_destination() {
                        ui.monospace(path.display().to_string());
                    } else {
                        ui.weak(tr(Text::VaultStorageUnavailable));
                    }
                    ui.checkbox(&mut state.overwrite_vault, tr(Text::VaultOverwrite));
                    if ui
                        .add_enabled(
                            state.storage_available() && !state.editing,
                            primary_button(colors, tr(Text::VaultSave)),
                        )
                        .clicked()
                    {
                        state.save();
                    }
                    ui.weak(tr(Text::VaultSessionHelp));
                });
                if !state.unlocked() || state.busy() {
                    return;
                }
                ui.add_space(14.0);
                card(ui, colors, |ui| {
                    ui.label(RichText::new(tr(Text::VaultAddTemplate)).strong());
                    ui.label(tr(Text::VaultImportHint));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.import_path)
                            .id_salt("vault-import-path")
                            .desired_width(f32::INFINITY),
                    );
                    ui.horizontal_wrapped(|ui| {
                        ui.label(tr(Text::Platform));
                        ui.selectable_value(&mut state.import_client, Client::Surge, "Surge");
                        ui.selectable_value(
                            &mut state.import_client,
                            Client::Shadowrocket,
                            "Shadowrocket",
                        );
                        ui.selectable_value(&mut state.import_client, Client::FlClash, "FlClash");
                        ui.selectable_value(
                            &mut state.import_client,
                            Client::Other,
                            tr(Text::VaultOther),
                        );
                        if ui
                            .add_enabled(
                                !state.editing
                                    && (!state.import_path.trim().is_empty()
                                        || state.queued_imports() > 0),
                                egui::Button::new(tr(Text::VaultImport)),
                            )
                            .clicked()
                        {
                            state.import();
                        }
                    });
                    if state.queued_imports() > 0 {
                        ui.label(format!(
                            "{}: {}",
                            tr(Text::VaultQueuedImports),
                            state.queued_imports()
                        ));
                    }
                    ui.weak(tr(Text::VaultImportHelp));
                });
                if state.busy() {
                    return;
                }
                ui.add_space(14.0);
                let previous = state.selected;
                card(ui, colors, |ui| {
                    ui.label(RichText::new(tr(Text::VaultChooseTemplate)).strong());
                    if let Some(vault) = state.vault.as_ref() {
                        if vault.library().templates.is_empty() {
                            ui.label(tr(Text::VaultEmpty));
                        }
                        ui.add_enabled_ui(!state.editing, |ui| {
                            for (index, template) in vault.library().templates.iter().enumerate() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.selectable_value(&mut state.selected, index, &template.name);
                                    ui.weak(client_label(language, template.client));
                                });
                            }
                        });
                    }
                });
                if state.selected != previous {
                    state.refresh_selection(ui.ctx());
                }
                if state.selected_template().is_none() {
                    return;
                }
                ui.add_space(14.0);
                if !state.editing && ui.button(tr(Text::VaultEditTemplate)).clicked() {
                    state.edit_selected(ui.ctx());
                }
                if state.editing {
                    card(ui, colors, |ui| {
                        ui.label(RichText::new(tr(Text::VaultEditTemplate)).strong());
                        ui.weak(tr(Text::VaultEditHelp));
                        let label = ui.label(tr(Text::VaultEditName));
                        let mut name = egui::TextEdit::singleline(&mut *state.edit_name)
                            .id(egui::Id::unique(EDIT_NAME_ID))
                            .char_limit(256)
                            .desired_width(f32::INFINITY)
                            .show(ui);
                        let _ = (*name.response).clone().labelled_by(label.id);
                        name.state.clear_undoer();
                        name.state.store(ui.ctx(), name.response.id);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(tr(Text::Platform));
                            ui.selectable_value(&mut state.edit_client, Client::Surge, "Surge");
                            ui.selectable_value(
                                &mut state.edit_client,
                                Client::Shadowrocket,
                                "Shadowrocket",
                            );
                            ui.selectable_value(&mut state.edit_client, Client::FlClash, "FlClash");
                            ui.selectable_value(
                                &mut state.edit_client,
                                Client::Other,
                                tr(Text::VaultOther),
                            );
                        });
                        let label = ui.label(tr(Text::VaultEditContent));
                        egui::ScrollArea::vertical()
                            .id_salt("vault-template-editor")
                            .max_height(320.0)
                            .show(ui, |ui| {
                                let mut content = egui::TextEdit::multiline(&mut *state.preview)
                                    .id(egui::Id::unique(EDIT_CONTENT_ID))
                                    .font(egui::TextStyle::Monospace)
                                    .char_limit(crate::vault::MAX_TEMPLATE_BYTES)
                                    .desired_rows(12)
                                    .desired_width(f32::INFINITY)
                                    .show(ui);
                                let _ = (*content.response).clone().labelled_by(label.id);
                                content.state.clear_undoer();
                                content.state.store(ui.ctx(), content.response.id);
                            });
                        ui.horizontal_wrapped(|ui| {
                            if ui.button(tr(Text::VaultApplyEdit)).clicked() {
                                state.apply_edit(ui.ctx());
                            }
                            if ui.button(tr(Text::Cancel)).clicked() {
                                state.cancel_edit(ui.ctx());
                            }
                        });
                    });
                    return;
                }
                card(ui, colors, |ui| {
                    ui.label(RichText::new(tr(Text::VaultConfigure)).strong());
                    let fl_clash = state
                        .selected_template()
                        .is_some_and(|template| template.client == Client::FlClash);
                    if fl_clash {
                        ui.weak(tr(Text::VaultFlClashHelp));
                    }
                    for (index, (slot, value)) in state.subscriptions.iter_mut().enumerate() {
                        let label = if slot.is_empty() {
                            tr(Text::VaultSeparateSubscription).to_owned()
                        } else {
                            format!("{} {}", tr(Text::VaultSubscription), index + 1)
                        };
                        secret_field(
                            ui,
                            value,
                            &format!("template-vault-url-{index}"),
                            &label,
                            "https://example.com/subscription",
                        );
                        if slot.is_empty() {
                            ui.weak(tr(Text::VaultShadowrocketHelp));
                        }
                        ui.add_space(8.0);
                    }
                    ui.weak(tr(Text::VaultNoFetch));
                    ui.add_space(10.0);
                    if ui
                        .checkbox(&mut state.show_preview, tr(Text::VaultShowTemplate))
                        .changed()
                    {
                        state.show_template();
                    }
                    if state.show_preview {
                        ui.weak(tr(Text::VaultTemplatePreviewHelp));
                        let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
                        egui::ScrollArea::both()
                            .id_salt("vault-template-preview")
                            .max_height(260.0)
                            .show_rows(
                                ui,
                                row_height,
                                state.preview.lines().count(),
                                |ui, rows| {
                                    for line in
                                        state.preview.lines().skip(rows.start).take(rows.len())
                                    {
                                        ui.add(
                                            egui::Label::new(RichText::new(line).monospace())
                                                .selectable(false)
                                                .wrap_mode(egui::TextWrapMode::Extend),
                                        );
                                    }
                                },
                            );
                    }
                    ui.add_space(12.0);
                    if fl_clash {
                        ui.checkbox(
                            &mut state.windows_dns_guard,
                            tr(Text::VaultWindowsStrictDns),
                        );
                        if state.windows_dns_guard {
                            ui.weak(tr(Text::VaultWindowsStrictDnsHelp));
                        }
                    }
                    ui.checkbox(&mut state.allow_plaintext, tr(Text::VaultPlaintextConsent));
                    ui.weak(tr(Text::VaultClipboardWarning));
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(
                                state.allow_plaintext,
                                egui::Button::new(tr(Text::VaultCopyConfiguration)),
                            )
                            .clicked()
                        {
                            state.copy(ui.ctx());
                        }
                        if state.subscriptions.iter().any(|(slot, _)| slot.is_empty())
                            && ui
                                .add_enabled(
                                    state.allow_plaintext,
                                    egui::Button::new(tr(Text::VaultCopySubscription)),
                                )
                                .clicked()
                        {
                            state.copy_subscription(ui.ctx());
                        }
                    });
                    ui.add_space(12.0);
                    ui.label(tr(Text::VaultExportFile));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.export_path)
                            .id_salt("vault-export-path")
                            .desired_width(f32::INFINITY)
                            .hint_text(tr(if fl_clash {
                                Text::VaultExportYamlHint
                            } else {
                                Text::VaultExportHint
                            })),
                    );
                    if fl_clash {
                        ui.weak(tr(Text::VaultFlClashExportHelp));
                    }
                    ui.checkbox(&mut state.overwrite_export, tr(Text::VaultOverwrite));
                    if ui
                        .add_enabled(
                            state.allow_plaintext && !state.export_path.trim().is_empty(),
                            egui::Button::new(tr(Text::VaultExport)),
                        )
                        .clicked()
                    {
                        state.export(workspace.as_deref());
                    }
                    ui.add_space(14.0);
                    if !state.remove_pending {
                        if ui
                            .add(
                                egui::Button::new(tr(Text::VaultRemove))
                                    .fill(colors.surface)
                                    .stroke(egui::Stroke::new(1.0, colors.error)),
                            )
                            .clicked()
                        {
                            state.remove_pending = true;
                        }
                    } else {
                        ui.colored_label(colors.error, tr(Text::VaultRemoveConfirm));
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(tr(Text::VaultRemove))
                                        .fill(colors.surface)
                                        .stroke(egui::Stroke::new(1.0, colors.error)),
                                )
                                .clicked()
                            {
                                state.remove_selected(ui.ctx());
                            }
                            if ui.button(tr(Text::Cancel)).clicked() {
                                state.remove_pending = false;
                            }
                        });
                    }
                });
            });
        show_lock_confirmation(ui.ctx(), state, colors.error, &tr);
    }
}

fn client_label(language: Language, client: Client) -> &'static str {
    match client {
        Client::Surge => "Surge",
        Client::Shadowrocket => "Shadowrocket",
        Client::FlClash => "FlClash",
        Client::Other => text(language, Text::VaultOther),
    }
}

fn show_lock_confirmation(
    ctx: &egui::Context,
    state: &mut TemplateState,
    error_color: egui::Color32,
    tr: &impl Fn(Text) -> &'static str,
) {
    let Some(opener) = state.lock_confirmation else {
        return;
    };
    if !state.dirty && !state.editing {
        state.lock_confirmation = None;
        return;
    }
    let modal = egui::Modal::new(egui::Id::unique("vault-confirm-lock")).show(ctx, |ui| {
        ui.set_max_width(380.0);
        ui.heading(tr(Text::UnsavedChanges));
        ui.label(tr(if state.editing {
            Text::VaultLockEditHelp
        } else {
            Text::VaultLockSaveHelp
        }));
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            let cancel = ui.button(tr(Text::Cancel));
            let discard = ui.add(
                egui::Button::new(tr(Text::VaultDiscardAndLock))
                    .stroke(egui::Stroke::new(1.0, error_color)),
            );
            if ui.is_visible() && !cancel.has_focus() && !discard.has_focus() {
                cancel.request_focus();
            }
            (cancel.clicked(), discard.clicked())
        })
        .inner
    });
    let (cancel, discard) = modal.inner;
    if cancel || modal.should_close() {
        state.lock_confirmation = None;
        ctx.memory_mut(|memory| memory.request_focus(opener));
    } else if discard {
        state.lock(ctx);
    }
}

fn secret_field(ui: &mut egui::Ui, value: &mut String, id: &str, label: &str, hint: &str) {
    let label = ui.label(label);
    let mut output = egui::TextEdit::singleline(value)
        .id(egui::Id::unique(id))
        .password(true)
        .hint_text(hint)
        .desired_width(f32::INFINITY)
        .show(ui);
    let _ = (*output.response).clone().labelled_by(label.id);
    output.state.clear_undoer();
    output.state.store(ui.ctx(), output.response.id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flclash_is_visible_in_preparation_import_editing_and_template_labels() {
        assert_eq!(client_label(Language::En, Client::FlClash), "FlClash");
        for workflow in 0..3 {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut app = crate::gui::app::test_app(Default::default());
            if workflow == 0 {
                app.templates.prepare_mode = true;
            } else {
                app.templates.vault = Some(
                    crate::vault::create(
                        crate::vault::Library {
                            templates: vec![crate::vault::Template {
                                name: "Synthetic YAML fixture".to_owned(),
                                client: Client::FlClash,
                                content: "proxy-providers: {}\nproxy-groups: []\nrules: []\n"
                                    .to_owned(),
                            }],
                        },
                        "synthetic client selection password",
                    )
                    .unwrap(),
                );
                if workflow == 2 {
                    app.templates.edit_selected(&ctx);
                }
            }
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1160.0, 2600.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| app.templates_page(ui));
                },
            );
            let count = output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .filter(|(_, node)| node.label() == Some("FlClash"))
                .count();
            output.platform_output.accesskit_update = None;
            output.drop_without_applying_deltas();
            let expected_selectors = if workflow == 2 { 2 } else { 1 };
            assert!(
                count >= expected_selectors,
                "workflow {workflow}: {count} FlClash labels"
            );
        }
    }

    #[test]
    fn lock_confirmation_defaults_to_cancel_and_preserves_the_draft() {
        for key in [egui::Key::Enter, egui::Key::Escape] {
            let ctx = egui::Context::default();
            let mut state = TemplateState::default();
            state.preview.push_str("synthetic uncommitted draft");
            state.editing = true;
            state.request_lock(&ctx, egui::Id::unique("lock-opener"));
            let draw = |state: &mut TemplateState, events| {
                let output = ctx.run_ui(
                    egui::RawInput {
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        show_lock_confirmation(ui.ctx(), state, egui::Color32::RED, &|key| {
                            text(Language::En, key)
                        });
                    },
                );
                output.drop_without_applying_deltas();
            };
            draw(&mut state, vec![]);
            draw(&mut state, vec![]);
            assert!(state.lock_confirmation.is_some());
            draw(
                &mut state,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            assert!(
                state.lock_confirmation.is_none(),
                "{key:?} cancels by default"
            );
            assert!(state.editing);
            assert_eq!(state.preview.as_str(), "synthetic uncommitted draft");
        }
    }

    #[test]
    fn password_widget_does_not_retain_undo_history() {
        let ctx = egui::Context::default();
        let mut password = "synthetic-password-fixture".to_owned();
        let mut initial = egui::text_edit::TextEditState::default();
        let mut undoer = initial.undoer();
        undoer.add_undo(&(Default::default(), "old fixture".to_owned()));
        undoer.add_undo(&(Default::default(), "new fixture".to_owned()));
        initial.set_undoer(undoer);
        initial.store(&ctx, egui::Id::unique(PASSWORD_ID));
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                secret_field(ui, &mut password, PASSWORD_ID, "Password", "")
            });
        });
        output.drop_without_applying_deltas();
        let state =
            egui::text_edit::TextEditState::load(&ctx, egui::Id::unique(PASSWORD_ID)).unwrap();
        let current = (Default::default(), String::new());
        assert!(!state.undoer().has_undo(&current) && !state.undoer().has_redo(&current));
    }
}
