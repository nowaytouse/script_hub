//! Offline help, build information and bundled legal documents.

use super::{Palette, button, heading, input::TextInput, localized, panel};
use crate::desktop::{
    i18n::{Text, text},
    support::{
        DEPENDENCIES, Diagnostics, LICENSE, NOTICE_RESULT_LIMIT, REPOSITORY, build_details,
        build_source_url, localized_documents, matching_notices,
    },
};
use crate::runtime::settings::{Language, Preferences};
use gpui::{
    AppContext as _, ClipboardItem, Context, Div, ElementId, Entity, FontWeight, SharedString,
    Stateful, Window, div, prelude::*, px,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Help,
    About,
    Privacy,
    Licenses,
}

impl Tab {
    fn label(self) -> Text {
        match self {
            Self::Help => Text::Help,
            Self::About => Text::About,
            Self::Privacy => Text::Privacy,
            Self::Licenses => Text::LicensesAndNotices,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum License {
    Project,
    Dependency(usize),
}

pub(super) struct Support {
    language: Language,
    tab: Tab,
    search: Entity<TextInput>,
    diagnostics: Diagnostics,
    open_license: Option<License>,
    copied: bool,
}

impl Support {
    pub(super) fn new(language: Language, cx: &mut Context<Self>) -> Self {
        let search =
            cx.new(|cx| TextInput::new(text(language, Text::SearchDependencyLicenses), false, cx));
        cx.observe(&search, |view, _, cx| {
            if matches!(view.open_license, Some(License::Dependency(_))) {
                view.open_license = None;
            }
            cx.notify();
        })
        .detach();
        Self {
            language,
            tab: Tab::Help,
            search,
            diagnostics: Diagnostics::new(None, false),
            open_license: None,
            copied: false,
        }
    }

    pub(super) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        if self.language != language {
            self.language = language;
            self.search.update(cx, |input, cx| {
                input.set_placeholder(text(language, Text::SearchDependencyLicenses), cx);
            });
            cx.notify();
        }
    }

    pub(super) fn set_diagnostics(
        &mut self,
        saved: Option<&Preferences>,
        checker_selected: bool,
        cx: &mut Context<Self>,
    ) {
        let diagnostics = Diagnostics::new(saved, checker_selected);
        if self.diagnostics != diagnostics {
            self.diagnostics = diagnostics;
            cx.notify();
        }
    }

    fn tr(&self, key: Text) -> &'static str {
        text(self.language, key)
    }

    fn open_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if self.tab != tab {
            self.tab = tab;
            self.copied = false;
            self.open_license = None;
            cx.notify();
        }
    }

    fn help(&self, palette: Palette) -> Div {
        let topics = [
            (Text::Workspace, self.tr(Text::HelpWorkspace)),
            (Text::Tasks, self.tr(Text::HelpUpdates)),
            (Text::Sources, self.tr(Text::HelpSources)),
            (Text::Repository, self.tr(Text::HelpGit)),
            (Text::Privacy, self.tr(Text::HelpPrivacy)),
            (Text::RecordHistory, self.tr(Text::RecordHistoryHelp)),
            (
                Text::KeyboardShortcuts,
                localized(
                    self.language,
                    "Press Ctrl+K on Windows or Command+K on macOS to search pages. Use Tab and Shift+Tab to move between controls.",
                    "在 Windows 按 Ctrl+K，在 macOS 按 Command+K 搜索页面。用 Tab 和 Shift+Tab 在控件间切换。",
                    "Windows では Ctrl+K、macOS では Command+K でページを検索します。Tab と Shift+Tab で操作項目を移動します。",
                ),
            ),
        ];
        let documentation = if self.language.resolved() == Language::ZhCn {
            format!("{REPOSITORY}/blob/master/README.zh-CN.md")
        } else {
            format!("{REPOSITORY}#readme")
        };
        column()
            .child(
                row()
                    .child(external(
                        "support-docs",
                        self.tr(Text::Documentation),
                        documentation,
                        palette,
                    ))
                    .child(external(
                        "support-issues",
                        self.tr(Text::IssueTracker),
                        format!("{REPOSITORY}/issues"),
                        palette,
                    )),
            )
            .child(
                panel(palette)
                    .p_5()
                    .gap_2()
                    .child(label(self.tr(Text::Relay)))
                    .child(description(self.tr(Text::HelpRelay), palette)),
            )
            .children(topics.into_iter().map(|(title, body)| {
                panel(palette)
                    .p_4()
                    .gap_1()
                    .child(label(self.tr(title)))
                    .child(description(body, palette))
            }))
    }

    fn about(&self, palette: Palette, cx: &Context<Self>) -> Div {
        let mode = if cfg!(debug_assertions) {
            Text::DevelopmentBuild
        } else {
            Text::ReleaseBuild
        };
        let entries = [
            (Text::Version, env!("CARGO_PKG_VERSION").to_owned()),
            (Text::License, env!("CARGO_PKG_LICENSE").to_owned()),
            (
                Text::BuildTarget,
                format!("{} / {}", std::env::consts::OS, std::env::consts::ARCH),
            ),
            (Text::BuildMode, self.tr(mode).to_owned()),
        ];
        let build = panel(palette)
            .p_5()
            .gap_4()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Script Hub"),
            )
            .child(description(self.tr(Text::AboutDescription), palette))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(entries.into_iter().map(|(name, value)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .min_w(px(128.))
                            .p_3()
                            .rounded_md()
                            .bg(palette.subtle)
                            .child(description(self.tr(name), palette))
                            .child(label(value))
                    })),
            )
            .child(description(self.tr(Text::LicenseScope), palette))
            .child(
                row()
                    .child(
                        button(
                            "support-copy-build",
                            self.tr(Text::CopyBuildDetails),
                            palette,
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.copy(build_details(), cx))),
                    )
                    .child(
                        button(
                            "support-open-licenses",
                            self.tr(Text::LicensesAndNotices),
                            palette,
                            false,
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.open_tab(Tab::Licenses, cx))),
                    )
                    .child(external(
                        "support-source",
                        self.tr(Text::SourceCode),
                        build_source_url(),
                        palette,
                    )),
            );
        column()
            .child(build)
            .child(self.diagnostics_card(palette, cx))
    }

    fn diagnostics_card(&self, palette: Palette, cx: &Context<Self>) -> Div {
        let statuses = [
            (Text::History, self.diagnostics.history()),
            (Text::DiagnosticLogs, self.diagnostics.logs()),
        ];
        panel(palette)
            .p_5()
            .gap_3()
            .child(label(self.tr(Text::DataAndDiagnostics)))
            .children(statuses.into_iter().map(|(key, value)| {
                row()
                    .justify_between()
                    .child(description(self.tr(key), palette))
                    .child(label(self.tr(value)))
            }))
            .child(description(self.tr(self.diagnostics.checker()), palette))
            .child(description(self.tr(Text::SupportDiagnosticsSafe), palette))
            .child(
                row().child(
                    button(
                        "support-copy-diagnostics",
                        self.tr(Text::SupportCopyDiagnostics),
                        palette,
                        false,
                    )
                    .on_click(
                        cx.listener(|view, _, _, cx| view.copy(view.diagnostics.summary(), cx)),
                    ),
                ),
            )
    }

    fn privacy(&self, palette: Palette) -> Div {
        let (_, policy) = localized_documents(self.language);
        let filename = if self.language.resolved() == Language::ZhCn {
            "PRIVACY.zh-CN.md"
        } else {
            "PRIVACY.md"
        };
        panel(palette)
            .p_5()
            .gap_3()
            .child(document(policy, true, palette))
            .child(row().child(external(
                "support-privacy-source",
                self.tr(Text::SourceCode),
                format!("{REPOSITORY}/blob/master/{filename}"),
                palette,
            )))
    }

    fn licenses(&self, palette: Palette, cx: &Context<Self>) -> Div {
        let (notices, _) = localized_documents(self.language);
        let query: String = self.search.read(cx).value().chars().take(96).collect();
        let words: Vec<_> = query
            .split_whitespace()
            .map(str::to_ascii_lowercase)
            .collect();
        let entries: Vec<_> = if words.is_empty() {
            Vec::new()
        } else {
            matching_notices(DEPENDENCIES, &words)
                .take(NOTICE_RESULT_LIMIT + 1)
                .collect()
        };
        let inventory_url = if self.language.resolved() == Language::ZhCn {
            format!("{REPOSITORY}/blob/master/rulesets/Sources/licenses/README.zh-CN.md")
        } else {
            format!("{REPOSITORY}/tree/master/rulesets/Sources/licenses")
        };
        column()
            .child(
                panel(palette)
                    .p_5()
                    .gap_3()
                    .child(description(self.tr(Text::LicenseScope), palette))
                    .child(self.license_button(
                        "support-project-license",
                        self.tr(Text::ProjectLicense),
                        License::Project,
                        palette,
                        cx,
                    ))
                    .when(self.open_license == Some(License::Project), |section| {
                        section.child(license_text(LICENSE, palette))
                    }),
            )
            .child(
                panel(palette)
                    .p_5()
                    .gap_3()
                    .child(label(self.tr(Text::DependencyLicenses)))
                    .child(description(self.tr(Text::DependencyLicenseHelp), palette))
                    .child(self.search.clone())
                    .when(!words.is_empty() && entries.is_empty(), |section| {
                        section.child(description(
                            self.tr(Text::NoDependencyLicenseMatches),
                            palette,
                        ))
                    })
                    .children(entries.iter().take(NOTICE_RESULT_LIMIT).map(
                        |&(index, name, crates, body)| {
                            let license = License::Dependency(index);
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .min_w_0()
                                .child(self.license_button(
                                    ("support-dependency", index),
                                    name,
                                    license,
                                    palette,
                                    cx,
                                ))
                                .child(description(crates, palette))
                                .when(self.open_license == Some(license), |entry| {
                                    entry.child(license_text(body, palette))
                                })
                        },
                    ))
                    .when(entries.len() > NOTICE_RESULT_LIMIT, |section| {
                        section.child(description(
                            self.tr(Text::RefineDependencyLicenseSearch),
                            palette,
                        ))
                    })
                    .child(
                        row().child(
                            button(
                                "support-copy-notices",
                                self.tr(Text::CopyLicense),
                                palette,
                                false,
                            )
                            .on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.copy(DEPENDENCIES.to_owned(), cx)
                                }),
                            ),
                        ),
                    ),
            )
            .child(
                panel(palette)
                    .p_5()
                    .gap_3()
                    .child(document(notices, false, palette))
                    .child(row().child(external(
                        "support-notice-sources",
                        self.tr(Text::LicensesAndNotices),
                        inventory_url,
                        palette,
                    ))),
            )
    }

    fn copy(&mut self, content: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(content));
        self.copied = true;
        cx.notify();
    }

    fn license_button(
        &self,
        id: impl Into<ElementId>,
        label: &str,
        license: License,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Div {
        let expanded = self.open_license == Some(license);
        let marker = if expanded { "▾" } else { "▸" };
        row().child(
            button(id, format!("{marker}  {label}"), palette, expanded).on_click(cx.listener(
                move |view, _, _, cx| {
                    view.open_license = (view.open_license != Some(license)).then_some(license);
                    cx.notify();
                },
            )),
        )
    }
}

impl Render for Support {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::for_window(window, cx);
        let intro = if self.tab == Tab::Help {
            self.tr(Text::HelpIntro)
        } else {
            ""
        };
        let tabs = [Tab::Help, Tab::About, Tab::Privacy, Tab::Licenses];
        let content = match self.tab {
            Tab::Help => self.help(palette),
            Tab::About => self.about(palette, cx),
            Tab::Privacy => self.privacy(palette),
            Tab::Licenses => self.licenses(palette, cx),
        };
        column()
            .max_w(px(1040.))
            .child(heading(self.tr(self.tab.label()), intro, palette))
            .child(
                row().children(tabs.into_iter().enumerate().map(|(index, tab)| {
                    button(
                        ("support-tab", index),
                        self.tr(tab.label()),
                        palette,
                        self.tab == tab,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| view.open_tab(tab, cx)))
                })),
            )
            .when(self.copied, |page| {
                page.child(
                    div()
                        .text_sm()
                        .text_color(palette.accent)
                        .child(self.tr(Text::Copied)),
                )
            })
            .child(content)
    }
}

fn external(id: impl Into<ElementId>, name: &str, url: String, palette: Palette) -> Stateful<Div> {
    button(id, name.to_owned(), palette, false).on_click(move |_, _, cx| cx.open_url(&url))
}

fn column() -> Div {
    div().flex().flex_col().w_full().min_w_0().gap_4()
}

fn row() -> Div {
    div().flex().items_center().flex_wrap().gap_2()
}

fn label(content: impl Into<SharedString>) -> Div {
    div()
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .child(content.into())
}

fn description(content: impl Into<SharedString>, palette: Palette) -> Div {
    div()
        .min_w_0()
        .text_sm()
        .line_height(px(21.))
        .text_color(palette.muted)
        .child(content.into())
}

fn license_text(content: &'static str, palette: Palette) -> Div {
    div()
        .min_w_0()
        .text_sm()
        .rounded_md()
        .p_3()
        .bg(palette.subtle)
        .child(content)
}

fn document(content: &'static str, skip_title: bool, palette: Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .gap_3()
        .children(content.lines().filter_map(|line| {
            let line = line.trim();
            if let Some(heading) = line.strip_prefix("# ").or_else(|| line.strip_prefix("## ")) {
                return (!skip_title || !line.starts_with("# ")).then(|| label(heading));
            }
            if line.is_empty() || line.starts_with("[English](") {
                return None;
            }
            Some(description(readable_markdown(line), palette))
        }))
}

fn readable_markdown(mut line: &str) -> String {
    // These offline Markdown documents use simple, inline [label](destination) links.
    let mut plain = String::with_capacity(line.len());
    while let Some(start) = line.find('[') {
        let Some((label, target)) = line[start + 1..].split_once("](") else {
            break;
        };
        let Some((_, remaining)) = target.split_once(')') else {
            break;
        };
        plain.push_str(&line[..start]);
        plain.push_str(label);
        line = remaining;
    }
    plain.push_str(line);
    plain.replace('`', "")
}
