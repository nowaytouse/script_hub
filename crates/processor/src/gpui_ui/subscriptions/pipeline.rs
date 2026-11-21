use super::super::{Palette, button, input::TextInput, localized, panel};
use super::view::{description, field};
use super::{documents_directory, read_limited};
use crate::desktop::i18n::{Text, text};
use crate::desktop::operations::{
    MAX_STEPS, new_operations, next_step_id, operation_group, operation_label,
};
use crate::relay::{
    model::{
        pipeline::{self, Operation, RegexSortUnmatched, SortKey, Step},
        script::{SyntaxError, check_syntax},
    },
    plan,
};
use crate::runtime::settings::Language;
use gpui::{
    App, AppContext as _, Context, Div, ElementId, Entity, EventEmitter, FontWeight,
    PathPromptOptions, Stateful, Window, div, prelude::*,
};
use zeroize::Zeroizing;

const MAX_PLAN_BYTES: usize = 1024 * 1024;

pub(super) struct PipelineChanged;

struct SyntaxFeedback {
    checked_json: Zeroizing<String>,
    result: Option<Result<(), SyntaxError>>,
}

impl SyntaxFeedback {
    fn message(&self, language: Language) -> String {
        let tr = |key| text(language, key);
        match self.result {
            None => localized(
                language,
                "Enter valid JSON parameters for a script step.",
                "请输入有效的脚本步骤 JSON 参数。",
                "スクリプトステップの有効な JSON パラメーターを入力してください。",
            )
            .to_owned(),
            Some(Ok(())) => tr(Text::OperationScriptSyntaxOk).to_owned(),
            Some(Err(SyntaxError::Size)) => tr(Text::OperationScriptSize).to_owned(),
            Some(Err(SyntaxError::EntryPoint)) => tr(Text::OperationScriptEntry).to_owned(),
            Some(Err(SyntaxError::Parse { line, column })) => {
                let message = tr(Text::OperationScriptSyntaxError);
                match (line, column) {
                    (Some(line), Some(column)) => format!("{message} ({line}:{column})."),
                    (Some(line), None) => format!("{message} ({line})."),
                    _ => format!("{message}."),
                }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Forms {
    NameFilter {
        keep: bool,
    },
    MemberNameFilter {
        keep: bool,
    },
    HostFilter {
        keep: bool,
    },
    ProtocolFilter {
        keep: bool,
    },
    RegionFilter {
        keep: bool,
        keep_unknown: bool,
    },
    ReplaceName {
        regex: bool,
    },
    FormatName,
    Sort {
        key: SortKey,
        descending: bool,
    },
    RegexSort {
        unmatched: RegexSortUnmatched,
        member: bool,
    },
    Limit,
    Slice,
}

impl Forms {
    fn fields(operation: &Operation) -> Option<(Self, String, String)> {
        let empty = String::new;
        Some(match operation {
            Operation::NameFilter { pattern, keep } => {
                (Self::NameFilter { keep: *keep }, pattern.clone(), empty())
            }
            Operation::MemberNameFilter { pattern, keep } => (
                Self::MemberNameFilter { keep: *keep },
                pattern.clone(),
                empty(),
            ),
            Operation::HostFilter { pattern, keep } => {
                (Self::HostFilter { keep: *keep }, pattern.clone(), empty())
            }
            Operation::ProtocolFilter { protocols, keep } => (
                Self::ProtocolFilter { keep: *keep },
                Self::list_value(protocols)?,
                empty(),
            ),
            Operation::RegionFilter {
                regions,
                keep,
                keep_unknown,
            } => (
                Self::RegionFilter {
                    keep: *keep,
                    keep_unknown: *keep_unknown,
                },
                Self::list_value(regions)?,
                empty(),
            ),
            Operation::ReplaceName {
                pattern,
                replacement,
                regex,
            } => (
                Self::ReplaceName { regex: *regex },
                pattern.clone(),
                replacement.clone(),
            ),
            Operation::FormatName { template } => (Self::FormatName, template.clone(), empty()),
            Operation::Sort { key, descending } => (
                Self::Sort {
                    key: *key,
                    descending: *descending,
                },
                empty(),
                empty(),
            ),
            Operation::RegexSort {
                patterns,
                unmatched,
            }
            | Operation::MemberRegexSort {
                patterns,
                unmatched,
            } => (
                Self::RegexSort {
                    unmatched: *unmatched,
                    member: matches!(operation, Operation::MemberRegexSort { .. }),
                },
                Self::list_value(patterns)?,
                empty(),
            ),
            Operation::Limit { count } => (Self::Limit, count.to_string(), empty()),
            Operation::Slice { skip, count } => (Self::Slice, skip.to_string(), count.to_string()),
            _ => return None,
        })
    }

    fn list_value(entries: &[String]) -> Option<String> {
        // Disabled steps may contain empty entries. Keep these editable losslessly in JSON.
        entries
            .iter()
            .all(|entry| !entry.is_empty() && !entry.contains(['\r', '\n']))
            .then(|| entries.join("\n"))
    }

    fn uses_list(self) -> bool {
        matches!(
            self,
            Self::ProtocolFilter { .. } | Self::RegionFilter { .. } | Self::RegexSort { .. }
        )
    }

    fn operation(self, primary: &str, secondary: &str) -> Option<Operation> {
        let entries = || primary.lines().map(str::to_owned).collect();
        Some(match self {
            Self::NameFilter { keep } => Operation::NameFilter {
                pattern: primary.into(),
                keep,
            },
            Self::MemberNameFilter { keep } => Operation::MemberNameFilter {
                pattern: primary.into(),
                keep,
            },
            Self::HostFilter { keep } => Operation::HostFilter {
                pattern: primary.into(),
                keep,
            },
            Self::ProtocolFilter { keep } => Operation::ProtocolFilter {
                protocols: entries(),
                keep,
            },
            Self::RegionFilter { keep, keep_unknown } => Operation::RegionFilter {
                regions: entries(),
                keep,
                keep_unknown,
            },
            Self::ReplaceName { regex } => Operation::ReplaceName {
                pattern: primary.into(),
                replacement: secondary.into(),
                regex,
            },
            Self::FormatName => Operation::FormatName {
                template: primary.into(),
            },
            Self::Sort { key, descending } => Operation::Sort { key, descending },
            Self::RegexSort {
                unmatched,
                member: false,
            } => Operation::RegexSort {
                patterns: entries(),
                unmatched,
            },
            Self::RegexSort {
                unmatched,
                member: true,
            } => Operation::MemberRegexSort {
                patterns: entries(),
                unmatched,
            },
            Self::Limit => Operation::Limit {
                count: primary.trim().parse().ok()?,
            },
            Self::Slice => Operation::Slice {
                skip: primary.trim().parse().ok()?,
                count: secondary.trim().parse().ok()?,
            },
        })
    }
}

pub(super) struct PipelineEditor {
    language: Language,
    steps: Vec<Step>,
    parameters: Entity<TextInput>,
    primary_text: Entity<TextInput>,
    secondary_text: Entity<TextInput>,
    list_text: Entity<TextInput>,
    form: Option<Forms>,
    editing: Option<u64>,
    picking: bool,
    busy: bool,
    pending_file: bool,
    replacement: Option<Vec<Step>>,
    export_requested: bool,
    status: Option<&'static str>,
    syntax_feedback: Option<SyntaxFeedback>,
}

impl EventEmitter<PipelineChanged> for PipelineEditor {}

impl PipelineEditor {
    pub(super) fn new(cx: &mut Context<Self>) -> Self {
        let parameters = cx.new(|cx| TextInput::multiline("{}", 7, cx));
        cx.observe(&parameters, |editor, parameters, cx| {
            if editor.syntax_feedback.as_ref().is_some_and(|feedback| {
                feedback.checked_json.as_str() != parameters.read(cx).value()
            }) {
                editor.syntax_feedback = None;
                cx.notify();
            }
        })
        .detach();
        Self {
            language: Language::En,
            steps: Vec::new(),
            parameters,
            primary_text: cx.new(|cx| TextInput::new("", false, cx)),
            secondary_text: cx.new(|cx| TextInput::new("", false, cx)),
            list_text: cx.new(|cx| TextInput::multiline("", 4, cx)),
            form: None,
            editing: None,
            picking: false,
            busy: false,
            pending_file: false,
            replacement: None,
            export_requested: false,
            status: None,
            syntax_feedback: None,
        }
    }

    pub(super) fn replace(&mut self, steps: Vec<Step>, cx: &mut Context<Self>) {
        self.steps = steps;
        self.hide_parameters(cx);
        self.picking = false;
        self.replacement = None;
        self.export_requested = false;
        self.status = None;
        self.pending_file = false;
        cx.notify();
    }

    pub(super) fn steps(&self) -> &[Step] {
        &self.steps
    }

    pub(super) fn can_submit(&self) -> bool {
        !self.pending_file && self.replacement.is_none() && self.editing.is_none()
    }

    pub(super) fn selecting_file(&self) -> bool {
        self.pending_file
    }

    pub(super) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        if self.language != language {
            self.language = language;
            cx.notify();
        }
    }

    pub(super) fn set_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        if self.busy != busy {
            self.busy = busy;
            cx.notify();
        }
    }

    fn tr(&self, en: &'static str, zh: &'static str, ja: &'static str) -> &'static str {
        localized(self.language, en, zh, ja)
    }

    fn change(&mut self, cx: &mut Context<Self>) {
        self.status = None;
        self.export_requested = false;
        cx.emit(PipelineChanged);
        cx.notify();
    }

    fn toggle(&mut self, index: usize, cx: &mut Context<Self>) {
        let step = &mut self.steps[index];
        step.enabled = !step.enabled;
        if pipeline::validate(&self.steps).is_err() {
            self.steps[index].enabled = false;
            self.status = Some(self.tr(
                "Check this step's parameters before enabling it.",
                "启用前请检查此步骤的参数。",
                "有効にする前に、このステップのパラメーターを確認してください。",
            ));
            cx.notify();
            return;
        }
        self.change(cx);
    }

    fn add(&mut self, operation: Operation, cx: &mut Context<Self>) {
        let Some(id) = next_step_id(&self.steps).filter(|_| self.steps.len() < MAX_STEPS) else {
            return;
        };
        self.steps.push(Step {
            id,
            enabled: false,
            operation,
        });
        self.picking = false;
        self.change(cx);
        if !matches!(
            self.steps.last().map(|step| &step.operation),
            Some(Operation::Reverse | Operation::Deduplicate)
        ) {
            self.edit(self.steps.len() - 1, cx);
        }
    }

    fn edit(&mut self, index: usize, cx: &mut Context<Self>) {
        let step = &self.steps[index];
        if let Some(form) = self.populate_form(&step.operation, cx) {
            self.form = Some(form);
        } else if let Ok(json) = serde_json::to_string_pretty(&step.operation) {
            self.parameters
                .update(cx, |input, cx| input.set_value(json, cx));
        } else {
            return;
        }
        self.editing = Some(step.id);
        self.picking = false;
        cx.notify();
    }

    fn populate_form(&self, operation: &Operation, cx: &mut Context<Self>) -> Option<Forms> {
        let (form, primary, secondary) = Forms::fields(operation)?;
        self.form_input(form)
            .update(cx, |input, cx| input.set_value(primary, cx));
        self.secondary_text
            .update(cx, |input, cx| input.set_value(secondary, cx));
        Some(form)
    }

    fn form_input(&self, form: Forms) -> &Entity<TextInput> {
        if form.uses_list() {
            &self.list_text
        } else {
            &self.primary_text
        }
    }

    fn draft_operation(&self, cx: &App) -> Option<Operation> {
        let Some(form) = self.form else {
            let draft = self.parameters.read(cx).value();
            return (draft.len() <= MAX_PLAN_BYTES)
                .then(|| serde_json::from_str::<Operation>(draft).ok())
                .flatten();
        };
        form.operation(
            self.form_input(form).read(cx).value(),
            self.secondary_text.read(cx).value(),
        )
    }

    fn show_json(&mut self, cx: &mut Context<Self>) {
        let Some(operation) = self.draft_operation(cx) else {
            self.status = Some(text(self.language, Text::OperationInvalid));
            cx.notify();
            return;
        };
        if let Ok(json) = serde_json::to_string_pretty(&operation) {
            self.parameters
                .update(cx, |input, cx| input.set_value(json, cx));
            self.form = None;
            self.clear_form(cx);
            self.status = None;
            cx.notify();
        }
    }

    fn show_fields(&mut self, cx: &mut Context<Self>) {
        if let Some(operation) = self.draft_operation(cx)
            && let Some(form) = self.populate_form(&operation, cx)
        {
            self.form = Some(form);
            self.parameters.update(cx, |input, cx| input.clear(cx));
            self.status = None;
        } else {
            self.status = Some(self.tr(
                "Check the JSON and whether these parameters can be edited in fields.",
                "请检查 JSON，并确认这些参数支持字段编辑。",
                "JSON を確認し、入力欄で編集できるパラメーターか確認してください。",
            ));
        }
        cx.notify();
    }

    fn hide_parameters(&mut self, cx: &mut Context<Self>) {
        self.editing = None;
        self.form = None;
        self.syntax_feedback = None;
        self.parameters.update(cx, |input, cx| input.clear(cx));
        self.clear_form(cx);
    }

    fn clear_form(&self, cx: &mut Context<Self>) {
        self.primary_text.update(cx, |input, cx| input.clear(cx));
        self.secondary_text.update(cx, |input, cx| input.clear(cx));
        self.list_text.update(cx, |input, cx| input.clear(cx));
    }

    fn check_script(&mut self, cx: &mut Context<Self>) {
        let draft = self.draft_operation(cx);
        self.status = None;
        self.syntax_feedback = Some(SyntaxFeedback {
            checked_json: Zeroizing::new(self.parameters.read(cx).value().to_owned()),
            result: match draft.as_ref() {
                Some(Operation::Script { source, .. }) => Some(check_syntax(source)),
                _ => None,
            },
        });
        cx.notify();
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self
            .steps
            .iter()
            .position(|step| Some(step.id) == self.editing)
        else {
            return;
        };
        if let Some(operation) = self.draft_operation(cx) {
            let previous = std::mem::replace(&mut self.steps[index].operation, operation);
            if pipeline::validate(&self.steps).is_ok() {
                self.hide_parameters(cx);
                self.change(cx);
                return;
            }
            self.steps[index].operation = previous;
        }
        self.status = Some(if self.form.is_some() {
            text(self.language, Text::OperationInvalid)
        } else {
            self.tr(
                "The parameters are invalid. Check the JSON and the operation's required values.",
                "参数无效。请检查 JSON 格式和此步骤的必填值。",
                "パラメーターが無効です。JSON とステップの必須項目を確認してください。",
            )
        });
        cx.notify();
    }

    fn import(&mut self, cx: &mut Context<Self>) {
        self.pending_file = true;
        self.export_requested = false;
        self.status = None;
        let pick = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                self.tr(
                    "Import operation plan",
                    "导入操作计划",
                    "操作プランを読み込む",
                )
                .into(),
            ),
        });
        let executor = cx.background_executor().clone();
        cx.spawn(async move |weak, cx| {
            let result = match pick.await {
                Ok(Ok(Some(files))) => {
                    executor
                        .spawn(async move {
                            let path = files.first().ok_or(())?;
                            let bytes = read_limited(path, MAX_PLAN_BYTES).map_err(|_| ())?;
                            plan::decode(&bytes).map(Some).map_err(|_| ())
                        })
                        .await
                }
                Ok(Ok(None)) => Ok(None),
                _ => Err(()),
            };
            let _ = weak.update(cx, |this, cx| {
                this.pending_file = false;
                match result {
                    Ok(steps) => this.replacement = steps,
                    Err(()) => {
                        this.status = Some(this.tr(
                            "Could not read a valid operation plan from this file.",
                            "无法从此文件读取有效的操作计划。",
                            "このファイルから有効な操作プランを読み込めませんでした。",
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn export(&mut self, cx: &mut Context<Self>) {
        self.export_requested = false;
        let Ok(bytes) = plan::encode(&self.steps) else {
            self.status = Some(self.tr(
                "The plan is invalid or exceeds the export size limit.",
                "操作计划无效或超出导出大小限制。",
                "プランが無効か、書き出しサイズの上限を超えています。",
            ));
            cx.notify();
            return;
        };
        self.pending_file = true;
        self.status = None;
        let prompt =
            cx.prompt_for_new_path(&documents_directory(), Some("scripthub-operations.json"));
        let executor = cx.background_executor().clone();
        cx.spawn(async move |weak, cx| {
            let result = match prompt.await {
                Ok(Ok(Some(path))) => executor.spawn(async move {
                    if !crate::desktop::template_paths::safe_export_destination(&path, None) {
                        return Err(());
                    }
                    crate::vault::write_private(&path, &bytes, false).map(|_| Some(())).map_err(|_| ())
                }).await,
                Ok(Ok(None)) => Ok(None),
                _ => Err(()),
            };
            let _ = weak.update(cx, |this, cx| {
                this.pending_file = false;
                this.status = match result {
                    Ok(Some(())) => Some(this.tr("Operation plan exported.", "操作计划已导出。", "操作プランを書き出しました。")),
                    Ok(None) => None,
                    Err(()) => Some(this.tr(
                        "Could not create this file. Choose a new file outside Git and vault folders.",
                        "无法创建此文件。请选择 Git 仓库与保险库目录之外的新文件。",
                        "ファイルを作成できませんでした。Git または保管庫の外にある新しいファイルを選んでください。",
                    )),
                };
                cx.notify();
            });
        }).detach();
        cx.notify();
    }

    fn action(
        &self,
        id: impl Into<ElementId>,
        title: &'static str,
        palette: Palette,
        active: bool,
        cx: &Context<Self>,
        action: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        button(id, title, palette, active).on_click(cx.listener(move |this, _, _, cx| {
            if !this.busy && !this.pending_file {
                action(this, cx);
            }
        }))
    }

    fn form_fields(&self, form: Forms, palette: Palette, cx: &Context<Self>) -> Div {
        let fields = div().flex().flex_col().gap_2();
        let primary = |label| field(label, self.form_input(form), palette);
        let secondary = |label| field(label, &self.secondary_text, palette);
        let pattern = || primary(text(self.language, Text::OperationPattern));
        let help = |label| description(label, palette);
        let filter = |choices: [Forms; 2]| {
            self.form_choices(
                "pipeline-filter-mode",
                form,
                choices.into_iter().zip([
                    self.tr("Exclude matches", "排除匹配项", "一致する項目を除外"),
                    self.tr("Keep matches", "保留匹配项", "一致する項目を保持"),
                ]),
                palette,
                cx,
            )
        };
        let directions = [
            self.tr("Ascending", "升序", "昇順"),
            self.tr("Descending", "降序", "降順"),
        ];
        let count = self.tr("Target node count", "目标节点数", "目標ノード数");
        match form {
            Forms::NameFilter { .. } => fields
                .child(pattern())
                .child(filter([false, true].map(|keep| Forms::NameFilter { keep }))),
            Forms::MemberNameFilter { .. } => fields.child(pattern()).child(filter(
                [false, true].map(|keep| Forms::MemberNameFilter { keep }),
            )),
            Forms::HostFilter { .. } => fields
                .child(primary(text(self.language, Text::OperationHostPattern)))
                .child(filter([false, true].map(|keep| Forms::HostFilter { keep }))),
            Forms::ProtocolFilter { .. } => fields
                .child(primary(self.tr("Protocols", "协议", "プロトコル")))
                .child(help(self.tr(
                    "Enter one protocol per line, such as ss, vless, or hysteria2.",
                    "每行填写一个协议，例如 ss、vless 或 hysteria2。",
                    "1 行に 1 つのプロトコルを入力します。例: ss、vless、hysteria2。",
                )))
                .child(filter([false, true].map(|keep| Forms::ProtocolFilter { keep }))),
            Forms::RegionFilter { keep, keep_unknown } => fields
                .child(primary(self.tr("Region codes", "地区代码", "地域コード")))
                .child(help(self.tr(
                    "Matches regions identified from node names. Enter one two-letter code per line, such as JP or US.",
                    "根据节点名称识别地区。每行填写一个两字母地区代码，例如 JP 或 US。",
                    "ノード名から地域を判定します。1 行に 1 つの 2 文字コードを入力します。例: JP、US。",
                )))
                .child(filter([false, true].map(|keep| Forms::RegionFilter { keep, keep_unknown })))
                .child(self.form_choices(
                    "pipeline-region-unknown",
                    form,
                    [false, true].map(|keep_unknown| Forms::RegionFilter { keep, keep_unknown }).into_iter().zip([
                        self.tr("Exclude unknown region", "排除未知地区", "地域不明を除外"),
                        self.tr("Keep unknown region", "保留未知地区", "地域不明を保持"),
                    ]),
                    palette,
                    cx,
                )),
            Forms::FormatName => fields
                .child(primary(text(self.language, Text::OperationTemplate)))
                .child(help(
                    text(self.language, Text::OperationTemplateHelp),
                )),
            Forms::ReplaceName { .. } => fields
                .child(pattern())
                .child(secondary(text(self.language, Text::OperationReplacement)))
                .child(self.form_choices(
                    "pipeline-replacement-mode",
                    form,
                    [false, true].map(|regex| Forms::ReplaceName { regex }).into_iter().zip([
                        self.tr("Literal text", "普通文本", "リテラル文字列"),
                        text(self.language, Text::OperationUseRegex),
                    ]),
                    palette,
                    cx,
                )),
            Forms::Sort { key, descending } => fields
                .child(help(self.tr("Sort by", "排序字段", "並べ替えの項目")))
                .child(self.form_choices(
                    "pipeline-sort-key",
                    form,
                    [
                        (SortKey::Name, self.tr("Node name", "节点名称", "ノード名")),
                        (SortKey::NameUtf16, self.tr("Node name (UTF-16)", "节点名称（UTF-16）", "ノード名（UTF-16）")),
                        (SortKey::MemberNameUtf16, self.tr("Collection member (UTF-16)", "集合成员名称（UTF-16）", "コレクションのメンバー名（UTF-16）")),
                        (SortKey::NaturalName, self.tr("Natural name order", "名称自然排序", "名前の自然順")),
                        (SortKey::Protocol, self.tr("Protocol", "协议", "プロトコル")),
                        (SortKey::Region, self.tr("Region", "地区", "地域")),
                        (SortKey::Server, self.tr("Server", "服务器", "サーバー")),
                        (SortKey::Port, self.tr("Port", "端口", "ポート")),
                    ].map(|(key, label)| (Forms::Sort { key, descending }, label)),
                    palette,
                    cx,
                ))
                .child(self.form_choices(
                    "pipeline-sort-direction",
                    form,
                    [false, true].map(|descending| Forms::Sort { key, descending }).into_iter().zip(directions),
                    palette,
                    cx,
                )),
            Forms::RegexSort { member, .. } => fields
                .child(primary(self.tr("Regex patterns", "正则表达式", "正規表現")))
                .child(help(self.tr(
                    "Enter one pattern per line. Earlier matches are placed first.",
                    "每行填写一个正则表达式，匹配靠前表达式的节点优先排列。",
                    "1 行に 1 つ入力します。上の正規表現に一致するノードが先になります。",
                )))
                .child(help(self.tr("Order for unmatched nodes", "未匹配节点的顺序", "一致しないノードの順序")))
                .child(self.form_choices(
                    "pipeline-regex-unmatched",
                    form,
                    [RegexSortUnmatched::Original, RegexSortUnmatched::Ascending, RegexSortUnmatched::Descending]
                        .map(|unmatched| Forms::RegexSort { unmatched, member })
                        .into_iter()
                        .zip([
                            self.tr("Original order", "原始顺序", "元の順序"),
                            directions[0],
                            directions[1],
                        ]),
                    palette,
                    cx,
                )),
            Forms::Limit => fields.child(primary(count)),
            Forms::Slice => fields
                .child(primary(self.tr("Nodes to skip", "开头跳过的节点数", "先頭から飛ばすノード数")))
                .child(secondary(count)),
        }
    }

    fn form_choices(
        &self,
        id: &'static str,
        current: Forms,
        choices: impl IntoIterator<Item = (Forms, &'static str)>,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Div {
        row().children(
            choices
                .into_iter()
                .enumerate()
                .map(|(index, (form, label))| {
                    self.action(
                        (id, index),
                        label,
                        palette,
                        current == form,
                        cx,
                        move |this, cx| {
                            this.form = Some(form);
                            cx.notify();
                        },
                    )
                }),
        )
    }

    fn step_row(&self, index: usize, step: &Step, palette: Palette, cx: &Context<Self>) -> Div {
        let id = step.id;
        let status = if step.enabled {
            self.tr("Enabled", "已启用", "有効")
        } else {
            self.tr("Disabled", "已停用", "無効")
        };
        let unavailable = self.busy || !self.can_submit();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .bg(palette.subtle)
            .rounded_md()
            .child(
                row()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!(
                                "{}. {}",
                                index + 1,
                                text(self.language, operation_label(&step.operation)),
                            )),
                    )
                    .child(div().text_sm().text_color(palette.muted).child(status)),
            )
            .when(!unavailable, |card| {
                card.child(
                    row()
                        .child(self.action(
                            ("pipeline-toggle", id),
                            if step.enabled {
                                self.tr("Disable", "停用", "無効にする")
                            } else {
                                self.tr("Enable", "启用", "有効にする")
                            },
                            palette,
                            false,
                            cx,
                            move |this, cx| this.toggle(index, cx),
                        ))
                        .when(
                            !matches!(step.operation, Operation::Reverse | Operation::Deduplicate),
                            |row| {
                                row.child(self.action(
                                    ("pipeline-edit", id),
                                    self.tr("Edit", "编辑", "編集"),
                                    palette,
                                    false,
                                    cx,
                                    move |this, cx| this.edit(index, cx),
                                ))
                            },
                        )
                        .when(index > 0, |row| {
                            row.child(self.action(
                                ("pipeline-up", id),
                                self.tr("Move up", "上移", "上へ"),
                                palette,
                                false,
                                cx,
                                move |this, cx| {
                                    this.steps.swap(index, index - 1);
                                    this.change(cx);
                                },
                            ))
                        })
                        .when(index + 1 < self.steps.len(), |row| {
                            row.child(self.action(
                                ("pipeline-down", id),
                                self.tr("Move down", "下移", "下へ"),
                                palette,
                                false,
                                cx,
                                move |this, cx| {
                                    this.steps.swap(index, index + 1);
                                    this.change(cx);
                                },
                            ))
                        })
                        .when(self.steps.len() < MAX_STEPS, |row| {
                            row.child(self.action(
                                ("pipeline-duplicate", id),
                                self.tr("Duplicate", "复制步骤", "複製"),
                                palette,
                                false,
                                cx,
                                move |this, cx| {
                                    if let Some(id) = next_step_id(&this.steps) {
                                        let mut copy = this.steps[index].clone();
                                        copy.id = id;
                                        this.steps.insert(index + 1, copy);
                                        this.change(cx);
                                    }
                                },
                            ))
                        })
                        .child(self.action(
                            ("pipeline-remove", id),
                            self.tr("Remove", "移除", "削除"),
                            palette,
                            false,
                            cx,
                            move |this, cx| {
                                this.steps.remove(index);
                                this.change(cx);
                            },
                        )),
                )
            })
    }

    fn picker(&self, palette: Palette, cx: &Context<Self>) -> Div {
        let groups = [
            self.tr("Filters", "筛选", "フィルター"),
            self.tr("Names and ordering", "名称与排序", "名前と並び順"),
            self.tr(
                "Network and properties",
                "网络与属性",
                "ネットワークとプロパティ",
            ),
            self.tr("Scripts", "脚本", "スクリプト"),
        ];
        let operations = new_operations().into_iter().chain([
            Operation::MemberNameFilter {
                pattern: String::new(),
                keep: true,
            },
            Operation::MemberRegexSort {
                patterns: Vec::new(),
                unmatched: RegexSortUnmatched::Original,
            },
        ]);
        let mut rows = std::array::from_fn::<_, 4, _>(|_| row());
        for (index, operation) in operations.enumerate() {
            let group = usize::from(operation_group(&operation));
            let item = self.action(
                ("pipeline-kind", index),
                text(self.language, operation_label(&operation)),
                palette,
                false,
                cx,
                move |this, cx| this.add(operation.clone(), cx),
            );
            rows[group] = std::mem::replace(&mut rows[group], row()).child(item);
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .children(groups.into_iter().zip(rows).map(|(group, actions)| {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .pt_2()
                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(group))
                    .child(actions)
            }))
    }
}

impl Render for PipelineEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::for_window(window, cx);
        let mut body = panel(palette).w_full().p_4().gap_3().child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(self.tr(
                    "Processing steps",
                    "处理步骤",
                    "処理ステップ",
                )))
                .child(div().text_sm().text_color(palette.muted).child(self.tr(
                    "Enabled steps run in order during preview.",
                    "预览时按顺序运行已启用的步骤。",
                    "有効なステップはプレビュー時に上から順に実行されます。",
                ))),
        );
        body = body.children(
            self.steps
                .iter()
                .enumerate()
                .map(|(index, step)| self.step_row(index, step, palette, cx)),
        );
        if self.pending_file {
            body = body.child(div().text_sm().text_color(palette.muted).child(self.tr(
                "Waiting for the file selection…",
                "正在等待选择文件…",
                "ファイルの選択を待っています…",
            )));
        } else if let Some(step) = self.steps.iter().find(|step| Some(step.id) == self.editing) {
            let editing_script = matches!(&step.operation, Operation::Script { .. });
            let parameters = if let Some(form) = self.form {
                self.form_fields(form, palette, cx)
            } else {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(description(
                        self.tr("Parameters (JSON)", "参数（JSON）", "パラメーター（JSON）"),
                        palette,
                    ))
                    .child(self.parameters.clone())
            };
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .rounded_md()
                    .bg(palette.subtle)
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(text(self.language, operation_label(&step.operation))),
                    )
                    .child(parameters)
                    .when(editing_script, |card| {
                        card.child(description(
                            text(self.language, Text::OperationScriptHelp),
                            palette,
                        ))
                    })
                    .when_some(self.syntax_feedback.as_ref(), |card, result| {
                        card.child(description(result.message(self.language), palette))
                    })
                    .child(
                        row()
                            .child(self.action(
                                "pipeline-apply-step",
                                self.tr("Apply step", "应用步骤", "適用"),
                                palette,
                                true,
                                cx,
                                Self::apply,
                            ))
                            .child(self.action(
                                "pipeline-cancel-step",
                                self.tr("Discard parameter edit", "放弃参数编辑", "編集を破棄"),
                                palette,
                                false,
                                cx,
                                |this, cx| {
                                    this.hide_parameters(cx);
                                    this.status = None;
                                    cx.notify();
                                },
                            ))
                            .when(editing_script, |row| {
                                row.child(self.action(
                                    "pipeline-check-script",
                                    text(self.language, Text::OperationScriptCheck),
                                    palette,
                                    false,
                                    cx,
                                    Self::check_script,
                                ))
                            })
                            .when(self.form.is_some(), |row| {
                                row.child(self.action(
                                    "pipeline-advanced-json",
                                    self.tr("Advanced JSON", "高级 JSON", "詳細 JSON"),
                                    palette,
                                    false,
                                    cx,
                                    Self::show_json,
                                ))
                            })
                            .when(
                                self.form.is_none() && Forms::fields(&step.operation).is_some(),
                                |row| {
                                    row.child(self.action(
                                        "pipeline-use-fields",
                                        self.tr("Use fields", "使用字段编辑", "入力欄を使う"),
                                        palette,
                                        false,
                                        cx,
                                        Self::show_fields,
                                    ))
                                },
                            ),
                    ),
            );
        } else if let Some(steps) = self.replacement.as_ref() {
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().text_sm().child(format!(
                        "{} ({})",
                        self.tr(
                            "Replace the current steps with this plan?",
                            "用导入的计划替换当前步骤？",
                            "現在のステップをこのプランに置き換えますか？"
                        ),
                        steps.len(),
                    )))
                    .child(
                        row()
                            .child(self.action(
                                "pipeline-apply-import",
                                self.tr("Replace steps", "替换步骤", "置き換える"),
                                palette,
                                true,
                                cx,
                                |this, cx| {
                                    if let Some(steps) = this.replacement.take() {
                                        this.steps = steps;
                                        this.picking = false;
                                        this.change(cx);
                                    }
                                },
                            ))
                            .child(self.action(
                                "pipeline-cancel-import",
                                self.tr(
                                    "Keep current steps",
                                    "保留当前步骤",
                                    "現在のステップを維持",
                                ),
                                palette,
                                false,
                                cx,
                                |this, cx| {
                                    this.replacement = None;
                                    cx.notify();
                                },
                            )),
                    ),
            );
        } else if !self.busy {
            body = body.child(
                row()
                    .when(self.steps.len() < MAX_STEPS, |row| row.child(self.action(
                        "pipeline-add-step",
                        if self.picking { self.tr("Close step menu", "关闭步骤菜单", "ステップメニューを閉じる") } else { self.tr("Add step", "添加步骤", "ステップを追加") },
                        palette, self.picking, cx, |this, cx| {
                            this.picking = !this.picking;
                            this.export_requested = false;
                            cx.notify();
                        },
                    )))
                    .child(self.action("pipeline-import", self.tr("Import plan", "导入计划", "プランを読み込む"), palette, false, cx, Self::import))
                    .when(!self.steps.is_empty(), |row| row.child(self.action(
                        "pipeline-ask-export", self.tr("Export plan", "导出计划", "プランを書き出す"), palette, self.export_requested, cx, |this, cx| {
                            this.picking = false;
                            this.export_requested = !this.export_requested;
                            cx.notify();
                        },
                    ))),
            ).when(self.picking, |body| body.child(self.picker(palette, cx)))
            .when(self.export_requested, |body| body.child(
                div().flex().flex_col().gap_2()
                    .child(div().text_sm().child(self.tr(
                        "The file contains operation parameters and scripts in plain text. Check them for private data before saving.",
                        "文件会以明文保存操作参数和脚本。保存前请检查其中的私密数据。",
                        "パラメーターとスクリプトは平文で保存されます。保存前に機密情報を確認してください。",
                    )))
                    .child(row().child(self.action(
                        "pipeline-export-confirm", self.tr("Choose export file", "选择导出位置", "保存先を選択"), palette, true, cx, Self::export,
                    ))),
            ));
        }
        body.when_some(self.status, |body, status| {
            body.child(div().text_sm().text_color(palette.muted).child(status))
        })
    }
}

fn row() -> Div {
    div().flex().flex_wrap().items_center().gap_2()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_fields_preserve_operations() {
        for operation in [
            Operation::ProtocolFilter {
                protocols: vec!["ss".into(), "vless".into()],
                keep: false,
            },
            Operation::RegionFilter {
                regions: vec!["jp".into(), "US".into()],
                keep: true,
                keep_unknown: false,
            },
            Operation::Sort {
                key: SortKey::NaturalName,
                descending: true,
            },
            Operation::RegexSort {
                patterns: vec![r" edge,\d+ ".into(), r"^a{1,2}$".into()],
                unmatched: RegexSortUnmatched::Ascending,
            },
            Operation::MemberRegexSort {
                patterns: vec!["^member".into()],
                unmatched: RegexSortUnmatched::Descending,
            },
            Operation::Limit { count: 0 },
            Operation::Slice { skip: 3, count: 4 },
        ] {
            let (form, first, second) = Forms::fields(&operation).unwrap();
            let rebuilt = form.operation(&first, &second).unwrap();
            assert_eq!(
                serde_json::to_value(rebuilt).unwrap(),
                serde_json::to_value(operation).unwrap()
            );
        }
    }

    #[test]
    fn list_fields_accept_windows_line_endings_and_preserve_regex_spaces() {
        let parsed = Forms::RegexSort {
            member: false,
            unmatched: RegexSortUnmatched::Original,
        }
        .operation(" ip, \\d+ \r\n^JP$\r\n", "")
        .unwrap();
        assert!(
            matches!(parsed, Operation::RegexSort { ref patterns, .. } if patterns == &[r" ip, \d+ ", "^JP$"])
        );
    }

    #[test]
    fn ambiguous_lists_stay_in_json_and_invalid_counts_cannot_be_applied() {
        let disabled_draft = Operation::ProtocolFilter {
            protocols: vec![String::new()],
            keep: true,
        };
        assert!(Forms::fields(&disabled_draft).is_none());
        let (form, first, second) = Forms::fields(&Operation::RegexSort {
            patterns: Vec::new(),
            unmatched: RegexSortUnmatched::Original,
        })
        .unwrap();
        assert!(
            matches!(form.operation(&first, &second), Some(Operation::RegexSort { ref patterns, .. }) if patterns.is_empty())
        );
        assert!(Forms::Limit.operation("-1", "").is_none());
        assert!(Forms::Slice.operation("1", "text").is_none());
    }
}
