//! Session-only edits bound to the original node order.

use super::super::{Palette, input::TextInput, panel};
use super::view::{column, description, field, row, section};
use super::{Item, Source, SourceRevision, Subscriptions, clear, message, set};
use crate::desktop::i18n::{Text, text};
use crate::relay::model::{Subscription, editor::NodeEdit};
use gpui::{AppContext as _, Context, Div, Entity, div, prelude::*};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

const PAGE_SIZE: usize = 24;

pub(super) struct Binding {
    pub(super) revision: SourceRevision,
    pub(super) source: Zeroizing<String>,
    names: Vec<Zeroizing<String>>,
    edits: BTreeMap<usize, NodeEdit>,
}

impl Binding {
    fn new(
        revision: SourceRevision,
        source: Zeroizing<String>,
        edits: Vec<NodeEdit>,
    ) -> Result<Self, String> {
        let names = Subscription::parse(&source)
            .map_err(message)?
            .nodes()
            .iter()
            .map(|node| Zeroizing::new(node.name().to_owned()))
            .collect::<Vec<_>>();
        let mut by_index = BTreeMap::new();
        for edit in edits {
            if edit.index >= names.len() || by_index.insert(edit.index, edit).is_some() {
                return Err(message(crate::relay::model::ModelError::InvalidEdit));
            }
        }
        Ok(Self {
            revision,
            source,
            names,
            edits: by_index,
        })
    }

    pub(super) fn edits(&self) -> Vec<NodeEdit> {
        self.edits.values().cloned().collect()
    }

    fn name(&self, index: usize) -> &str {
        self.edits
            .get(&index)
            .and_then(|edit| edit.name.as_deref())
            .unwrap_or(self.names[index].as_str())
    }

    fn included(&self, index: usize) -> bool {
        self.edits.get(&index).is_none_or(|edit| edit.included)
    }

    fn change(&mut self, index: usize, included: bool, name: Option<String>) {
        let Some(original) = self.names.get(index) else {
            return;
        };
        let name = name.filter(|value| value != original.as_str());
        if included && name.is_none() {
            self.edits.remove(&index);
        } else {
            self.edits.insert(
                index,
                NodeEdit {
                    index,
                    included,
                    name,
                },
            );
        }
    }
}

pub(super) struct Editor {
    pub(super) binding: Option<Binding>,
    pub(super) open: bool,
    query: Entity<TextInput>,
    replacement: Entity<TextInput>,
    renaming: Option<usize>,
    page: usize,
}

impl Editor {
    pub(super) fn new(cx: &mut Context<Subscriptions>) -> Self {
        let query = cx.new(|cx| TextInput::new("", false, cx));
        let replacement = cx.new(|cx| TextInput::new("", false, cx));
        cx.observe(&query, |view, _, cx| {
            view.node_editor.page = 0;
            cx.notify();
        })
        .detach();
        Self {
            binding: None,
            open: false,
            query,
            replacement,
            renaming: None,
            page: 0,
        }
    }

    pub(super) fn hide(&mut self, cx: &mut Context<Subscriptions>) {
        if self.open {
            self.open = false;
            self.renaming = None;
            self.page = 0;
            clear(&self.query, cx);
            clear(&self.replacement, cx);
        }
    }

    pub(super) fn clear(&mut self, cx: &mut Context<Subscriptions>) {
        self.hide(cx);
        self.binding = None;
    }
}

impl Subscriptions {
    fn node_source(
        &self,
        cx: &Context<Self>,
    ) -> Option<(Zeroizing<String>, SourceRevision, Vec<NodeEdit>)> {
        if let Some(preview) = self.current_preview(cx) {
            let config = &preview.config.0;
            let content = config.local_content.clone().or_else(|| {
                preview
                    .result
                    .source
                    .as_ref()
                    .map(|source| Zeroizing::new(source.content().to_owned()))
            });
            if let Some(content) = content {
                return Some((
                    content,
                    preview.source_revision.clone(),
                    config.processing.nodes.clone(),
                ));
            }
        }
        let edits = match self.source {
            Source::Saved(Item::Entry(id)) => self.entry(id)?.processing.nodes.clone(),
            Source::Saved(Item::Collection(id)) => self.collection(id)?.processing.nodes.clone(),
            _ => Vec::new(),
        };
        if let Some(review) = self.reviewed_source.as_ref().filter(|review| {
            review.revision.matches(
                self.relay_url.read(cx).value(),
                self.relay_file.read(cx).value(),
            )
        }) {
            return Some((review.content.clone(), review.revision.clone(), edits));
        }
        let content = match &self.source {
            Source::Local(content) => content.clone(),
            Source::Saved(Item::Entry(id)) => {
                let entry = self.entry(*id)?;
                if entry.content.is_empty() {
                    return None;
                }
                Zeroizing::new(entry.content.clone())
            }
            _ => return None,
        };
        Some((content, SourceRevision::Bound, edits))
    }

    pub(super) fn edit_source_nodes(&mut self, cx: &mut Context<Self>) {
        if self.node_editor.binding.is_none() {
            let Some((content, revision, edits)) = self.node_source(cx) else {
                self.preview_inner(true, None, cx);
                return;
            };
            match Binding::new(revision, content, edits) {
                Ok(binding) => self.node_editor.binding = Some(binding),
                Err(error) => {
                    self.feedback(false, error, cx);
                    return;
                }
            }
        }
        self.invalidate_preview(cx);
        self.node_editor.open = true;
        self.notice = None;
        cx.notify();
    }

    fn toggle_input_node(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(binding) = self.node_editor.binding.as_mut() {
            let name = binding.edits.get(&index).and_then(|edit| edit.name.clone());
            binding.change(index, !binding.included(index), name);
            cx.notify();
        }
    }

    fn save_input_node_name(&mut self, index: usize, cx: &mut Context<Self>) {
        let value = self.node_editor.replacement.read(cx).value();
        if value.trim().is_empty() || value.len() > 512 || value.chars().any(char::is_control) {
            self.feedback(false, text(self.language, Text::RelayEditInvalid), cx);
            return;
        }
        if let Some(binding) = self.node_editor.binding.as_mut() {
            binding.change(index, binding.included(index), Some(value.to_owned()));
        }
        self.node_editor.renaming = None;
        clear(&self.node_editor.replacement, cx);
        self.notice = None;
        cx.notify();
    }

    pub(super) fn node_review_panel(&self, palette: Palette, cx: &Context<Self>) -> Option<Div> {
        let editor = &self.node_editor;
        let binding = editor.binding.as_ref().filter(|_| editor.open)?;
        let tr = |key| text(self.language, key);
        let query = Zeroizing::new(editor.query.read(cx).value().to_lowercase());
        let matches = binding
            .names
            .iter()
            .enumerate()
            .filter_map(|(index, original)| {
                (query.is_empty()
                    || Zeroizing::new(original.to_lowercase()).contains(query.as_str())
                    || Zeroizing::new(binding.name(index).to_lowercase()).contains(query.as_str()))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        let pages = matches.len().div_ceil(PAGE_SIZE).max(1);
        let page = editor.page.min(pages - 1);
        let matches_count = matches.len();
        let actions = row()
            .child(self.action(
                "close-input-nodes",
                self.tr("Back to preview", "返回预览"),
                palette,
                true,
                cx,
                |view, cx| {
                    view.node_editor.hide(cx);
                    view.notice = None;
                    cx.notify();
                },
            ))
            .when(!binding.edits.is_empty(), |actions| {
                actions.child(self.action(
                    "reset-input-nodes",
                    tr(Text::RelayResetNodeEdits),
                    palette,
                    false,
                    cx,
                    |view, cx| {
                        if let Some(binding) = view.node_editor.binding.as_mut() {
                            binding.edits.clear();
                        }
                        view.node_editor.renaming = None;
                        clear(&view.node_editor.replacement, cx);
                        cx.notify();
                    },
                ))
            });
        let mut card = panel(palette).w_full().p_5().gap_3()
            .child(section(tr(Text::RelayReviewNodes), palette))
            .child(description(self.tr(
                "Edits apply to this relay session. Run a fresh preview to export or publish the result.",
                "编辑仅用于本次中转。重新运行预览后可导出或发布。",
            ), palette))
            .child(actions)
            .child(field(self.tr("Search original and edited names", "搜索原始或修改后的名称"), &editor.query, palette))
            .child(description(format!("{matches_count} / {}", binding.names.len()), palette));

        if let Some(index) = editor.renaming {
            card = card.child(
                column()
                    .gap_2()
                    .child(field(
                        self.tr("New name", "新名称"),
                        &editor.replacement,
                        palette,
                    ))
                    .child(
                        row()
                            .child(self.action(
                                "save-input-name",
                                self.tr("Apply name", "应用名称"),
                                palette,
                                true,
                                cx,
                                move |view, cx| view.save_input_node_name(index, cx),
                            ))
                            .child(self.action(
                                "cancel-input-name",
                                tr(Text::Cancel),
                                palette,
                                false,
                                cx,
                                |view, cx| {
                                    view.node_editor.renaming = None;
                                    clear(&view.node_editor.replacement, cx);
                                    cx.notify();
                                },
                            )),
                    ),
            );
        }
        card = card.children(
            matches
                .into_iter()
                .skip(PAGE_SIZE * page)
                .take(PAGE_SIZE)
                .map(|index| {
                    let included = binding.included(index);
                    let name = binding.name(index);
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .bg(palette.subtle)
                        .child(
                            div()
                                .min_w_0()
                                .max_w_full()
                                .overflow_hidden()
                                .child(name.to_owned()),
                        )
                        .child(
                            row()
                                .child(self.action(
                                    ("include-input-node", index),
                                    if included {
                                        self.tr("Exclude", "排除")
                                    } else {
                                        tr(Text::RelayNodeEnabled)
                                    },
                                    palette,
                                    false,
                                    cx,
                                    move |view, cx| view.toggle_input_node(index, cx),
                                ))
                                .child(self.action(
                                    ("rename-input-node", index),
                                    self.tr("Rename", "重命名"),
                                    palette,
                                    false,
                                    cx,
                                    move |view, cx| {
                                        if let Some(binding) = &view.node_editor.binding {
                                            set(
                                                &view.node_editor.replacement,
                                                binding.name(index),
                                                cx,
                                            );
                                            view.node_editor.renaming = Some(index);
                                            cx.notify();
                                        }
                                    },
                                )),
                        )
                }),
        );
        Some(card.when(pages > 1, |card| {
            card.child(
                row()
                    .when(page > 0, |row| {
                        row.child(self.action(
                            "input-nodes-prev",
                            self.tr("Previous", "上一页"),
                            palette,
                            false,
                            cx,
                            move |view, cx| {
                                view.node_editor.page = page - 1;
                                cx.notify();
                            },
                        ))
                    })
                    .child(description(format!("{} / {pages}", page + 1), palette))
                    .when(page + 1 < pages, |row| {
                        row.child(self.action(
                            "input-nodes-next",
                            self.tr("Next", "下一页"),
                            palette,
                            false,
                            cx,
                            move |view, cx| {
                                view.node_editor.page = page + 1;
                                cx.notify();
                            },
                        ))
                    }),
            )
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::model::{EnhanceOptions, Health, editor::Options};

    #[test]
    fn positional_edits_preview_against_original_node_order() {
        let source = "First = socks5, first.invalid, 1080\nSecond = socks5, second.invalid, 1080\n";
        let mut binding = Binding::new(
            SourceRevision::Bound,
            Zeroizing::new(source.to_owned()),
            vec![],
        )
        .unwrap();
        binding.change(1, false, None);
        binding.change(0, true, Some("Renamed".to_owned()));
        let mut options = Options::default();
        options.nodes.extend(binding.edits());
        let rendered = Subscription::parse(&binding.source)
            .unwrap()
            .render_processed(
                &[Health::Unknown; 2],
                &EnhanceOptions {
                    rename: false,
                    sort: false,
                },
                &options,
            )
            .unwrap();
        assert_eq!(rendered.retained, [0]);
        assert_eq!(
            Subscription::parse(&rendered.content).unwrap().nodes()[0].name(),
            "Renamed"
        );
        binding.change(0, true, Some("First".to_owned()));
        binding.change(1, true, None);
        assert!(binding.edits().is_empty());
    }
}
