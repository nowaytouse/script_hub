use std::path::{Path, PathBuf};

use gpui::{
    AppContext, ClipboardItem, Context, Div, Entity, EventEmitter, FontWeight, Window, div,
    prelude::*, px,
};

use super::{Palette, button, heading, input::TextInput, localized, panel};
use crate::gui::i18n::{Text, text};
use crate::pipeline::custom_sources::{self, CustomSource, SourceCategory};
use crate::runtime::{job::acquire_write_lock, settings::Language};

const CATEGORIES: [SourceCategory; 3] = [
    SourceCategory::Wool,
    SourceCategory::Weibo,
    SourceCategory::Devtools,
];

pub(super) struct SourcesChanged(pub(super) PathBuf);

enum Change {
    Add(CustomSource),
    Remove(CustomSource),
}

impl Change {
    fn source(&self) -> &CustomSource {
        match self {
            Self::Add(source) | Self::Remove(source) => source,
        }
    }

    fn label(&self) -> Text {
        match self {
            Self::Add(_) => Text::AddSource,
            Self::Remove(_) => Text::RemoveSource,
        }
    }

    fn apply(self, root: &Path) -> Result<(), String> {
        let _lock = acquire_write_lock(root)?;
        match self {
            Self::Add(source) => custom_sources::add(root, source),
            Self::Remove(source) => {
                if !custom_sources::load(root)?.contains(&source) {
                    return Err(
                        "The selected source changed. Review the updated list before removing it."
                            .into(),
                    );
                }
                custom_sources::remove(root, source.category, &source.name)
            }
        }
    }
}

pub(super) struct SourceEditor {
    language: Language,
    workspace: Option<PathBuf>,
    workspace_busy: bool,
    read_revision: u64,
    listing: Option<Result<Vec<CustomSource>, String>>,
    category: SourceCategory,
    name: Entity<TextInput>,
    url: Entity<TextInput>,
    pending: Option<Change>,
    writing: bool,
    notice: Option<Text>,
    error: Option<String>,
}

impl EventEmitter<SourcesChanged> for SourceEditor {}

impl SourceEditor {
    pub(super) fn new(language: Language, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| TextInput::new(text(language, Text::SourceName), false, cx));
        let url = cx.new(|cx| {
            TextInput::new(
                "https://raw.githubusercontent.com/owner/repo/main/module.sgmodule",
                false,
                cx,
            )
        });
        for input in [&name, &url] {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        Self {
            language,
            workspace: None,
            workspace_busy: false,
            read_revision: 0,
            listing: None,
            category: SourceCategory::Wool,
            name,
            url,
            pending: None,
            writing: false,
            notice: None,
            error: None,
        }
    }

    pub(super) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        if self.language != language {
            self.language = language;
            self.name.update(cx, |input, cx| {
                input.set_placeholder(text(language, Text::SourceName), cx);
            });
            cx.notify();
        }
    }

    pub(super) fn set_workspace(&mut self, workspace: Option<PathBuf>, cx: &mut Context<Self>) {
        if self.workspace != workspace {
            self.workspace = workspace;
            self.notice = None;
            self.error = None;
            self.clear_inputs(cx);
        }
        self.refresh(cx);
    }

    pub(super) fn set_workspace_busy(&mut self, busy: bool, cx: &mut Context<Self>) {
        if self.workspace_busy == busy {
            return;
        }
        self.workspace_busy = busy;
        if busy {
            self.pending = None;
        }
        cx.notify();
    }

    pub(super) fn is_writing(&self) -> bool {
        self.writing
    }

    fn tr(&self, key: Text) -> &'static str {
        text(self.language, key)
    }

    fn clear_inputs(&mut self, cx: &mut Context<Self>) {
        self.name.update(cx, |input, cx| input.clear(cx));
        self.url.update(cx, |input, cx| input.clear(cx));
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.writing {
            return;
        }
        self.pending = None;
        self.listing = None;
        self.read_revision = self.read_revision.wrapping_add(1);
        let revision = self.read_revision;
        if let Some(root) = self.workspace.clone() {
            let read = cx
                .background_executor()
                .spawn(async move { custom_sources::load(&root) });
            cx.spawn(async move |this, cx| {
                let listing = read.await;
                let _ = this.update(cx, |editor, cx| {
                    if editor.read_revision == revision {
                        editor.listing = Some(listing);
                        cx.notify();
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    fn can_edit(&self) -> bool {
        !self.workspace_busy && !self.writing && matches!(self.listing, Some(Ok(_)))
    }

    fn review_add(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().trim();
        let url = self.url.read(cx).value().trim();
        if self.can_edit()
            && self.pending.is_none()
            && custom_sources::validate_name(name).is_ok()
            && custom_sources::validate_url(url).is_ok()
        {
            self.pending = Some(Change::Add(CustomSource {
                category: self.category,
                name: name.to_owned(),
                url: url.to_owned(),
            }));
            self.error = None;
            self.notice = None;
            cx.notify();
        }
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        if !self.can_edit() {
            return;
        }
        let Some((root, change)) = self.workspace.clone().zip(self.pending.take()) else {
            return;
        };
        self.writing = true;
        let adding = matches!(change, Change::Add(_));
        let write_root = root.clone();
        let write = cx
            .background_executor()
            .spawn(async move { change.apply(&write_root) });
        cx.spawn(async move |this, cx| {
            let result = write.await;
            let _ = this.update(cx, |editor, cx| {
                editor.writing = false;
                if editor.workspace.as_ref() == Some(&root) {
                    match &result {
                        Ok(()) => {
                            editor.notice = Some(Text::SourceSaved);
                            if adding {
                                editor.clear_inputs(cx);
                            }
                        }
                        Err(error) => editor.error = Some(error.clone()),
                    }
                }
                editor.refresh(cx);
                if result.is_ok() {
                    cx.emit(SourcesChanged(root));
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn add_form(&self, palette: Palette, cx: &Context<Self>) -> Div {
        let name = self.name.read(cx).value().trim();
        let url = self.url.read(cx).value().trim();
        let name_result = custom_sources::validate_name(name);
        let url_result = custom_sources::validate_url(url);
        let ready = name_result.is_ok() && url_result.is_ok();
        let categories =
            row().children(CATEGORIES.into_iter().enumerate().map(|(index, category)| {
                button(
                    ("source-category", index),
                    self.tr(category_label(category)),
                    palette,
                    self.category == category,
                )
                .on_click(cx.listener(move |editor, _, _, cx| {
                    if editor.can_edit() && editor.pending.is_none() {
                        editor.category = category;
                        cx.notify();
                    }
                }))
            }));
        panel(palette)
            .p_5()
            .gap_3()
            .child(title(self.tr(Text::AddSource)))
            .child(muted(self.tr(Text::SourceTrust), palette))
            .child(title(self.tr(Text::SourceCategory)))
            .child(categories)
            .child(title(self.tr(Text::SourceName)))
            .child(self.name.clone())
            .when_some(
                name_result.err().filter(|_| !name.is_empty()),
                |panel, error| panel.child(muted(error, palette)),
            )
            .child(title(self.tr(Text::SourceUrl)))
            .child(self.url.clone())
            .child(muted(self.tr(Text::SourceValidationHint), palette))
            .when_some(
                url_result.err().filter(|_| !url.is_empty()),
                |panel, error| panel.child(muted(error, palette)),
            )
            .child(
                row().child(
                    button(
                        "review-source-add",
                        self.tr(Text::SaveSource),
                        palette,
                        ready,
                    )
                    .when(!ready, |button| button.opacity(0.55).tab_stop(false))
                    .on_click(cx.listener(|editor, _, _, cx| editor.review_add(cx))),
                ),
            )
    }

    fn confirmation(&self, change: &Change, palette: Palette, cx: &Context<Self>) -> Div {
        let source = change.source();
        panel(palette)
            .p_5()
            .gap_3()
            .border_color(palette.accent)
            .child(title(self.tr(change.label())))
            .child(title(source.name.clone()))
            .child(muted(self.tr(category_label(source.category)), palette))
            .child(muted(source.url.clone(), palette))
            .when(matches!(change, Change::Add(_)), |panel| {
                panel.child(muted(self.tr(Text::SourceTrust), palette))
            })
            .child(
                row()
                    .child(
                        button(
                            "cancel-source-change",
                            self.tr(Text::Cancel),
                            palette,
                            false,
                        )
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.pending = None;
                            cx.notify();
                        })),
                    )
                    .child(
                        button(
                            "confirm-source-change",
                            self.tr(Text::Confirm),
                            palette,
                            true,
                        )
                        .on_click(cx.listener(|editor, _, _, cx| editor.commit(cx))),
                    ),
            )
    }

    fn source_row(
        &self,
        index: usize,
        source: &CustomSource,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Div {
        let reviewed = source.clone();
        let copied_url = source.url.clone();
        let allow_remove = self.can_edit() && self.pending.is_none();
        panel(palette)
            .p_4()
            .gap_2()
            .child(title(source.name.clone()))
            .child(muted(self.tr(category_label(source.category)), palette))
            .child(muted(source.url.clone(), palette).truncate())
            .child(
                row()
                    .child(
                        button(
                            ("copy-source-url", index),
                            self.tr(Text::CopyAddress),
                            palette,
                            false,
                        )
                        .on_click(cx.listener(move |editor, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copied_url.clone()));
                            editor.notice = Some(Text::Copied);
                            cx.notify();
                        })),
                    )
                    .when(allow_remove, |buttons| {
                        buttons.child(
                            button(
                                ("review-source-removal", index),
                                self.tr(Text::RemoveSource),
                                palette,
                                false,
                            )
                            .on_click(cx.listener(
                                move |editor, _, _, cx| {
                                    if editor.can_edit() && editor.pending.is_none() {
                                        editor.pending = Some(Change::Remove(reviewed.clone()));
                                        editor.error = None;
                                        editor.notice = None;
                                        cx.notify();
                                    }
                                },
                            )),
                        )
                    }),
            )
    }
}

impl Render for SourceEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::for_window(window, cx);
        let page = div()
            .flex()
            .flex_col()
            .w_full()
            .max_w(px(1040.))
            .min_w_0()
            .gap_4()
            .child(heading(
                self.tr(Text::Sources),
                self.tr(Text::HelpSources),
                palette,
            ));
        if self.workspace.is_none() {
            return page.child(
                panel(palette)
                    .p_5()
                    .gap_2()
                    .child(title(self.tr(Text::WorkspaceMissing)))
                    .child(muted(
                        localized(
                            self.language,
                            "Open a Script Hub workspace to manage its custom sources.",
                            "打开 Script Hub 工作区后可以管理自定义来源。",
                            "Script Hub のワークスペースを開くと、カスタムソースを管理できます。",
                        ),
                        palette,
                    )),
            );
        }
        let page = page
            .when_some(self.notice, |page, notice| {
                page.child(title(self.tr(notice)))
            })
            .when_some(self.error.as_ref(), |page, error| {
                page.child(muted(error.clone(), palette))
            });
        if self.writing {
            return page.child(muted(
                localized(
                    self.language,
                    "Saving source configuration…",
                    "正在保存来源配置…",
                    "ソースの設定を保存しています…",
                ),
                palette,
            ));
        }
        match &self.listing {
            None => page.child(muted(
                localized(
                    self.language,
                    "Reading source configuration…",
                    "正在读取来源配置…",
                    "ソースの設定を読み込んでいます…",
                ),
                palette,
            )),
            Some(Err(error)) => page.child(
                panel(palette)
                    .p_5()
                    .gap_3()
                    .child(muted(error.clone(), palette))
                    .child(
                        row().child(
                            button(
                                "retry-read-sources",
                                self.tr(Text::RefreshStatus),
                                palette,
                                false,
                            )
                            .on_click(cx.listener(|editor, _, _, cx| editor.refresh(cx))),
                        ),
                    ),
            ),
            Some(Ok(sources)) => page
                .when_some(self.pending.as_ref(), |page, change| {
                    page.child(self.confirmation(change, palette, cx))
                })
                .when(self.can_edit() && self.pending.is_none(), |page| {
                    page.child(self.add_form(palette, cx))
                })
                .when(self.workspace_busy, |page| {
                    page.child(muted(
                        localized(
                            self.language,
                            "Source editing is paused while a workspace task is running.",
                            "工作区任务运行期间暂停编辑来源。",
                            "ワークスペースのタスク実行中はソースを編集できません。",
                        ),
                        palette,
                    ))
                })
                .when(sources.is_empty(), |page| {
                    page.child(muted(self.tr(Text::NoSources), palette))
                })
                .children(
                    sources
                        .iter()
                        .enumerate()
                        .map(|(index, source)| self.source_row(index, source, palette, cx)),
                ),
        }
    }
}

fn title(label: impl Into<gpui::SharedString>) -> Div {
    div()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .child(label.into())
}

fn muted(label: impl Into<gpui::SharedString>, palette: Palette) -> Div {
    div()
        .min_w_0()
        .text_sm()
        .text_color(palette.muted)
        .child(label.into())
}

fn row() -> Div {
    div().flex().items_center().flex_wrap().gap_2()
}

fn category_label(category: SourceCategory) -> Text {
    match category {
        SourceCategory::Wool => Text::WoolSource,
        SourceCategory::Weibo => Text::WeiboSource,
        SourceCategory::Devtools => Text::DevtoolsSource,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reviewed_removal_cannot_delete_a_replaced_source() {
        struct TestWorkspace(PathBuf);
        impl Drop for TestWorkspace {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "script-hub-source-review-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let workspace = TestWorkspace(root);
        let reviewed = CustomSource {
            name: "Synthetic test source".into(),
            category: SourceCategory::Wool,
            url: "https://raw.githubusercontent.com/example/repo/main/original.sgmodule".into(),
        };
        let current = CustomSource {
            url: "https://raw.githubusercontent.com/example/repo/main/replacement.sgmodule".into(),
            ..reviewed.clone()
        };
        Change::Add(current.clone()).apply(&workspace.0).unwrap();

        assert!(Change::Remove(reviewed).apply(&workspace.0).is_err());
        assert_eq!(
            custom_sources::load(&workspace.0).unwrap().as_slice(),
            std::slice::from_ref(&current)
        );

        Change::Remove(current).apply(&workspace.0).unwrap();
        assert!(custom_sources::load(&workspace.0).unwrap().is_empty());
    }
}
