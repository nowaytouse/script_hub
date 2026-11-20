use super::super::{Palette, localized};
use super::view::{column, description, row, section};
use super::{Preview, Subscriptions};
use crate::desktop::i18n::{Text, text};
use crate::relay::model::{ModelError, Subscription};
use gpui::{Context, Div, div, prelude::*, px};
use std::{ops::Range, sync::Arc};
use zeroize::Zeroizing;

const PAGE_SIZE: usize = 24;

pub(super) struct NodeDetail {
    name: Zeroizing<String>,
    endpoint: Zeroizing<String>,
}

#[derive(Default)]
pub(super) enum Details {
    #[default]
    Hidden,
    Loading(Arc<()>),
    Revealed {
        nodes: Vec<NodeDetail>,
        page: usize,
    },
}

impl Details {
    pub(super) fn is_hidden(&self) -> bool {
        matches!(self, Self::Hidden)
    }
}

fn endpoint(server: &str, port: u16) -> String {
    if server.contains(':') && !server.starts_with('[') {
        format!("[{server}]:{port}")
    } else {
        format!("{server}:{port}")
    }
}

fn prepared_nodes(content: &str) -> Result<Vec<NodeDetail>, ModelError> {
    let subscription = Subscription::parse(content)?;
    Ok(subscription
        .nodes()
        .iter()
        .map(|node| NodeDetail {
            name: Zeroizing::new(node.name().to_owned()),
            endpoint: Zeroizing::new(endpoint(node.server(), node.port())),
        })
        .collect())
}

fn page_bounds(count: usize, page: usize) -> Range<usize> {
    let last_page = count.div_ceil(PAGE_SIZE).saturating_sub(1);
    let start = page.min(last_page) * PAGE_SIZE;
    start..count.min(start.saturating_add(PAGE_SIZE))
}

impl Subscriptions {
    pub(super) fn toggle_preview_details(&mut self, cx: &mut Context<Self>) {
        if self.current_preview(cx).is_none() {
            return;
        }
        let Some(preview) = self.preview.as_mut() else {
            return;
        };
        if !preview.details.is_hidden() {
            preview.details = Details::Hidden;
            cx.notify();
            return;
        }
        if preview.result.content.is_empty() {
            return;
        }
        let content = preview.result.content.clone();
        let request = Arc::new(());
        preview.details = Details::Loading(request.clone());
        let task = cx
            .background_executor()
            .spawn(async move { prepared_nodes(&content) });
        cx.spawn(async move |weak, cx| {
            let details = task.await;
            let _ = weak.update(cx, |view, cx| {
                let active_request = view.current_preview(cx).is_some_and(|preview| {
                    matches!(&preview.details, Details::Loading(active) if Arc::ptr_eq(active, &request))
                });
                if !active_request {
                    return;
                }
                match details {
                    Ok(nodes) => {
                        if let Some(preview) = view.preview.as_mut() {
                            preview.details = Details::Revealed { nodes, page: 0 };
                        }
                        cx.notify();
                    }
                    Err(_) => {
                        if let Some(preview) = view.preview.as_mut() {
                            preview.details = Details::Hidden;
                        }
                        view.feedback(false, localized(
                            view.language,
                            "Could not read the prepared node details.",
                            "无法读取处理后节点的详情。",
                            "処理後のノード詳細を読み込めませんでした。",
                        ), cx);
                    }
                }
            });
        }).detach();
        cx.notify();
    }

    fn set_preview_details_page(&mut self, page: usize, cx: &mut Context<Self>) {
        if let Some(preview) = self.preview.as_mut()
            && let Details::Revealed {
                nodes,
                page: current,
            } = &mut preview.details
        {
            *current = page.min(nodes.len().div_ceil(PAGE_SIZE).saturating_sub(1));
            cx.notify();
        }
    }

    pub(super) fn preview_details_panel(
        &self,
        preview: &Preview,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let (nodes, page) = match &preview.details {
            Details::Hidden => return None,
            Details::Loading(_) => {
                return Some(description(
                    localized(
                        self.language,
                        "Reading prepared node details…",
                        "正在读取处理后节点的详情…",
                        "処理後のノード詳細を読み込み中…",
                    ),
                    palette,
                ));
            }
            Details::Revealed { nodes, page } => (nodes, *page),
        };
        let count = nodes.len();
        let pages = count.div_ceil(PAGE_SIZE);
        let details = column()
            .gap(px(8.))
            .child(section(
                text(self.language, Text::RelayProcessedNames),
                palette,
            ))
            .child(description(
                format!(
                    "{} · {count}",
                    text(self.language, Text::RelayOutputNodeCount)
                ),
                palette,
            ))
            .children(nodes[page_bounds(count, page)].iter().map(|node| {
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .gap(px(3.))
                    .p_2()
                    .rounded_sm()
                    .bg(palette.subtle)
                    .child(section(node.name.to_string(), palette))
                    .child(description(node.endpoint.to_string(), palette))
            }))
            .when(pages > 1, |body| {
                body.child(
                    row()
                        .when(page > 0, |row| {
                            row.child(self.action(
                                "prepared-nodes-previous",
                                localized(self.language, "Previous", "上一页", "前へ"),
                                palette,
                                false,
                                cx,
                                move |view, cx| {
                                    view.set_preview_details_page(page.saturating_sub(1), cx)
                                },
                            ))
                        })
                        .child(description(format!("{} / {pages}", page + 1), palette))
                        .when(page + 1 < pages, |row| {
                            row.child(self.action(
                                "prepared-nodes-next",
                                localized(self.language, "Next", "下一页", "次へ"),
                                palette,
                                false,
                                cx,
                                move |view, cx| view.set_preview_details_page(page + 1, cx),
                            ))
                        }),
                )
            });
        Some(details)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_details_show_ipv6_ports_without_connection_secrets() {
        let nodes = prepared_nodes(r#"[
            {"type":"shadowsocks","tag":"Synthetic IPv6","server":"2001:db8::1","server_port":8443,"method":"aes-128-gcm","password":"test-only"},
            {"type":"socks","tag":"Synthetic DNS","server":"example.invalid","server_port":1080}
        ]"#).unwrap();
        assert_eq!(nodes[0].name.as_str(), "Synthetic IPv6");
        assert_eq!(nodes[0].endpoint.as_str(), "[2001:db8::1]:8443");
        assert_eq!(nodes[1].endpoint.as_str(), "example.invalid:1080");
        assert_eq!(endpoint("[2001:db8::2]", 443), "[2001:db8::2]:443");
    }

    #[test]
    fn page_bounds_reach_all_rows_and_clamp_invalid_pages() {
        assert_eq!(page_bounds(0, usize::MAX), 0..0);
        assert_eq!(page_bounds(PAGE_SIZE, 1), 0..PAGE_SIZE);
        assert_eq!(page_bounds(PAGE_SIZE * 2 + 1, 1), PAGE_SIZE..PAGE_SIZE * 2);
        assert_eq!(
            page_bounds(PAGE_SIZE * 2 + 1, usize::MAX),
            PAGE_SIZE * 2..PAGE_SIZE * 2 + 1
        );
    }
}
