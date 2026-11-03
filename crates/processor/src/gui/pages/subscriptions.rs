use super::{card, primary_button, subscription_list};
use crate::gui::{
    app::HubApp,
    i18n::{Text, text},
    relay::RelayStep,
    subscriptions::{Action, Selection},
};
use eframe::egui;
use zeroize::Zeroize;

impl HubApp {
    pub(crate) fn relay_page(&mut self, ui: &mut egui::Ui) {
        self.relay.workspace.check_storage();
        let language = self.draft.language;
        let colors = self.colors;
        let tr = |key| text(language, key);
        ui.heading(tr(Text::Relay));
        ui.weak(tr(Text::WorkspaceIntro));
        ui.add_space(12.0);
        if let Some(error) = self.relay.workspace.error {
            ui.colored_label(colors.error, tr(error));
        }
        if let Some(notice) = self.relay.workspace.notice {
            ui.label(tr(notice));
        }
        if let Some(summary) = self.relay.workspace.import_summary {
            ui.label(format!(
                "{}: {} / {} / {}",
                tr(Text::WorkspaceImportSummary),
                summary.subscriptions,
                summary.collections,
                summary.skipped
            ));
        }
        if self.relay.workspace.busy() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tr(Text::RelayWorking));
            });
            if let Some(progress) = self.relay.workspace.progress() {
                ui.label(format!("{} / {}", progress.checked, progress.total));
                if ui.button(tr(Text::RelayCancel)).clicked() {
                    self.relay
                        .workspace
                        .cancel
                        .store(true, std::sync::atomic::Ordering::Release);
                }
            }
            return;
        }
        if self.relay.workspace.locked || self.relay.workspace.password_form {
            card(ui, colors, |ui| {
                let locked = self.relay.workspace.locked;
                ui.strong(tr(if locked {
                    Text::WorkspaceUnlock
                } else {
                    Text::WorkspaceCreateStorage
                }));
                ui.weak(tr(Text::WorkspaceStorageHelp));
                password_field(
                    ui,
                    &mut self.relay.workspace.password,
                    "workspace-password",
                    tr(Text::WorkspacePassword),
                );
                if !locked {
                    password_field(
                        ui,
                        &mut self.relay.workspace.confirmation,
                        "workspace-password-confirm",
                        tr(Text::WorkspacePasswordConfirm),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(primary_button(
                            colors,
                            tr(if locked {
                                Text::WorkspaceUnlock
                            } else {
                                Text::WorkspaceCreateStorage
                            }),
                        ))
                        .clicked()
                    {
                        if locked {
                            self.relay.workspace.unlock(ui.ctx());
                        } else {
                            self.relay.workspace.save_storage(ui.ctx());
                        }
                    }
                    if !locked && ui.button(tr(Text::RelayCancel)).clicked() {
                        self.relay.workspace.password_form = false;
                        self.relay.workspace.cancel_after_save();
                        self.relay.workspace.password.zeroize();
                        self.relay.workspace.confirmation.zeroize();
                    }
                });
            });
            if self.relay.workspace.locked {
                return;
            }
            ui.add_space(12.0);
        }
        let wide = ui.available_width() >= 980.0 && self.relay.workspace.show_editor;
        let allow_actions =
            !self.active() && self.pending.is_none() && !self.relay.schedule.armed();
        let mut action = None;
        if wide {
            let width = ui.available_width();
            let height = ui.available_height();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(260.0, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let state = &mut self.relay.workspace;
                        action =
                            subscription_list::show(ui, language, colors, state, allow_actions);
                    },
                );
                ui.separator();
                ui.allocate_ui_with_layout(
                    egui::vec2((width - 280.0).max(320.0), height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        self.subscription_detail(ui);
                    },
                );
            });
        } else if self.relay.workspace.show_editor {
            self.subscription_detail(ui);
        } else {
            let state = &mut self.relay.workspace;
            action = subscription_list::show(ui, language, colors, state, allow_actions);
        }
        if let Some(action) = action {
            self.relay.workspace_request(ui.ctx(), action);
        }
        if let Some(action) = self.relay.workspace.confirm
            && !self.relay.busy()
        {
            let modal =
                egui::Modal::new(egui::Id::unique("workspace-confirm")).show(ui.ctx(), |ui| {
                    ui.set_max_width(360.0);
                    ui.heading(tr(if action == Action::Delete {
                        Text::WorkspaceDelete
                    } else {
                        Text::WorkspaceUnsaved
                    }));
                    ui.label(tr(if action == Action::Delete {
                        Text::WorkspaceDeleteHelp
                    } else {
                        Text::WorkspaceUnsavedHelp
                    }));
                    ui.horizontal_wrapped(|ui| {
                        if action != Action::Delete
                            && ui.button(tr(Text::WorkspaceSaveContinue)).clicked()
                        {
                            self.relay.workspace_save_and_continue(ui.ctx());
                        }
                        if ui
                            .button(tr(if action == Action::Delete {
                                Text::WorkspaceDelete
                            } else {
                                Text::WorkspaceDiscard
                            }))
                            .clicked()
                        {
                            self.relay.workspace.draft_dirty = false;
                            self.relay.execute_workspace(ui.ctx(), action);
                        }
                        if ui.button(tr(Text::RelayCancel)).clicked() {
                            self.relay.workspace.confirm = None;
                        }
                    });
                });
            if modal.should_close() {
                self.relay.workspace.confirm = None;
            }
        }
    }

    fn subscription_detail(&mut self, ui: &mut egui::Ui) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        let colors = self.colors;
        let busy = self.active() || self.pending.is_some() || self.relay.schedule.armed();
        ui.add_enabled_ui(!busy, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(tr(Text::WorkspaceBack)).clicked() { self.relay.workspace_request(ui.ctx(), Action::Back); }
                if self.relay.workspace.draft_dirty { ui.colored_label(colors.warning, tr(Text::WorkspaceUnsaved)); }
                if ui.button(tr(Text::WorkspaceSave)).clicked() && self.relay.save_workspace_draft()
                    && (self.relay.workspace.dirty || !self.relay.workspace.stored) {
                    self.relay.workspace.save_storage(ui.ctx());
                }
                if matches!(self.relay.workspace.selected, Some(Selection::Subscription(id) | Selection::Collection(id)) if id != 0)
                    && ui.button(tr(Text::WorkspaceDelete)).clicked() { self.relay.workspace_request(ui.ctx(), Action::Delete); }
            });
            let label = ui.label(tr(Text::WorkspaceName));
            let mut edit = egui::TextEdit::singleline(&mut *self.relay.workspace.name).id(egui::Id::unique("workspace-item-name")).char_limit(128).desired_width(ui.available_width()).show(ui);
            let _ = (*edit.response).clone().labelled_by(label.id);
            if edit.response.changed() { self.relay.workspace.draft_dirty = true; }
            edit.state.clear_undoer(); edit.state.store(ui.ctx(), edit.response.id);
        });
        ui.add_space(8.0);
        if matches!(
            self.relay.workspace.selected,
            Some(Selection::Collection(_))
        ) && self.relay.step == RelayStep::Source
        {
            ui.horizontal_wrapped(|ui| {
                let _ = ui.selectable_label(true, tr(Text::WorkspaceSourceTab));
                if ui
                    .add_enabled(
                        self.relay.editor.is_some(),
                        egui::Button::new(tr(Text::WorkspaceOperationsTab)),
                    )
                    .clicked()
                {
                    self.relay.step = RelayStep::Review;
                }
                if ui
                    .add_enabled(!busy, egui::Button::new(tr(Text::WorkspaceOutputTab)))
                    .clicked()
                {
                    self.relay.step = RelayStep::Publish;
                }
            });
            ui.strong(tr(Text::WorkspaceMembers));
            ui.weak(tr(Text::WorkspaceMembersHelp));
            if let Some(error) = &self.relay.error {
                ui.colored_label(
                    colors.error,
                    crate::gui::relay::failure_message(language, error),
                );
            }
            let cooldown = self.relay.cooldown_seconds();
            if cooldown > 0 {
                ui.weak(format!("{} {cooldown} s", tr(Text::RelayRetryWait)));
            }
            let mut changed = false;
            let entries: Vec<_> = self
                .relay
                .workspace
                .data
                .subscriptions
                .iter()
                .map(|e| (e.id, e.name.clone(), e.content.is_empty()))
                .collect();
            egui::ScrollArea::vertical()
                .id_salt("workspace-members")
                .max_height(300.0)
                .show_rows(ui, 32.0, entries.len(), |ui, range| {
                    for (id, name, missing) in &entries[range] {
                        let position = self
                            .relay
                            .workspace
                            .members
                            .iter()
                            .position(|member| member == id);
                        let mut checked = position.is_some();
                        let label = format!(
                            "{}{name}{}",
                            position.map_or_else(String::new, |i| format!("{}. ", i + 1)),
                            if *missing {
                                format!(" · {}", tr(Text::WorkspaceNotFetched))
                            } else {
                                String::new()
                            }
                        );
                        if ui
                            .add_enabled(!busy, egui::Checkbox::new(&mut checked, label))
                            .changed()
                        {
                            if checked {
                                self.relay.workspace.members.push(*id);
                            } else {
                                self.relay.workspace.members.retain(|member| member != id);
                            }
                            changed = true;
                        }
                    }
                });
            if changed {
                if let Some(editor) = &self.relay.editor {
                    self.relay.workspace.collection_options = editor.processing();
                }
                self.relay.clear_editor_widgets(ui.ctx());
                self.relay.editor = None;
                self.relay.invalidate_result();
                self.relay.workspace.members_changed = true;
            }
            if entries.is_empty() {
                ui.label(tr(Text::WorkspaceListEmptyCollections));
            }
            ui.horizontal_wrapped(|ui| {
                let enabled = !busy && !self.relay.workspace.members.is_empty();
                if ui
                    .add_enabled(
                        enabled && self.relay.cooldown_seconds() == 0,
                        primary_button(colors, tr(Text::WorkspaceRefreshBuild)),
                    )
                    .clicked()
                {
                    self.relay.prepare_collection(true);
                }
                if ui
                    .add_enabled(enabled, egui::Button::new(tr(Text::WorkspaceBuildCached)))
                    .clicked()
                {
                    self.relay.prepare_collection(false);
                }
            });
        } else {
            self.relay_editor(ui);
        }
    }
}

fn password_field(ui: &mut egui::Ui, value: &mut String, id: &str, label: &str) {
    let label = ui.label(label);
    let mut edit = egui::TextEdit::singleline(value)
        .password(true)
        .char_limit(1024)
        .id(egui::Id::unique(id))
        .desired_width(ui.available_width().min(440.0))
        .show(ui);
    let _ = (*edit.response).clone().labelled_by(label.id);
    edit.state.clear_undoer();
    edit.state.store(ui.ctx(), edit.response.id);
}
