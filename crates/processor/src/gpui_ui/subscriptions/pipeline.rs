use super::super::{Palette, button, input::TextInput, localized, panel};
use super::view::{description, field};
use super::{documents_directory, read_limited};
use crate::desktop::i18n::{Text, text};
use crate::desktop::operations::{
    MAX_STEPS, new_operations, next_step_id, operation_group, operation_label,
};
use crate::relay::{
    model::{
        pipeline::{self, Operation, RegexSortUnmatched, Step},
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

#[derive(Clone, Copy)]
enum Forms {
    NameFilter,
    MemberNameFilter,
    HostFilter,
    ReplaceName,
    FormatName,
}

impl Forms {
    fn fields(operation: &Operation) -> Option<(Self, &str, &str, bool)> {
        Some(match operation {
            Operation::NameFilter { pattern, keep } => (Self::NameFilter, pattern, "", *keep),
            Operation::MemberNameFilter { pattern, keep } => {
                (Self::MemberNameFilter, pattern, "", *keep)
            }
            Operation::HostFilter { pattern, keep } => (Self::HostFilter, pattern, "", *keep),
            Operation::ReplaceName {
                pattern,
                replacement,
                regex,
            } => (Self::ReplaceName, pattern, replacement, *regex),
            Operation::FormatName { template } => (Self::FormatName, template, "", false),
            _ => return None,
        })
    }
}

pub(super) struct PipelineEditor {
    language: Language,
    steps: Vec<Step>,
    parameters: Entity<TextInput>,
    primary_text: Entity<TextInput>,
    secondary_text: Entity<TextInput>,
    form: Option<Forms>,
    form_option: bool,
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
            form: None,
            form_option: false,
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
        if let Some((form, option)) = self.populate_form(&step.operation, cx) {
            self.form = Some(form);
            self.form_option = option;
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

    fn populate_form(
        &self,
        operation: &Operation,
        cx: &mut Context<Self>,
    ) -> Option<(Forms, bool)> {
        let (form, primary, secondary, option) = Forms::fields(operation)?;
        self.primary_text
            .update(cx, |input, cx| input.set_value(primary, cx));
        self.secondary_text
            .update(cx, |input, cx| input.set_value(secondary, cx));
        Some((form, option))
    }

    fn draft_operation(&self, cx: &App) -> Option<Operation> {
        let Some(form) = self.form else {
            let draft = self.parameters.read(cx).value();
            return (draft.len() <= MAX_PLAN_BYTES)
                .then(|| serde_json::from_str::<Operation>(draft).ok())
                .flatten();
        };
        let primary = self.primary_text.read(cx).value().to_owned();
        let option = self.form_option;
        Some(match form {
            Forms::NameFilter => Operation::NameFilter {
                pattern: primary,
                keep: option,
            },
            Forms::MemberNameFilter => Operation::MemberNameFilter {
                pattern: primary,
                keep: option,
            },
            Forms::HostFilter => Operation::HostFilter {
                pattern: primary,
                keep: option,
            },
            Forms::ReplaceName => Operation::ReplaceName {
                pattern: primary,
                replacement: self.secondary_text.read(cx).value().to_owned(),
                regex: option,
            },
            Forms::FormatName => Operation::FormatName { template: primary },
        })
    }

    fn show_json(&mut self, cx: &mut Context<Self>) {
        let Some(operation) = self.draft_operation(cx) else {
            return;
        };
        if let Ok(json) = serde_json::to_string_pretty(&operation) {
            self.parameters
                .update(cx, |input, cx| input.set_value(json, cx));
            self.form = None;
            self.primary_text.update(cx, |input, cx| input.clear(cx));
            self.secondary_text.update(cx, |input, cx| input.clear(cx));
            self.status = None;
            cx.notify();
        }
    }

    fn show_fields(&mut self, cx: &mut Context<Self>) {
        if let Some(operation) = self.draft_operation(cx)
            && let Some((form, option)) = self.populate_form(&operation, cx)
        {
            self.form = Some(form);
            self.form_option = option;
            self.parameters.update(cx, |input, cx| input.clear(cx));
            self.status = None;
        } else {
            self.status = Some(self.tr(
                "Check the JSON and choose a supported filter or rename operation.",
                "请检查 JSON，并选择支持字段编辑的筛选或改名操作。",
                "JSON を確認し、対応するフィルターまたは名前変更を選択してください。",
            ));
        }
        cx.notify();
    }

    fn hide_parameters(&mut self, cx: &mut Context<Self>) {
        self.editing = None;
        self.form = None;
        self.syntax_feedback = None;
        self.parameters.update(cx, |input, cx| input.clear(cx));
        self.primary_text.update(cx, |input, cx| input.clear(cx));
        self.secondary_text.update(cx, |input, cx| input.clear(cx));
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
        let key = match form {
            Forms::FormatName => Text::OperationTemplate,
            Forms::HostFilter => Text::OperationHostPattern,
            _ => Text::OperationPattern,
        };
        let mut fields = div().flex().flex_col().gap_2().child(field(
            text(self.language, key),
            &self.primary_text,
            palette,
        ));

        let options = match form {
            Forms::FormatName => {
                return fields.child(description(
                    text(self.language, Text::OperationTemplateHelp),
                    palette,
                ));
            }
            Forms::ReplaceName => {
                fields = fields.child(field(
                    text(self.language, Text::OperationReplacement),
                    &self.secondary_text,
                    palette,
                ));
                [
                    self.tr("Literal text", "普通文本", "リテラル文字列"),
                    text(self.language, Text::OperationUseRegex),
                ]
            }
            Forms::NameFilter | Forms::MemberNameFilter | Forms::HostFilter => [
                self.tr("Exclude matches", "排除匹配项", "一致する項目を除外"),
                self.tr("Keep matches", "保留匹配项", "一致する項目を保持"),
            ],
        };
        fields.child(
            row().children(options.into_iter().enumerate().map(|(index, label)| {
                let selected = index == 1;
                self.action(
                    ("pipeline-form-option", index),
                    label,
                    palette,
                    self.form_option == selected,
                    cx,
                    move |this, cx| {
                        this.form_option = selected;
                        cx.notify();
                    },
                )
            })),
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
