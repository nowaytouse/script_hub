use super::{length_counter, primary_button, secret_field, visible_field};
use crate::gui::file_dialog::FileAction;
use crate::gui::i18n::{Text, text};
use crate::gui::relay::{RelayState, RelayStep, SECRET_IDS};
use crate::gui::subscriptions::Selection;
use crate::gui::theme::Palette;
use crate::relay::RunMode;
use crate::relay::export::OutputFormat;
use crate::runtime::settings::Language;
use eframe::egui;
use zeroize::Zeroize;

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Tab {
    #[default]
    Local,
    Gist,
}

pub(super) fn show(
    ui: &mut egui::Ui,
    state: &mut RelayState,
    language: Language,
    colors: Palette,
    cooldown: u64,
) -> (Option<RunMode>, Option<FileAction>) {
    let tr = |key| text(language, key);
    let tab_id = ui.make_persistent_id("relay-output-tab");
    let current = ui.data(|data| data.get_temp::<Tab>(tab_id).unwrap_or_default());
    let mut tab = current;
    ui.horizontal_wrapped(|ui| {
        for (value, label) in [
            (Tab::Local, Text::RelayOutputLocalTab),
            (Tab::Gist, Text::RelayOutputGistTab),
        ] {
            if ui
                .add(
                    egui::Button::new(tr(label))
                        .selected(tab == value)
                        .min_size(egui::vec2(0.0, 32.0)),
                )
                .clicked()
            {
                tab = value;
            }
        }
    });
    if tab != current {
        ui.data_mut(|data| data.insert_temp(tab_id, tab));
        state.reveal_token = false;
        state.reveal_output = false;
    }
    ui.add_space(12.0);

    let has_content = if let Some(result) = state
        .result
        .as_ref()
        .filter(|result| !result.content.is_empty())
    {
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!(
                "{}: {}",
                tr(Text::RelayOutputNodeCount),
                result.total.saturating_sub(result.removed),
            ));
            if let Some(format) = state.result_format {
                ui.weak(format);
            }
        });
        true
    } else {
        ui.label(tr(Text::RelayOutputEmpty));
        let have_input = state.editor.is_some() || state.local_content.is_some();
        if ui
            .button(tr(if have_input {
                Text::RelayOutputGoOperations
            } else {
                Text::RelayOutputGoSource
            }))
            .clicked()
        {
            state.step = if have_input {
                RelayStep::Review
            } else {
                RelayStep::Source
            };
        }
        false
    };
    ui.add_space(8.0);

    match tab {
        Tab::Local if has_content => (None, local(ui, state, colors, &tr)),
        Tab::Local => (None, None),
        Tab::Gist => (gist(ui, state, colors, cooldown, has_content, &tr), None),
    }
}

fn local(
    ui: &mut egui::Ui,
    state: &mut RelayState,
    colors: Palette,
    tr: &impl Fn(Text) -> &'static str,
) -> Option<FileAction> {
    let format_label = |format| {
        tr(match format {
            OutputFormat::Original => Text::RelayExportOriginal,
            OutputFormat::Surge => Text::RelayExportSurge,
            OutputFormat::Mihomo => Text::RelayExportMihomo,
            OutputFormat::SingBox => Text::RelayExportSingBox,
        })
    };
    ui.horizontal_wrapped(|ui| {
        let label = ui.label(tr(Text::RelayExportFormat));
        egui::ComboBox::from_id_salt("relay-export-format")
            .selected_text(format_label(state.output_format))
            .show_ui(ui, |ui| {
                for format in [
                    OutputFormat::Original,
                    OutputFormat::Surge,
                    OutputFormat::Mihomo,
                    OutputFormat::SingBox,
                ] {
                    ui.selectable_value(&mut state.output_format, format, format_label(format));
                }
            })
            .response
            .labelled_by(label.id);
    });
    ui.weak(tr(Text::RelayExportFormatHelp));
    ui.add_space(8.0);
    ui.checkbox(&mut state.allow_plaintext, tr(Text::VaultPlaintextConsent));

    let mut file_action = None;
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                state.allow_plaintext,
                primary_button(colors, tr(Text::RelayCopy)),
            )
            .clicked()
        {
            state.copy(ui.ctx());
        }
        if ui
            .add_enabled(
                state.allow_plaintext,
                egui::Button::new(tr(Text::RelayExportNodes)),
            )
            .clicked()
        {
            file_action = Some(FileAction::ExportNodes(state.output_format));
        }
    });
    ui.add_space(12.0);
    ui.collapsing(tr(Text::RelayOutputBackupOptions), |ui| {
        ui.weak(tr(Text::RelayBackupWarning));
        if ui
            .add_enabled(
                state.allow_plaintext,
                egui::Button::new(tr(Text::RelayExportBackup)),
            )
            .clicked()
        {
            file_action = Some(FileAction::ExportBackup);
        }
        ui.separator();
        ui.strong(tr(Text::ArchiveExport));
        ui.weak(tr(Text::ArchiveExportHelp));
        visible_field(
            ui,
            &mut state.archive_recipient,
            SECRET_IDS[7],
            tr(Text::ArchiveRecipient),
        );
        if ui
            .add_enabled(
                !state.archive_recipient.trim().is_empty(),
                egui::Button::new(tr(Text::ArchiveExport)),
            )
            .clicked()
        {
            file_action = Some(FileAction::ExportEncrypted);
        }
    });
    file_action
}

fn gist(
    ui: &mut egui::Ui,
    state: &mut RelayState,
    colors: Palette,
    cooldown: u64,
    has_content: bool,
    tr: &impl Fn(Text) -> &'static str,
) -> Option<RunMode> {
    if secret_field(
        ui,
        &mut state.token,
        SECRET_IDS[0],
        tr(Text::RelayOutputGistToken),
        &mut state.reveal_token,
        tr(Text::RelayShow),
    ) {
        state.acknowledge_publish = false;
        state.allow_public_output = false;
        state.error = None;
    }
    let mut destination_changed = secret_field(
        ui,
        &mut state.output,
        SECRET_IDS[3],
        tr(Text::RelayOutputGistTarget),
        &mut state.reveal_output,
        tr(Text::RelayShow),
    );
    if state.output.trim().is_empty() {
        ui.weak(tr(Text::RelayCreateHelp));
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.weak(tr(Text::RelayReuseOutput));
            if ui.button(tr(Text::RelayCreateAnother)).clicked() {
                state.output.zeroize();
                destination_changed = true;
            }
        });
    }
    destination_changed |= visible_field(
        ui,
        &mut state.output_file,
        SECRET_IDS[4],
        tr(Text::RelayOutputFile),
    );
    length_counter(ui, state.output_file.chars().count(), colors.error);

    if destination_changed {
        invalidate_gist_destination(state);
    }
    ui.collapsing(tr(Text::RelayOutputOptions), |ui| {
        let title_changed =
            visible_field(ui, &mut state.title, SECRET_IDS[5], tr(Text::RelayTitle));
        length_counter(ui, state.title.chars().count(), colors.error);
        if title_changed {
            invalidate_gist_destination(state);
        }
        if !state.output.trim().is_empty()
            && ui
                .checkbox(&mut state.allow_public_output, tr(Text::RelayPublicConsent))
                .changed()
        {
            state.acknowledge_publish = false;
        }
    });
    if state.workspace.selected.is_some() {
        ui.weak(tr(Text::WorkspaceOutputSavedHelp));
    }

    ui.add_space(8.0);
    ui.checkbox(
        &mut state.acknowledge_publish,
        tr(Text::RelayPublishConsent),
    );
    let issue = state.validate(RunMode::Publish, state.engine_path()).err();
    if let Some(error) = issue {
        ui.label(tr(error));
    }
    let unverified = state
        .result
        .as_ref()
        .is_some_and(|result| result.tested && result.healthy == 0);
    if unverified {
        ui.label(tr(Text::RelayNoHealthyHelp));
    }
    let publish = ui
        .add_enabled(
            has_content && !unverified && issue.is_none() && cooldown == 0,
            primary_button(
                colors,
                tr(if state.output.trim().is_empty() {
                    Text::RelayOutputCreate
                } else {
                    Text::RelayOutputUpdate
                }),
            ),
        )
        .clicked();
    if let Some(created) = state
        .result
        .as_ref()
        .and_then(|result| result.created.as_ref())
    {
        ui.label(tr(Text::RelayCreated));
        ui.hyperlink_to(tr(Text::RelayOutputGistTab), &created.url);
    }

    ui.add_space(12.0);
    ui.collapsing(tr(Text::ScheduleTitle), |ui| {
        ui.weak(tr(Text::ScheduleHelp));
        if matches!(state.workspace.selected, Some(Selection::Collection(_))) {
            ui.weak(tr(Text::WorkspaceCollectionSchedule));
        }
        ui.horizontal_wrapped(|ui| {
            let label = ui.label(tr(Text::ScheduleInterval));
            let interval = ui
                .add(egui::DragValue::new(&mut state.schedule.interval_minutes).range(1..=10080))
                .labelled_by(label.id);
            if interval.changed() {
                state.workspace.draft_dirty |= state.workspace.selected.is_some();
            }
        });
        ui.weak(tr(Text::ScheduleSessionOnly));
        let issue = if !has_content {
            Some(Text::ScheduleNeedsPreview)
        } else if state.output.trim().is_empty() {
            Some(Text::ScheduleNeedsOutput)
        } else {
            issue
        };
        if let Some(error) = issue {
            ui.label(tr(error));
        }
        if ui
            .add_enabled(
                issue.is_none() && cooldown == 0,
                egui::Button::new(tr(Text::ScheduleEnable)),
            )
            .clicked()
        {
            state.arm_schedule();
        }
    });
    publish.then_some(RunMode::Publish)
}

fn invalidate_gist_destination(state: &mut RelayState) {
    state.workspace.draft_dirty |= state.workspace.selected.is_some();
    state.allow_public_output = false;
    state.acknowledge_publish = false;
    state.error = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::RelayConfig;
    use crate::runtime::settings::Preferences;
    use std::sync::atomic::AtomicBool;

    fn prepared() -> RelayState {
        let mut state = RelayState::default();
        state.local_content = Some("Example = http, example.invalid, 8080\n".to_owned().into());
        let config = RelayConfig {
            local_content: state.local_content.clone(),
            test_nodes: false,
            ..RelayConfig::default()
        };
        state.result = Some(
            crate::relay::run(
                &config,
                "",
                RunMode::Preview,
                &AtomicBool::new(false),
                &|_| {},
            )
            .unwrap(),
        );
        state
    }

    fn render(state: &mut RelayState, tab: Tab, dark: bool) -> Vec<(String, bool)> {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let colors = crate::gui::theme::palette(dark, Preferences::default().accent);
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(420.0, 900.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let tab_id = ui.make_persistent_id("relay-output-tab");
                    ui.data_mut(|data| data.insert_temp(tab_id, tab));
                    assert_eq!(show(ui, state, Language::En, colors, 0), (None, None));
                });
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
    fn local_export_has_no_gist_controls_and_requires_plaintext_consent() {
        for dark in [false, true] {
            let mut state = prepared();
            let nodes = render(&mut state, Tab::Local, dark);
            assert!(nodes.contains(&(text(Language::En, Text::RelayCopy).to_owned(), true)));
            assert!(
                !nodes
                    .iter()
                    .any(|(label, _)| label == text(Language::En, Text::RelayOutputGistToken))
            );
            state.allow_plaintext = true;
            let nodes = render(&mut state, Tab::Local, dark);
            assert!(nodes.contains(&(text(Language::En, Text::RelayCopy).to_owned(), false)));
        }
    }

    #[test]
    fn empty_output_points_back_to_the_next_useful_step() {
        let mut state = RelayState::default();
        let no_source = render(&mut state, Tab::Local, false);
        assert!(no_source.contains(&(
            text(Language::En, Text::RelayOutputGoSource).to_owned(),
            false
        )));
        assert!(
            !no_source
                .iter()
                .any(|(label, _)| label == text(Language::En, Text::RelayCopy))
        );

        state.local_content = Some("Example = http, example.invalid, 8080\n".to_owned().into());
        let source_loaded = render(&mut state, Tab::Local, true);
        assert!(source_loaded.contains(&(
            text(Language::En, Text::RelayOutputGoOperations).to_owned(),
            false
        )));
    }

    #[test]
    fn gist_actions_show_target_specific_labels_and_explain_missing_authorization() {
        let mut state = prepared();
        let missing_token = render(&mut state, Tab::Gist, false);
        assert!(
            missing_token
                .iter()
                .any(|(label, _)| label == text(Language::En, Text::RelayTokenInvalid))
        );
        assert!(
            missing_token.contains(&(text(Language::En, Text::RelayOutputCreate).to_owned(), true))
        );

        state.token.push_str("synthetic-test-token");
        state.output.push_str("0123456789abcdef0123456789abcdef");
        let missing_consent = render(&mut state, Tab::Gist, true);
        assert!(
            missing_consent
                .iter()
                .any(|(label, _)| label == text(Language::En, Text::RelayConsentRequired))
        );
        assert!(
            missing_consent
                .contains(&(text(Language::En, Text::RelayOutputUpdate).to_owned(), true))
        );
        assert!(
            !missing_consent
                .iter()
                .any(|(label, _)| label.contains("synthetic-test-token")
                    || label.contains("0123456789abcdef"))
        );
    }
}
