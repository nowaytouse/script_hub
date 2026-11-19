//! Native window and navigation for the Script Hub desktop application.

mod catalog;
mod input;
mod subscriptions;
mod templates;
mod workspace;

use crate::gui::i18n::{Text, text};
use crate::runtime::catalog::{Catalog, ModulePlatform};
use crate::runtime::job::{Job, JobKind};
use crate::runtime::repository::RepositoryStatus;
use crate::runtime::settings::{self, Accent, Language, Preferences, Theme};
use gpui::{
    App, Application, Bounds, ClipboardItem, Context, Div, ElementId, Entity, FontWeight, Global,
    Image, ImageFormat, PathPromptOptions, Rgba, SharedString, Stateful, Window, WindowAppearance,
    WindowBounds, WindowOptions, div, img, prelude::*, px, rgb, size,
};
use input::TextInput;
use std::cell::Cell;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::rc::Rc;
use std::sync::Arc;
use subscriptions::Subscriptions;
use templates::Templates;

gpui::actions!(script_hub, [NextControl, PreviousControl]);

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
    Modules,
    Rules,
    Templates,
    Subscriptions,
    Relay,
    Workspace,
    Tasks,
    Settings,
}

impl Page {
    fn label(self, language: Language) -> &'static str {
        match self {
            Self::Modules => text(language, Text::Modules),
            Self::Rules => localized(language, "Rule sets", "规则集", "ルールセット"),
            Self::Templates => text(language, Text::Templates),
            Self::Subscriptions => {
                localized(language, "Subscriptions", "订阅", "サブスクリプション")
            }
            Self::Relay => localized(language, "Relay", "中转", "リレー"),
            Self::Workspace => text(language, Text::Workspace),
            Self::Tasks => text(language, Text::Tasks),
            Self::Settings => text(language, Text::Settings),
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Modules => "◫",
            Self::Rules => "≡",
            Self::Templates => "▤",
            Self::Subscriptions => "◎",
            Self::Relay => "⇄",
            Self::Workspace => "⌘",
            Self::Tasks => "◴",
            Self::Settings => "⚙",
        }
    }
}

struct Hub {
    page: Page,
    root: Option<PathBuf>,
    requested_root: Option<PathBuf>,
    repository: Option<RepositoryStatus>,
    loading: bool,
    preferences: Preferences,
    saved_preferences: Preferences,
    preference_path: Option<PathBuf>,
    module_platform: ModulePlatform,
    surge: Catalog,
    shadowrocket: Catalog,
    search: Entity<TextInput>,
    rule_search: Entity<TextInput>,
    rule_policy: Entity<TextInput>,
    singbox: Entity<TextInput>,
    active_category: Option<String>,
    inspect_module: Option<usize>,
    checked_modules: BTreeSet<usize>,
    checked_rules: BTreeSet<usize>,
    templates: Entity<Templates>,
    subscriptions: Entity<Subscriptions>,
    logo: Arc<Image>,
    message: Option<String>,
    job: Option<Job>,
    job_kind: JobKind,
    pending_job: Option<JobKind>,
    show_logs: bool,
    commit: Entity<TextInput>,
    selected_changes: BTreeSet<String>,
}

impl Hub {
    fn new(
        preferences: Preferences,
        preference_path: Option<PathBuf>,
        message: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let language = preferences.language;
        let search = cx.new(|cx| TextInput::new(text(language, Text::SearchModules), false, cx));
        let rule_search = cx.new(|cx| TextInput::new(text(language, Text::SearchRules), false, cx));
        let rule_policy =
            cx.new(|cx| TextInput::new(text(language, Text::RuleSetPolicyHint), false, cx));
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
        for input in [&search, &rule_search, &rule_policy, &singbox, &commit] {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        let templates = cx.new(Templates::new);
        let subscriptions = cx.new(Subscriptions::new);
        cx.subscribe(
            &subscriptions,
            |hub, _, _: &subscriptions::OpenRelay, cx| hub.navigate(Page::Relay, cx),
        )
        .detach();
        templates.update(cx, |state, cx| state.set_language(language, cx));
        subscriptions.update(cx, |state, cx| state.set_language(language, cx));
        Self {
            page: Page::Modules,
            root: None,
            requested_root: None,
            repository: None,
            loading: false,
            saved_preferences: preferences.clone(),
            preferences,
            preference_path,
            module_platform: ModulePlatform::Surge,
            surge: Catalog::bundled(ModulePlatform::Surge),
            shadowrocket: Catalog::bundled(ModulePlatform::Shadowrocket),
            search,
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
            logo: Arc::new(Image::from_bytes(
                ImageFormat::Png,
                include_bytes!("../../assets/logo.png").to_vec(),
            )),
            message,
            job: None,
            job_kind: JobKind::Validate,
            pending_job: None,
            show_logs: true,
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
            (&self.rule_search, self.tr(Text::SearchRules)),
            (&self.rule_policy, self.tr(Text::RuleSetPolicyHint)),
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
        cx.notify();
    }

    fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.pending_job = None;
        self.message = None;
        self.subscriptions
            .update(cx, |state, cx| state.show_relay(page == Page::Relay, cx));
        if page == Page::Templates && self.root.is_none() && !self.loading {
            self.find_template_store(cx);
        }
        cx.notify();
    }

    fn find_template_store(&self, cx: &mut Context<Self>) {
        let requested = self.requested_root.clone();
        let selected = requested.clone();
        let remembered = PathBuf::from(self.saved_preferences.root.as_str());
        let task = cx.background_executor().spawn(async move {
            crate::gui::app::template_storage_root(
                selected.as_deref(),
                (!remembered.as_os_str().is_empty()).then_some(remembered.as_path()),
                std::env::current_exe().ok().as_deref(),
            )
        });
        cx.spawn(async move |this, cx| {
            let root = task.await;
            let _ = this.update(cx, |hub, cx| {
                if hub.root.is_none() && hub.requested_root == requested && !hub.loading {
                    hub.templates
                        .update(cx, |state, cx| state.set_workspace(root, cx));
                }
            });
        })
        .detach();
    }

    fn working(&self) -> bool {
        self.loading || self.job.as_ref().is_some_and(Job::is_running)
    }

    fn can_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.working()
            || !self.templates.read(cx).can_close()
            || !self.subscriptions.read(cx).can_close()
        {
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
        let dirty = self.preferences != self.saved_preferences
            || self.singbox.read(cx).value().trim() != self.saved_preferences.singbox;
        if dirty {
            self.navigate(Page::Settings, cx);
            self.message = Some(
                self.local(
                    "Save or revert your settings before closing.",
                    "请保存或撤销设置后再关闭。",
                    "閉じる前に設定を保存するか取り消してください。",
                )
                .into(),
            );
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

    fn open_classic(&mut self, cx: &mut Context<Self>) {
        if !self.can_close(cx) {
            return;
        }
        let started = std::env::current_exe().and_then(|executable| {
            let mut command = Command::new(executable);
            command.arg("--classic");
            if let Some(root) = &self.root {
                command.arg("--root").arg(root);
            }
            command.spawn()
        });
        match started {
            Ok(_) => cx.quit(),
            Err(error) => {
                self.message = Some(error.to_string());
                cx.notify();
            }
        }
    }

    fn pick_workspace(&mut self, cx: &mut Context<Self>) {
        if self.working() {
            return;
        }
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |hub, cx| hub.load_workspace(path, cx));
            }
        })
        .detach();
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
                    .child(img(self.logo.clone()).size(px(30.)).flex_shrink_0())
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(17.))
                            .child("Script Hub"),
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
                            .child(self.local("Library", "资源库", "ライブラリ")),
                    )
                    .children(
                        [
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
                    .children([Page::Workspace, Page::Tasks].map(link)),
            )
            .child(div().flex_1())
            .child(link(Page::Settings))
    }

    fn header(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        let scope = self
            .root
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or(self.local("Bundled catalog", "内置目录", "内蔵カタログ"));
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
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_sm()
                    .child(
                        div()
                            .text_color(palette.muted)
                            .truncate()
                            .child(scope.to_owned()),
                    )
                    .child(div().text_color(palette.muted).child("/"))
                    .child(
                        div()
                            .text_color(palette.ink)
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.page.label(self.preferences.language)),
                    ),
            )
            .child(
                button(
                    "pick-workspace",
                    self.local("Open workspace", "打开工作区", "ワークスペースを開く"),
                    palette,
                    false,
                )
                .on_click(cx.listener(|hub, _, _, cx| hub.pick_workspace(cx))),
            )
    }
}

impl Render for Hub {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = f32::from(self.preferences.scale_percent) / 100.;
        window.set_rem_size(px(16. * scale));
        let palette = Palette::for_window(window, cx);
        let gap = if window.viewport_size().width < px(990.) {
            20.
        } else {
            32.
        };
        let content = match self.page {
            Page::Modules => self.modules_page(window, palette, cx).into_any_element(),
            Page::Rules => self.rules_page(palette, cx).into_any_element(),
            Page::Templates => self.templates.clone().into_any_element(),
            Page::Subscriptions | Page::Relay => self.subscriptions.clone().into_any_element(),
            Page::Workspace => self.workspace_page(palette, cx).into_any_element(),
            Page::Tasks => self.tasks_page(palette, cx).into_any_element(),
            Page::Settings => self.settings_page(palette, cx).into_any_element(),
        };
        div()
            .id("script-hub")
            .tab_group()
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

pub fn run(mut args: impl Iterator<Item = String>) -> ExitCode {
    let mut prepare = None;
    let root = match args.next().as_deref() {
        Some("--worker") => return crate::runtime::worker::run(args),
        Some("--classic") => return crate::gui::app::run(args),
        Some("--demo") => {
            return crate::gui::app::run(std::iter::once("--demo".to_owned()).chain(args));
        }
        Some("--prepare-templates") => {
            let Some(setup) = crate::gui::app::parse_template_setup(args) else {
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
                "Usage: script-hub-gui [--root <workspace>] | --classic [--root <workspace>]\n       script-hub-gui --demo | --prepare-templates <public.vault> <local-backup.vault> <config>..."
            );
            return ExitCode::SUCCESS;
        }
        Some(_) => return ExitCode::from(2),
        None => None,
    };
    let saved = settings::preferences_path().and_then(|path| {
        let preferences = settings::load(&path)?.unwrap_or_default();
        Ok((path, preferences))
    });
    let (preference_path, preferences, message) = match saved {
        Ok((path, preferences)) => (Some(path), preferences, None),
        Err(error) => (None, Preferences::default(), Some(error)),
    };
    let root = root.or_else(|| {
        (prepare.is_none() && preferences.reopen_workspace && !preferences.root.is_empty())
            .then(|| PathBuf::from(&preferences.root))
    });
    let result = Rc::new(Cell::new(ExitCode::SUCCESS));
    let exit = result.clone();
    Application::new().run(move |cx| {
        TextInput::bind_keys(cx);
        cx.bind_keys([
            gpui::KeyBinding::new("tab", NextControl, None),
            gpui::KeyBinding::new("shift-tab", PreviousControl, None),
        ]);
        cx.set_global(UiStyle::from(&preferences));
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
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
                    let mut hub = Hub::new(preferences, preference_path, message, cx);
                    cx.observe_window_appearance(window, |_, _, cx| cx.notify())
                        .detach();
                    if let Some(path) = root {
                        hub.load_workspace(path, cx);
                    }
                    if let Some((sources, public, backup)) = prepare {
                        hub.page = Page::Templates;
                        hub.templates.update(cx, |templates, cx| {
                            templates.prepare(sources, public, backup, cx)
                        });
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
