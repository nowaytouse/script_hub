use super::card;
use crate::gui::i18n::{Text, text};
use crate::gui::theme::Palette;
use crate::relay::model::dns::{self, Format, Provider, Selection};
use crate::relay::model::pipeline::{
    self, AddressFamily, Condition, Field, KeepOccurrence, MatchMode, NameCase, Operation,
    RegionLabel, RenameRule, SortKey, StageReport, Step, TextMatch,
};
use crate::relay::model::properties::Setting;
use crate::relay::model::script::{self, SyntaxError};
use crate::runtime::settings::Language;
use eframe::egui;
use std::collections::BTreeMap;
use zeroize::{Zeroize, Zeroizing};

const MAX_STEPS: usize = 64;
const BATCH_SCRIPT_EXAMPLE: &str = "fn operator(nodes, context) {
    if nodes.len > 1 {
        let first = nodes[0];
        nodes[0] = nodes[1];
        nodes[1] = first;
    }
    nodes
}";
const FIELDS: [&str; 14] = [
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
    "script",
];

fn field_id(id: u64, field: &str) -> egui::Id {
    egui::Id::unique("relay-operation-field")
        .with(id)
        .with(field)
}

fn script_arguments_id(step_id: u64) -> egui::Id {
    field_id(step_id, "script-arguments")
}

fn validation_id() -> egui::Id {
    egui::Id::unique("relay-operation-validation")
}

fn opening_step_id() -> egui::Id {
    egui::Id::unique("relay-operation-opening")
}

fn search_id() -> egui::Id {
    egui::Id::unique("relay-operation-search")
}

#[derive(Clone, Copy)]
struct Validation {
    valid: bool,
    first_invalid: Option<u64>,
}

fn validate_chain(ctx: &egui::Context, steps: &[Step], changed: bool) -> Validation {
    if !changed
        && let Some(cached) = ctx.data_mut(|data| data.get_temp::<Validation>(validation_id()))
    {
        return cached;
    }
    let valid = pipeline::validate(steps).is_ok();
    let first_invalid = if valid {
        None
    } else {
        steps
            .iter()
            .filter(|step| step.enabled)
            .find(|step| pipeline::validate(std::slice::from_ref(*step)).is_err())
            .map(|step| step.id)
    };
    let validation = Validation {
        valid,
        first_invalid,
    };
    ctx.data_mut(|data| data.insert_temp(validation_id(), validation));
    validation
}

pub(super) fn clear_widgets(ctx: &egui::Context, steps: &[Step]) {
    ctx.data_mut(|data| {
        data.remove::<Validation>(validation_id());
        data.remove::<u64>(opening_step_id());
        data.remove::<Zeroizing<String>>(search_id());
        data.remove::<egui::text_edit::TextEditState>(search_id());
        for step in steps {
            data.remove::<Result<(), SyntaxError>>(field_id(step.id, "script").with("syntax"));
            if let Operation::Script { arguments, .. } = &step.operation {
                let id = script_arguments_id(step.id);
                data.remove::<ScriptArgumentDraft>(id);
                for field in ["new-name", "new-value"] {
                    data.remove::<egui::text_edit::TextEditState>(id.with(field));
                }
                for name in arguments.keys() {
                    data.remove::<egui::text_edit::TextEditState>(id.with(("value", name)));
                }
            }
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

fn script_field(
    ui: &mut egui::Ui,
    language: Language,
    colors: Palette,
    step_id: u64,
    source: &mut String,
) -> bool {
    let tr = |key| text(language, key);
    let id = field_id(step_id, "script");
    let syntax_id = id.with("syntax");
    ui.label(tr(Text::OperationScriptHelp));
    egui::CollapsingHeader::new(tr(Text::OperationScriptReference))
        .id_salt(id.with("reference"))
        .show(ui, |ui| {
            ui.label(tr(Text::OperationScriptSignatures));
            ui.label(tr(Text::OperationScriptContext));
            ui.label(tr(Text::OperationScriptSourceContext));
            ui.label(tr(Text::OperationScriptFields));
            ui.label(tr(Text::OperationScriptReadOnlyFields));
            ui.label(tr(Text::OperationScriptUnderlyingProxy));
            ui.label(tr(Text::OperationScriptRegex));
            ui.label(tr(Text::OperationScriptBatchExampleHelp));
        });
    let mut changed = field_with_limit(
        ui,
        tr(Text::OperationScriptSource),
        source,
        id,
        true,
        script::MAX_SOURCE,
    );
    let check = ui
        .horizontal_wrapped(|ui| {
            let check = ui.button(tr(Text::OperationScriptCheck)).clicked();
            let empty = source.trim().is_empty();
            let example = if empty {
                Text::OperationScriptInsertBatchExample
            } else {
                Text::OperationScriptCopyBatchExample
            };
            if ui.button(tr(example)).clicked() {
                if empty {
                    source.zeroize();
                    source.push_str(BATCH_SCRIPT_EXAMPLE);
                    changed = true;
                    ui.memory_mut(|memory| memory.request_focus(id));
                } else {
                    ui.ctx().copy_text(BATCH_SCRIPT_EXAMPLE.to_owned());
                }
            }
            check
        })
        .inner;
    if changed {
        ui.data_mut(|data| data.remove::<Result<(), SyntaxError>>(syntax_id));
    }
    if check {
        let result = script::check_syntax(source);
        ui.data_mut(|data| data.insert_temp(syntax_id, result));
    }
    match ui.data(|data| data.get_temp::<Result<(), SyntaxError>>(syntax_id)) {
        None => {}
        Some(Ok(())) => {
            ui.label(tr(Text::OperationScriptSyntaxOk));
        }
        Some(Err(SyntaxError::Size)) => {
            ui.colored_label(colors.error, tr(Text::OperationScriptSize));
        }
        Some(Err(SyntaxError::EntryPoint)) => {
            ui.colored_label(colors.error, tr(Text::OperationScriptEntry));
        }
        Some(Err(SyntaxError::Parse { line, column })) => {
            let message = tr(Text::OperationScriptSyntaxError);
            let message = match (line, column) {
                (Some(line), Some(column)) => format!("{message} ({line}:{column})."),
                (Some(line), None) => format!("{message} ({line})."),
                _ => format!("{message}."),
            };
            ui.colored_label(colors.error, message);
        }
    }
    changed
}

#[derive(Clone, Default)]
struct ScriptArgumentDraft {
    name: Zeroizing<String>,
    value: Zeroizing<String>,
}

fn script_arguments(
    ui: &mut egui::Ui,
    language: Language,
    colors: Palette,
    step_id: u64,
    arguments: &mut BTreeMap<String, String>,
) -> bool {
    let tr = |key| text(language, key);
    let id = script_arguments_id(step_id);
    let mut changed = false;
    egui::CollapsingHeader::new(tr(Text::OperationScriptArguments))
        .id_salt(id)
        .default_open(!arguments.is_empty())
        .show(ui, |ui| {
            ui.weak(tr(Text::OperationScriptArgumentsHelp));
            let mut remove = None;
            for (name, value) in arguments.iter_mut() {
                ui.push_id(name, |ui| {
                    changed |=
                        field_with_limit(ui, name, value, id.with(("value", name)), false, 512);
                    if !script::valid_argument_value(value) {
                        ui.colored_label(
                            colors.error,
                            tr(Text::OperationScriptArgumentValueInvalid),
                        );
                    }
                    let control = ui.button(tr(Text::OperationScriptArgumentRemove));
                    control.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            ui.is_enabled(),
                            format!("{}: {name}", tr(Text::OperationScriptArgumentRemove)),
                        )
                    });
                    if control.clicked() {
                        remove = Some(name.clone());
                    }
                });
            }
            if let Some(name) = remove
                && let Some(mut value) = arguments.remove(&name)
            {
                value.zeroize();
                ui.data_mut(|data| {
                    data.remove::<egui::text_edit::TextEditState>(id.with(("value", &name)))
                });
                changed = true;
            }

            if !arguments.is_empty() {
                ui.separator();
            }
            let mut draft = ui
                .data(|data| data.get_temp::<ScriptArgumentDraft>(id))
                .unwrap_or_default();
            let mut draft_changed = field_with_limit(
                ui,
                tr(Text::OperationScriptArgumentName),
                &mut draft.name,
                id.with("new-name"),
                false,
                128,
            );
            draft_changed |= field_with_limit(
                ui,
                tr(Text::OperationScriptArgumentValue),
                &mut draft.value,
                id.with("new-value"),
                false,
                512,
            );
            let issue = if arguments.len() >= script::MAX_ARGUMENTS {
                Some(Text::OperationScriptArgumentLimit)
            } else if draft.name.is_empty() && draft.value.is_empty() {
                None
            } else if !script::valid_argument_name(&draft.name) {
                Some(Text::OperationScriptArgumentNameInvalid)
            } else if arguments.contains_key(draft.name.as_str()) {
                Some(Text::OperationScriptArgumentDuplicate)
            } else if !script::valid_argument_value(&draft.value) {
                Some(Text::OperationScriptArgumentValueInvalid)
            } else {
                None
            };
            if let Some(issue) = issue {
                ui.colored_label(colors.error, tr(issue));
            }
            if ui
                .add_enabled(
                    !draft.name.is_empty() && issue.is_none(),
                    egui::Button::new(tr(Text::OperationScriptArgumentAdd)),
                )
                .clicked()
            {
                arguments.insert(
                    std::mem::take(&mut *draft.name),
                    std::mem::take(&mut *draft.value),
                );
                ui.memory_mut(|memory| memory.request_focus(id.with("new-name")));
                draft_changed = true;
                changed = true;
            }
            if draft_changed {
                ui.data_mut(|data| {
                    if draft.name.is_empty() && draft.value.is_empty() {
                        data.remove::<ScriptArgumentDraft>(id);
                    } else {
                        data.insert_temp(id, draft);
                    }
                });
            }
        });
    changed
}

fn operation_label(op: &Operation) -> Text {
    match op {
        Operation::Script { .. } => Text::OperationScript,
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

fn new_operations() -> [Operation; 26] {
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
        Operation::Script {
            source: "fn transform(node) { node.name = \"Hub \" + node.name; node }".into(),
            arguments: Default::default(),
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

fn operation_matches(label: Text, language: Language, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let localized = text(language, label).to_lowercase();
    let english = text(Language::En, label).to_lowercase();
    query
        .split_whitespace()
        .all(|word| localized.contains(word) || english.contains(word))
}

fn operation_menu(ui: &mut egui::Ui, language: Language) -> Option<Operation> {
    use egui::containers::menu::{MenuButton, MenuConfig};

    let tr = |key| text(language, key);
    let id = search_id();
    let (_, menu) = MenuButton::new(tr(Text::OperationAdd))
        .config(MenuConfig::new().close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside))
        .ui(ui, |ui| {
            ui.set_max_width(300.0);
            let previous = ui.data(|data| data.get_temp::<Zeroizing<String>>(id));
            let opening = previous.is_none();
            let mut query = previous.unwrap_or_default();
            let label = ui.label(tr(Text::OperationSearch));
            let input = ui
                .add(
                    egui::TextEdit::singleline(&mut *query)
                        .id(id)
                        .desired_width(220.0)
                        .char_limit(80),
                )
                .labelled_by(label.id);
            if !ui.is_sizing_pass() {
                if opening {
                    input.request_focus();
                }
                if opening || input.changed() {
                    ui.data_mut(|data| data.insert_temp(id, query.clone()));
                }
            }
            let query = Zeroizing::new(query.trim().to_lowercase());
            let list = egui::ScrollArea::vertical().max_height(320.0);
            let list = if input.changed() {
                list.vertical_scroll_offset(0.0)
            } else {
                list
            };
            list.show(ui, |ui| {
                let operations = new_operations();
                let mut any = false;
                for (group, label) in [
                    (0, Text::OperationGroupFilters),
                    (1, Text::OperationGroupOrder),
                    (2, Text::OperationGroupNetwork),
                    (3, Text::OperationGroupCustom),
                ] {
                    let mut heading = false;
                    for (index, operation) in operations.iter().enumerate() {
                        let key = operation_label(operation);
                        if operation_group(operation) != group
                            || !operation_matches(key, language, &query)
                        {
                            continue;
                        }
                        if !heading {
                            if any {
                                ui.separator();
                            }
                            ui.strong(tr(label));
                            heading = true;
                            any = true;
                        }
                        if ui.button(tr(key)).clicked() {
                            ui.close();
                            return operations.into_iter().nth(index);
                        }
                    }
                }
                if !any {
                    ui.weak(tr(Text::OperationSearchNoResults));
                }
                None
            })
            .inner
        });
    if menu
        .as_ref()
        .is_none_or(|response| response.inner.is_some())
    {
        ui.data_mut(|data| {
            data.remove::<Zeroizing<String>>(id);
            data.remove::<egui::text_edit::TextEditState>(id);
        });
    }
    menu.and_then(|response| response.inner)
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
    ui.horizontal_wrapped(|ui| {
        ui.strong(tr(Text::OperationChain));
        if !steps.is_empty() {
            let enabled = steps.iter().filter(|step| step.enabled).count();
            ui.weak(format!(
                "{}: {enabled}/{}",
                tr(Text::OperationEnabled),
                steps.len()
            ));
        }
    });
    ui.weak(tr(Text::OperationChainHelp));
    let next_id = next_step_id(steps);
    let can_add = steps.len() < MAX_STEPS && next_id.is_some();
    let mut add = None;
    ui.add_enabled_ui(can_add, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (label, kind) in [
                (Text::OperationNameFilter, Text::OperationNameFilter),
                (Text::OperationDeduplicate, Text::OperationDeduplicate),
                (Text::OperationSort, Text::OperationSort),
                (Text::ResolutionQuickAdd, Text::OperationResolveDomain),
            ] {
                if ui.button(tr(label)).clicked() {
                    add = new_operations()
                        .into_iter()
                        .find(|op| operation_label(op) == kind);
                }
            }
            if let Some(operation) = operation_menu(ui, language) {
                add = Some(operation);
            }
        });
    });
    let mut opening = ui.ctx().data_mut(|data| {
        let opening = data.get_temp::<u64>(opening_step_id());
        data.remove::<u64>(opening_step_id());
        opening
    });
    if can_add && let (Some(id), Some(operation)) = (next_id, add) {
        steps.push(Step {
            id,
            enabled: true,
            operation,
        });
        opening = Some(id);
        changed = true;
    }
    if steps.is_empty() {
        ui.weak(tr(Text::OperationEmpty));
    } else if !can_add {
        ui.weak(tr(Text::OperationStepLimit));
    }
    let mut action = None;
    let count = steps.len();
    let validation = validate_chain(ui.ctx(), steps, false);
    for (index, step) in steps.iter_mut().enumerate() {
        ui.add_space(6.0);
        ui.push_id(("relay-operation", step.id), |ui| {
            ui.spacing_mut().window_margin = egui::Margin::same(8);
            ui.spacing_mut().item_spacing.y = 6.0;
            card(ui, colors, |ui| {
                let title = format!("{}. {}", index + 1, tr(operation_label(&step.operation)));
                let configurable =
                    !matches!(step.operation, Operation::Deduplicate | Operation::Reverse);
                let details = |ui: &mut egui::Ui| match &mut step.operation {
                    Operation::Script { source, arguments } => {
                        changed |= script_field(ui, language, colors, step.id, source);
                        changed |= script_arguments(ui, language, colors, step.id, arguments);
                    }
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
                            Setting::UdpOverTcp { .. } => 7,
                        };
                        let labels = [
                            Text::PropertyUdp,
                            Text::PropertyTfo,
                            Text::PropertyVerifyTls,
                            Text::PropertyServerName,
                            Text::PropertyAlpn,
                            Text::PropertyServer,
                            Text::PropertyPort,
                            Text::PropertyUdpOverTcp,
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
                                            6 => Setting::Port { value: 443 },
                                            _ => Setting::UdpOverTcp { enabled: true },
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
                            Setting::UdpOverTcp { enabled } => {
                                changed |=
                                    ui.checkbox(enabled, tr(Text::PropertyEnabled)).changed();
                                ui.weak(tr(Text::PropertyUdpOverTcpHelp));
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
                };
                let response = if configurable {
                    egui::CollapsingHeader::new(title)
                        .id_salt(("relay-operation-card", step.id))
                        .default_open(opening == Some(step.id))
                        .open((opening == Some(step.id)).then_some(true))
                        .show(ui, details)
                        .header_response
                } else {
                    ui.strong(title)
                };
                if opening == Some(step.id) {
                    response.scroll_to_me(Some(egui::Align::Center));
                    if configurable {
                        response.request_focus();
                    }
                }
                if step.enabled && validation.first_invalid == Some(step.id) {
                    ui.colored_label(colors.error, tr(Text::OperationNeedsAttention));
                }
                if !changed
                    && step.enabled
                    && let Some(report) = reports.iter().find(|report| report.id == step.id)
                {
                    let mut summary = format!("{} → {}", report.before, report.after);
                    for (key, count) in [
                        (Text::OperationRenamed, report.renamed),
                        (Text::OperationModified, report.modified),
                        (Text::OperationSkipped, report.skipped),
                    ] {
                        if count > 0 {
                            use std::fmt::Write as _;
                            let _ = write!(summary, " · {} {count}", tr(key));
                        }
                    }
                    ui.weak(summary);
                }
                ui.horizontal_wrapped(|ui| {
                    changed |= ui
                        .checkbox(&mut step.enabled, tr(Text::OperationEnabled))
                        .changed();
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
                    ui.menu_button(tr(Text::OperationRowActions), |ui| {
                        if ui
                            .add_enabled(
                                count < MAX_STEPS,
                                egui::Button::new(tr(Text::OperationDuplicate)),
                            )
                            .clicked()
                        {
                            action = Some((index, Action::Duplicate));
                            ui.close();
                        }
                        if ui.button(tr(Text::OperationDelete)).clicked() {
                            action = Some((index, Action::Delete));
                            ui.close();
                        }
                    });
                });
            });
        });
    }
    if let Some((index, kind)) = action {
        match kind {
            Action::Up => steps.swap(index, index - 1),
            Action::Down => steps.swap(index, index + 1),
            Action::Duplicate => {
                if steps.len() < MAX_STEPS
                    && let Some(id) = next_step_id(steps)
                {
                    let mut copy = steps[index].clone();
                    copy.id = id;
                    steps.insert(index + 1, copy);
                    ui.ctx()
                        .data_mut(|data| data.insert_temp(opening_step_id(), id));
                }
            }
            Action::Delete => {
                clear_widgets(ui.ctx(), &steps[index..=index]);
                steps.remove(index);
            }
        }
        changed = true;
    }
    if !validate_chain(ui.ctx(), steps, changed).valid {
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
        Operation::Script { .. } => 3,
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
        assert_eq!(operations.len(), 26);
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
        assert_eq!(
            operations
                .iter()
                .filter(|op| operation_group(op) == 3)
                .count(),
            1
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
        assert!(
            matches!(&operations[25], Operation::Script { source, arguments } if source == "fn transform(node) { node.name = \"Hub \" + node.name; node }" && arguments.is_empty())
        );
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
            "script",
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
            "script",
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
        assert_eq!(validate_chain(&ctx, &steps, false).first_invalid, Some(1));
        steps[0].enabled = false;
        assert!(!validate_chain(&ctx, &steps, false).valid);
        let updated = validate_chain(&ctx, &steps, true);
        assert!(updated.valid);
        assert_eq!(updated.first_invalid, None);
        steps[0].enabled = true;
        clear_widgets(&ctx, &[]);
        assert_eq!(validate_chain(&ctx, &steps, false).first_invalid, Some(1));
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

    fn open_for_next_frame(ctx: &egui::Context, id: u64) {
        ctx.data_mut(|data| data.insert_temp(opening_step_id(), id));
    }

    fn click_label(
        ctx: &egui::Context,
        steps: &mut Vec<Step>,
        output: egui::FullOutput,
        label: &str,
    ) -> egui::FullOutput {
        let bounds = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find_map(|(_, node)| {
                (node.label() == Some(label))
                    .then(|| node.bounds())
                    .flatten()
            })
            .unwrap_or_else(|| panic!("missing {label}"));
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
            ctx,
            steps,
            vec![egui::Event::PointerMoved(pos), event(true)],
        )
        .drop_without_applying_deltas();
        draw(
            ctx,
            steps,
            vec![egui::Event::PointerMoved(pos), event(false)],
        )
    }

    #[test]
    fn script_syntax_status_is_local_to_the_step_and_clears_on_edit() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut steps = vec![Step {
            id: 1,
            enabled: true,
            operation: Operation::Script {
                source: "fn helper(node) { node }".into(),
                arguments: Default::default(),
            },
        }];
        open_for_next_frame(&ctx, 1);
        let output = draw(&ctx, &mut steps, vec![]);
        let output = click_label(
            &ctx,
            &mut steps,
            output,
            text(Language::En, Text::OperationScriptCheck),
        );
        output.drop_without_applying_deltas();
        let field = field_id(1, "script");
        let status = |data: &egui::util::IdTypeMap| {
            data.get_temp::<Result<(), SyntaxError>>(field.with("syntax"))
        };
        assert_eq!(ctx.data(status), Some(Err(SyntaxError::EntryPoint)));
        ctx.memory_mut(|memory| memory.request_focus(field));
        draw(&ctx, &mut steps, vec![egui::Event::Text(" ".into())]).drop_without_applying_deltas();
        assert_eq!(ctx.data(status), None);
    }

    #[test]
    fn batch_example_inserts_into_empty_scripts_and_copies_without_replacing_edits() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut steps = vec![Step {
            id: 1,
            enabled: true,
            operation: Operation::Script {
                source: String::new(),
                arguments: Default::default(),
            },
        }];
        open_for_next_frame(&ctx, 1);
        let output = draw(&ctx, &mut steps, vec![]);
        let inserted = click_label(
            &ctx,
            &mut steps,
            output,
            text(Language::En, Text::OperationScriptInsertBatchExample),
        );
        assert!(
            !inserted
                .platform_output
                .commands
                .iter()
                .any(|command| { matches!(command, egui::OutputCommand::CopyText(_)) })
        );
        inserted.drop_without_applying_deltas();
        let Operation::Script { source, .. } = &mut steps[0].operation else {
            panic!("expected script operation");
        };
        assert_eq!(source.as_str(), BATCH_SCRIPT_EXAMPLE);
        assert_eq!(script::check_syntax(source), Ok(()));

        let original = "fn transform(node) { node }";
        *source = original.into();
        let output = draw(&ctx, &mut steps, vec![]);
        let copied = click_label(
            &ctx,
            &mut steps,
            output,
            text(Language::En, Text::OperationScriptCopyBatchExample),
        );
        assert!(copied.platform_output.commands.iter().any(|command| {
            matches!(command, egui::OutputCommand::CopyText(value) if value == BATCH_SCRIPT_EXAMPLE)
        }));
        copied.drop_without_applying_deltas();
        assert!(
            matches!(&steps[0].operation, Operation::Script { source, .. } if source.as_str() == original)
        );
    }

    #[test]
    fn script_argument_form_preserves_invalid_drafts_and_adds_only_valid_text() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut steps = vec![Step {
            id: 1,
            enabled: true,
            operation: Operation::Script {
                source: "fn transform(node) { node }".into(),
                arguments: BTreeMap::from([("prefix".into(), "original".into())]).into(),
            },
        }];
        open_for_next_frame(&ctx, 1);
        let id = script_arguments_id(1);
        let add = text(Language::En, Text::OperationScriptArgumentAdd);
        let draft = |name: &str, value: &str| ScriptArgumentDraft {
            name: Zeroizing::new(name.into()),
            value: Zeroizing::new(value.into()),
        };
        for (name, value, error) in [
            (
                "prefix",
                "duplicate".to_owned(),
                Text::OperationScriptArgumentDuplicate,
            ),
            (
                "new_value",
                "雪".repeat(86),
                Text::OperationScriptArgumentValueInvalid,
            ),
        ] {
            ctx.data_mut(|data| data.insert_temp(id, draft(name, &value)));
            let output = draw(&ctx, &mut steps, vec![]);
            let update = output.platform_output.accesskit_update.as_ref().unwrap();
            assert!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some(add) && node.is_disabled())
            );
            assert!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label().or_else(|| node.value())
                        == Some(text(Language::En, error)))
            );
            output.drop_without_applying_deltas();
            let retained = ctx
                .data(|data| data.get_temp::<ScriptArgumentDraft>(id))
                .unwrap();
            assert_eq!(retained.name.as_str(), name);
            assert_eq!(retained.value.as_str(), value);
        }

        ctx.data_mut(|data| data.insert_temp(id, draft("new_value", "")));
        let output = draw(&ctx, &mut steps, vec![]);
        click_label(&ctx, &mut steps, output, add).drop_without_applying_deltas();
        let Operation::Script { arguments, .. } = &steps[0].operation else {
            panic!("expected script operation");
        };
        assert_eq!(
            arguments.get("prefix").map(String::as_str),
            Some("original")
        );
        assert_eq!(arguments.get("new_value").map(String::as_str), Some(""));
        assert!(
            ctx.data(|data| data.get_temp::<ScriptArgumentDraft>(id))
                .is_none()
        );

        ctx.data_mut(|data| data.insert_temp(id, draft("not_added", "pending")));
        clear_widgets(&ctx, &steps);
        assert!(
            ctx.data(|data| data.get_temp::<ScriptArgumentDraft>(id))
                .is_none()
        );
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
        ops.push(Operation::SetProperty {
            setting: Setting::UdpOverTcp { enabled: true },
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
            open_for_next_frame(&ctx, steps[0].id);
            let output = draw(&ctx, &mut steps, vec![]);
            let update = output
                .platform_output
                .accesskit_update
                .as_ref()
                .expect("accessible controls");
            let expected = match &steps[0].operation {
                Operation::ConditionFilter { .. } => "Add condition",
                Operation::NormalizeName { .. } => "Letter case",
                Operation::UniqueNames { .. } => "Separator",
                Operation::RegionNames { .. } => "Add region",
                Operation::Shuffle { .. } => "Seed",
                Operation::BatchRename { .. } => "Add rule",
                Operation::Slice { .. } => "Skip nodes",
                Operation::DeduplicateName { .. } => "Keep occurrence",
                Operation::NetworkFilter { .. } => "CIDR networks, comma separated (maximum 64)",
                Operation::Script { .. } => "Rhai script",
                Operation::Sort { .. } => "Natural name",
                Operation::SetProperty {
                    setting: Setting::Server { .. },
                } => "Server address",
                Operation::SetProperty {
                    setting: Setting::Port { .. },
                } => "Server port",
                Operation::SetProperty {
                    setting: Setting::UdpOverTcp { .. },
                } => text(Language::En, Text::PropertyUdpOverTcpHelp),
                _ => panic!("unexpected operation in UI fixture"),
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
            open_for_next_frame(&ctx, 1);
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
            operation: Operation::Sort {
                key: SortKey::Name,
                descending: false,
            },
        }];
        let output = draw(&ctx, &mut steps, vec![]);
        let update = output
            .platform_output
            .accesskit_update
            .as_ref()
            .expect("accessibility update");
        for label in ["1. Sort", "Enabled", "Move up", "Move down", "Step actions"] {
            assert!(
                update
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some(label)),
                "missing {label}"
            );
        }
        assert!(
            !update
                .nodes
                .iter()
                .any(|(_, node)| node.label() == Some("Delete")),
            "destructive action should not be next to everyday controls"
        );
        assert!(!steps[0].enabled);
        output.drop_without_applying_deltas();
    }

    #[test]
    fn quick_add_opens_its_settings_and_move_preserves_step_identity() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        theme::apply(&ctx, &Preferences::default());
        let mut steps = vec![Step {
            id: 9,
            enabled: false,
            operation: Operation::Deduplicate,
        }];
        let output = draw(&ctx, &mut steps, vec![]);
        let output = click_label(&ctx, &mut steps, output, "Name filter");
        assert_eq!(steps.len(), 2);
        assert!(steps[1].enabled);
        let new_id = steps[1].id;
        assert!(
            output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.value() == Some("Pattern"))
        );
        click_label(&ctx, &mut steps, output, "Move down").drop_without_applying_deltas();
        assert_eq!((steps[0].id, steps[1].id), (new_id, 9));
        assert!(steps[0].enabled && !steps[1].enabled);
    }

    #[test]
    fn operation_search_accepts_localized_and_english_labels() {
        assert!(operation_matches(
            Text::OperationScript,
            Language::ZhCn,
            "脚本"
        ));
        assert!(operation_matches(
            Text::OperationScript,
            Language::Ja,
            "スクリプト rhai"
        ));
        assert!(operation_matches(
            Text::OperationScript,
            Language::ZhCn,
            "script transform"
        ));
        assert!(!operation_matches(
            Text::OperationNameFilter,
            Language::En,
            "rhai"
        ));
    }

    #[test]
    fn searchable_operation_menu_reveals_and_adds_custom_script() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        theme::apply(&ctx, &Preferences::default());
        let mut steps = vec![];
        let add = text(Language::En, Text::OperationAdd);
        let output = draw(&ctx, &mut steps, vec![]);
        click_label(&ctx, &mut steps, output, add).drop_without_applying_deltas();
        draw(&ctx, &mut steps, vec![]).drop_without_applying_deltas();
        assert!(ctx.memory(|memory| memory.has_focus(search_id())));

        let output = draw(
            &ctx,
            &mut steps,
            vec![egui::Event::Text("unmatched".into())],
        );
        let label = text(Language::En, Text::OperationSearchNoResults);
        assert!(
            output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.label().or_else(|| node.value()) == Some(label))
        );
        output.drop_without_applying_deltas();

        egui::Popup::close_all(&ctx);
        let output = draw(&ctx, &mut steps, vec![]);
        assert!(ctx.data(|data| data.get_temp::<Zeroizing<String>>(search_id()).is_none()));
        click_label(&ctx, &mut steps, output, add).drop_without_applying_deltas();
        draw(&ctx, &mut steps, vec![]).drop_without_applying_deltas();
        draw(&ctx, &mut steps, vec![egui::Event::Text("RHAI".into())])
            .drop_without_applying_deltas();
        let output = draw(&ctx, &mut steps, vec![]);
        assert!(
            output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.label().or_else(|| node.value())
                    == Some(text(Language::En, Text::OperationGroupCustom)))
        );
        click_label(
            &ctx,
            &mut steps,
            output,
            text(Language::En, Text::OperationScript),
        )
        .drop_without_applying_deltas();
        assert!(matches!(
            steps.as_slice(),
            [Step {
                operation: Operation::Script { .. },
                ..
            }]
        ));
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
            open_for_next_frame(&ctx, 1);
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
            open_for_next_frame(&ctx, 1);
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
