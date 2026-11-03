use crate::gui::i18n::{Text, text};
use crate::gui::subscriptions::{Action, ListKind, Selection, State};
use crate::gui::theme::Palette;
use crate::runtime::settings::Language;
use eframe::egui;

pub(super) fn show(
    ui: &mut egui::Ui,
    language: Language,
    colors: Palette,
    state: &mut State,
    allow_actions: bool,
) -> Option<Action> {
    let storage_dirty = state.dirty || state.draft_dirty;
    let stored = state.stored;
    let selected = state.selected;
    let workspace = &state.data;
    let list = &mut state.list;
    let tr = |key| text(language, key);
    let mut action = None;
    ui.heading(tr(Text::WorkspaceListTitle));
    ui.add_enabled_ui(allow_actions, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut list.kind,
                ListKind::Subscriptions,
                format!(
                    "{} ({})",
                    tr(Text::WorkspaceListSubscriptions),
                    workspace.subscriptions.len()
                ),
            );
            ui.selectable_value(
                &mut list.kind,
                ListKind::Collections,
                format!(
                    "{} ({})",
                    tr(Text::WorkspaceListCollections),
                    workspace.collections.len()
                ),
            );
        });
        let label = ui.label(tr(Text::WorkspaceListSearch));
        let mut edit = egui::TextEdit::singleline(&mut list.query)
            .char_limit(128)
            .id(egui::Id::unique("workspace-search"))
            .desired_width(ui.available_width())
            .show(ui);
        let _ = (*edit.response).clone().labelled_by(label.id);
        edit.state.clear_undoer();
        edit.state.store(ui.ctx(), edit.response.id);
    });

    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                allow_actions,
                egui::Button::new(tr(match list.kind {
                    ListKind::Subscriptions => Text::WorkspaceListNewSubscription,
                    ListKind::Collections => Text::WorkspaceListNewCollection,
                })),
            )
            .clicked()
        {
            action = Some(match list.kind {
                ListKind::Subscriptions => Action::NewSubscription,
                ListKind::Collections => Action::NewCollection,
            });
        }
        if ui
            .add_enabled(
                allow_actions,
                egui::Button::new(tr(Text::WorkspaceListImport)),
            )
            .clicked()
        {
            action = Some(Action::Import);
        }
    });
    let query = list.query.trim().to_lowercase();
    let rows: Vec<(Selection, &str, Option<usize>)> = match list.kind {
        ListKind::Subscriptions => workspace
            .subscriptions
            .iter()
            .filter(|item| item.name.to_lowercase().contains(&query))
            .map(|item| (Selection::Subscription(item.id), item.name.as_str(), None))
            .collect(),
        ListKind::Collections => workspace
            .collections
            .iter()
            .filter(|item| item.name.to_lowercase().contains(&query))
            .map(|item| {
                (
                    Selection::Collection(item.id),
                    item.name.as_str(),
                    Some(item.members.len()),
                )
            })
            .collect(),
    };
    if rows.is_empty() {
        ui.label(tr(if !query.is_empty() {
            Text::WorkspaceListNoResults
        } else if list.kind == ListKind::Subscriptions {
            Text::WorkspaceListEmptySubscriptions
        } else {
            Text::WorkspaceListEmptyCollections
        }));
    } else {
        egui::ScrollArea::vertical()
            .id_salt("workspace-list")
            .max_height(360.0)
            .show_rows(ui, 36.0, rows.len(), |ui, range| {
                for (selection, name, count) in &rows[range] {
                    let title = match count {
                        Some(count) => {
                            format!("{name} · {count} {}", tr(Text::WorkspaceListMembers))
                        }
                        None => (*name).to_owned(),
                    };
                    let width = ui.available_width();
                    if ui
                        .add_enabled_ui(allow_actions, |ui| {
                            ui.add_sized(
                                [width, 32.0],
                                egui::Button::new(title)
                                    .selected(selected == Some(*selection))
                                    .truncate(),
                            )
                        })
                        .inner
                        .clicked()
                    {
                        action = Some(Action::Open(*selection));
                    }
                }
            });
    }
    ui.separator();
    ui.colored_label(
        if storage_dirty {
            colors.warning
        } else {
            colors.muted
        },
        tr(if storage_dirty {
            Text::WorkspaceListUnsaved
        } else if stored {
            Text::WorkspaceListSaved
        } else {
            Text::WorkspaceListSession
        }),
    );
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                allow_actions,
                egui::Button::new(tr(Text::WorkspaceListSave)),
            )
            .clicked()
        {
            action = Some(Action::SaveLibrary);
        }
        if ui
            .add_enabled(
                allow_actions,
                egui::Button::new(tr(Text::WorkspaceListLock)),
            )
            .clicked()
        {
            action = Some(Action::Lock);
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme;
    use crate::runtime::settings::{Accent, Preferences, Theme};

    #[test]
    fn new_button_is_accessible_and_clickable_in_compact_views() {
        fn draw(
            ctx: &egui::Context,
            state: &mut State,
            language: Language,
            dark: bool,
            enabled: bool,
            events: Vec<egui::Event>,
        ) -> (egui::FullOutput, Option<Action>) {
            let mut action = None;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(320.0, 700.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        action = show(
                            ui,
                            language,
                            theme::palette(dark, Accent::Purple),
                            state,
                            enabled,
                        );
                    });
                },
            );
            output.textures_delta.clear();
            (output, action)
        }
        for dark in [false, true] {
            for language in [Language::En, Language::ZhCn] {
                let ctx = egui::Context::default();
                ctx.enable_accesskit();
                let preferences = Preferences {
                    theme: if dark { Theme::Dark } else { Theme::Light },
                    ..Preferences::default()
                };
                theme::apply(&ctx, &preferences);
                let mut state = State::default();
                let (output, _) = draw(&ctx, &mut state, language, dark, true, vec![]);
                let update = output
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .expect("accessibility tree");
                let (_, node) = update
                    .nodes
                    .iter()
                    .find(|(_, node)| {
                        node.label() == Some(text(language, Text::WorkspaceListNewSubscription))
                    })
                    .expect("named new button");
                let bounds = node.bounds().expect("button bounds");
                assert!(bounds.x0 >= 0.0 && bounds.x1 <= 320.0);
                let pos = egui::pos2(
                    ((bounds.x0 + bounds.x1) / 2.0) as f32,
                    ((bounds.y0 + bounds.y1) / 2.0) as f32,
                );
                let button = |pressed| egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                };
                let _ = draw(
                    &ctx,
                    &mut state,
                    language,
                    dark,
                    true,
                    vec![egui::Event::PointerMoved(pos), button(true)],
                );
                let (_, action) = draw(
                    &ctx,
                    &mut state,
                    language,
                    dark,
                    true,
                    vec![egui::Event::PointerMoved(pos), button(false)],
                );
                assert!(matches!(action, Some(Action::NewSubscription)));
                let _ = draw(
                    &ctx,
                    &mut state,
                    language,
                    dark,
                    false,
                    vec![egui::Event::PointerMoved(pos), button(true)],
                );
                let (_, action) = draw(
                    &ctx,
                    &mut state,
                    language,
                    dark,
                    false,
                    vec![egui::Event::PointerMoved(pos), button(false)],
                );
                assert!(action.is_none());
            }
        }
    }
}
