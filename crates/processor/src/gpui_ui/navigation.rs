use super::{Hub, Page, Palette, button, heading, panel};
use crate::desktop::i18n::Text;
use crate::runtime::job::JobOutcome;
use gpui::{App, Context, Div, Focusable, FontWeight, Window, div, prelude::*};

pub(super) const COMMAND_CONTEXT: &str = "HubPageSearch";
pub(super) const HUB_CONTEXT: &str = "ScriptHub";

const PAGES: [Page; 11] = [
    Page::Overview,
    Page::Modules,
    Page::Rules,
    Page::Templates,
    Page::Subscriptions,
    Page::Relay,
    Page::Sources,
    Page::Tasks,
    Page::History,
    Page::Settings,
    Page::Help,
];

impl Hub {
    pub(super) fn toggle_page_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page_search_open {
            self.close_page_search(cx);
            self.main_focus.focus(window);
        } else {
            self.page_search_open = true;
            self.page_query.read(cx).focus_handle(cx).focus(window);
            cx.notify();
        }
    }

    pub(super) fn close_page_search(&mut self, cx: &mut Context<Self>) {
        if self.page_search_open {
            self.page_search_open = false;
            self.page_query.update(cx, super::input::TextInput::clear);
            cx.notify();
        }
    }

    fn matching_pages(&self, cx: &App) -> impl Iterator<Item = Page> + use<> {
        let query = self.page_query.read(cx).value().trim().to_lowercase();
        let language = self.preferences.language;
        PAGES.into_iter().filter(move |page| {
            query.is_empty() || page.label(language).to_lowercase().contains(&query)
        })
    }

    pub(super) fn open_first_matching_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page_search_open
            && let Some(page) = self.matching_pages(cx).next()
        {
            self.navigate(page, cx);
            self.main_focus.focus(window);
        }
    }

    pub(super) fn page_search(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        let matches = self.matching_pages(cx).collect::<Vec<_>>();
        div().px_5().py_3().child(
            panel(palette).p_4().gap_3().child(
                div()
                    .id("page-search")
                    .key_context(COMMAND_CONTEXT)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .on_action(cx.listener(|hub, _: &super::ClosePageSearch, window, cx| {
                        hub.close_page_search(cx);
                        hub.main_focus.focus(window);
                    }))
                    .on_action(
                        cx.listener(|hub, _: &super::OpenPageSearchResult, window, cx| {
                            hub.open_first_matching_page(window, cx)
                        }),
                    )
                    .child(self.page_query.clone())
                    .when(matches.is_empty(), |search| {
                        search.child(div().text_sm().text_color(palette.muted).child(self.local(
                            "No matching pages",
                            "没有匹配的页面",
                            "一致するページがありません",
                        )))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(matches.into_iter().map(|page| {
                                button(
                                    ("page-search-result", page as usize),
                                    page.label(self.preferences.language),
                                    palette,
                                    self.page == page,
                                )
                                .on_click(cx.listener(
                                    move |hub, _, window, cx| {
                                        hub.navigate(page, cx);
                                        hub.main_focus.focus(window);
                                    },
                                ))
                            })),
                    ),
            ),
        )
    }

    pub(super) fn overview_page(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        let title = |label: &'static str| {
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(label)
        };
        let language = self.preferences.language;
        let task_status = self.job.as_ref().map_or(self.tr(Text::NoTask), |job| {
            let view = job.summary();
            self.tr(if view.running {
                Text::Running
            } else {
                match view.outcome {
                    Some(JobOutcome::Success) => Text::Succeeded,
                    Some(JobOutcome::Warnings) => Text::Warnings,
                    _ => Text::Failed,
                }
            })
        });
        let actions = |pages: &[Page], cx: &mut Context<Self>| {
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(pages.iter().copied().map(|page| {
                    button(
                        ("overview-open", page as usize),
                        page.label(language),
                        palette,
                        false,
                    )
                    .on_click(cx.listener(move |hub, _, _, cx| hub.navigate(page, cx)))
                }))
        };
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(heading(
                self.tr(Text::Overview),
                self.tr(Text::OverviewDescription),
                palette,
            ))
            .child(
                panel(palette)
                    .p_5()
                    .gap_3()
                    .child(title(self.tr(Text::QuickStart)))
                    .child(actions(
                        &[
                            Page::Modules,
                            Page::Rules,
                            Page::Templates,
                            Page::Subscriptions,
                            Page::Relay,
                        ],
                        cx,
                    ))
                    .child(div().text_sm().text_color(palette.muted).child(format!(
                        "{} {}   ·   {} {}",
                        self.surge.modules.len(),
                        self.tr(Text::ModuleCount),
                        self.surge.rules.len(),
                        self.tr(Text::RuleCount),
                    ))),
            )
            .child(
                panel(palette)
                    .p_5()
                    .gap_3()
                    .child(title(self.tr(Text::LatestActivity)))
                    .child(div().text_sm().text_color(palette.muted).child(task_status))
                    .child(actions(&[Page::Tasks, Page::History], cx)),
            )
    }
}
