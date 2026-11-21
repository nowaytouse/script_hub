use super::{Hub, Palette, button, heading, panel};
use crate::desktop::i18n::Text;
use crate::runtime::catalog::{
    Catalog, ModuleItem, ModulePlatform, selected_module_urls, selected_rule_urls,
    selected_surge_rule_set_snippets,
};
use gpui::{Context, Div, FontWeight, Window, div, prelude::*, px};
use std::collections::BTreeSet;

impl Hub {
    fn category_label<'a>(&self, category: &'a str) -> &'a str {
        match category {
            "amplify_nexus" => self.local("App enhancements", "应用增强", "アプリ拡張"),
            "head_expanse" => self.local(
                "Network and filtering",
                "网络与过滤",
                "ネットワークとフィルタリング",
            ),
            _ => category,
        }
    }

    fn module_catalog(&self) -> &Catalog {
        match self.module_platform {
            ModulePlatform::Surge => &self.surge,
            ModulePlatform::Shadowrocket => &self.shadowrocket,
        }
    }

    fn change_platform(&mut self, platform: ModulePlatform, cx: &mut Context<Self>) {
        self.module_platform = platform;
        self.active_category = None;
        self.inspect_module = None;
        self.checked_modules.clear();
        cx.notify();
    }

    pub(super) fn modules_page(
        &mut self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let filter = self.search.read(cx).value().trim().to_lowercase();
        let catalog = self.module_catalog();
        let categories: BTreeSet<_> = catalog
            .modules
            .iter()
            .map(|module| module.category.clone())
            .collect();
        let category = self.active_category.as_deref().unwrap_or("");
        let mut visible: Vec<_> = catalog
            .modules
            .iter()
            .enumerate()
            .filter(|(_, module)| module.matches(&filter, category))
            .map(|(index, _)| index)
            .collect();
        visible.sort_by_key(|&index| {
            !catalog.modules[index]
                .filename
                .contains("PROMAX (Kali-style)")
        });
        let inspection = self
            .inspect_module
            .filter(|index| visible.contains(index))
            .or(visible.first().copied());
        let module_count = format!("{} / {}", visible.len(), catalog.modules.len());
        let all_label = self.local("All categories", "所有类别", "すべてのカテゴリ");
        let category_filters: Vec<_> = std::iter::once((None, all_label.to_owned()))
            .chain(categories.into_iter().map(|name| {
                let label = self.category_label(&name).to_owned();
                (Some(name), label)
            }))
            .enumerate()
            .map(|(index, (choice, label))| {
                button(
                    ("category", index),
                    label,
                    palette,
                    self.active_category == choice,
                )
                .on_click(cx.listener(move |hub, _, _, cx| {
                    hub.active_category = choice.clone();
                    cx.notify();
                }))
            })
            .collect();
        let all_visible = visible
            .iter()
            .copied()
            .filter(|&index| catalog.modules[index].merged_into.is_empty())
            .collect::<Vec<_>>();
        let wide = window.viewport_size().width >= px(1050.);
        let count = self.checked_modules.len();
        let column = div().flex().flex_col().gap_3().min_w_0().flex_1().child(
            panel(palette)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .border_b_1()
                        .border_color(palette.border)
                        .text_sm()
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .child(self.tr(Text::Modules)),
                        )
                        .child(div().text_color(palette.muted).child(module_count)),
                )
                .child(
                    div()
                        .id("module-list")
                        .overflow_y_scroll()
                        .max_h(px(if wide { 485. } else { 245. }))
                        .when(visible.is_empty(), |list| {
                            list.child(div().p_5().text_color(palette.muted).child(self.local(
                                "No matching modules",
                                "找不到符合条件的模块",
                                "一致するモジュールがありません",
                            )))
                        })
                        .children(visible.iter().map(|&index| {
                            self.module_row(
                                index,
                                &catalog.modules[index],
                                inspection == Some(index),
                                palette,
                                cx,
                            )
                        })),
                ),
        );
        let detail = if let Some(index) = inspection {
            self.module_detail(index, &catalog.modules[index], palette, cx)
        } else {
            panel(palette)
                .p_5()
                .text_color(palette.muted)
                .child(self.local(
                    "Select a module to view its links.",
                    "选择模块查看订阅链接。",
                    "モジュールを選択してリンクを表示します。",
                ))
        };
        let compact_copy =
            inspection.filter(|&index| !wide && catalog.modules[index].merged_into.is_empty());
        let list_and_detail = div()
            .flex()
            .gap_4()
            .items_start()
            .when(!wide, |node| node.flex_col())
            .child(column.w_full())
            .child(
                detail
                    .w_full()
                    .when(wide, |node| node.w(px(350.)).flex_shrink_0()),
            );
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(heading(
                self.tr(Text::Modules),
                self.local(
                    "Browse published modules and copy an installation link for your client.",
                    "浏览已发布的模块，复制链接到对应客户端导入。",
                    "公開モジュールを探し、クライアント用のインストールリンクをコピーできます。",
                ),
                palette,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div().flex().items_center().flex_wrap().gap_2().children(
                            [
                                (ModulePlatform::Surge, "Surge / FlClash"),
                                (ModulePlatform::Shadowrocket, "Shadowrocket"),
                            ]
                            .map(|(platform, name)| {
                                button(name, name, palette, platform == self.module_platform)
                                    .on_click(cx.listener(move |hub, _, _, cx| {
                                        hub.change_platform(platform, cx)
                                    }))
                            }),
                        ),
                    )
                    .child(self.search.clone())
                    .child(div().flex().flex_wrap().gap_2().children(category_filters)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        div()
                            .text_sm()
                            .text_color(palette.muted)
                            .mr_1()
                            .child(format!("{} {}", self.tr(Text::Selected), count)),
                    )
                    .child(
                        button(
                            "select-visible-modules",
                            self.tr(Text::SelectVisible),
                            palette,
                            false,
                        )
                        .on_click(cx.listener(move |hub, _, _, cx| {
                            hub.checked_modules.extend(all_visible.iter().copied());
                            cx.notify();
                        })),
                    )
                    .when_some(compact_copy, |node, index| {
                        node.child(
                            button(
                                "copy-visible-module",
                                self.tr(Text::CopyGithub),
                                palette,
                                true,
                            )
                            .on_click(cx.listener(
                                move |hub, _, _, cx| {
                                    let links = selected_module_urls(
                                        &hub.module_catalog().modules,
                                        &BTreeSet::from([index]),
                                        true,
                                    );
                                    hub.copy(links, cx);
                                },
                            )),
                        )
                    })
                    .when(count != 0, |node| {
                        node.child(
                            button(
                                "clear-module-selection",
                                self.local("Clear selection", "清除选择", "選択を解除"),
                                palette,
                                false,
                            )
                            .on_click(cx.listener(|hub, _, _, cx| {
                                hub.checked_modules.clear();
                                cx.notify();
                            })),
                        )
                        .children([false, true].map(|github| {
                            button(
                                if github {
                                    "copy-modules-github"
                                } else {
                                    "copy-modules-cdn"
                                },
                                self.tr(if github {
                                    Text::CopySelectedGithub
                                } else {
                                    Text::CopySelectedCdn
                                }),
                                palette,
                                false,
                            )
                            .on_click(cx.listener(
                                move |hub, _, _, cx| {
                                    let links = selected_module_urls(
                                        &hub.module_catalog().modules,
                                        &hub.checked_modules,
                                        github,
                                    );
                                    hub.copy(links, cx);
                                },
                            ))
                        }))
                    }),
            )
            .child(list_and_detail)
    }

    fn module_row(
        &self,
        index: usize,
        module: &ModuleItem,
        active: bool,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let checked = self.checked_modules.contains(&index);
        let status = if module.merged_into.is_empty() {
            self.category_label(&module.category).to_owned()
        } else {
            format!("{} {}", self.tr(Text::MergedInto), module.merged_into)
        };
        let padding = if self.preferences.density == crate::runtime::settings::Density::Compact {
            7.
        } else {
            11.
        };
        div()
            .flex()
            .items_center()
            .gap_3()
            .border_b_1()
            .border_color(palette.border)
            .px_4()
            .py(px(padding))
            .bg(if active {
                palette.subtle
            } else {
                palette.surface
            })
            .when(module.merged_into.is_empty(), |row| {
                row.child(
                    div()
                        .id(("module-check", index))
                        .tab_index(0)
                        .focus(move |style| style.border_color(palette.accent))
                        .cursor_pointer()
                        .size(px(18.))
                        .rounded_sm()
                        .border_1()
                        .border_color(if checked {
                            palette.accent
                        } else {
                            palette.muted
                        })
                        .bg(if checked {
                            palette.accent
                        } else {
                            palette.surface
                        })
                        .text_color(palette.on_accent)
                        .text_xs()
                        .text_center()
                        .child(if checked { "✓" } else { "" })
                        .on_click(cx.listener(move |hub, _, _, cx| {
                            if !hub.checked_modules.remove(&index) {
                                hub.checked_modules.insert(index);
                            }
                            cx.notify();
                        })),
                )
            })
            .child(
                div()
                    .id(("module-row", index))
                    .tab_index(0)
                    .focus(move |style| style.text_color(palette.accent))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .min_w_0()
                    .flex_1()
                    .cursor_pointer()
                    .text_color(if active { palette.accent } else { palette.ink })
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(module.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(palette.muted)
                            .truncate()
                            .child(status),
                    )
                    .on_click(cx.listener(move |hub, _, _, cx| {
                        hub.inspect_module = Some(index);
                        cx.notify();
                    })),
            )
            .child(div().text_color(palette.muted).child("›"))
    }

    fn module_detail(
        &self,
        index: usize,
        module: &ModuleItem,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let version: Vec<_> = [&module.author, &module.date]
            .into_iter()
            .filter(|value| !value.is_empty())
            .map(|value| value.as_str())
            .collect();
        panel(palette)
            .p_5()
            .gap_4()
            .child(
                div()
                    .text_xs()
                    .text_color(palette.accent)
                    .font_weight(FontWeight::MEDIUM)
                    .child(self.category_label(&module.category).to_owned()),
            )
            .child(
                div()
                    .text_size(px(21.))
                    .line_height(px(27.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(module.name.clone()),
            )
            .when(!version.is_empty(), |node| {
                node.child(
                    div()
                        .text_sm()
                        .text_color(palette.muted)
                        .child(version.join("  ·  ")),
                )
            })
            .when_else(
                module.merged_into.is_empty(),
                |node| {
                    node.child(div().flex().gap_2().flex_wrap().children([true, false].map(
                        |github| {
                            button(
                                if github {
                                    "copy-module-github"
                                } else {
                                    "copy-module-cdn"
                                },
                                self.tr(if github {
                                    Text::CopyGithub
                                } else {
                                    Text::CopyCdn
                                }),
                                palette,
                                github,
                            )
                            .on_click(cx.listener(
                                move |hub, _, _, cx| {
                                    let links = selected_module_urls(
                                        &hub.module_catalog().modules,
                                        &BTreeSet::from([index]),
                                        github,
                                    );
                                    hub.copy(links, cx);
                                },
                            ))
                        },
                    )))
                },
                |node| {
                    node.child(div().text_sm().text_color(palette.accent).child(format!(
                        "{} {}",
                        self.tr(Text::MergedInto),
                        module.merged_into
                    )))
                },
            )
            .when(!module.desc.is_empty(), |node| {
                node.child(
                    div()
                        .border_t_1()
                        .border_color(palette.border)
                        .pt_4()
                        .text_sm()
                        .line_height(px(20.))
                        .text_color(palette.muted)
                        .child(module.desc.replace("\\n", "\n")),
                )
            })
    }

    pub(super) fn rules_page(&self, palette: Palette, cx: &mut Context<Self>) -> Div {
        let page = div().flex().flex_col().gap_5().child(heading(
            self.local("Rule sets", "规则集", "ルールセット"),
            self.local(
                "Copy public rule set links or Surge RULE-SET entries.",
                "复制公开规则集链接或 Surge RULE-SET 条目。",
                "公開ルールセットのリンクや Surge RULE-SET をコピーできます。",
            ),
            palette,
        ));
        let query = self.rule_search.read(cx).value().trim().to_lowercase();
        let visible: Vec<_> = self
            .surge
            .rules
            .iter()
            .enumerate()
            .filter(|(_, rule)| {
                rule.name.to_lowercase().contains(&query)
                    || rule.category.to_lowercase().contains(&query)
            })
            .collect();
        let shown: Vec<_> = visible.iter().map(|(index, _)| *index).collect();
        let page = page
            .child(self.rule_search.clone())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(palette.muted)
                            .child(self.tr(Text::RuleSetPolicy)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .max_w(px(360.))
                            .child(self.rule_policy.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .child(div().text_sm().text_color(palette.muted).child(format!(
                        "{} {}",
                        self.tr(Text::Selected),
                        self.checked_rules.len()
                    )))
                    .child(
                        button(
                            "select-visible-rules",
                            self.tr(Text::SelectVisible),
                            palette,
                            false,
                        )
                        .on_click(cx.listener(move |hub, _, _, cx| {
                            hub.checked_rules.extend(shown.iter().copied());
                            cx.notify();
                        })),
                    )
                    .when(!self.checked_rules.is_empty(), |actions| {
                        actions
                            .child(
                                button(
                                    "clear-rule-selection",
                                    self.local("Clear selection", "清除选择", "選択を解除"),
                                    palette,
                                    false,
                                )
                                .on_click(cx.listener(
                                    |hub, _, _, cx| {
                                        hub.checked_rules.clear();
                                        cx.notify();
                                    },
                                )),
                            )
                            .children(
                                [true, false]
                                    .into_iter()
                                    .flat_map(|github| {
                                        [true, false].map(move |snippet| (github, snippet))
                                    })
                                    .enumerate()
                                    .map(|(index, (github, snippet))| {
                                        button(
                                            ("rule-copy", index),
                                            if snippet {
                                                if github {
                                                    "RULE-SET · GitHub"
                                                } else {
                                                    "RULE-SET · CDN"
                                                }
                                            } else if github {
                                                self.tr(Text::CopySelectedGithub)
                                            } else {
                                                self.tr(Text::CopySelectedCdn)
                                            },
                                            palette,
                                            false,
                                        )
                                        .on_click(
                                            cx.listener(move |hub, _, _, cx| {
                                                let rules = &hub.surge.rules;
                                                let result = if snippet {
                                                    selected_surge_rule_set_snippets(
                                                        rules,
                                                        &hub.checked_rules,
                                                        github,
                                                        hub.rule_policy.read(cx).value(),
                                                    )
                                                } else {
                                                    Some(selected_rule_urls(
                                                        rules,
                                                        &hub.checked_rules,
                                                        github,
                                                    ))
                                                };
                                                if let Some(value) = result {
                                                    hub.copy(value, cx);
                                                } else {
                                                    hub.message = Some(
                                                        hub.tr(Text::InvalidRuleSetPolicy).into(),
                                                    );
                                                    cx.notify();
                                                }
                                            }),
                                        )
                                    }),
                            )
                    }),
            );
        page.child(
            panel(palette)
                .when(visible.is_empty(), |node| {
                    node.child(div().p_5().text_color(palette.muted).child(self.local(
                        "No matching rule sets",
                        "找不到符合条件的规则集",
                        "一致するルールセットがありません",
                    )))
                })
                .children(visible.into_iter().map(|(index, rule)| {
                    let checked = self.checked_rules.contains(&index);
                    div()
                        .id(("rule-row", index))
                        .tab_index(0)
                        .focus(move |style| style.bg(palette.subtle).text_color(palette.accent))
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .cursor_pointer()
                        .border_b_1()
                        .border_color(palette.border)
                        .bg(if checked {
                            palette.subtle
                        } else {
                            palette.surface
                        })
                        .hover(move |style| style.bg(palette.subtle))
                        .child(
                            div()
                                .w(px(20.))
                                .text_color(if checked {
                                    palette.accent
                                } else {
                                    palette.muted
                                })
                                .child(if checked { "✓" } else { "□" }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::MEDIUM)
                                .child(rule.name.clone()),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(palette.muted)
                                .child(rule.category.clone()),
                        )
                        .when_some(rule.count, |row, count| {
                            row.child(
                                div()
                                    .text_sm()
                                    .text_color(palette.muted)
                                    .child(count.to_string()),
                            )
                        })
                        .on_click(cx.listener(move |hub, _, _, cx| {
                            if !hub.checked_rules.remove(&index) {
                                hub.checked_rules.insert(index);
                            }
                            cx.notify();
                        }))
                })),
        )
    }
}
