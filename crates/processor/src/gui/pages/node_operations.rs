use super::card;
use crate::gui::i18n::{Text, text};
use crate::gui::theme::Palette;
use crate::relay::model::pipeline::{self, Operation, SortKey, StageReport, Step};
use crate::runtime::settings::Language;
use eframe::egui;
use zeroize::{Zeroize, Zeroizing};

const MAX_STEPS: usize = 64;
const FIELDS: [&str; 5] = [
    "pattern",
    "replacement",
    "template",
    "protocols",
    "patterns",
];

fn field_id(id: u64, field: &str) -> egui::Id {
    egui::Id::unique("relay-operation-field")
        .with(id)
        .with(field)
}

fn validation_id() -> egui::Id {
    egui::Id::unique("relay-operation-validation")
}

fn valid_chain(ctx: &egui::Context, steps: &[Step], changed: bool) -> bool {
    let cached = ctx.data_mut(|data| data.get_temp::<bool>(validation_id()));
    if !changed && let Some(valid) = cached {
        return valid;
    }
    let valid = pipeline::validate(steps).is_ok();
    ctx.data_mut(|data| data.insert_temp(validation_id(), valid));
    valid
}

pub(super) fn clear_widgets(ctx: &egui::Context, steps: &[Step]) {
    ctx.data_mut(|data| {
        data.remove::<bool>(validation_id());
        for step in steps {
            for field in FIELDS {
                data.remove::<egui::text_edit::TextEditState>(field_id(step.id, field));
                data.remove::<Zeroizing<String>>(field_id(step.id, field).with("draft"));
            }
        }
    });
}

fn field(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    id: egui::Id,
    multiline: bool,
) -> bool {
    let label = ui.label(label);
    let edit = if multiline {
        egui::TextEdit::multiline(value).desired_rows(3)
    } else {
        egui::TextEdit::singleline(value)
    };
    let mut output = edit
        .id(id)
        .char_limit(512)
        .desired_width(ui.available_width().min(480.0))
        .show(ui);
    let _ = (*output.response).clone().labelled_by(label.id);
    let changed = output.response.changed();
    output.state.clear_undoer();
    output.state.store(ui.ctx(), output.response.id);
    changed
}

fn operation_label(op: &Operation) -> Text {
    match op {
        Operation::NameFilter { .. } => Text::OperationNameFilter,
        Operation::ProtocolFilter { .. } => Text::OperationProtocolFilter,
        Operation::ReplaceName { .. } => Text::OperationReplaceName,
        Operation::FormatName { .. } => Text::OperationFormatName,
        Operation::Flags { .. } => Text::OperationFlags,
        Operation::Sort { .. } => Text::OperationSort,
        Operation::RegexSort { .. } => Text::OperationRegexSort,
        Operation::Deduplicate => Text::OperationDeduplicate,
    }
}

enum Action {
    Up,
    Down,
    Duplicate,
    Delete,
}

fn new_operations() -> [Operation; 8] {
    [
        Operation::NameFilter {
            pattern: String::new(),
            keep: true,
        },
        Operation::ProtocolFilter {
            protocols: Vec::new(),
            keep: true,
        },
        Operation::ReplaceName {
            pattern: String::new(),
            replacement: String::new(),
            regex: false,
        },
        Operation::FormatName {
            template: "{protocol} | {name}".into(),
        },
        Operation::Flags { add: true },
        Operation::Sort {
            key: SortKey::Name,
            descending: false,
        },
        Operation::RegexSort {
            patterns: Vec::new(),
        },
        Operation::Deduplicate,
    ]
}

pub(super) fn show(
    ui: &mut egui::Ui,
    language: Language,
    colors: Palette,
    steps: &mut Vec<Step>,
    reports: &[StageReport],
) -> bool {
    let tr = |key| text(language, key);
    let mut changed = false;
    ui.strong(tr(Text::OperationChain));
    ui.weak(tr(Text::OperationChainHelp));
    let next_id = steps
        .iter()
        .map(|step| step.id)
        .max()
        .unwrap_or(0)
        .checked_add(1);
    ui.add_enabled_ui(steps.len() < MAX_STEPS && next_id.is_some(), |ui| {
        ui.menu_button(tr(Text::OperationAdd), |ui| {
            for operation in new_operations() {
                if ui.button(tr(operation_label(&operation))).clicked()
                    && let Some(id) = next_id
                {
                    steps.push(Step {
                        id,
                        enabled: true,
                        operation,
                    });
                    changed = true;
                    ui.close();
                }
            }
        });
    });
    if steps.is_empty() {
        ui.weak(tr(Text::OperationEmpty));
    }
    let mut action = None;
    let count = steps.len();
    for (index, step) in steps.iter_mut().enumerate() {
        ui.add_space(6.0);
        ui.push_id(("relay-operation", step.id), |ui| {
            ui.spacing_mut().window_margin = egui::Margin::same(8);
            ui.spacing_mut().item_spacing.y = 6.0;
            card(ui, colors, |ui| {
                egui::CollapsingHeader::new(format!(
                    "{}. {}",
                    index + 1,
                    tr(operation_label(&step.operation))
                ))
                .id_salt(("relay-operation-card", step.id))
                .default_open(true)
                .show(ui, |ui| match &mut step.operation {
                    Operation::NameFilter { pattern, keep } => {
                        changed |= field(
                            ui,
                            tr(Text::OperationPattern),
                            pattern,
                            field_id(step.id, "pattern"),
                            false,
                        );
                        changed |= ui.checkbox(keep, tr(Text::OperationKeepMatches)).changed();
                    }
                    Operation::ProtocolFilter { protocols, keep } => {
                        let id = field_id(step.id, "protocols");
                        let mut value = ui
                            .ctx()
                            .data_mut(|data| data.get_temp::<Zeroizing<String>>(id.with("draft")))
                            .unwrap_or_else(|| {
                                let mut value = Zeroizing::new(String::new());
                                for (i, protocol) in protocols.iter().enumerate() {
                                    if i > 0 {
                                        value.push_str(", ");
                                    }
                                    value.push_str(protocol);
                                }
                                value
                            });
                        if field(
                            ui,
                            tr(Text::OperationProtocols),
                            &mut value,
                            field_id(step.id, "protocols"),
                            false,
                        ) {
                            protocols.zeroize();
                            *protocols = value
                                .split(',')
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(str::to_owned)
                                .collect();
                            ui.ctx()
                                .data_mut(|data| data.insert_temp(id.with("draft"), value.clone()));
                            changed = true;
                        }
                        ui.weak(tr(Text::OperationProtocolsHelp));
                        changed |= ui.checkbox(keep, tr(Text::OperationKeepMatches)).changed();
                    }
                    Operation::ReplaceName {
                        pattern,
                        replacement,
                        regex,
                    } => {
                        changed |= field(
                            ui,
                            tr(Text::OperationPattern),
                            pattern,
                            field_id(step.id, "pattern"),
                            false,
                        );
                        changed |= field(
                            ui,
                            tr(Text::OperationReplacement),
                            replacement,
                            field_id(step.id, "replacement"),
                            false,
                        );
                        changed |= ui.checkbox(regex, tr(Text::OperationUseRegex)).changed();
                    }
                    Operation::FormatName { template } => {
                        changed |= field(
                            ui,
                            tr(Text::OperationTemplate),
                            template,
                            field_id(step.id, "template"),
                            false,
                        );
                        ui.weak(tr(Text::OperationTemplateHelp));
                    }
                    Operation::Flags { add } => {
                        changed |= ui.checkbox(add, tr(Text::OperationAddFlags)).changed();
                    }
                    Operation::Sort { key, descending } => {
                        ui.horizontal_wrapped(|ui| {
                            for (candidate, label) in [
                                (SortKey::Name, Text::OperationSortName),
                                (SortKey::Protocol, Text::OperationSortProtocol),
                                (SortKey::Region, Text::OperationSortRegion),
                            ] {
                                if ui.selectable_label(*key == candidate, tr(label)).clicked() {
                                    *key = candidate;
                                    changed = true;
                                }
                            }
                        });
                        changed |= ui
                            .checkbox(descending, tr(Text::OperationDescending))
                            .changed();
                    }
                    Operation::RegexSort { patterns } => {
                        let id = field_id(step.id, "patterns");
                        let mut value = ui
                            .ctx()
                            .data_mut(|data| data.get_temp::<Zeroizing<String>>(id.with("draft")))
                            .unwrap_or_else(|| {
                                let mut value = Zeroizing::new(String::new());
                                for (i, pattern) in patterns.iter().enumerate() {
                                    if i > 0 {
                                        value.push('\n');
                                    }
                                    value.push_str(pattern);
                                }
                                value
                            });
                        if field(
                            ui,
                            tr(Text::OperationPatterns),
                            &mut value,
                            field_id(step.id, "patterns"),
                            true,
                        ) {
                            patterns.zeroize();
                            *patterns = value
                                .lines()
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(str::to_owned)
                                .collect();
                            ui.ctx()
                                .data_mut(|data| data.insert_temp(id.with("draft"), value.clone()));
                            changed = true;
                        }
                    }
                    Operation::Deduplicate => {}
                });
                ui.horizontal_wrapped(|ui| {
                    changed |= ui
                        .checkbox(&mut step.enabled, tr(Text::OperationEnabled))
                        .changed();
                    if let Some(report) = reports.iter().find(|report| report.id == step.id) {
                        ui.weak(format!(
                            "{} → {} · {} {}",
                            report.before,
                            report.after,
                            tr(Text::OperationRenamed),
                            report.renamed
                        ));
                    }
                    if ui
                        .add_enabled(index > 0, egui::Button::new(tr(Text::OperationMoveUp)))
                        .clicked()
                    {
                        action = Some((index, Action::Up));
                    }
                    if ui
                        .add_enabled(
                            index + 1 < count,
                            egui::Button::new(tr(Text::OperationMoveDown)),
                        )
                        .clicked()
                    {
                        action = Some((index, Action::Down));
                    }
                    if ui
                        .add_enabled(
                            count < MAX_STEPS && next_id.is_some(),
                            egui::Button::new(tr(Text::OperationDuplicate)),
                        )
                        .clicked()
                    {
                        action = Some((index, Action::Duplicate));
                    }
                    if ui.button(tr(Text::OperationDelete)).clicked() {
                        action = Some((index, Action::Delete));
                    }
                });
            });
        });
    }
    if let Some((index, kind)) = action {
        match kind {
            Action::Up => steps.swap(index, index - 1),
            Action::Down => steps.swap(index, index + 1),
            Action::Duplicate => {
                if let Some(id) = next_id {
                    let mut copy = steps[index].clone();
                    copy.id = id;
                    steps.insert(index + 1, copy);
                }
            }
            Action::Delete => {
                clear_widgets(ui.ctx(), &steps[index..=index]);
                steps.remove(index);
            }
        }
        changed = true;
    }
    if !valid_chain(ui.ctx(), steps, changed) {
        ui.colored_label(colors.error, tr(Text::OperationInvalid));
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme;
    use crate::runtime::settings::Preferences;

    #[test]
    fn validation_cache_invalidates_on_change_and_empty_reset() {
        let ctx = egui::Context::default();
        let mut steps = vec![Step {
            id: 1,
            enabled: true,
            operation: Operation::NameFilter {
                pattern: String::new(),
                keep: true,
            },
        }];
        assert!(!valid_chain(&ctx, &steps, false));
        steps[0].enabled = false;
        assert!(!valid_chain(&ctx, &steps, false));
        assert!(valid_chain(&ctx, &steps, true));
        steps[0].enabled = true;
        clear_widgets(&ctx, &[]);
        assert!(!valid_chain(&ctx, &steps, false));
    }

    fn draw(
        ctx: &egui::Context,
        steps: &mut Vec<Step>,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let colors = theme::palette(false, Preferences::default().accent);
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(320.0, 900.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let boundary = ui.available_rect_before_wrap();
                    show(ui, Language::En, colors, steps, &[]);
                    assert!(ui.min_rect().right() <= boundary.right() + 0.5);
                });
            },
        );
        output.textures_delta.clear();
        output
    }

    #[test]
    fn narrow_collapsed_card_keeps_actions_accessible() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        theme::apply(&ctx, &Preferences::default());
        let mut steps = vec![Step {
            id: 1,
            enabled: false,
            operation: Operation::Deduplicate,
        }];
        let output = draw(&ctx, &mut steps, vec![]);
        let bounds = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find_map(|(_, node)| {
                (node.label() == Some("1. Deduplicate"))
                    .then(|| node.bounds())
                    .flatten()
            })
            .expect("card header must be accessible");
        let pos = egui::pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            ((bounds.y0 + bounds.y1) / 2.0) as f32,
        );
        output.drop_without_applying_deltas();
        let event = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        draw(
            &ctx,
            &mut steps,
            vec![egui::Event::PointerMoved(pos), event(true)],
        )
        .drop_without_applying_deltas();
        let output = draw(
            &ctx,
            &mut steps,
            vec![egui::Event::PointerMoved(pos), event(false)],
        );
        let update = output
            .platform_output
            .accesskit_update
            .as_ref()
            .expect("accessibility update");
        for label in ["Enabled", "Move up", "Move down", "Duplicate", "Delete"] {
            assert!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some(label)),
                "missing {label}"
            );
        }
        assert!(!steps[0].enabled);
        output.drop_without_applying_deltas();
    }
}
