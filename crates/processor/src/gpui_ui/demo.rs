//! An isolated native demo backed exclusively by the bundled synthetic relay fixtures.

use super::{NextControl, Palette, PreviousControl, UiStyle, button, heading, panel};
use crate::desktop::i18n::{Text, text};
use crate::relay::{
    self, RelayConfig, RelayError, RunMode,
    demo::{self, DemoReport},
    export::{OutputFormat, export_subscription},
    library::Workspace,
    model::{Subscription, script::SourceContext},
};
use crate::runtime::settings::{Language, Preferences};
use gpui::{
    App, AppContext as _, Application, Bounds, Context, FontWeight, Window, WindowBounds,
    WindowHandle, WindowOptions, div, prelude::*, px, size,
};
use std::{cell::Cell, process::ExitCode, rc::Rc, sync::atomic::AtomicBool};

pub(super) struct Demo {
    workspace: Workspace,
    language: Language,
    collection: Option<u64>,
    format: OutputFormat,
    backup_added: bool,
    busy: bool,
    preview: Option<Preview>,
    report: Option<DemoReport>,
    error: Option<Text>,
}

struct Preview {
    before: usize,
    after: usize,
    names: Vec<String>,
}

fn import_sample_backup(workspace: &Workspace) -> Result<(Workspace, u64), RelayError> {
    let backup = relay::compat::import(demo::backup()).map_err(|_| RelayError::InvalidInput)?;
    let (candidate, summary) = relay::library_import::import(workspace, &backup, true)
        .map_err(|_| RelayError::InvalidInput)?;
    if backup.unsupported_count != 0 || summary.skipped != 0 || summary.collections != 1 {
        return Err(RelayError::InvalidInput);
    }
    let id = candidate
        .collections
        .last()
        .ok_or(RelayError::InvalidInput)?
        .id;
    Ok((candidate, id))
}

fn preview(workspace: &Workspace, id: u64, format: OutputFormat) -> Result<Preview, RelayError> {
    let collection = workspace
        .collections
        .iter()
        .find(|item| item.id == id)
        .ok_or(RelayError::InvalidInput)?;
    let merged = workspace.merge(id).map_err(|_| RelayError::InvalidInput)?;
    let result = relay::run(
        &RelayConfig {
            expected_source: Some(merged.clone()),
            local_content: Some(merged),
            processing: collection.processing.clone(),
            script_source: SourceContext {
                kind: "collection",
                label: RelayConfig::script_source_label(&collection.name),
                member_index: None,
            },
            output_format: format,
            rename: false,
            sort: false,
            test_nodes: false,
            ..Default::default()
        },
        "",
        RunMode::Preview,
        &AtomicBool::new(false),
        &|_| {},
    )?;
    let output = export_subscription(&result.content, format)?;
    let parsed = Subscription::parse(&output).map_err(RelayError::Model)?;
    Ok(Preview {
        before: result.total,
        after: parsed.nodes().len(),
        names: parsed
            .nodes()
            .iter()
            .map(|node| node.name().to_owned())
            .collect(),
    })
}

impl Demo {
    fn new(workspace: Workspace, language: Language) -> Self {
        let collection = workspace.collections.first().map(|item| item.id);
        Self {
            workspace,
            language,
            collection,
            format: OutputFormat::Original,
            backup_added: false,
            busy: false,
            preview: None,
            report: None,
            error: None,
        }
    }

    fn tr(&self, key: Text) -> &'static str {
        text(self.language, key)
    }

    fn add_backup(&mut self, cx: &mut Context<Self>) {
        if self.backup_added || self.busy {
            return;
        }
        match import_sample_backup(&self.workspace) {
            Ok((workspace, id)) => {
                self.collection = Some(id);
                self.workspace = workspace;
                self.backup_added = true;
                self.preview = None;
                self.error = None;
            }
            Err(_) => self.error = Some(Text::RelayDemoFailed),
        }
        cx.notify();
    }

    fn run_preview(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.collection else {
            return;
        };
        if self.busy {
            return;
        }
        self.busy = true;
        self.preview = None;
        self.error = None;
        let workspace = self.workspace.clone();
        let format = self.format;
        let task = cx
            .background_executor()
            .spawn(async move { preview(&workspace, id, format) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(preview) => view.preview = Some(preview),
                    Err(_) => view.error = Some(Text::RelayDemoFailed),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn self_check(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.report = None;
        self.error = None;
        let task = cx.background_executor().spawn(async { demo::run() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(report) => view.report = Some(report),
                    Err(_) => view.error = Some(Text::RelayDemoFailed),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for Demo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::for_window(window, cx);
        let formats = [
            (OutputFormat::Original, self.tr(Text::RelayExportOriginal)),
            (OutputFormat::Surge, "Surge"),
            (OutputFormat::Mihomo, "Mihomo"),
            (OutputFormat::SingBox, "sing-box"),
        ];
        let row = || div().flex().flex_wrap().items_center().gap_2();
        let mut controls = panel(p)
            .p_4()
            .gap_3()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(super::localized(
                        self.language,
                        "Sample collection",
                        "示例订阅集合",
                        "サンプルコレクション",
                    )),
            )
            .child(
                row().children(self.workspace.collections.iter().map(|item| {
                    let id = item.id;
                    button(
                        ("demo-collection", id as usize),
                        item.name.clone(),
                        p,
                        self.collection == Some(id),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.busy && view.collection != Some(id) {
                            view.collection = Some(id);
                            view.preview = None;
                            cx.notify();
                        }
                    }))
                })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(p.muted)
                    .child(self.tr(Text::RelaySandboxFormatHelp)),
            )
            .child(row().children(formats.map(|(format, label)| {
                button(
                    ("demo-format", format as usize),
                    label,
                    p,
                    self.format == format,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    if !view.busy && view.format != format {
                        view.format = format;
                        view.preview = None;
                        cx.notify();
                    }
                }))
            })));
        if !self.busy {
            controls = controls.child(
                row()
                    .child(
                        button(
                            "demo-preview",
                            super::localized(
                                self.language,
                                "Run preview",
                                "运行预览",
                                "プレビューを実行",
                            ),
                            p,
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.run_preview(cx))),
                    )
                    .when(!self.backup_added, |row| {
                        row.child(
                            button(
                                "demo-import",
                                self.tr(Text::RelayDemoImportSample),
                                p,
                                false,
                            )
                            .on_click(cx.listener(|view, _, _, cx| view.add_backup(cx))),
                        )
                    })
                    .child(
                        button("demo-check", self.tr(Text::RelayDemoRun), p, false)
                            .on_click(cx.listener(|view, _, _, cx| view.self_check(cx))),
                    ),
            );
        }
        div()
            .id("offline-demo")
            .tab_group()
            .on_action(|_: &NextControl, window, _| window.focus_next())
            .on_action(|_: &PreviousControl, window, _| window.focus_prev())
            .size_full()
            .overflow_y_scroll()
            .bg(p.canvas)
            .text_color(p.ink)
            .p_6()
            .child(
                div()
                    .mx_auto()
                    .max_w(px(930.))
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(heading(
                        self.tr(Text::RelaySandboxTitle),
                        self.tr(Text::RelaySandboxOffline),
                        p,
                    ))
                    .child(
                        row().children(
                            [
                                (Language::En, "English"),
                                (Language::ZhCn, "简体中文"),
                                (Language::Ja, "日本語"),
                            ]
                            .map(|(lang, label)| {
                                button(
                                    ("demo-language", lang as usize),
                                    label,
                                    p,
                                    self.language.resolved() == lang,
                                )
                                .on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        view.language = lang;
                                        cx.notify();
                                    },
                                ))
                            }),
                        ),
                    )
                    .child(controls)
                    .when(self.busy, |body| {
                        body.child(super::localized(
                            self.language,
                            "Processing sample data…",
                            "正在处理示例数据…",
                            "サンプルデータを処理中…",
                        ))
                    })
                    .when_some(self.error, |body, err| {
                        body.child(panel(p).p_4().child(self.tr(err)))
                    })
                    .when_some(self.preview.as_ref(), |body, result| {
                        body.child(
                            panel(p)
                                .p_4()
                                .gap_2()
                                .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                                    "{}  {} → {}",
                                    self.tr(Text::RelayDemoNodes),
                                    result.before,
                                    result.after,
                                )))
                                .children(
                                    result
                                        .names
                                        .iter()
                                        .map(|name| div().text_sm().child(name.clone())),
                                ),
                        )
                    })
                    .when_some(self.report.as_ref(), |body, report| {
                        body.child(
                            panel(p)
                                .p_4()
                                .gap_2()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(p.accent)
                                        .child(self.tr(Text::RelayDemoPassed)),
                                )
                                .child(format!(
                                    "{}  {} / {}",
                                    self.tr(Text::RelayDemoBackup),
                                    report.backup_subscriptions,
                                    report.backup_collections
                                ))
                                .child(format!(
                                    "{}  {}",
                                    self.tr(Text::RelayDemoScheduled),
                                    report.scheduled_runs
                                ))
                                .child(format!(
                                    "{}  {}",
                                    self.tr(Text::RelayDemoRefreshed),
                                    report.refreshed_members
                                ))
                                .child(format!(
                                    "{}  {}",
                                    self.tr(Text::RelayDemoPlanned),
                                    report.planned_uploads
                                ))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(p.muted)
                                        .child(self.tr(Text::RelayDemoChecks)),
                                ),
                        )
                    }),
            )
    }
}

pub(super) fn open(language: Language, cx: &mut App) -> Result<WindowHandle<Demo>, String> {
    let workspace = demo::workspace().map_err(|error| error.to_string())?;
    let bounds = Bounds::centered(None, size(px(820.), px(690.)), cx);
    cx.open_window(
        WindowOptions {
            titlebar: Some(gpui::TitlebarOptions {
                title: Some(
                    format!("Script Hub · {}", text(language, Text::RelaySandboxTitle)).into(),
                ),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(520.), px(480.))),
            ..Default::default()
        },
        move |window, cx| {
            cx.new(|cx| {
                cx.observe_window_appearance(window, |_, _, cx| cx.notify())
                    .detach();
                Demo::new(workspace, language)
            })
        },
    )
    .map_err(|error| error.to_string())
}

pub(super) fn run() -> ExitCode {
    let exit = Rc::new(Cell::new(ExitCode::SUCCESS));
    let result = exit.clone();
    Application::new().run(move |cx| {
        super::bind_keys(cx);
        super::exit_when_all_windows_close(cx);
        cx.set_global(UiStyle::from(&Preferences::default()));
        if let Err(error) = open(Language::System, cx) {
            eprintln!("Cannot open Script Hub demo: {error}");
            result.set(ExitCode::FAILURE);
            cx.quit();
        } else {
            cx.activate(true);
        }
    });
    exit.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_available_formats(workspace: &Workspace, id: u64) {
        for format in [
            OutputFormat::Original,
            OutputFormat::Surge,
            OutputFormat::Mihomo,
            OutputFormat::SingBox,
        ] {
            let output = preview(workspace, id, format)
                .unwrap_or_else(|err| panic!("Preview format {format:?} failed: {err}"));
            assert_eq!((output.before, output.after), (3, 2));
            assert_eq!(output.names, ["1: Ready", "2: Secondary"]);
        }
    }

    #[test]
    fn native_demo_previews_bundled_collection_for_each_client() {
        let workspace = demo::workspace().unwrap();
        assert_available_formats(&workspace, workspace.collections[0].id);
    }

    #[test]
    fn native_demo_imports_and_previews_backup_for_each_client() {
        let workspace = demo::workspace().unwrap();
        let (candidate, id) = import_sample_backup(&workspace).unwrap();
        assert_eq!(candidate.collections.len(), workspace.collections.len() + 1);
        assert_eq!(candidate.collections.last().unwrap().name, "Backup Demo");
        assert!(candidate.subscriptions.iter().all(|entry| {
            entry.source.is_empty()
                && entry.source_file.is_empty()
                && entry.publish.output.is_empty()
        }));
        assert!(
            candidate
                .collections
                .iter()
                .all(|entry| entry.publish.output.is_empty())
        );
        assert_available_formats(&candidate, id);
    }
}
