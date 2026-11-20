//! Refresh a saved source or save its output settings without losing the relay draft.

use super::super::{Palette, localized};
use super::view::{column, description, field, row};
use super::{Completed, Item, Source, Subscriptions, clear};
use crate::desktop::i18n::{Text, text};
use crate::relay::{
    RelayError,
    library::{Error, PublishSettings, Workspace},
    library_refresh,
};
use gpui::{Context, Div, prelude::*};
use std::sync::atomic::Ordering;
use zeroize::Zeroizing;

fn save_settings(
    candidate: &mut Workspace,
    item: Item,
    settings: PublishSettings,
) -> Result<(), Error> {
    match item {
        Item::Entry(id) => {
            let mut entry = candidate
                .subscriptions
                .iter()
                .find(|entry| entry.id == id)
                .cloned()
                .ok_or(Error::NotFound)?;
            entry.publish = settings;
            candidate.update_entry(entry)
        }
        Item::Collection(id) => {
            let mut collection = candidate
                .collections
                .iter()
                .find(|collection| collection.id == id)
                .cloned()
                .ok_or(Error::NotFound)?;
            collection.publish = settings;
            candidate.update_collection(collection)
        }
    }
}

impl Subscriptions {
    fn is_refreshable(&self, item: Item) -> bool {
        let remote = |id| self.entry(id).is_some_and(|entry| !entry.source.is_empty());
        match item {
            Item::Entry(id) => remote(id),
            Item::Collection(id) => self
                .collection(id)
                .is_some_and(|collection| collection.members.iter().copied().any(remote)),
        }
    }

    fn refresh_cache(&mut self, cx: &mut Context<Self>) {
        let (Source::Saved(item), Some(library)) = (&self.source, self.library.writer()) else {
            return;
        };
        let item = *item;
        if !self.is_refreshable(item) || self.check_schedule_cooldown(cx) {
            return;
        }
        let workspace = self.workspace.clone();
        let token = Zeroizing::new(self.source_token.read(cx).value().trim().to_owned());
        clear(&self.source_token, cx);
        let notice = localized(
            self.language,
            "Cache refreshed. Run a new preview.",
            "缓存已刷新，请重新运行预览。",
            "キャッシュを更新しました。プレビューを再実行してください。",
        );
        let merge_failed = text(self.language, Text::WorkspaceMergeFailed);
        let (cancel, progress) = self.begin_run();
        self.start(
            move || {
                let refreshed = match item {
                    Item::Entry(id) => {
                        library_refresh::refresh_entry(&workspace, id, &token, &cancel, &progress)
                    }
                    Item::Collection(id) => {
                        library_refresh::refresh(&workspace, id, &token, &cancel, &progress)
                    }
                };
                let candidate = match refreshed {
                    Ok(candidate) => candidate,
                    Err(RelayError::Cancelled) => return Ok(Completed::Cancelled),
                    Err(error) => return Ok(Completed::RelayFailed(error)),
                };
                if let Item::Collection(id) = item {
                    candidate.merge(id).map_err(|_| merge_failed)?;
                }
                if cancel.load(Ordering::Acquire) {
                    return Ok(Completed::Cancelled);
                }
                library.save(&candidate)?;
                Ok(Completed::RefreshedCache(candidate, notice))
            },
            cx,
        );
    }

    pub(super) fn finish_cache_refresh(
        &mut self,
        workspace: Workspace,
        notice: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.workspace = workspace;
        self.reviewed_source = None;
        self.node_editor.clear(cx);
        self.invalidate_preview(cx);
        self.feedback(true, notice, cx);
    }

    pub(super) fn cache_refresh_controls(
        &self,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let Source::Saved(item) = self.source else {
            return None;
        };
        if !self.is_refreshable(item) || !self.library.is_open() || self.schedule_active() {
            return None;
        }
        Some(
            column()
                .child(description(
                    localized(
                        self.language,
                        "Refresh downloads the saved remote sources and replaces their cache together. Preview uses the saved cache when available.",
                        "刷新会下载已保存的远程来源，并一次替换它们的缓存。预览会优先使用现有缓存。",
                        "更新は保存済みのリモートソースを取得し、キャッシュをまとめて置き換えます。プレビューは利用可能な保存済みキャッシュを使います。",
                    ),
                    palette,
                ))
                .when(!self.is_remote_source(), |body| {
                    body.child(field(
                        self.tr("Private Gist token (optional)", "私密 Gist 令牌（可选）"),
                        &self.source_token,
                        palette,
                    ))
                })
                .child(row().child(self.action(
                    "refresh-library-cache",
                    localized(self.language, "Refresh saved cache", "刷新已保存缓存", "保存済みキャッシュを更新"),
                    palette,
                    false,
                    cx,
                    Self::refresh_cache,
                ))),
        )
    }

    fn save_output_settings(&mut self, cx: &mut Context<Self>) {
        let (Source::Saved(item), Some(library)) = (&self.source, self.library.writer()) else {
            return;
        };
        let item = *item;
        if !self.reviewing_publish || self.current_preview(cx).is_none() {
            return;
        }
        let Some(interval_minutes) = self.schedule_interval_input(cx) else {
            self.feedback(false, text(self.language, Text::ScheduleInterval), cx);
            return;
        };
        let output = self.output_url.read(cx).value().trim().to_owned();
        let filename = self.output_name.read(cx).value().trim();
        let output_file = if filename.is_empty() {
            format!("nodes.{}", self.format.extension())
        } else {
            filename.to_owned()
        };
        let settings = PublishSettings {
            output,
            output_file,
            format: self.format,
            title: self.output_title.read(cx).value().trim().to_owned(),
            interval_minutes,
        };
        let mut candidate = self.workspace.clone();
        let invalid = text(self.language, Text::RelayOutputInvalid);
        let notice = localized(
            self.language,
            "Output settings saved in the encrypted library.",
            "输出设置已保存到加密资料库。",
            "出力設定を暗号化ライブラリへ保存しました。",
        );
        self.start(
            move || {
                save_settings(&mut candidate, item, settings).map_err(|_| invalid)?;
                library.save(&candidate)?;
                Ok(Completed::SavedOutput(candidate, notice))
            },
            cx,
        );
    }

    pub(super) fn finish_output_save(
        &mut self,
        workspace: Workspace,
        notice: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.workspace = workspace;
        self.conceal_preview(cx);
        self.feedback(true, notice, cx);
    }

    pub(super) fn saved_output_controls(
        &self,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Option<Div> {
        if !matches!(self.source, Source::Saved(_)) || !self.library.is_open() {
            return None;
        }
        Some(
            column()
                .child(description(
                    text(self.language, Text::WorkspaceOutputSavedHelp),
                    palette,
                ))
                .child(row().child(self.action(
                    "save-library-output-settings",
                    localized(
                        self.language,
                        "Save output settings",
                        "保存输出设置",
                        "出力設定を保存",
                    ),
                    palette,
                    false,
                    cx,
                    Self::save_output_settings,
                ))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::{export::OutputFormat, library::Entry};

    #[test]
    fn source_gist_cannot_be_saved_as_the_destination() {
        let gist = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let mut workspace = Workspace::default();
        let id = workspace
            .add_entry(Entry {
                id: 0,
                name: "Sample".into(),
                source: gist.into(),
                source_file: "input.txt".into(),
                content: String::new(),
                processing: Default::default(),
                publish: Default::default(),
            })
            .unwrap();
        let conflicting = PublishSettings {
            output: gist.into(),
            output_file: "output.yaml".into(),
            format: OutputFormat::Mihomo,
            title: String::new(),
            interval_minutes: 60,
        };
        assert!(matches!(
            save_settings(&mut workspace, Item::Entry(id), conflicting),
            Err(Error::Invalid)
        ));
        assert!(workspace.subscriptions[0].publish.output.is_empty());

        let valid = PublishSettings {
            output: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            output_file: "output.yaml".into(),
            format: OutputFormat::Mihomo,
            interval_minutes: 30,
            title: String::new(),
        };
        save_settings(&mut workspace, Item::Entry(id), valid.clone()).unwrap();
        assert!(workspace.subscriptions[0].publish == valid);
    }
}
