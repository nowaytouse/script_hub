use super::card;
use crate::gui::i18n::{Text, text};
use crate::gui::theme::Palette;
use crate::relay::model::dns::{self, Format, Provider, Selection};
use crate::relay::model::pipeline::{
    self, AddressFamily, Condition, Field, KeepOccurrence, MatchMode, NameCase, Operation,
    RegionLabel, RenameRule, SortKey, StageReport, Step, TextMatch,
};
use crate::relay::model::properties::Setting;
use crate::runtime::settings::Language;
use eframe::egui;
use zeroize::{Zeroize, Zeroizing};

const MAX_STEPS: usize = 64;
const FIELDS: [&str; 13] = [
    "pattern",
    "replacement",
    "template",
    "protocols",
    "patterns",
    "regions",
    "server_name",
    "alpn",
    "separator",
    "server",
    "dns_include",
    "dns_exclude",
    "networks",
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
            for i in 0..64 {
                for name in [
                    "condition",
                    "region_code",
                    "region_label",
                    "rename_pattern",
                    "rename_replacement",
                ] {
                    let id = field_id(step.id, name).with(i);
                    data.remove::<egui::text_edit::TextEditState>(id);
                    data.remove::<Zeroizing<String>>(id.with("draft"));
                }
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
    field_with_limit(ui, label, value, id, multiline, 512)
}

fn field_with_limit(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    id: egui::Id,
    multiline: bool,
    limit: usize,
) -> bool {
    let label = ui.label(label);
    let edit = if multiline {
        egui::TextEdit::multiline(value).desired_rows(3)
    } else {
        egui::TextEdit::singleline(value)
    };
    let mut output = edit
        .id(id)
        .char_limit(limit)
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
        Operation::HostFilter { .. } => Text::OperationHostFilter,
        Operation::PortFilter { .. } => Text::OperationPortFilter,
        Operation::RegionFilter { .. } => Text::OperationRegionFilter,
        Operation::IpFilter { .. } => Text::OperationIpFilter,
        Operation::Limit { .. } => Text::OperationLimit,
        Operation::Reverse => Text::OperationReverse,
        Operation::ResolveDomain { .. } => Text::OperationResolveDomain,
        Operation::SetProperty { .. } => Text::OperationSetProperty,
        Operation::ConditionFilter { .. } => Text::OperationConditionFilter,
        Operation::NormalizeName { .. } => Text::OperationNormalizeName,
        Operation::UniqueNames { .. } => Text::OperationUniqueNames,
        Operation::RegionNames { .. } => Text::OperationRegionNames,
        Operation::Shuffle { .. } => Text::OperationShuffle,
        Operation::BatchRename { .. } => Text::OperationBatchRename,
        Operation::Slice { .. } => Text::OperationSlice,
        Operation::DeduplicateName { .. } => Text::OperationDeduplicateName,
        Operation::NetworkFilter { .. } => Text::OperationNetworkFilter,
    }
}

enum Action {
    Up,
    Down,
    Duplicate,
    Delete,
}

fn new_operations() -> [Operation; 25] {
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
        Operation::HostFilter {
            pattern: String::new(),
            keep: true,
        },
        Operation::PortFilter {
            start: 1,
            end: 65535,
            keep: true,
        },
        Operation::RegionFilter {
            regions: vec!["JP".into()],
            keep: true,
            keep_unknown: true,
        },
        Operation::IpFilter {
            family: AddressFamily::V4,
            keep_domains: true,
        },
        Operation::Limit { count: 50 },
        Operation::Reverse,
        Operation::ResolveDomain {
            family: AddressFamily::PreferV4,
            options: dns::Options::default(),
        },
        Operation::SetProperty {
            setting: Setting::TcpFastOpen { enabled: true },
        },
        Operation::ConditionFilter {
            conditions: vec![Condition {
                field: Field::Name,
                matcher: TextMatch::Contains,
                value: String::new(),
                negate: false,
            }],
            mode: MatchMode::All,
            keep: true,
        },
        Operation::NormalizeName {
            case: NameCase::Preserve,
            trim: true,
            collapse_whitespace: true,
        },
        Operation::UniqueNames {
            separator: " · ".into(),
        },
        Operation::RegionNames {
            labels: vec![
                RegionLabel {
                    region: "JP".into(),
                    label: "Japan".into(),
                },
                RegionLabel {
                    region: "US".into(),
                    label: "United States".into(),
                },
            ],
            separator: " | ".into(),
            keep_name: true,
        },
        Operation::Shuffle { seed: 0 },
        Operation::BatchRename {
            rules: vec![RenameRule {
                pattern: String::new(),
                replacement: String::new(),
                regex: false,
            }],
        },
        Operation::Slice { skip: 0, count: 50 },
        Operation::DeduplicateName {
            keep: KeepOccurrence::First,
            ignore_case: false,
        },
        Operation::NetworkFilter {
            networks: Vec::new(),
            keep: true,
            keep_domains: true,
        },
    ]
}

fn list_field(ui: &mut egui::Ui, id: egui::Id, label: &str, values: &mut Vec<String>) -> bool {
    list_field_with_limit(ui, id, label, values, 512)
}

fn list_field_with_limit(
    ui: &mut egui::Ui,
    id: egui::Id,
    label: &str,
    values: &mut Vec<String>,
    limit: usize,
) -> bool {
    let mut draft = ui
        .ctx()
        .data_mut(|data| data.get_temp::<Zeroizing<String>>(id.with("draft")))
        .unwrap_or_else(|| Zeroizing::new(values.join(", ")));
    if !field_with_limit(ui, label, &mut draft, id, false, limit) {
        return false;
    }
    values.zeroize();
    *values = draft
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    ui.ctx()
        .data_mut(|data| data.insert_temp(id.with("draft"), draft));
    true
}

fn family_control(
    ui: &mut egui::Ui,
    language: Language,
    family: &mut AddressFamily,
    preferred: bool,
) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for (value, label) in [
            (AddressFamily::V4, Text::OperationIpv4),
            (AddressFamily::V6, Text::OperationIpv6),
            (AddressFamily::PreferV4, Text::OperationPreferIpv4),
            (AddressFamily::PreferV6, Text::OperationPreferIpv6),
        ] {
            if !preferred && matches!(value, AddressFamily::PreferV4 | AddressFamily::PreferV6) {
                continue;
            }
            if ui
                .selectable_label(*family == value, text(language, label))
                .clicked()
                && *family != value
            {
                *family = value;
                changed = true;
            }
        }
    });
    changed
}

fn choice<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    language: Language,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: Text,
    value: &mut T,
    options: &[(T, Text)],
) -> bool {
    let label_response = ui.label(text(language, label));
    let selected = options
        .iter()
        .find(|(candidate, _)| *candidate == *value)
        .map(|(_, label)| *label)
        .unwrap_or(label);
    let mut changed = false;
    let response = egui::ComboBox::from_id_salt(id)
        .width(ui.available_width().min(220.0))
        .selected_text(text(language, selected))
        .show_ui(ui, |ui| {
            for (candidate, label) in options {
                if ui
                    .selectable_label(*candidate == *value, text(language, *label))
                    .clicked()
                    && *candidate != *value
                {
                    *value = *candidate;
                    changed = true;
                }
            }
        });
    let _ = response.response.labelled_by(label_response.id);
    changed
}

fn clear_rows(ctx: &egui::Context, step: u64, names: &[&str], limit: usize) {
    ctx.data_mut(|data| {
        for i in 0..limit {
            for name in names {
                let id = field_id(step, name).with(i);
                data.remove::<egui::text_edit::TextEditState>(id);
                data.remove::<Zeroizing<String>>(id.with("draft"));
            }
        }
    });
}

fn next_step_id(steps: &[Step]) -> Option<u64> {
    steps
        .iter()
        .map(|step| step.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .or_else(|| (0..=MAX_STEPS as u64).find(|id| steps.iter().all(|step| step.id != *id)))
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
    let next_id = next_step_id(steps);
    ui.add_enabled_ui(steps.len() < MAX_STEPS && next_id.is_some(), |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.menu_button(tr(Text::OperationAdd), |ui| {
                for (group, label) in [
                    (0, Text::OperationGroupFilters),
                    (1, Text::OperationGroupOrder),
                    (2, Text::OperationGroupNetwork),
                ] {
                    ui.menu_button(tr(label), |ui| {
                        for operation in new_operations()
                            .into_iter()
                            .filter(|op| operation_group(op) == group)
                        {
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
                }
            });
            if ui.button(tr(Text::ResolutionQuickAdd)).clicked()
                && let Some(id) = next_id
            {
                steps.push(Step {
                    id,
                    enabled: true,
                    operation: Operation::ResolveDomain {
                        family: AddressFamily::PreferV4,
                        options: dns::Options::default(),
                    },
                });
                changed = true;
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
                    Operation::BatchRename { rules } => {
                        let mut action = None;
                        let len = rules.len();
                        for (i, rule) in rules.iter_mut().enumerate() {
                            ui.separator();
                            ui.label(format!("{} {}", tr(Text::RenameRule), i + 1));
                            changed |= field(
                                ui,
                                tr(Text::OperationPattern),
                                &mut rule.pattern,
                                field_id(step.id, "rename_pattern").with(i),
                                false,
                            );
                            changed |= field(
                                ui,
                                tr(Text::OperationReplacement),
                                &mut rule.replacement,
                                field_id(step.id, "rename_replacement").with(i),
                                false,
                            );
                            changed |= ui
                                .checkbox(&mut rule.regex, tr(Text::OperationUseRegex))
                                .changed();
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .add_enabled(
                                        i > 0,
                                        egui::Button::new(tr(Text::OperationMoveUp)),
                                    )
                                    .clicked()
                                {
                                    action = Some((i, 0));
                                }
                                if ui
                                    .add_enabled(
                                        i + 1 < len,
                                        egui::Button::new(tr(Text::OperationMoveDown)),
                                    )
                                    .clicked()
                                {
                                    action = Some((i, 1));
                                }
                                if ui.button(tr(Text::OperationDelete)).clicked() {
                                    action = Some((i, 2));
                                }
                            });
                        }
                        if let Some((i, kind)) = action {
                            match kind {
                                0 => rules.swap(i, i - 1),
                                1 => rules.swap(i, i + 1),
                                _ => {
                                    rules.remove(i);
                                }
                            }
                            clear_rows(
                                ui.ctx(),
                                step.id,
                                &["rename_pattern", "rename_replacement"],
                                32,
                            );
                            changed = true;
                        }
                        if ui
                            .add_enabled(rules.len() < 32, egui::Button::new(tr(Text::RenameAdd)))
                            .clicked()
                        {
                            rules.push(RenameRule {
                                pattern: String::new(),
                                replacement: String::new(),
                                regex: false,
                            });
                            changed = true;
                        }
                    }
                    Operation::Slice { skip, count } => {
                        ui.label(tr(Text::SliceSkip));
                        changed |= ui
                            .add(egui::DragValue::new(skip).range(0..=100000))
                            .changed();
                        ui.label(tr(Text::SliceCount));
                        changed |= ui
                            .add(egui::DragValue::new(count).range(0..=100000))
                            .changed();
                        ui.weak(tr(Text::SliceHelp));
                    }
                    Operation::DeduplicateName { keep, ignore_case } => {
                        ui.weak(tr(Text::DeduplicateNameHelp));
                        changed |= choice(
                            ui,
                            language,
                            (step.id, "occurrence"),
                            Text::DeduplicateOccurrence,
                            keep,
                            &[
                                (KeepOccurrence::First, Text::DeduplicateFirst),
                                (KeepOccurrence::Last, Text::DeduplicateLast),
                            ],
                        );
                        changed |= ui
                            .checkbox(ignore_case, tr(Text::DeduplicateIgnoreCase))
                            .changed();
                    }
                    Operation::NetworkFilter {
                        networks,
                        keep,
                        keep_domains,
                    } => {
                        changed |= list_field_with_limit(
                            ui,
                            field_id(step.id, "networks"),
                            tr(Text::NetworkList),
                            networks,
                            4096,
                        );
                        ui.weak(tr(Text::NetworkHelp));
                        changed |= ui.checkbox(keep, tr(Text::OperationKeepMatches)).changed();
                        changed |= ui
                            .checkbox(keep_domains, tr(Text::OperationKeepDomains))
                            .changed();
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
                                (SortKey::NaturalName, Text::OperationSortNatural),
                                (SortKey::Server, Text::OperationSortServer),
                                (SortKey::Port, Text::OperationSortPort),
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
                    Operation::HostFilter { pattern, keep } => {
                        changed |= field(
                            ui,
                            tr(Text::OperationHostPattern),
                            pattern,
                            field_id(step.id, "pattern"),
                            false,
                        );
                        changed |= ui.checkbox(keep, tr(Text::OperationKeepMatches)).changed();
                    }
                    Operation::PortFilter { start, end, keep } => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(tr(Text::OperationPortRange));
                            changed |= ui
                                .add(egui::DragValue::new(start).range(1..=65535))
                                .changed();
                            ui.label("–");
                            changed |= ui.add(egui::DragValue::new(end).range(1..=65535)).changed();
                        });
                        changed |= ui.checkbox(keep, tr(Text::OperationKeepMatches)).changed();
                    }
                    Operation::RegionFilter {
                        regions,
                        keep,
                        keep_unknown,
                    } => {
                        changed |= list_field(
                            ui,
                            field_id(step.id, "regions"),
                            tr(Text::OperationRegions),
                            regions,
                        );
                        changed |= ui.checkbox(keep, tr(Text::OperationKeepMatches)).changed();
                        changed |= ui
                            .checkbox(keep_unknown, tr(Text::OperationKeepUnknown))
                            .changed();
                    }
                    Operation::IpFilter {
                        family,
                        keep_domains,
                    } => {
                        changed |= family_control(ui, language, family, false);
                        changed |= ui
                            .checkbox(keep_domains, tr(Text::OperationKeepDomains))
                            .changed();
                    }
                    Operation::Limit { count } => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(tr(Text::OperationLimitCount));
                            changed |= ui
                                .add(egui::DragValue::new(count).range(1..=100000))
                                .changed();
                        });
                    }
                    Operation::Reverse => {}
                    Operation::ConditionFilter {
                        conditions,
                        mode,
                        keep,
                    } => {
                        changed |= choice(
                            ui,
                            language,
                            (step.id, "mode"),
                            Text::ConditionMode,
                            mode,
                            &[
                                (MatchMode::All, Text::ConditionAll),
                                (MatchMode::Any, Text::ConditionAny),
                            ],
                        );
                        let mut remove = None;
                        for (i, condition) in conditions.iter_mut().enumerate() {
                            ui.separator();
                            changed |= choice(
                                ui,
                                language,
                                (step.id, "field", i),
                                Text::ConditionField,
                                &mut condition.field,
                                &[
                                    (Field::Name, Text::OperationSortName),
                                    (Field::Server, Text::OperationSortServer),
                                    (Field::Protocol, Text::OperationSortProtocol),
                                    (Field::Port, Text::OperationSortPort),
                                    (Field::Region, Text::OperationSortRegion),
                                ],
                            );
                            changed |= choice(
                                ui,
                                language,
                                (step.id, "matcher", i),
                                Text::ConditionMatcher,
                                &mut condition.matcher,
                                &[
                                    (TextMatch::Equals, Text::ConditionEquals),
                                    (TextMatch::Contains, Text::ConditionContains),
                                    (TextMatch::Regex, Text::ConditionRegex),
                                ],
                            );
                            changed |= field(
                                ui,
                                tr(Text::ConditionValue),
                                &mut condition.value,
                                field_id(step.id, "condition").with(i),
                                false,
                            );
                            changed |= ui
                                .checkbox(&mut condition.negate, tr(Text::ConditionNegate))
                                .changed();
                            if ui.button(tr(Text::ConditionDelete)).clicked() {
                                remove = Some(i);
                            }
                        }
                        if let Some(i) = remove {
                            conditions.remove(i);
                            clear_rows(ui.ctx(), step.id, &["condition"], 32);
                            changed = true;
                        }
                        if ui
                            .add_enabled(
                                conditions.len() < 32,
                                egui::Button::new(tr(Text::ConditionAdd)),
                            )
                            .clicked()
                        {
                            conditions.push(Condition {
                                field: Field::Name,
                                matcher: TextMatch::Contains,
                                value: String::new(),
                                negate: false,
                            });
                            changed = true;
                        }
                        changed |= ui.checkbox(keep, tr(Text::OperationKeepMatches)).changed();
                    }
                    Operation::NormalizeName {
                        case,
                        trim,
                        collapse_whitespace,
                    } => {
                        changed |= choice(
                            ui,
                            language,
                            (step.id, "case"),
                            Text::NameCase,
                            case,
                            &[
                                (NameCase::Preserve, Text::NameCasePreserve),
                                (NameCase::Lower, Text::NameCaseLower),
                                (NameCase::Upper, Text::NameCaseUpper),
                            ],
                        );
                        changed |= ui.checkbox(trim, tr(Text::NameTrim)).changed();
                        changed |= ui
                            .checkbox(collapse_whitespace, tr(Text::NameCollapseWhitespace))
                            .changed();
                    }
                    Operation::UniqueNames { separator } => {
                        changed |= field(
                            ui,
                            tr(Text::NameSeparator),
                            separator,
                            field_id(step.id, "separator"),
                            false,
                        );
                    }
                    Operation::RegionNames {
                        labels,
                        separator,
                        keep_name,
                    } => {
                        ui.weak(tr(Text::RegionNamesHelp));
                        let mut remove = None;
                        for (i, label) in labels.iter_mut().enumerate() {
                            ui.separator();
                            changed |= field(
                                ui,
                                tr(Text::RegionCode),
                                &mut label.region,
                                field_id(step.id, "region_code").with(i),
                                false,
                            );
                            changed |= field(
                                ui,
                                tr(Text::RegionLabel),
                                &mut label.label,
                                field_id(step.id, "region_label").with(i),
                                false,
                            );
                            if ui.button(tr(Text::RegionDelete)).clicked() {
                                remove = Some(i);
                            }
                        }
                        if let Some(i) = remove {
                            labels.remove(i);
                            clear_rows(ui.ctx(), step.id, &["region_code", "region_label"], 64);
                            changed = true;
                        }
                        if ui
                            .add_enabled(labels.len() < 64, egui::Button::new(tr(Text::RegionAdd)))
                            .clicked()
                        {
                            labels.push(RegionLabel {
                                region: String::new(),
                                label: String::new(),
                            });
                            changed = true;
                        }
                        changed |= field(
                            ui,
                            tr(Text::NameSeparator),
                            separator,
                            field_id(step.id, "separator"),
                            false,
                        );
                        changed |= ui.checkbox(keep_name, tr(Text::RegionKeepName)).changed();
                    }
                    Operation::Shuffle { seed } => {
                        ui.label(tr(Text::ShuffleSeed));
                        changed |= ui.add(egui::DragValue::new(seed)).changed();
                        ui.weak(tr(Text::ShuffleHelp));
                    }
                    Operation::ResolveDomain { family, options } => {
                        changed |= family_control(ui, language, family, true);
                        changed |= choice(
                            ui,
                            language,
                            (step.id, "dns_provider"),
                            Text::ResolutionProviders,
                            &mut options.provider,
                            &[
                                (Provider::Cloudflare, Text::ResolutionCloudflare),
                                (Provider::Google, Text::ResolutionGoogle),
                            ],
                        );
                        changed |= choice(
                            ui,
                            language,
                            (step.id, "dns_format"),
                            Text::ResolutionFormat,
                            &mut options.format,
                            &[
                                (Format::Post, Text::ResolutionPost),
                                (Format::Get, Text::ResolutionGet),
                                (Format::Json, Text::ResolutionJson),
                            ],
                        );
                        changed |= choice(
                            ui,
                            language,
                            (step.id, "dns_selection"),
                            Text::ResolutionSelection,
                            &mut options.selection,
                            &[
                                (Selection::Lowest, Text::ResolutionLowest),
                                (Selection::First, Text::ResolutionFirst),
                                (Selection::Stable, Text::ResolutionStable),
                            ],
                        );
                        if options.selection == Selection::Stable {
                            let label = ui.label(tr(Text::ShuffleSeed));
                            let control = ui.add(egui::DragValue::new(&mut options.seed));
                            changed |= control.changed();
                            let _ = control.labelled_by(label.id);
                        }
                        ui.weak(tr(Text::ResolutionPreview));
                        ui.collapsing(tr(Text::ResolutionAdvanced), |ui| {
                            changed |= ui
                                .checkbox(&mut options.cache, tr(Text::ResolutionCache))
                                .changed();
                            changed |= field(
                                ui,
                                tr(Text::ResolutionInclude),
                                &mut options.include,
                                field_id(step.id, "dns_include"),
                                false,
                            );
                            changed |= field(
                                ui,
                                tr(Text::ResolutionExclude),
                                &mut options.exclude,
                                field_id(step.id, "dns_exclude"),
                                false,
                            );
                            let label = ui.label(tr(Text::ResolutionTimeout));
                            let control = ui.add(
                                egui::DragValue::new(&mut options.timeout_seconds).range(1..=20),
                            );
                            changed |= control.changed();
                            let _ = control.labelled_by(label.id);
                            let label = ui.label(tr(Text::ResolutionConcurrency));
                            let control =
                                ui.add(egui::DragValue::new(&mut options.concurrency).range(1..=4));
                            changed |= control.changed();
                            let _ = control.labelled_by(label.id);
                            changed |= ui
                                .checkbox(&mut options.require_ad, tr(Text::ResolutionRequireAd))
                                .changed();
                            ui.weak(tr(Text::ResolutionAdHelp));
                        });
                        if let Some(report) = reports
                            .iter()
                            .find(|r| r.id == step.id)
                            .and_then(|r| r.resolution.as_ref())
                        {
                            ui.collapsing(tr(Text::ResolutionStats), |ui| {
                                for (label, value) in [
                                    (Text::ResolutionStatUnique, report.unique_hosts),
                                    (Text::ResolutionStatQueries, report.queries),
                                    (Text::ResolutionStatCache, report.cache_hits),
                                    (Text::ResolutionStatNegative, report.negative_hits),
                                    (Text::ResolutionStatNx, report.nxdomain),
                                    (Text::ResolutionStatNodata, report.nodata),
                                    (Text::ResolutionStatResolved, report.resolved),
                                    (Text::ResolutionStatFiltered, report.filtered),
                                    (Text::ResolutionStatUnsafe, report.unsafe_answers),
                                    (Text::ResolutionStatAuthenticated, report.authenticated),
                                    (Text::ResolutionStatUnverified, report.unverified),
                                    (Text::ResolutionStatCandidates, report.candidates),
                                ] {
                                    ui.label(format!("{}: {value}", tr(label)));
                                }
                            });
                        }
                    }
                    Operation::SetProperty { setting } => {
                        let current = match setting {
                            Setting::Udp { .. } => 0,
                            Setting::TcpFastOpen { .. } => 1,
                            Setting::VerifyTls => 2,
                            Setting::ServerName { .. } => 3,
                            Setting::Alpn { .. } => 4,
                            Setting::Server { .. } => 5,
                            Setting::Port { .. } => 6,
                        };
                        let labels = [
                            Text::PropertyUdp,
                            Text::PropertyTfo,
                            Text::PropertyVerifyTls,
                            Text::PropertyServerName,
                            Text::PropertyAlpn,
                            Text::PropertyServer,
                            Text::PropertyPort,
                        ];
                        egui::ComboBox::from_id_salt(("property-kind", step.id))
                            .selected_text(tr(labels[current]))
                            .show_ui(ui, |ui| {
                                for (index, label) in labels.iter().enumerate() {
                                    if ui.selectable_label(index == current, tr(*label)).clicked()
                                        && index != current
                                    {
                                        *setting = match index {
                                            0 => Setting::Udp { enabled: true },
                                            1 => Setting::TcpFastOpen { enabled: true },
                                            2 => Setting::VerifyTls,
                                            3 => Setting::ServerName {
                                                value: String::new(),
                                            },
                                            4 => Setting::Alpn { values: Vec::new() },
                                            5 => Setting::Server {
                                                value: String::new(),
                                            },
                                            _ => Setting::Port { value: 443 },
                                        };
                                        for name in ["server_name", "alpn", "server"] {
                                            let id = field_id(step.id, name);
                                            ui.ctx().data_mut(|data| {
                                                data.remove::<egui::text_edit::TextEditState>(id);
                                                data.remove::<Zeroizing<String>>(id.with("draft"));
                                            });
                                        }
                                        changed = true;
                                    }
                                }
                            });
                        match setting {
                            Setting::Udp { enabled } | Setting::TcpFastOpen { enabled } => {
                                changed |=
                                    ui.checkbox(enabled, tr(Text::PropertyEnabled)).changed();
                            }
                            Setting::VerifyTls => {
                                ui.weak(tr(Text::PropertyVerifyTlsHelp));
                            }
                            Setting::ServerName { value } => {
                                changed |= field(
                                    ui,
                                    tr(Text::PropertyServerName),
                                    value,
                                    field_id(step.id, "server_name"),
                                    false,
                                );
                            }
                            Setting::Alpn { values } => {
                                changed |= list_field(
                                    ui,
                                    field_id(step.id, "alpn"),
                                    tr(Text::PropertyAlpnHelp),
                                    values,
                                );
                            }
                            Setting::Server { value } => {
                                changed |= field(
                                    ui,
                                    tr(Text::PropertyServer),
                                    value,
                                    field_id(step.id, "server"),
                                    false,
                                );
                            }
                            Setting::Port { value } => {
                                ui.label(tr(Text::PropertyPort));
                                changed |= ui
                                    .add(egui::DragValue::new(value).range(1..=65535))
                                    .changed();
                            }
                        }
                        ui.weak(tr(Text::PropertySkippedHelp));
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    changed |= ui
                        .checkbox(&mut step.enabled, tr(Text::OperationEnabled))
                        .changed();
                    if let Some(report) = reports.iter().find(|report| report.id == step.id) {
                        ui.weak(format!(
                            "{} → {} · {} {} · {} {} · {} {}",
                            report.before,
                            report.after,
                            tr(Text::OperationRenamed),
                            report.renamed,
                            tr(Text::OperationModified),
                            report.modified,
                            tr(Text::OperationSkipped),
                            report.skipped
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

fn operation_group(op: &Operation) -> u8 {
    match op {
        Operation::NameFilter { .. }
        | Operation::ProtocolFilter { .. }
        | Operation::HostFilter { .. }
        | Operation::PortFilter { .. }
        | Operation::RegionFilter { .. }
        | Operation::IpFilter { .. }
        | Operation::NetworkFilter { .. }
        | Operation::ConditionFilter { .. } => 0,
        Operation::ResolveDomain { .. } | Operation::SetProperty { .. } => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_maximum_identifier_does_not_disable_new_operations() {
        let steps = [Step {
            id: u64::MAX,
            enabled: true,
            operation: Operation::Reverse,
        }];
        assert_eq!(next_step_id(&steps), Some(0));
    }
    use crate::gui::theme;
    use crate::runtime::settings::Preferences;

    #[test]
    fn operation_defaults_and_groups_cover_all_choices() {
        let operations = new_operations();
        assert_eq!(operations.len(), 25);
        assert_eq!(
            operations
                .iter()
                .filter(|op| operation_group(op) == 0)
                .count(),
            8
        );
        assert_eq!(
            operations
                .iter()
                .filter(|op| operation_group(op) == 1)
                .count(),
            15
        );
        assert_eq!(
            operations
                .iter()
                .filter(|op| operation_group(op) == 2)
                .count(),
            2
        );
        assert!(
            matches!(&operations[8], Operation::HostFilter { pattern, keep: true } if pattern.is_empty())
        );
        assert!(matches!(
            &operations[9],
            Operation::PortFilter {
                start: 1,
                end: 65535,
                keep: true
            }
        ));
        assert!(
            matches!(&operations[10], Operation::RegionFilter { regions, keep: true, keep_unknown: true } if regions == &["JP"])
        );
        assert!(matches!(
            &operations[11],
            Operation::IpFilter {
                family: AddressFamily::V4,
                keep_domains: true
            }
        ));
        assert!(matches!(&operations[12], Operation::Limit { count: 50 }));
        assert!(matches!(
            &operations[14],
            Operation::ResolveDomain {
                family: AddressFamily::PreferV4,
                ..
            }
        ));
        assert!(matches!(
            &operations[15],
            Operation::SetProperty {
                setting: Setting::TcpFastOpen { enabled: true }
            }
        ));
        assert!(
            matches!(&operations[16], Operation::ConditionFilter { conditions, mode: MatchMode::All, keep: true } if conditions.len() == 1 && conditions[0].value.is_empty() && conditions[0].field == Field::Name && conditions[0].matcher == TextMatch::Contains)
        );
        assert!(matches!(
            &operations[17],
            Operation::NormalizeName {
                case: NameCase::Preserve,
                trim: true,
                collapse_whitespace: true
            }
        ));
        assert!(
            matches!(&operations[18], Operation::UniqueNames { separator } if separator == " · ")
        );
        assert!(
            matches!(&operations[19], Operation::RegionNames { labels, separator, keep_name: true } if labels.len() == 2 && labels[0].region == "JP" && labels[0].label == "Japan" && labels[1].region == "US" && labels[1].label == "United States" && separator == " | ")
        );
        assert!(matches!(&operations[20], Operation::Shuffle { seed: 0 }));
        let condition = Step {
            id: 1,
            enabled: true,
            operation: operations[16].clone(),
        };
        assert!(
            pipeline::validate(&[condition]).is_err(),
            "empty condition must require input"
        );
    }

    #[test]
    fn clearing_widgets_removes_new_sensitive_drafts() {
        let ctx = egui::Context::default();
        let steps = [Step {
            id: 4,
            enabled: true,
            operation: Operation::Reverse,
        }];
        for name in [
            "regions",
            "server_name",
            "alpn",
            "dns_include",
            "dns_exclude",
            "networks",
        ] {
            let id = field_id(4, name).with("draft");
            ctx.data_mut(|data| data.insert_temp(id, Zeroizing::new("private".to_owned())));
        }
        let row_id = field_id(4, "condition").with(0).with("draft");
        ctx.data_mut(|data| data.insert_temp(row_id, Zeroizing::new("private value".to_owned())));
        for name in ["rename_pattern", "rename_replacement"] {
            let id = field_id(4, name).with(31).with("draft");
            ctx.data_mut(|data| data.insert_temp(id, Zeroizing::new("private".to_owned())));
        }
        clear_widgets(&ctx, &steps);
        for name in ["rename_pattern", "rename_replacement"] {
            assert!(
                ctx.data_mut(|data| data
                    .get_temp::<Zeroizing<String>>(field_id(4, name).with(31).with("draft")))
                    .is_none()
            );
        }
        assert!(
            ctx.data_mut(|data| data.get_temp::<Zeroizing<String>>(row_id))
                .is_none()
        );
        for name in [
            "regions",
            "server_name",
            "alpn",
            "dns_include",
            "dns_exclude",
            "networks",
        ] {
            assert!(
                ctx.data_mut(
                    |data| data.get_temp::<Zeroizing<String>>(field_id(4, name).with("draft"))
                )
                .is_none()
            );
        }
    }

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
        let colors = theme::palette(
            ctx.theme() == egui::Theme::Dark,
            Preferences::default().accent,
        );
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
    fn new_operations_and_properties_fit_narrow_cards() {
        let mut ops: Vec<_> = new_operations().into_iter().skip(16).collect();
        ops.push(Operation::Sort {
            key: SortKey::NaturalName,
            descending: false,
        });
        ops.push(Operation::SetProperty {
            setting: Setting::Server {
                value: String::new(),
            },
        });
        ops.push(Operation::SetProperty {
            setting: Setting::Port { value: 443 },
        });
        for (i, operation) in ops.into_iter().enumerate() {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            theme::apply(&ctx, &Preferences::default());
            let mut steps = vec![Step {
                id: i as u64 + 1,
                enabled: true,
                operation,
            }];
            let output = draw(&ctx, &mut steps, vec![]);
            let update = output
                .platform_output
                .accesskit_update
                .as_ref()
                .expect("accessible controls");
            let expected = match i {
                0 => "Add condition",
                1 => "Letter case",
                2 => "Separator",
                3 => "Add region",
                4 => "Seed",
                5 => "Add rule",
                6 => "Skip nodes",
                7 => "Keep occurrence",
                8 => "CIDR networks, comma separated (maximum 64)",
                9 => "Natural name",
                10 => "Server address",
                _ => "Server port",
            };
            assert!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some(expected)
                        || node.value() == Some(expected)),
                "missing {expected}"
            );
            output.drop_without_applying_deltas();
        }
    }

    #[test]
    fn batch_rename_add_is_accessible_and_works_in_both_themes() {
        for dark in [false, true] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            let mut steps = vec![Step {
                id: 1,
                enabled: true,
                operation: Operation::BatchRename { rules: vec![] },
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
                    (node.label() == Some("Add rule"))
                        .then(|| node.bounds())
                        .flatten()
                })
                .expect("accessible add rule button");
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
            draw(
                &ctx,
                &mut steps,
                vec![egui::Event::PointerMoved(pos), event(false)],
            )
            .drop_without_applying_deltas();
            assert!(
                matches!(&steps[0].operation, Operation::BatchRename { rules } if rules.len() == 1 && !rules[0].regex)
            );
        }
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

    #[test]
    fn every_expanded_operation_fits_the_narrow_panel() {
        for operation in new_operations() {
            let ctx = egui::Context::default();
            theme::apply(&ctx, &Preferences::default());
            let mut steps = vec![Step {
                id: 1,
                enabled: true,
                operation,
            }];
            draw(&ctx, &mut steps, vec![]).drop_without_applying_deltas();
        }
    }

    #[test]
    fn dns_controls_fit_narrow_light_and_dark_cards() {
        for dark in [false, true] {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            let colors = theme::palette(dark, Preferences::default().accent);
            let mut steps = vec![Step {
                id: 1,
                enabled: true,
                operation: Operation::ResolveDomain {
                    family: AddressFamily::PreferV4,
                    options: dns::Options {
                        selection: Selection::Stable,
                        ..Default::default()
                    },
                },
            }];
            let draw_frame = |steps: &mut Vec<Step>, events: Vec<egui::Event>| {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(320.0, 1200.0),
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
                )
            };
            let output = draw_frame(&mut steps, vec![]);
            let nodes = &output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes;
            for label in [
                "DNS provider",
                "Request format",
                "Address selection",
                "Seed",
                "Advanced DNS options",
            ] {
                assert!(
                    nodes
                        .iter()
                        .any(|(_, n)| n.label() == Some(label) || n.value() == Some(label)),
                    "missing {label}"
                );
            }
            let bounds = nodes
                .iter()
                .find_map(|(_, n)| {
                    (n.label() == Some("Advanced DNS options"))
                        .then(|| n.bounds())
                        .flatten()
                })
                .unwrap();
            let pos = egui::pos2(
                ((bounds.x0 + bounds.x1) / 2.0) as f32,
                ((bounds.y0 + bounds.y1) / 2.0) as f32,
            );
            output.drop_without_applying_deltas();
            let pointer = |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            };
            draw_frame(
                &mut steps,
                vec![egui::Event::PointerMoved(pos), pointer(true)],
            )
            .drop_without_applying_deltas();
            let output = draw_frame(
                &mut steps,
                vec![egui::Event::PointerMoved(pos), pointer(false)],
            );
            let nodes = &output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes;
            for label in [
                "Use session cache",
                "Include hostnames (regex; empty means all)",
                "Exclude hostnames (regex)",
                "Request timeout (seconds)",
                "Maximum concurrent requests",
                "Require provider-asserted DNSSEC AD",
            ] {
                assert!(
                    nodes
                        .iter()
                        .any(|(_, n)| n.label() == Some(label) || n.value() == Some(label)),
                    "missing {label}"
                );
            }
            output.drop_without_applying_deltas();
        }
    }
}
