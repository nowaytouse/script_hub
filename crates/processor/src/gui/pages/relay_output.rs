use super::{length_counter, primary_button, secret_field, visible_field};
use crate::gui::file_dialog::FileAction;
use crate::gui::i18n::{Text, text};
use crate::gui::relay::{Failure, RelayState, RelayStep, SECRET_IDS};
use crate::gui::subscriptions::Selection;
use crate::gui::theme::Palette;
use crate::relay::export::OutputFormat;
use crate::relay::gist::{GistFile, validate_filename, validate_name};
use crate::relay::{RelayError, RunMode};
use crate::runtime::settings::Language;
use eframe::egui;
use zeroize::Zeroize;

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Tab {
    #[default]
    Local,
    Gist,
}

fn output_tab_id() -> egui::Id {
    egui::Id::unique("relay-output-tab")
}

fn schedule_request_id() -> egui::Id {
    egui::Id::unique("relay-open-scheduled-sync")
}

pub(super) fn open_gist(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        data.insert_temp(output_tab_id(), Tab::Gist);
        data.insert_temp(schedule_request_id(), true);
    });
}

pub(super) fn show(
    ui: &mut egui::Ui,
    state: &mut RelayState,
    language: Language,
    colors: Palette,
    cooldown: u64,
) -> (Option<RunMode>, Option<FileAction>) {
    let tr = |key| text(language, key);
    let tab_id = output_tab_id();
    let current = if state.demo_mode {
        Tab::Local
    } else {
        ui.data(|data| data.get_temp::<Tab>(tab_id).unwrap_or_default())
    };
    let mut tab = current;
    if !state.demo_mode {
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
        ui.add_space(12.0);
    }
    if tab != current {
        ui.data_mut(|data| data.insert_temp(tab_id, tab));
        state.reveal_token = false;
        state.reveal_output = false;
        clear_format_check_notice(state);
    }
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
    let mut selected_format = state.output_format;
    format_picker(ui, Text::RelayExportFormat, &mut selected_format, tr);
    select_local_format(state, selected_format);
    ui.weak(tr(if state.demo_mode {
        Text::RelaySandboxFormatHelp
    } else {
        Text::RelayExportFormatHelp
    }));
    ui.horizontal_wrapped(|ui| {
        if ui.button(tr(Text::RelayExportCheck)).clicked() {
            state.start_format_check();
        }
        if state.output_format != OutputFormat::Original
            && incompatible_format(state)
            && ui.button(tr(Text::RelayExportUseOriginal)).clicked()
        {
            select_local_format(state, OutputFormat::Original);
        }
    });
    if state.demo_mode {
        return None;
    }
    ui.add_space(8.0);
    ui.checkbox(&mut state.allow_plaintext, tr(Text::VaultPlaintextConsent));

    let can_export = state.allow_plaintext && !incompatible_format(state);
    let mut file_action = None;
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(can_export, primary_button(colors, tr(Text::RelayCopy)))
            .clicked()
        {
            state.copy(ui.ctx());
        }
        if ui
            .add_enabled(can_export, egui::Button::new(tr(Text::RelayExportNodes)))
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

fn format_picker(
    ui: &mut egui::Ui,
    label: Text,
    selected: &mut OutputFormat,
    tr: &impl Fn(Text) -> &'static str,
) {
    let name = |format| {
        tr(match format {
            OutputFormat::Original => Text::RelayExportOriginal,
            OutputFormat::Surge => Text::RelayExportSurge,
            OutputFormat::Mihomo => Text::RelayExportMihomo,
            OutputFormat::SingBox => Text::RelayExportSingBox,
        })
    };
    ui.horizontal_wrapped(|ui| {
        let label_widget = ui.label(tr(label));
        egui::ComboBox::from_id_salt(label as usize)
            .selected_text(name(*selected))
            .show_ui(ui, |ui| {
                for format in [
                    OutputFormat::Original,
                    OutputFormat::Surge,
                    OutputFormat::Mihomo,
                    OutputFormat::SingBox,
                ] {
                    if ui
                        .selectable_value(selected, format, name(format))
                        .clicked()
                    {
                        ui.close();
                    }
                }
            })
            .response
            .labelled_by(label_widget.id);
    });
}

fn incompatible_format(state: &RelayState) -> bool {
    matches!(
        state.error,
        Some(Failure::Operation(
            RelayError::ExportFormat | RelayError::ExportNode { .. }
        ))
    )
}

fn clear_format_check_notice(state: &mut RelayState) {
    if state.notice == Some(Text::RelayExportCheckPassed) {
        state.notice = None;
    }
}

fn select_local_format(state: &mut RelayState, format: OutputFormat) {
    if state.output_format != format {
        state.output_format = format;
        clear_format_check_notice(state);
        if incompatible_format(state) {
            state.error = None;
        }
    }
}

fn gist(
    ui: &mut egui::Ui,
    state: &mut RelayState,
    colors: Palette,
    cooldown: u64,
    has_content: bool,
    tr: &impl Fn(Text) -> &'static str,
) -> Option<RunMode> {
    let mut selected_format = state.publish_format;
    format_picker(ui, Text::RelayPublishFormat, &mut selected_format, tr);
    select_gist_format(state, selected_format);
    ui.weak(tr(Text::RelayPublishFormatHelp));
    if has_content && state.publish_format != OutputFormat::Original {
        ui.horizontal_wrapped(|ui| {
            if ui.button(tr(Text::RelayExportCheck)).clicked() {
                state.start_format_check_for(state.publish_format);
            }
            if incompatible_format(state) && ui.button(tr(Text::RelayExportUseOriginal)).clicked() {
                select_gist_format(state, OutputFormat::Original);
            }
        });
    }
    ui.add_space(12.0);
    if secret_field(
        ui,
        &mut state.token,
        SECRET_IDS[0],
        tr(Text::RelayOutputGistToken),
        &mut state.reveal_token,
        tr(Text::RelayShow),
    ) {
        clear_gist_authorization(state);
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
    let filename_invalid = validate_filename(&state.output_file).is_err()
        && !(state.output_file.is_empty() && GistFile::parse(state.output.trim(), "").is_ok());
    if filename_invalid {
        ui.colored_label(colors.error, tr(Text::RelayOutputFilenameInvalid));
    }

    if destination_changed {
        invalidate_gist_destination(state);
    }
    let mut title_invalid = validate_name(&state.title).is_err();
    egui::CollapsingHeader::new(tr(Text::RelayOutputOptions))
        .open(title_invalid.then_some(true))
        .show(ui, |ui| {
            let title_changed =
                visible_field(ui, &mut state.title, SECRET_IDS[5], tr(Text::RelayTitle));
            length_counter(ui, state.title.chars().count(), colors.error);
            title_invalid = validate_name(&state.title).is_err();
            if title_invalid {
                ui.colored_label(colors.error, tr(Text::RelayOutputTitleInvalid));
            }
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
    if !has_content {
        state.acknowledge_publish = false;
    }
    ui.add_enabled(
        has_content,
        egui::Checkbox::new(
            &mut state.acknowledge_publish,
            tr(Text::RelayPublishConsent),
        ),
    );
    let issue = state.validate(RunMode::Publish, state.engine_path()).err();
    if has_content
        && let Some(error) = issue
        && !(error == Text::RelayNameInvalid && (filename_invalid || title_invalid))
    {
        ui.label(tr(error));
    }
    let unverified = state
        .result
        .as_ref()
        .is_some_and(|result| result.tested && result.healthy == 0);
    if unverified {
        ui.label(tr(Text::RelayNoHealthyHelp));
        if ui.button(tr(Text::RelayOutputGoOperations)).clicked() {
            state.step = RelayStep::Review;
        }
    }
    let publish = ui
        .add_enabled(
            has_content
                && !unverified
                && !incompatible_format(state)
                && issue.is_none()
                && cooldown == 0,
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
        .filter(|created| {
            GistFile::parse_source(state.output.trim(), "")
                .is_ok_and(|target| target.id == created.id)
        })
    {
        ui.label(tr(Text::RelayCreated));
        ui.hyperlink_to(tr(Text::RelayOutputGistTab), &created.url);
    }

    ui.add_space(12.0);
    let open_schedule = ui.data_mut(|data| {
        let open = data.get_temp::<bool>(schedule_request_id());
        data.remove::<bool>(schedule_request_id());
        open
    });
    let schedule_section = egui::CollapsingHeader::new(tr(Text::ScheduleTitle))
        .open(open_schedule)
        .show(ui, |ui| {
            ui.weak(tr(Text::ScheduleHelp));
            if matches!(state.workspace.selected, Some(Selection::Collection(_))) {
                ui.weak(tr(Text::WorkspaceCollectionSchedule));
            }
            ui.horizontal_wrapped(|ui| {
                let label = ui.label(tr(Text::ScheduleInterval));
                let interval = ui
                    .add(
                        egui::DragValue::new(&mut state.schedule.interval_minutes).range(1..=10080),
                    )
                    .labelled_by(label.id);
                if interval.changed() {
                    state.workspace.draft_dirty |= state.workspace.selected.is_some();
                }
            });
            ui.weak(tr(Text::ScheduleSessionOnly));
            let issue = state.schedule_setup_issue();
            if let Some(error) = issue {
                ui.label(tr(error));
            }
            if ui
                .add_enabled(
                    issue.is_none() && !incompatible_format(state) && cooldown == 0,
                    egui::Button::new(tr(Text::ScheduleEnable)),
                )
                .clicked()
            {
                state.arm_schedule();
            }
        });
    if open_schedule == Some(true) {
        schedule_section
            .header_response
            .scroll_to_me(Some(egui::Align::TOP));
        schedule_section.header_response.request_focus();
    }
    publish.then_some(RunMode::Publish)
}

fn select_gist_format(state: &mut RelayState, format: OutputFormat) {
    if state.publish_format != format {
        state.publish_format = format;
        clear_format_check_notice(state);
        invalidate_gist_destination(state);
    }
}

fn invalidate_gist_destination(state: &mut RelayState) {
    state.workspace.draft_dirty |= state.workspace.selected.is_some();
    clear_gist_authorization(state);
    if matches!(
        state.notice,
        Some(Text::RelayPublished | Text::RelayUnchanged)
    ) {
        state.notice = None;
    }
}

fn clear_gist_authorization(state: &mut RelayState) {
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
        ctx.data_mut(|data| data.insert_temp(output_tab_id(), tab));
        render_in_context(state, &ctx, dark)
    }

    fn render_in_context(
        state: &mut RelayState,
        ctx: &egui::Context,
        dark: bool,
    ) -> Vec<(String, bool)> {
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
    fn recovery_opens_gist_and_expands_the_schedule_without_authorizing_it() {
        let mut state = prepared();
        let ctx = egui::Context::default();
        open_gist(&ctx);
        let nodes = render_in_context(&mut state, &ctx, false);
        assert!(nodes.contains(&(text(Language::En, Text::ScheduleEnable).to_owned(), true)));
        assert!(!state.acknowledge_publish && !state.schedule.armed());
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
    fn format_check_is_available_without_plaintext_consent() {
        let mut state = prepared();
        state.output_format = OutputFormat::SingBox;
        assert!(
            render(&mut state, Tab::Local, false)
                .contains(&(text(Language::En, Text::RelayExportCheck).to_owned(), false))
        );
        assert!(!state.allow_plaintext);
        state.notice = Some(Text::RelayExportCheckPassed);
        select_local_format(&mut state, OutputFormat::Original);
        assert!(state.notice.is_none());
    }

    #[test]
    fn incompatible_format_can_recover_without_rechecking_the_nodes() {
        let mut state = prepared();
        state.output_format = OutputFormat::SingBox;
        state.allow_plaintext = true;
        state.error = Some(Failure::Operation(RelayError::ExportNode {
            index: 1,
            error: crate::relay::model::ModelError::Unsupported,
        }));
        let nodes = render(&mut state, Tab::Local, true);
        assert!(nodes.contains(&(text(Language::En, Text::RelayCopy).to_owned(), true)));
        assert!(nodes.contains(&(
            text(Language::En, Text::RelayExportUseOriginal).to_owned(),
            false
        )));
        select_local_format(&mut state, OutputFormat::Original);
        assert!(state.error.is_none());
        assert!(
            render(&mut state, Tab::Local, true)
                .contains(&(text(Language::En, Text::RelayCopy).to_owned(), false))
        );
    }

    #[test]
    fn gist_format_is_independent_and_requires_fresh_publish_approval() {
        let mut state = prepared();
        state.output_format = OutputFormat::Mihomo;
        state.allow_plaintext = true;
        state.allow_public_output = true;
        state.acknowledge_publish = true;
        state.workspace.selected = Some(Selection::Subscription(1));

        select_gist_format(&mut state, OutputFormat::Surge);
        assert_eq!(state.output_format, OutputFormat::Mihomo);
        assert!(state.allow_plaintext);
        assert!(!state.allow_public_output && !state.acknowledge_publish);
        assert!(state.workspace.draft_dirty);
        assert_eq!(
            state.private_config(state.engine_path()).0.output_format,
            OutputFormat::Surge
        );
        let nodes = render(&mut state, Tab::Gist, false);
        assert!(nodes.contains(&(text(Language::En, Text::RelayExportCheck).to_owned(), false)));
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

    #[test]
    fn gist_draft_settings_do_not_authorize_an_empty_result() {
        let mut state = RelayState::default();
        state.acknowledge_publish = true;
        let nodes = render(&mut state, Tab::Gist, false);
        assert!(!state.acknowledge_publish);
        assert!(nodes.contains(&(
            text(Language::En, Text::RelayPublishConsent).to_owned(),
            true
        )));
        assert!(nodes.contains(&(text(Language::En, Text::RelayOutputCreate).to_owned(), true)));
    }

    #[test]
    fn gist_filename_and_collapsed_title_errors_are_visible_together() {
        let mut state = prepared();
        state.output_file.clear();
        state.output_file.push_str("invalid/path.txt");
        state.title.push_str(&"a".repeat(65));
        let nodes = render(&mut state, Tab::Gist, true);
        assert!(
            nodes
                .iter()
                .any(|(label, _)| label == text(Language::En, Text::RelayOutputFilenameInvalid))
        );
        assert!(
            nodes
                .iter()
                .any(|(label, _)| label == text(Language::En, Text::RelayOutputTitleInvalid))
        );
    }

    #[test]
    fn gist_receipt_is_only_shown_for_its_own_destination() {
        let mut state = prepared();
        let id = "0123456789abcdef0123456789abcdef";
        state.result.as_mut().unwrap().created = Some(crate::relay::gist::CreatedGist {
            id: id.to_owned(),
            url: format!("https://gist.github.com/{id}"),
        });
        state
            .output
            .push_str(&format!("https://gist.github.com/{id}"));
        let nodes = render(&mut state, Tab::Gist, false);
        assert!(
            nodes
                .iter()
                .any(|(label, _)| label == text(Language::En, Text::RelayCreated))
        );
        state.output.clear();
        let nodes = render(&mut state, Tab::Gist, false);
        assert!(
            !nodes
                .iter()
                .any(|(label, _)| label == text(Language::En, Text::RelayCreated))
        );
    }
}
