//! Native window and navigation for the Script Hub desktop application.

mod catalog;
mod demo;
mod history;
mod input;
mod navigation;
mod project;
mod sources;
mod subscriptions;
mod support;
mod templates;

use crate::desktop::i18n::{Text, text};
use crate::runtime::catalog::{Catalog, ModulePlatform};
use crate::runtime::job::{Job, JobKind};
use crate::runtime::repository::RepositoryStatus;
use crate::runtime::settings::{self, Accent, Language, Preferences, Theme};
use gpui::{
    App, Application, Bounds, ClipboardItem, Context, Div, ElementId, Entity, FocusHandle,
    FontWeight, Global, Image, ImageFormat, Rgba, SharedString, Stateful, Window, WindowAppearance,
    WindowBounds, WindowHandle, WindowOptions, div, img, prelude::*, px, rgb, size,
};
use history::TaskHistory;
use input::TextInput;
use sources::{SourceEditor, SourcesChanged};
use std::cell::Cell;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;
use subscriptions::Subscriptions;
use support::Support;
use templates::Templates;

gpui::actions!(
    script_hub,
    [
        NextControl,
        PreviousControl,
        TogglePageSearch,
        ClosePageSearch,
        OpenPageSearchResult
    ]
);

#[derive(Clone, Copy)]
struct UiStyle {
    theme: Theme,
    accent: Accent,
}

impl Global for UiStyle {}

impl From<&Preferences> for UiStyle {
    fn from(preferences: &Preferences) -> Self {
        Self {
            theme: preferences.theme,
            accent: preferences.accent,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub(super) canvas: Rgba,
    pub(super) surface: Rgba,
    pub(super) subtle: Rgba,
    pub(super) border: Rgba,
    pub(super) ink: Rgba,
    pub(super) muted: Rgba,
    pub(super) accent: Rgba,
    pub(super) on_accent: Rgba,
}

impl Palette {
    pub(super) fn for_window(window: &Window, cx: &App) -> Self {
        let style = cx.try_global::<UiStyle>().copied().unwrap_or(UiStyle {
            theme: Theme::System,
            accent: Accent::Purple,
        });
        let dark = match style.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => matches!(
                window.appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        };
        let accent = match (style.accent, dark) {
            (Accent::Purple, false) => 0x5743a4,
            (Accent::Purple, true) => 0xbbaafa,
            (Accent::Blue, false) => 0x2459a0,
            (Accent::Blue, true) => 0x8ebdff,
            (Accent::Teal, false) => 0x0c6859,
            (Accent::Teal, true) => 0x7ed6ba,
            (Accent::Amber, false) => 0x845018,
            (Accent::Amber, true) => 0xf1c37b,
        };
        let (canvas, surface, subtle, border, ink, muted, on_accent) = if dark {
            (
                0x15171b, 0x1d2026, 0x272b33, 0x353b45, 0xf0f2f7, 0xa6afbd, 0x181323,
            )
        } else {
            (
                0xf6f6f8, 0xffffff, 0xeeeff3, 0xdfe0e7, 0x1f2130, 0x586176, 0xffffff,
            )
        };
        Self {
            canvas: rgb(canvas),
            surface: rgb(surface),
            subtle: rgb(subtle),
            border: rgb(border),
            ink: rgb(ink),
            muted: rgb(muted),
            accent: rgb(accent),
            on_accent: rgb(on_accent),
        }
    }
}

pub(super) fn button(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
    palette: Palette,
    primary: bool,
) -> Stateful<Div> {
    let (bg, fg, line) = if primary {
        (palette.accent, palette.on_accent, palette.accent)
    } else {
        (palette.surface, palette.ink, palette.border)
    };
    div()
        .id(id)
        .tab_index(0)
        .cursor_pointer()
        .px_3()
        .h(px(36.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .border_1()
        .border_color(line)
        .bg(bg)
        .text_color(fg)
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .hover(move |style| style.border_color(palette.accent).opacity(0.88))
        .focus(move |style| style.border_color(palette.accent).bg(bg))
        .active(|style| style.opacity(0.72))
        .child(title.into())
}

pub(super) fn panel(palette: Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .rounded_lg()
        .bg(palette.surface)
        .border_1()
        .border_color(palette.border)
}

pub(super) fn heading(
    title: impl Into<SharedString>,
    help: impl Into<SharedString>,
    palette: Palette,
) -> Div {
    let help = help.into();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(30.))
                .line_height(px(38.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.into()),
        )
        .when(!help.is_empty(), |node| {
            node.child(
                div()
                    .text_sm()
                    .line_height(px(21.))
                    .text_color(palette.muted)
                    .child(help),
            )
        })
}

pub(super) fn localized(
    lang: Language,
    en: &'static str,
    zh: &'static str,
    ja: &'static str,
) -> &'static str {
    match lang.resolved() {
        Language::ZhCn => zh,
        Language::Ja => ja,
        Language::En | Language::System => en,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Overview,
    Modules,
    Rules,
    Templates,
    Subscriptions,
    Relay,
    Sources,
    Tasks,
    History,
    Settings,
    Help,
}

impl Page {
    fn label(self, language: Language) -> &'static str {
        match self {
            Self::Overview => text(language, Text::Overview),
            Self::Modules => text(language, Text::Modules),
            Self::Rules => localized(language, "Rule sets", "规则集", "ルールセット"),
            Self::Templates => text(language, Text::Templates),
            Self::Subscriptions => {
                localized(language, "Subscriptions", "订阅", "サブスクリプション")
            }
            Self::Relay => localized(language, "Relay", "中转", "リレー"),
            Self::Sources => text(language, Text::Sources),
            Self::Tasks => text(language, Text::Tasks),
            Self::History => text(language, Text::History),
            Self::Settings => text(language, Text::Settings),
            Self::Help => text(language, Text::Help),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Overview => "⌂",
            Self::Modules => "◫",
            Self::Rules => "≡",
            Self::Templates => "▤",
            Self::Subscriptions => "◎",
            Self::Relay => "⇄",
            Self::Sources => "↗",
            Self::Tasks => "◴",
            Self::History => "◷",
            Self::Settings => "⚙",
            Self::Help => "?",
        }
    }
}

struct Hub {
    page: Page,
    main_focus: FocusHandle,
    root: Option<PathBuf>,
    repository: Option<RepositoryStatus>,
    repository_open: bool,
    repository_loading: bool,
    loading: bool,
    preferences: Preferences,
    saved_preferences: Preferences,
    preference_path: Option<PathBuf>,
    preferences_ready: bool,
    module_platform: ModulePlatform,
    surge: Catalog,
    shadowrocket: Catalog,
    search: Entity<TextInput>,
    page_query: Entity<TextInput>,
    page_search_open: bool,
    rule_search: Entity<TextInput>,
    rule_policy: Entity<TextInput>,
    singbox: Entity<TextInput>,
    active_category: Option<String>,
    inspect_module: Option<usize>,
    checked_modules: BTreeSet<usize>,
    checked_rules: BTreeSet<usize>,
    templates: Entity<Templates>,
    subscriptions: Entity<Subscriptions>,
    sources: Entity<SourceEditor>,
    history: Entity<TaskHistory>,
    support: Entity<Support>,
    demo: Option<WindowHandle<demo::Demo>>,
    logo: Arc<Image>,
    message: Option<String>,
    job: Option<Job>,
    job_kind: JobKind,
    pending_job: Option<JobKind>,
    show_logs: bool,
    log_query: Entity<TextInput>,
    problems_only: bool,
    confirm_quit: bool,
    commit: Entity<TextInput>,
    selected_changes: BTreeSet<String>,
}

impl Hub {
    fn new(
        preferences: Preferences,
        preference_path: Option<PathBuf>,
        preferences_ready: bool,
        message: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let language = preferences.language;
        let search = cx.new(|cx| TextInput::new(text(language, Text::SearchModules), false, cx));
        let page_query = cx.new(|cx| {
            TextInput::new(
                localized(language, "Search pages", "搜索页面", "ページを検索"),
                false,
                cx,
            )
        });
        let rule_search = cx.new(|cx| TextInput::new(text(language, Text::SearchRules), false, cx));
        let rule_policy =
            cx.new(|cx| TextInput::new(text(language, Text::RuleSetPolicyHint), false, cx));
        let log_query = cx.new(|cx| TextInput::new(text(language, Text::SearchLogs), false, cx));
        let singbox = cx.new(|cx| {
            let mut input = TextInput::new("sing-box", false, cx);
            input.set_value(preferences.singbox.clone(), cx);
            input
        });
        let commit = cx.new(|cx| {
            TextInput::new(
                localized(language, "Commit message", "提交说明", "コミットメッセージ"),
                false,
                cx,
            )
        });
        for input in [
            &search,
            &page_query,
            &rule_search,
            &rule_policy,
            &singbox,
            &commit,
            &log_query,
        ] {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        let templates = cx.new(Templates::new);
        let subscriptions = cx.new(Subscriptions::new);
        let sources = cx.new(|cx| SourceEditor::new(language, cx));
        let history_path = preference_path
            .as_ref()
            .map(|path| path.with_file_name("history.sqlite3"));
        let history = cx.new(|cx| TaskHistory::new(history_path, language, cx));
        let support = cx.new(|cx| Support::new(language, cx));
        cx.subscribe(&sources, |hub, _, event: &SourcesChanged, cx| {
            if hub.root.as_ref() == Some(&event.0) {
                hub.reload_project(cx);
            }
        })
        .detach();
        cx.subscribe(
            &subscriptions,
            |hub, _, _: &subscriptions::OpenRelay, cx| hub.navigate(Page::Relay, cx),
        )
        .detach();
        templates.update(cx, |state, cx| state.set_language(language, cx));
        subscriptions.update(cx, |state, cx| state.set_language(language, cx));
        Self {
            page: Page::Modules,
            main_focus: cx.focus_handle().tab_stop(false),
            root: None,
            repository: None,
            repository_open: false,
            repository_loading: false,
            loading: false,
            saved_preferences: preferences.clone(),
            preferences,
            preference_path,
            preferences_ready,
            module_platform: ModulePlatform::Surge,
            surge: Catalog::bundled(ModulePlatform::Surge),
            shadowrocket: Catalog::bundled(ModulePlatform::Shadowrocket),
            search,
            page_query,
            page_search_open: false,
            rule_search,
            rule_policy,
            singbox,
            commit,
            active_category: None,
            inspect_module: None,
            checked_modules: BTreeSet::new(),
            checked_rules: BTreeSet::new(),
            templates,
            subscriptions,
            sources,
            history,
            support,
            demo: None,
            logo: Arc::new(Image::from_bytes(
                ImageFormat::Png,
                include_bytes!("../../assets/logo.png").to_vec(),
            )),
            message,
            job: None,
            job_kind: JobKind::Validate,
            pending_job: None,
            show_logs: true,
            log_query,
            problems_only: false,
            confirm_quit: false,
            selected_changes: BTreeSet::new(),
        }
    }

    fn local(&self, en: &'static str, zh: &'static str, ja: &'static str) -> &'static str {
        localized(self.preferences.language, en, zh, ja)
    }

    fn tr(&self, key: Text) -> &'static str {
        text(self.preferences.language, key)
    }

    fn reset_language(&mut self, cx: &mut Context<Self>) {
        let language = self.preferences.language;
        let fields = [
            (&self.search, self.tr(Text::SearchModules)),
            (
                &self.page_query,
                self.local("Search pages", "搜索页面", "ページを検索"),
            ),
            (&self.rule_search, self.tr(Text::SearchRules)),
            (&self.rule_policy, self.tr(Text::RuleSetPolicyHint)),
            (&self.log_query, self.tr(Text::SearchLogs)),
            (
                &self.commit,
                self.local("Commit message", "提交说明", "コミットメッセージ"),
            ),
        ];
        for (field, label) in fields {
            field.update(cx, |field, cx| field.set_placeholder(label, cx));
        }
        self.templates
            .update(cx, |state, cx| state.set_language(language, cx));
        self.subscriptions
            .update(cx, |state, cx| state.set_language(language, cx));
        self.sources
            .update(cx, |state, cx| state.set_language(language, cx));
        self.history
            .update(cx, |state, cx| state.set_language(language, cx));
        self.support
            .update(cx, |state, cx| state.set_language(language, cx));
        cx.notify();
    }

    fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        self.close_page_search(cx);
        self.page = page;
        self.confirm_quit = false;
        self.pending_job = None;
        self.message = None;
        self.subscriptions
            .update(cx, |state, cx| state.show_relay(page == Page::Relay, cx));
        if page == Page::Tasks && self.repository_open && self.repository.is_none() {
            self.read_repository(cx);
        }
        if page == Page::Templates {
            let root = self.root.clone();
            self.templates
                .update(cx, |state, cx| state.visit_project(root, cx));
        }
        if page == Page::History {
            self.history.update(cx, |view, cx| view.refresh(cx));
        } else {
            self.history
                .update(cx, |view, _| view.dismiss_confirmation());
        }
        cx.notify();
    }

    fn project_writes_pending(&self, cx: &App) -> bool {
        self.job.as_ref().is_some_and(Job::is_running)
            || self.sources.read(cx).is_writing()
            || self.history.read(cx).is_writing()
    }

    fn project_busy(&self, cx: &App) -> bool {
        self.loading || self.repository_loading || self.project_writes_pending(cx)
    }

    fn working(&self, cx: &App) -> bool {
        self.project_busy(cx) || self.subscriptions.read(cx).schedule_active()
    }

    fn project_action_blocked(&mut self, cx: &mut Context<Self>) -> bool {
        let message = if self.subscriptions.read(cx).schedule_active() {
            self.local(
                "Stop scheduled sync on the Relay page before running a maintenance task.",
                "运行维护任务前，请先在中转页停止定时同步。",
                "メンテナンスタスクの実行前に、リレーページで定期同期を停止してください。",
            )
        } else if self.project_busy(cx) {
            self.local(
                "Wait for the current project operation to finish.",
                "请等待当前项目操作完成。",
                "現在のプロジェクト操作が完了するまで待ってください。",
            )
        } else {
            return false;
        };
        self.message = Some(message.into());
        cx.notify();
        true
    }

    fn close_blocked(&self, cx: &App) -> bool {
        self.project_writes_pending(cx)
            || !self.templates.read(cx).can_close()
            || !self.subscriptions.read(cx).can_close(cx)
    }

    fn settings_dirty(&self, cx: &App) -> bool {
        self.preferences != self.saved_preferences
            || self.singbox.read(cx).value().trim() != self.saved_preferences.singbox
    }

    fn can_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.close_blocked(cx) {
            self.message = Some(
                self.local(
                    "Finish the current operation or review your changes before closing.",
                    "请先完成当前操作或处理未保存的更改。",
                    "現在の操作を完了するか、変更を確認してから閉じてください。",
                )
                .into(),
            );
            cx.notify();
            return false;
        }
        if self.subscriptions.read(cx).has_unsaved_session() {
            self.navigate(Page::Subscriptions, cx);
            self.subscriptions
                .update(cx, |view, cx| view.confirm_end_session(cx));
            return false;
        }
        if self.settings_dirty(cx) {
            self.navigate(Page::Settings, cx);
            self.confirm_quit = true;
            cx.notify();
            return false;
        }
        true
    }

    fn copy(&mut self, value: String, cx: &mut Context<Self>) {
        if value.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(value));
        self.message = Some(
            self.local(
                "Copied to clipboard",
                "已复制到剪贴板",
                "クリップボードにコピーしました",
            )
            .into(),
        );
        cx.notify();
    }

    fn open_demo(&mut self, cx: &mut Context<Self>) {
        if self.demo.as_ref().is_some_and(|handle| {
            handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
        }) {
            return;
        }
        match demo::open(self.preferences.language, cx) {
            Ok(handle) => self.demo = Some(handle),
            Err(error) => {
                self.message = Some(error);
                cx.notify();
            }
        }
    }

    fn sidebar(&self, window: &Window, palette: Palette, cx: &mut Context<Self>) -> Div {
        let compact = window.viewport_size().width < px(910.);
        let link = |page: Page| {
            let selected = self.page == page;
            div()
                .id(page.icon())
                .tab_index(0)
                .flex()
                .gap_3()
                .items_center()
                .h(px(40.))
                .flex_shrink_0()
                .px_3()
                .rounded_md()
                .cursor_pointer()
                .bg(if selected {
                    palette.subtle
                } else {
                    palette.surface
                })
                .text_color(if selected {
                    palette.accent
                } else {
                    palette.ink
                })
                .hover(move |style| style.bg(palette.subtle))
                .focus(move |style| style.bg(palette.subtle).text_color(palette.accent))
                .child(div().w(px(18.)).text_center().text_lg().child(page.icon()))
                .child(
                    div()
                        .text_sm()
                        .font_weight(if selected {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::MEDIUM
                        })
                        .child(page.label(self.preferences.language)),
                )
                .on_click(cx.listener(move |hub, _, _, cx| hub.navigate(page, cx)))
        };
        let navigation = div()
            .id("sidebar-navigation")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .mt_5()
                    .child(
                        div()
                            .px_3()
                            .mb_1()
                            .text_xs()
                            .text_color(palette.muted)
                            .child(self.local("Library", "资源库", "ライブラリ")),
                    )
                    .children(
                        [
                            Page::Overview,
                            Page::Modules,
                            Page::Rules,
                            Page::Templates,
                            Page::Subscriptions,
                            Page::Relay,
                        ]
                        .map(link),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .mt_5()
                    .child(
                        div()
                            .px_3()
                            .mb_1()
                            .text_xs()
                            .text_color(palette.muted)
                            .child(self.local("Project", "项目", "プロジェクト")),
                    )
                    .children([Page::Sources, Page::Tasks, Page::History].map(link)),
            );
        div()
            .flex()
            .flex_col()
            .flex_shrink_0()
            .w(px(if compact { 166. } else { 208. }))
            .h_full()
            .bg(palette.surface)
            .border_r_1()
            .border_color(palette.border)
            .px_3()
            .pt_4()
            .pb_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .h(px(45.))
                    .flex_shrink_0()
                    .child(img(self.logo.clone()).size(px(30.)).flex_shrink_0())
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(17.))
                            .child("Script Hub"),
                    ),
            )
            .child(navigation)
            .child(link(Page::Help))
            .child(link(Page::Settings))
    }

    fn header(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .h(px(57.))
            .flex_shrink_0()
            .items_center()
            .justify_between()
            .gap_4()
            .px_5()
            .bg(palette.surface)
            .border_b_1()
            .border_color(palette.border)
            .child(
                div()
                    .min_w_0()
                    .text_sm()
                    .text_color(palette.ink)
                    .font_weight(FontWeight::MEDIUM)
                    .child(self.page.label(self.preferences.language)),
            )
            .child(
                button(
                    "search-pages",
                    self.local("Search pages", "搜索页面", "ページを検索"),
                    palette,
                    self.page_search_open,
                )
                .on_click(cx.listener(|hub, _, window, cx| hub.toggle_page_search(window, cx))),
            )
    }
}

impl Render for Hub {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let project_busy = self.project_busy(cx);
        self.subscriptions
            .update(cx, |view, _| view.set_schedule_project_busy(project_busy));
        let source_busy = project_busy || self.subscriptions.read(cx).schedule_active();
        self.sources
            .update(cx, |view, cx| view.set_workspace_busy(source_busy, cx));
        let saved = self.preferences_ready.then_some(&self.saved_preferences);
        let checker_selected = self.subscriptions.read(cx).checker_selected();
        self.support.update(cx, |support, cx| {
            support.set_diagnostics(saved, checker_selected, cx)
        });
        let scale = f32::from(self.preferences.scale_percent) / 100.;
        window.set_rem_size(px(16. * scale));
        let palette = Palette::for_window(window, cx);
        let gap = if window.viewport_size().width < px(990.) {
            20.
        } else {
            32.
        };
        let content = match self.page {
            Page::Overview => self.overview_page(palette, cx).into_any_element(),
            Page::Modules => self.modules_page(window, palette, cx).into_any_element(),
            Page::Rules => self.rules_page(palette, cx).into_any_element(),
            Page::Templates => self.templates.clone().into_any_element(),
            Page::Subscriptions | Page::Relay => self.subscriptions.clone().into_any_element(),
            Page::Sources => self.sources.clone().into_any_element(),
            Page::Tasks => self.tasks_page(palette, cx).into_any_element(),
            Page::History => self.history.clone().into_any_element(),
            Page::Settings => self.settings_page(palette, cx).into_any_element(),
            Page::Help => self.support.clone().into_any_element(),
        };
        div()
            .id("script-hub")
            .track_focus(&self.main_focus)
            .key_context(navigation::HUB_CONTEXT)
            .tab_group()
            .on_action(cx.listener(|hub, _: &TogglePageSearch, window, cx| {
                hub.toggle_page_search(window, cx)
            }))
            .on_action(|_: &NextControl, window, _| window.focus_next())
            .on_action(|_: &PreviousControl, window, _| window.focus_prev())
            .size_full()
            .flex()
            .bg(palette.canvas)
            .text_color(palette.ink)
            .text_size(px(14. * scale))
            .child(self.sidebar(window, palette, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(self.header(palette, cx))
                    .when(self.page_search_open, |view| {
                        view.child(self.page_search(palette, cx))
                    })
                    .when_some(self.message.as_ref(), |node, message| {
                        node.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .px_5()
                                .py_2()
                                .bg(palette.subtle)
                                .text_sm()
                                .child(div().min_w_0().child(message.clone()))
                                .child(
                                    div()
                                        .id("clear-notice")
                                        .cursor_pointer()
                                        .text_color(palette.muted)
                                        .child("×")
                                        .on_click(cx.listener(|hub, _, _, cx| {
                                            hub.message = None;
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
                    .child(
                        div()
                            .id(("page-scroll", self.page as usize))
                            .overflow_y_scroll()
                            .flex_1()
                            .min_h_0()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_5()
                                    .px(px(gap))
                                    .py_6()
                                    .w_full()
                                    .max_w(px(1450.))
                                    .mx_auto()
                                    .child(content),
                            ),
                    ),
            )
    }
}

fn bind_keys(cx: &mut App) {
    TextInput::bind_keys(cx);
    cx.bind_keys([
        gpui::KeyBinding::new("tab", NextControl, None),
        gpui::KeyBinding::new("shift-tab", PreviousControl, None),
        gpui::KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-k"
            } else {
                "ctrl-k"
            },
            TogglePageSearch,
            Some(navigation::HUB_CONTEXT),
        ),
        gpui::KeyBinding::new("escape", ClosePageSearch, Some(navigation::COMMAND_CONTEXT)),
        gpui::KeyBinding::new(
            "enter",
            OpenPageSearchResult,
            Some(navigation::COMMAND_CONTEXT),
        ),
    ]);
}

fn exit_when_all_windows_close(cx: &mut App) {
    cx.on_window_closed(|cx| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}

pub fn run(mut args: impl Iterator<Item = String>) -> ExitCode {
    let mut prepare = None;
    let root = match args.next().as_deref() {
        Some("--worker") => return crate::runtime::worker::run(args),
        Some("--demo") if args.next().is_none() => return demo::run(),
        Some("--prepare-templates") => {
            let Some(setup) = crate::desktop::template_paths::parse_template_setup(args) else {
                return ExitCode::from(2);
            };
            prepare = Some(setup);
            None
        }
        Some("--root") => match args.next() {
            Some(root) if !root.is_empty() && args.next().is_none() => Some(PathBuf::from(root)),
            _ => return ExitCode::from(2),
        },
        Some("--help" | "-h") => {
            println!(
                "Usage: script-hub-gui [--root <repository>]\n       script-hub-gui --demo | --prepare-templates <public.vault> <local-backup.vault> <config>..."
            );
            return ExitCode::SUCCESS;
        }
        Some(_) => return ExitCode::from(2),
        None => None,
    };
    let (preference_path, saved) = match settings::preferences_path() {
        Ok(path) => {
            let saved = settings::load(&path);
            (Some(path), saved)
        }
        Err(error) => (None, Err(error)),
    };
    let preferences_ready = saved.is_ok();
    let (preferences, message) = match saved {
        Ok(preferences) => (preferences.unwrap_or_default(), None),
        Err(error) => (Preferences::default(), Some(error)),
    };
    let result = Rc::new(Cell::new(ExitCode::SUCCESS));
    let exit = result.clone();
    Application::new().run(move |cx| {
        bind_keys(cx);
        cx.set_global(UiStyle::from(&preferences));
        exit_when_all_windows_close(cx);
        let [width, height] = preferences.window.dimensions();
        let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
        let opened = cx.open_window(
            WindowOptions {
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Script Hub".into()),
                    ..Default::default()
                }),
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(750.), px(540.))),
                ..Default::default()
            },
            move |window, cx| {
                let root = cx.new(|cx| {
                    let mut hub =
                        Hub::new(preferences, preference_path, preferences_ready, message, cx);
                    hub.main_focus.focus(window);
                    cx.subscribe_in(
                        &hub.subscriptions,
                        window,
                        |hub, _, _: &subscriptions::QuitAfterLibraryReview, window, cx| {
                            if hub.can_close(cx) {
                                window.remove_window();
                            }
                        },
                    )
                    .detach();
                    cx.observe_window_appearance(window, |_, _, cx| cx.notify())
                        .detach();
                    if let Some((sources, public, backup)) = prepare {
                        hub.page = Page::Templates;
                        hub.templates.update(cx, |templates, cx| {
                            templates.prepare(sources, public, backup, cx)
                        });
                    } else {
                        hub.load_project(root, cx);
                    }
                    hub
                });
                let weak = root.downgrade();
                window.on_window_should_close(cx, move |_, cx| {
                    weak.update(cx, |hub, cx| hub.can_close(cx)).unwrap_or(true)
                });
                root
            },
        );
        if let Err(error) = opened {
            eprintln!("Cannot start Script Hub: {error}");
            exit.set(ExitCode::FAILURE);
            cx.quit();
        } else {
            cx.activate(true);
        }
    });
    result.get()
}
