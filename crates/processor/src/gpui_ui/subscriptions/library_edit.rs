//! Library operations shared by the entry and collection editors.

use super::super::{Palette, localized};
use super::view::{column, description, row, section};
use super::{Completed, Item, Subscriptions, message};
use crate::desktop::i18n::{Text, text};
use crate::relay::{
    library::{Error, Workspace},
    library_import::unique_name,
};
use gpui::{Context, Div, div, prelude::*, px};

fn duplicate_item(workspace: &mut Workspace, item: Item) -> Result<(), Error> {
    match item {
        Item::Entry(id) => {
            let mut duplicate = workspace
                .subscriptions
                .iter()
                .find(|entry| entry.id == id)
                .cloned()
                .ok_or(Error::NotFound)?;
            duplicate.id = 0;
            duplicate.name = unique_name(
                &duplicate.name,
                |name| {
                    workspace
                        .subscriptions
                        .iter()
                        .any(|entry| entry.name == name)
                },
                workspace.next_id,
            )
            .map_err(|_| Error::Invalid)?;
            duplicate.publish = Default::default();
            workspace.add_entry(duplicate)?;
        }
        Item::Collection(id) => {
            let mut duplicate = workspace
                .collections
                .iter()
                .find(|collection| collection.id == id)
                .cloned()
                .ok_or(Error::NotFound)?;
            duplicate.id = 0;
            duplicate.name = unique_name(
                &duplicate.name,
                |name| workspace.collections.iter().any(|entry| entry.name == name),
                workspace.next_id,
            )
            .map_err(|_| Error::Invalid)?;
            duplicate.publish = Default::default();
            workspace.add_collection(duplicate)?;
        }
    }
    Ok(())
}

fn reorder_member(members: &mut [u64], id: u64, offset: isize) {
    if let Some(from) = members.iter().position(|&member| member == id)
        && let Some(to) = from
            .checked_add_signed(offset)
            .filter(|&to| to < members.len())
    {
        members.swap(from, to);
    }
}

impl Subscriptions {
    pub(super) fn duplicate_saved(&mut self, item: Item, cx: &mut Context<Self>) {
        let Some(library) = self.library.writer() else {
            return;
        };
        if self.editor.is_some() || self.deleting.is_some() {
            return;
        }
        let mut candidate = self.workspace.clone();
        let notice = localized(
            self.language,
            "Copy saved to the encrypted library.",
            "副本已保存到加密资料库。",
            "コピーを暗号化ライブラリへ保存しました。",
        );
        self.start(
            move || {
                duplicate_item(&mut candidate, item).map_err(message)?;
                library.save(&candidate)?;
                Ok(Completed::Saved(candidate, notice))
            },
            cx,
        );
    }

    pub(super) fn collection_members_editor(&self, palette: Palette, cx: &Context<Self>) -> Div {
        let tr = |key| text(self.language, key);
        let mut body = column()
            .child(section(tr(Text::WorkspaceMembers), palette))
            .child(description(tr(Text::WorkspaceMembersHelp), palette));
        if self.workspace.subscriptions.is_empty() {
            return body.child(description(
                tr(Text::WorkspaceListNeedsSubscriptions),
                palette,
            ));
        }

        let selected = self
            .members
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(position, id)| {
                let entry = self.entry(id)?;
                let title = format!("{}. {}", position + 1, entry.name);
                let mut member = row()
                    .child(div().flex_1().min_w_0().child(title))
                    .when(entry.content.is_empty(), |member| {
                        member.child(description(tr(Text::WorkspaceNotFetched), palette))
                    });
                for (offset, key, direction, enabled) in [
                    (-1, Text::OperationMoveUp, "up", position > 0),
                    (
                        1,
                        Text::OperationMoveDown,
                        "down",
                        position + 1 < self.members.len(),
                    ),
                ] {
                    if enabled {
                        member = member.child(self.action(
                            (direction, id),
                            tr(key),
                            palette,
                            false,
                            cx,
                            move |this, cx| {
                                reorder_member(&mut this.members, id, offset);
                                cx.notify();
                            },
                        ));
                    }
                }
                Some(member.child(self.action(
                    ("exclude-collection-member", id),
                    localized(self.language, "Remove", "移除", "削除"),
                    palette,
                    false,
                    cx,
                    move |this, cx| {
                        this.members.retain(|member| *member != id);
                        cx.notify();
                    },
                )))
            });
        let other_entries = self
            .workspace
            .subscriptions
            .iter()
            .filter(|entry| !self.members.contains(&entry.id));
        body = body.child(column().gap(px(8.)).children(selected));
        if other_entries.clone().next().is_some() {
            body = body
                .child(section(
                    localized(
                        self.language,
                        "Add subscriptions",
                        "添加订阅",
                        "サブスクリプションを追加",
                    ),
                    palette,
                ))
                .child(row().children(other_entries.map(|entry| {
                    let id = entry.id;
                    self.action(
                        ("include-collection-member", id),
                        entry.name.clone(),
                        palette,
                        false,
                        cx,
                        move |this, cx| {
                            if !this.members.contains(&id) {
                                this.members.push(id);
                            }
                            cx.notify();
                        },
                    )
                })));
        }
        if let Some(Item::Collection(id)) = self.editor
            && self.collection(id).is_some_and(|collection| {
                collection.members != self.members && !collection.processing.nodes.is_empty()
            })
        {
            body = body.child(description(localized(
                self.language,
                "Saving this member order clears prior per-node overrides because their positions have changed.",
                "保存这个成员顺序会清除原有的逐节点改动，因为节点位置已更改。",
                "メンバー順を保存すると、位置が変わったため以前のノード個別編集はクリアされます。",
            ), palette));
        }
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::{
        library::{Collection, Entry, PublishSettings},
        model::editor::NodeEdit,
    };

    #[test]
    fn copies_keep_cached_entries_and_order_without_a_publish_destination() {
        let mut workspace = Workspace::default();
        let publish = |output: &str, filename: &str| PublishSettings {
            output: output.into(),
            output_file: filename.into(),
            format: Default::default(),
            title: String::new(),
            interval_minutes: 60,
        };
        let entry = |name: &str| {
            let mut processing = crate::relay::model::editor::Options::default();
            processing.nodes.push(NodeEdit {
                index: 0,
                name: Some("Renamed".into()),
                included: true,
            });
            Entry {
                id: 0,
                name: name.into(),
                content: "Sample = socks5, sample.example.invalid, 1080\n".into(),
                source: String::new(),
                source_file: String::new(),
                processing,
                publish: publish("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "sample.txt"),
            }
        };
        let first = workspace.add_entry(entry("Example")).unwrap();
        let second = workspace.add_entry(entry("Other")).unwrap();
        let collection = workspace
            .add_collection(Collection {
                id: 0,
                name: "Group".into(),
                members: vec![second, first],
                processing: Default::default(),
                publish: publish("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "group.txt"),
            })
            .unwrap();
        duplicate_item(&mut workspace, Item::Entry(first)).unwrap();
        duplicate_item(&mut workspace, Item::Collection(collection)).unwrap();
        assert_eq!(workspace.subscriptions.len(), 3);
        let [edit] = workspace.subscriptions[2].processing.nodes.as_slice() else {
            panic!("The copy should retain positional edits");
        };
        assert_eq!(edit.index, 0);
        assert!(edit.included);
        assert_eq!(edit.name.as_deref(), Some("Renamed"));
        assert!(workspace.subscriptions[2].publish.output.is_empty());
        assert!(workspace.collections[1].publish.output.is_empty());
        assert_eq!(workspace.collections[1].members, [second, first]);
        reorder_member(&mut workspace.collections[1].members, first, -1);
        assert_eq!(workspace.collections[1].members, [first, second]);
        assert_eq!(workspace.collections[0].members, [second, first]);
    }
}
