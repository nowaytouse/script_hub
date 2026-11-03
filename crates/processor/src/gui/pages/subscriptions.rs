use super::{card, primary_button, subscription_list};
use crate::gui::{
    app::HubApp,
    i18n::{Text, text},
    relay::{RelayState, RelayStep},
    subscriptions::{Action, Selection, State},
};
use eframe::egui;
use zeroize::Zeroize;

impl HubApp {
    pub(crate) fn relay_page(&mut self, ui: &mut egui::Ui) {
        if !self.relay.demo_mode {
            self.relay.workspace.check_storage();
        }
        let language = self.draft.language;
        let colors = self.colors;
        let tr = |key| text(language, key);
        let allow_actions =
            !self.active() && self.pending.is_none() && !self.relay.schedule.armed();
        let locked = self.relay.workspace.locked;
        let mut demo = false;
        let mut library_action = None;
        ui.horizontal_wrapped(|ui| {
            ui.heading(tr(Text::Relay));
            self.demo_launcher(ui);
            if !locked && !self.relay.demo_mode {
                ui.menu_button(tr(Text::WorkspaceLibraryTools), |ui| {
                    if ui
                        .add_enabled(
                            allow_actions && can_save_library(&self.relay.workspace),
                            egui::Button::new(tr(Text::WorkspaceListSave)),
                        )
                        .clicked()
                    {
                        library_action = Some(Action::SaveLibrary);
                        ui.close();
                    }
                    if ui
                        .add_enabled(
                            allow_actions,
                            egui::Button::new(tr(Text::WorkspaceListLock)),
                        )
                        .clicked()
                    {
                        library_action = Some(Action::Lock);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(tr(Text::RelayDemoRun)).clicked() {
                        demo = true;
                        ui.close();
                    }
                    ui.label(tr(Text::RelayDemoOffline));
                });
            }
        });
        self.demo_launcher_feedback(ui);
        if let Some(action) = library_action {
            if action == Action::SaveLibrary {
                self.relay.execute_workspace(ui.ctx(), action);
            } else {
                self.relay.workspace_request(ui.ctx(), action);
            }
        }
        if locked {
            ui.weak(tr(Text::WorkspaceIntro));
            ui.horizontal_wrapped(|ui| {
                demo = ui.button(tr(Text::RelayDemoRun)).clicked();
                ui.weak(tr(Text::RelayDemoOffline));
            });
        } else if !self.relay.demo_mode {
            let workspace = &self.relay.workspace;
            let dirty = workspace.dirty || workspace.draft_dirty;
            ui.colored_label(
                if dirty { colors.warning } else { colors.muted },
                tr(if dirty {
                    Text::WorkspaceListUnsaved
                } else if workspace.stored {
                    Text::WorkspaceListSaved
                } else {
                    Text::WorkspaceListSession
                }),
            );
        }
        if demo {
            self.relay.demo_report = Some(crate::relay::demo::run());
        }
        ui.add_space(8.0);
        if let Some(report) = &self.relay.demo_report {
            let dismiss = card(ui, colors, |ui| {
                match report {
                    Ok(report) => {
                        ui.strong(tr(Text::RelayDemoPassed));
                        ui.label(format!(
                            "{} {} / {} · {} {} → {} · {} {}",
                            tr(Text::RelayDemoRefreshed),
                            report.refreshed_members,
                            report.members,
                            tr(Text::RelayDemoNodes),
                            report.input_nodes,
                            report.exported_nodes,
                            tr(Text::RelayDemoPlanned),
                            report.planned_uploads,
                        ));
                        ui.weak(tr(Text::RelayDemoChecks));
                    }
                    Err(_) => {
                        ui.colored_label(colors.error, tr(Text::RelayDemoFailed));
                    }
                }
                ui.small_button(tr(Text::Close)).clicked()
            });
            if dismiss {
                self.relay.demo_report = None;
            }
            ui.add_space(8.0);
        }
        ui.add_space(12.0);
        if let Some(error) = self.relay.workspace.error
            && self.relay.workspace.confirm.is_none()
        {
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
        if self.relay.workspace_import_pending()
            && (!self.relay.workspace.show_editor
                || matches!(
                    self.relay.workspace.selected,
                    Some(Selection::Collection(_))
                ))
        {
            ui.horizontal_wrapped(|ui| {
                ui.spinner();
                ui.label(tr(Text::RelayWorking));
            });
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
        let wide = ui.available_width() >= 840.0 && self.relay.workspace.show_editor;
        let mut action = None;
        if wide {
            let list_width = (ui.available_width() * 0.28).clamp(224.0, 300.0);
            let height = ui.available_height();
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(list_width, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_max_width(list_width);
                        let state = &mut self.relay.workspace;
                        action = subscription_list::show(
                            ui,
                            language,
                            colors,
                            state,
                            allow_actions,
                            self.relay.demo_mode,
                        );
                    },
                );
                ui.separator();
                let detail_width = ui.available_width().min(880.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(detail_width, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_max_width(detail_width);
                        self.subscription_detail(ui, false);
                    },
                );
            });
        } else if self.relay.workspace.show_editor {
            ui.set_max_width(ui.available_width().min(880.0));
            self.subscription_detail(ui, true);
        } else {
            ui.set_max_width(ui.available_width().min(680.0));
            let state = &mut self.relay.workspace;
            action = subscription_list::show(
                ui,
                language,
                colors,
                state,
                allow_actions,
                self.relay.demo_mode,
            );
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
                    if let Some(error) = self.relay.workspace.error {
                        ui.colored_label(colors.error, tr(error));
                    }
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
                            if action != Action::Import {
                                self.relay.workspace.draft_dirty = false;
                            }
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

    fn subscription_detail(&mut self, ui: &mut egui::Ui, compact: bool) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        let colors = self.colors;
        let busy = self.active() || self.pending.is_some() || self.relay.schedule.armed();
        ui.add_enabled_ui(!busy, |ui| {
            let can_save = can_save_library(&self.relay.workspace);
            let saved_item = self.relay.workspace.selected.filter(|selection| {
                matches!(selection, Selection::Subscription(id) | Selection::Collection(id) if *id != 0)
            });
            ui.horizontal_wrapped(|ui| {
                if compact && ui.button(tr(Text::WorkspaceBack)).clicked() {
                    self.relay.execute_workspace(ui.ctx(), Action::Back);
                }
                if ui
                    .add_enabled(can_save, egui::Button::new(tr(Text::WorkspaceSave)))
                    .clicked()
                {
                    self.relay.execute_workspace(ui.ctx(), Action::SaveLibrary);
                }
                if let Some(selection) = saved_item {
                    ui.menu_button(tr(Text::WorkspaceItemActions), |ui| {
                        if ui.button(tr(Text::WorkspaceDuplicate)).clicked() {
                            self.relay.workspace_request(
                                ui.ctx(),
                                Action::Duplicate(selection, tr(Text::WorkspaceDuplicateSuffix)),
                            );
                            ui.close();
                        }
                        ui.separator();
                        if ui.button(tr(Text::WorkspaceDelete)).clicked() {
                            self.relay.workspace_request(ui.ctx(), Action::Delete);
                            ui.close();
                        }
                    });
                }
            });
            let label = ui.label(tr(Text::WorkspaceName));
            let mut edit = egui::TextEdit::singleline(&mut *self.relay.workspace.name)
                .id(egui::Id::unique("workspace-item-name"))
                .char_limit(128)
                .desired_width(ui.available_width())
                .show(ui);
            let _ = (*edit.response).clone().labelled_by(label.id);
            if edit.response.changed() {
                self.relay.workspace.draft_dirty = true;
            }
            edit.state.clear_undoer();
            edit.state.store(ui.ctx(), edit.response.id);
        });
        ui.add_space(8.0);
        if matches!(
            self.relay.workspace.selected,
            Some(Selection::Collection(_))
        ) && self.relay.step == RelayStep::Source
        {
            relay_tabs(ui, &mut self.relay, &tr);
            ui.add_space(12.0);
            if let Some(error) = &self.relay.error {
                ui.colored_label(
                    colors.error,
                    crate::gui::relay::failure_message(language, error),
                );
            }
            if let Some(message) = self.relay.cooldown_message(language) {
                ui.weak(message);
            }
            let selection = self.relay.workspace.selected;
            egui::ScrollArea::vertical()
                .id_salt((
                    "workspace-collection",
                    selection.map(|selection| match selection {
                        Selection::Collection(id) | Selection::Subscription(id) => id,
                    }),
                ))
                .show(ui, |ui| {
                    card(ui, colors, |ui| self.collection_members(ui, busy));
                });
        } else {
            self.relay_editor(ui);
        }
    }

    fn collection_members(&mut self, ui: &mut egui::Ui, busy: bool) {
        let language = self.draft.language;
        let tr = |key| text(language, key);
        let colors = self.colors;
        let workspace = &self.relay.workspace;
        if workspace.data.subscriptions.is_empty() {
            ui.strong(tr(Text::WorkspaceMembers));
            ui.label(tr(Text::WorkspaceListNeedsSubscriptions));
            if ui
                .add_enabled(
                    !busy,
                    primary_button(colors, tr(Text::WorkspaceListNewSubscription)),
                )
                .clicked()
            {
                self.relay
                    .workspace_request(ui.ctx(), Action::NewSubscription);
            }
            return;
        }
        let positions: std::collections::HashMap<_, _> = workspace
            .members
            .iter()
            .copied()
            .enumerate()
            .map(|(position, id)| (id, position))
            .collect();
        let position = |id| positions.get(&id).copied();
        let mut entries: Vec<_> = workspace.data.subscriptions.iter().collect();
        entries.sort_by_key(|entry| position(entry.id).unwrap_or(usize::MAX));
        ui.strong(format!(
            "{} · {}",
            tr(Text::WorkspaceMembers),
            workspace.members.len()
        ));
        ui.label(tr(Text::WorkspaceMembersHelp));
        ui.add_space(8.0);
        enum Edit {
            Add(u64),
            Remove(usize),
            Move(usize, usize),
        }
        let mut edit = None;
        let member_count = workspace.members.len();
        egui::ScrollArea::vertical()
            .id_salt("workspace-members")
            .max_height((ui.available_height() - 72.0).clamp(120.0, 320.0))
            .show_rows(ui, 40.0, entries.len(), |ui, range| {
                for entry in &entries[range] {
                    let at = position(entry.id);
                    let title = match at {
                        Some(index) => format!("{}. {}", index + 1, entry.name),
                        None => entry.name.clone(),
                    };
                    let title = if entry.content.is_empty() {
                        format!("{title} · {}", tr(Text::WorkspaceNotFetched))
                    } else {
                        title
                    };
                    ui.push_id(entry.id, |ui| {
                        ui.horizontal(|ui| {
                            let width = if at.is_some() {
                                ui.available_width() - 2.0 * (32.0 + ui.spacing().item_spacing.x)
                            } else {
                                ui.available_width()
                            };
                            let row = ui
                                .add_enabled_ui(!busy, |ui| {
                                    ui.add_sized(
                                        [width.max(32.0), 40.0],
                                        egui::Button::new(&title).selected(at.is_some()).truncate(),
                                    )
                                })
                                .inner
                                .on_hover_text(&title);
                            row.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::Role::Button,
                                    !busy,
                                    at.is_some(),
                                    &title,
                                )
                            });
                            if row.clicked() {
                                edit = Some(at.map_or(Edit::Add(entry.id), Edit::Remove));
                            }
                            if let Some(index) = at {
                                for (glyph, key, target) in [
                                    ("↑", Text::OperationMoveUp, index.checked_sub(1)),
                                    (
                                        "↓",
                                        Text::OperationMoveDown,
                                        (index + 1 < member_count).then_some(index + 1),
                                    ),
                                ] {
                                    let enabled = !busy && target.is_some();
                                    let control = ui
                                        .add_enabled(
                                            enabled,
                                            egui::Button::new(glyph)
                                                .min_size(egui::vec2(32.0, 32.0)),
                                        )
                                        .on_hover_text(tr(key));
                                    control.widget_info(|| {
                                        egui::WidgetInfo::labeled(
                                            egui::Role::Button,
                                            enabled,
                                            format!("{}: {}", tr(key), entry.name),
                                        )
                                    });
                                    if control.clicked()
                                        && let Some(target) = target
                                    {
                                        edit = Some(Edit::Move(index, target));
                                    }
                                }
                            }
                        });
                    });
                }
            });
        if let Some(edit) = edit {
            let members = &mut self.relay.workspace.members;
            match edit {
                Edit::Add(id) => members.push(id),
                Edit::Remove(index) => {
                    members.remove(index);
                }
                Edit::Move(from, to) => members.swap(from, to),
            }
            if let Some(editor) = &self.relay.editor {
                self.relay.workspace.collection_options = editor.processing();
            }
            self.relay.clear_editor_widgets(ui.ctx());
            self.relay.editor = None;
            self.relay.local_content = None;
            self.relay.local_summary = None;
            self.relay.invalidate_result();
            self.relay.workspace.members_changed = true;
        }

        let workspace = &self.relay.workspace;
        let (has_remote, has_uncached) = workspace
            .data
            .subscriptions
            .iter()
            .filter(|entry| workspace.members.contains(&entry.id))
            .fold((false, false), |(remote, uncached), entry| {
                (
                    remote || !entry.source.trim().is_empty(),
                    uncached || entry.content.is_empty(),
                )
            });
        let has_members = !workspace.members.is_empty();
        let enabled = !busy && has_members;
        ui.add_space(8.0);
        if !has_members {
            ui.label(tr(Text::WorkspaceChooseMembers));
        } else if has_uncached {
            ui.label(tr(Text::WorkspaceCachedUnavailable));
        }
        ui.horizontal_wrapped(|ui| {
            if has_remote
                && ui
                    .add_enabled(
                        enabled && self.relay.cooldown_seconds() == 0,
                        primary_button(colors, tr(Text::WorkspaceRefreshBuild)),
                    )
                    .clicked()
            {
                self.relay.prepare_collection(true);
            }
            let cached = tr(Text::WorkspaceBuildCached);
            if ui
                .add_enabled(
                    enabled && !has_uncached,
                    if has_remote {
                        egui::Button::new(cached)
                    } else {
                        primary_button(colors, cached)
                    },
                )
                .clicked()
            {
                self.relay.prepare_collection(false);
            }
        });
    }
}

pub(super) fn relay_tabs(
    ui: &mut egui::Ui,
    state: &mut RelayState,
    tr: &impl Fn(Text) -> &'static str,
) {
    let source = if matches!(state.workspace.selected, Some(Selection::Collection(_))) {
        Text::WorkspaceMembersTab
    } else {
        Text::WorkspaceSourceTab
    };
    ui.horizontal_wrapped(|ui| {
        for (step, label) in [
            (RelayStep::Source, source),
            (RelayStep::Review, Text::WorkspaceOperationsTab),
            (
                RelayStep::Publish,
                if state.demo_mode {
                    Text::RelaySandboxFormat
                } else {
                    Text::WorkspaceOutputTab
                },
            ),
        ] {
            if ui
                .add_enabled(
                    !state.busy()
                        && !state.schedule.armed()
                        && (!state.demo_mode
                            || step != RelayStep::Source
                            || source == Text::WorkspaceMembersTab),
                    egui::Button::new(tr(label))
                        .selected(state.step == step)
                        .min_size(egui::vec2(0.0, 32.0)),
                )
                .clicked()
            {
                state.step = step;
            }
        }
    });
}

fn can_save_library(workspace: &State) -> bool {
    workspace.dirty || workspace.draft_dirty || !workspace.stored
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::app::test_app;
    use crate::relay::library::Entry;
    use crate::runtime::settings::{Language, Preferences};

    fn click(ctx: &egui::Context, label: &str, mut render: impl FnMut(&mut egui::Ui)) {
        let mut draw = |events| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(620.0, 720.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, &mut render);
                },
            )
        };
        let frame = draw(vec![]);
        let bounds = frame
            .platform_output
            .accesskit_update
            .as_ref()
            .expect("accessibility tree")
            .nodes
            .iter()
            .find(|(_, node)| node.label() == Some(label))
            .and_then(|(_, node)| node.bounds())
            .expect("named button bounds");
        frame.drop_without_applying_deltas();
        let pos = egui::pos2(
            ((bounds.x0 + bounds.x1) * 0.5) as f32,
            ((bounds.y0 + bounds.y1) * 0.5) as f32,
        );
        for pressed in [true, false] {
            draw(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ])
            .drop_without_applying_deltas();
        }
    }

    #[test]
    fn compact_navigation_keeps_an_unsaved_item_resumable() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = test_app(Preferences::default());
        app.relay.workspace.checked = true;
        app.relay.workspace.selected = Some(Selection::Subscription(0));
        app.relay.workspace.show_editor = true;
        app.relay.workspace.name = "Synthetic draft".to_owned().into();
        app.relay.workspace.draft_dirty = true;
        click(&ctx, text(Language::En, Text::WorkspaceBack), |ui| {
            app.relay_page(ui);
        });
        assert!(!app.relay.workspace.show_editor);
        assert!(app.relay.workspace.confirm.is_none());
        click(&ctx, text(Language::En, Text::WorkspaceResumeItem), |ui| {
            app.relay_page(ui);
        });
        assert!(app.relay.workspace.show_editor && app.relay.workspace.draft_dirty);
        assert_eq!(app.relay.workspace.name.as_str(), "Synthetic draft");
    }

    #[test]
    fn member_move_updates_order_and_invalidates_the_previous_merge() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut app = test_app(Preferences::default());
        let workspace = &mut app.relay.workspace;
        for (id, name) in [(1, "Alpha"), (2, "Beta")] {
            workspace.data.subscriptions.push(Entry {
                id,
                name: name.to_owned(),
                source: String::new(),
                source_file: String::new(),
                content: "ss://synthetic".to_owned(),
                processing: Default::default(),
                publish: Default::default(),
            });
        }
        workspace.selected = Some(Selection::Collection(3));
        workspace.members = vec![1, 2];
        app.relay.local_content = Some("Previous merge".to_owned().into());
        click(
            &ctx,
            &format!("{}: Alpha", text(Language::En, Text::OperationMoveDown)),
            |ui| app.collection_members(ui, false),
        );
        assert_eq!(app.relay.workspace.members, [2, 1]);
        assert!(app.relay.workspace.draft_dirty && app.relay.workspace.members_changed);
        assert!(app.relay.local_content.is_none());
    }
}
