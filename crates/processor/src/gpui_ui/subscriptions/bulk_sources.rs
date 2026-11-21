//! Review pasted subscription links offline before saving them to the library.

use super::super::{Palette, localized, panel};
use super::view::{column, description, row, section};
use super::{Completed, Subscriptions, message, session};
use crate::desktop::i18n::{Text, text};
use crate::relay::{
    RelayConfig,
    library::{Collection, Entry, Error, MAX_COLLECTIONS, MAX_SUBSCRIPTIONS, Workspace},
    library_import::unique_name,
};
use gpui::{Context, Div, Stateful, div, prelude::*, px};
use std::collections::HashSet;
use zeroize::Zeroizing;

const MAX_PASTE_BYTES: usize = 256 * 1024;
const MAX_PASTE_LINES: usize = MAX_SUBSCRIPTIONS * 2;

#[derive(Default)]
pub(super) struct Form {
    review: Option<Review>,
    make_collection: bool,
}

#[derive(Clone, Default)]
struct Review {
    entries: Vec<(usize, Entry)>,
    duplicates: Vec<usize>,
    invalid: Vec<usize>,
}

fn is_https(input: &str) -> bool {
    input
        .get(..8)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

fn link_parts<'a>(line: &'a str, default_name: &str) -> Option<(String, &'a str)> {
    let (label, source) = if is_https(line) {
        (None, line)
    } else {
        let (label, source) = line.split_once('|')?;
        (Some(label.trim()), source.trim())
    };
    if !is_https(source) {
        return None;
    }
    let name = match label {
        Some(label) => RelayConfig::script_source_label(label)
            .filter(|name| !name.contains(['?', '#', '@', '=', '/', '\\']))?,
        None => default_name.to_owned(),
    };
    Some((name, source))
}

impl Review {
    fn read(input: &str, workspace: &Workspace, default_name: &str) -> Result<Self, Error> {
        if input.len() > MAX_PASTE_BYTES {
            return Err(Error::TooLarge);
        }
        let mut sources = workspace
            .subscriptions
            .iter()
            .filter(|entry| entry.source_file.is_empty())
            .map(|entry| entry.source.trim())
            .collect::<HashSet<_>>();
        let mut names = workspace
            .subscriptions
            .iter()
            .map(|entry| entry.name.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let mut review = Self::default();
        for (index, line) in input.lines().enumerate() {
            if index >= MAX_PASTE_LINES {
                return Err(Error::TooLarge);
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let number = index + 1;
            let Some((name, source)) = link_parts(line, default_name) else {
                review.invalid.push(number);
                continue;
            };
            if sources.contains(source) {
                review.duplicates.push(number);
                continue;
            }
            let Ok(name) = unique_name(&name, |name| names.contains(&name.to_ascii_lowercase()), 2)
            else {
                review.invalid.push(number);
                continue;
            };
            // Validate each row with the same rules used by the library, without
            // copying existing cached subscriptions or performing a network request.
            let mut check = Workspace::default();
            let entry = check
                .add_entry(Entry {
                    id: 0,
                    name,
                    source: source.to_owned(),
                    source_file: String::new(),
                    content: String::new(),
                    processing: Default::default(),
                    publish: Default::default(),
                })
                .ok()
                .and_then(|_| check.subscriptions.pop());
            let Some(entry) = entry else {
                review.invalid.push(number);
                continue;
            };
            sources.insert(source);
            names.insert(entry.name.to_ascii_lowercase());
            review.entries.push((number, entry));
        }
        Ok(review)
    }

    fn fits(&self, workspace: &Workspace) -> bool {
        self.entries.len() + workspace.subscriptions.len() <= MAX_SUBSCRIPTIONS
    }

    fn can_save(&self, workspace: &Workspace) -> bool {
        !self.entries.is_empty() && self.invalid.is_empty() && self.fits(workspace)
    }

    fn apply(
        self,
        mut workspace: Workspace,
        collection: Option<String>,
    ) -> Result<Workspace, Error> {
        if !self.can_save(&workspace) {
            return Err(Error::Invalid);
        }
        let first_id = workspace.next_id;
        workspace.next_id = first_id
            .checked_add(self.entries.len() as u64)
            .ok_or(Error::Invalid)?;
        let ids = first_id..workspace.next_id;
        workspace
            .subscriptions
            .extend(
                self.entries
                    .into_iter()
                    .zip(ids.clone())
                    .map(|((_, mut entry), id)| {
                        entry.id = id;
                        entry
                    }),
            );
        if let Some(name) = collection {
            workspace.add_collection(Collection {
                id: 0,
                name,
                members: ids.collect(),
                processing: Default::default(),
                publish: Default::default(),
            })?;
        } else {
            workspace.validate()?;
        }
        Ok(workspace)
    }
}

impl Subscriptions {
    pub(in crate::gpui_ui) fn clear_bulk_sources(&mut self, cx: &mut Context<Self>) {
        if self.bulk_sources.take().is_some() {
            self.notice = None;
            cx.notify();
        }
    }

    fn read_bulk_sources(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.bulk_sources.as_mut() else {
            return;
        };
        form.review = None;
        let input = cx
            .read_from_clipboard()
            .and_then(|clipboard| clipboard.text())
            .map(Zeroizing::new);
        let language = self.language;
        let local = |en, zh, ja| localized(language, en, zh, ja);
        let default_name = local("Subscription", "订阅", "サブスクリプション");
        match input.as_deref() {
            Some(input) if !input.trim().is_empty() => {
                match Review::read(input, &self.workspace, default_name) {
                    Ok(review) => {
                        form.review = Some(review);
                        self.notice = None;
                        cx.notify();
                    }
                    Err(_) => self.feedback(
                        false,
                        local(
                            "Could not read the list. Paste at most 512 lines and 256 KiB.",
                            "无法读取列表。请粘贴最多 512 行、256 KiB 的文本。",
                            "一覧を読み込めませんでした。512 行、256 KiB 以内のテキストを貼り付けてください。",
                        ),
                        cx,
                    ),
                }
            }
            _ => self.feedback(
                false,
                local(
                    "The clipboard has no subscription text.",
                    "剪贴板没有订阅文本。",
                    "クリップボードにサブスクリプションのテキストがありません。",
                ),
                cx,
            ),
        }
    }

    fn bulk_collection_name(&self) -> Option<String> {
        if self.workspace.collections.len() >= MAX_COLLECTIONS {
            return None;
        }
        unique_name(
            localized(
                self.language,
                "Imported subscriptions",
                "已导入订阅",
                "インポートしたサブスクリプション",
            ),
            |name| {
                self.workspace
                    .collections
                    .iter()
                    .any(|collection| collection.name == name)
            },
            2,
        )
        .ok()
    }

    fn save_bulk_sources(&mut self, cx: &mut Context<Self>) {
        let Some(library) = self.library.writer() else {
            return;
        };
        let Some(form) = self.bulk_sources.as_ref() else {
            return;
        };
        let collection = if form.make_collection {
            let Some(name) = self.bulk_collection_name() else {
                return;
            };
            Some(name)
        } else {
            None
        };
        let Some(review) = form
            .review
            .as_ref()
            .filter(|review| review.can_save(&self.workspace))
            .cloned()
        else {
            return;
        };
        let workspace = self.workspace.clone();
        let notice = if collection.is_some() {
            localized(
                self.language,
                "Collection created. Select ‘Open in Relay’ on the collection, then ‘Refresh saved cache’ before previewing.",
                "合集已创建。请在合集上点「在中继中打开」，再点「刷新已保存缓存」，然后运行预览。",
                "コレクションを作成しました。コレクションの「Open in Relay」を選び、「保存済みキャッシュを更新」を押してからプレビューしてください。",
            )
        } else {
            text(
                self.language,
                if matches!(library, session::Library::Temporary) {
                    Text::WorkspaceDraftSaved
                } else {
                    Text::WorkspaceSaved
                },
            )
        };
        self.start(
            move || {
                let workspace = review.apply(workspace, collection).map_err(message)?;
                library.save(&workspace)?;
                Ok(Completed::SavedBulk(workspace, notice))
            },
            cx,
        );
    }

    pub(super) fn bulk_sources_button(
        &self,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        self.action(
            "open-bulk-subscriptions",
            localized(
                self.language,
                "Add multiple subscriptions",
                "批量添加订阅",
                "サブスクリプションをまとめて追加",
            ),
            palette,
            false,
            cx,
            |this, cx| {
                this.bulk_sources = Some(Form::default());
                this.notice = None;
                cx.notify();
            },
        )
    }

    pub(super) fn bulk_sources_panel(&self, palette: Palette, cx: &Context<Self>) -> Option<Div> {
        let form = self.bulk_sources.as_ref()?;
        let local = |en, zh, ja| localized(self.language, en, zh, ja);
        let mut card = panel(palette)
            .w_full()
            .p_5()
            .gap_4()
            .child(section(
                local(
                    "Add multiple subscriptions",
                    "批量添加订阅",
                    "サブスクリプションをまとめて追加",
                ),
                palette,
            ))
            .child(description(
                local(
                    "Copy one HTTPS link per line. Use Name | URL to set a display name. Review the names before saving.",
                    "每行复制一个 HTTPS 链接。可用「名称 | 链接」指定显示名称，保存前先核对。",
                    "1 行に 1 件の HTTPS リンクをコピーしてください。表示名は「名前 | URL」で指定でき、保存前に確認できます。",
                ),
                palette,
            ));

        if let Some(review) = &form.review {
            card = card.child(bulk_review(review, &self.workspace, self.language, palette));
        }

        let mut actions = row().child(self.action(
            "read-bulk-subscriptions",
            local(
                "Read clipboard",
                "从剪贴板读取",
                "クリップボードから読み込む",
            ),
            palette,
            form.review.is_none(),
            cx,
            Self::read_bulk_sources,
        ));
        if form
            .review
            .as_ref()
            .is_some_and(|review| review.can_save(&self.workspace))
        {
            if let Some(name) = self.bulk_collection_name() {
                let label = if form.make_collection {
                    local(
                        "Skip creating a collection",
                        "取消创建合集",
                        "コレクションの作成を取り消す",
                    )
                } else {
                    local(
                        "Also create a collection",
                        "同时创建合集",
                        "コレクションも作成",
                    )
                };
                if form.make_collection {
                    let prefix = local("New collection", "新建合集", "新規コレクション");
                    card = card.child(description(format!("{prefix} · {name}"), palette));
                }
                actions = actions.child(self.action(
                    "toggle-bulk-collection",
                    label,
                    palette,
                    false,
                    cx,
                    |this, cx| {
                        if let Some(form) = &mut this.bulk_sources {
                            form.make_collection = !form.make_collection;
                            cx.notify();
                        }
                    },
                ));
            }
            let title = if form.make_collection {
                local(
                    "Save subscriptions and collection",
                    "保存订阅和合集",
                    "サブスクリプションとコレクションを保存",
                )
            } else {
                local("Save subscriptions", "保存订阅", "サブスクリプションを保存")
            };
            actions = actions.child(self.action(
                "save-bulk-subscriptions",
                title,
                palette,
                true,
                cx,
                Self::save_bulk_sources,
            ));
        }
        Some(card.child(actions.child(self.action(
            "cancel-bulk-subscriptions",
            text(self.language, Text::Cancel),
            palette,
            false,
            cx,
            Self::clear_bulk_sources,
        ))))
    }
}

fn bulk_review(
    review: &Review,
    workspace: &Workspace,
    language: crate::runtime::settings::Language,
    palette: Palette,
) -> Div {
    let local = |en, zh, ja| localized(language, en, zh, ja);
    let ready = local("New subscriptions", "待添加", "追加する項目");
    let duplicates = local("Duplicates skipped", "重复已跳过", "重複をスキップ");
    let invalid = local("Lines to correct", "需修正的行", "修正が必要な行");
    let status = format!(
        "{ready} {} · {duplicates} {} · {invalid} {}",
        review.entries.len(),
        review.duplicates.len(),
        review.invalid.len(),
    );
    let lines = column()
        .gap(px(5.))
        .children(review.invalid.iter().map(|number| {
            description(
                format!(
                    "{number}. {}",
                    local(
                        "Invalid link or name",
                        "链接或名称无效",
                        "リンクまたは名前が無効です"
                    ),
                ),
                palette,
            )
        }))
        .children(
            review
                .entries
                .iter()
                .map(|(number, entry)| description(format!("{number}. {}", entry.name), palette)),
        )
        .children(review.duplicates.iter().map(|number| {
            description(
                format!(
                    "{number}. {}",
                    local("Duplicate", "重复链接", "重複したリンク")
                ),
                palette,
            )
        }));
    column()
        .gap(px(8.))
        .child(section(status, palette))
        .when(!review.fits(workspace), |body| {
            body.child(description(
                local(
                    "The library has room for fewer subscriptions. Copy a shorter list and try again.",
                    "资料库的剩余容量不足。请缩短列表后重新读取。",
                    "ライブラリの空き容量が不足しています。短い一覧をコピーし直してください。",
                ),
                palette,
            ))
        })
        .child(
            div()
                .id("bulk-subscription-names")
                .w_full()
                .max_h(px(230.))
                .overflow_y_scroll()
                .p_3()
                .rounded_md()
                .bg(palette.subtle)
                .child(lines),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_names_links_and_skips_duplicates_in_the_library_and_batch() {
        let workspace = Review::read(
            "Alpha | https://one.example.invalid/first?token=fixture",
            &Workspace::default(),
            "Subscription",
        )
        .unwrap()
        .apply(Workspace::default(), None)
        .unwrap();
        let text = "alpha | https://one.example.invalid/next\r\n\
                    https://fixture-in-host.example.invalid/sub?key=fixture\r\n\
                    https://two.example.invalid/sub?key=fixture\r\n\
                    https://one.example.invalid/first?token=fixture\r\n\
                    Duplicate | https://two.example.invalid/sub?key=fixture";
        let review = Review::read(text, &workspace, "Subscription").unwrap();
        assert_eq!(review.duplicates, [4, 5]);
        assert_eq!(review.entries[0].1.name, "alpha (2)");
        assert_eq!(review.entries[1].1.name, "Subscription");
        assert_eq!(review.entries[2].1.name, "Subscription (2)");
        let candidate = review
            .apply(workspace.clone(), Some("Imported fixtures".to_owned()))
            .unwrap();
        assert_eq!(workspace.subscriptions.len(), 1);
        assert_eq!(candidate.collections[0].members, [2, 3, 4]);
        assert_eq!(candidate.collections[0].id, 5);
        assert_eq!(candidate.next_id, 6);
        assert!(
            candidate
                .subscriptions
                .iter()
                .all(|entry| entry.content.is_empty())
        );
    }

    #[test]
    fn invalid_rows_block_the_whole_batch_without_disclosing_links() {
        let workspace = Workspace::default();
        let result = Review::read(
            "https://one.example.invalid/sub\n\
             http://one.example.invalid/a\n\
             https://127.0.0.1/sub\n\
             https://fixture@one.example.invalid/sub\n\
             token=fixture | https://one.example.invalid/next",
            &workspace,
            "Subscription",
        )
        .unwrap();
        assert_eq!(result.invalid, [2, 3, 4, 5]);
        assert!(!result.can_save(&workspace));
        assert!(result.apply(workspace, None).is_err());
    }

    #[test]
    fn an_oversized_batch_cannot_partially_fill_the_library() {
        let workspace = Workspace::default();
        let text = (0..=MAX_SUBSCRIPTIONS)
            .map(|i| format!("https://one.example.invalid/{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let review = Review::read(&text, &workspace, "Subscription").unwrap();
        assert!(!review.can_save(&workspace));
        assert!(review.apply(workspace, None).is_err());
    }
}
