use super::super::{Palette, localized, panel};
use super::session::Library;
use super::view::{description, field, row, section};
use super::{
    Completed, Entry, OpenRelay, OpenSubscriptions, Source, Subscriptions, clear, message,
};
use crate::relay::{RelayConfig, RelayResult, model::pipeline::has_script};
use gpui::{Div, prelude::*, px};

impl Subscriptions {
    pub(super) fn clear_relay_save(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.reviewing_relay_save) {
            clear(&self.relay_save_name, cx);
        }
    }

    pub(in crate::gpui_ui) fn leave_relay_save(&mut self, cx: &mut Context<Self>) {
        self.resume_relay_save = false;
        self.clear_relay_save(cx);
    }

    pub(super) fn resume_relay_save_control(
        &self,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Option<Div> {
        (self.resume_relay_save && self.library.is_open() && self.current_preview(cx).is_some())
            .then(|| {
                row().child(self.action(
                    "resume-relay-save",
                    localized(
                        self.language,
                        "Return to relay to save",
                        "返回中转继续保存",
                        "中継に戻って保存",
                    ),
                    palette,
                    true,
                    cx,
                    |_, cx| cx.emit(OpenRelay),
                ))
            })
    }

    pub(super) fn relay_save_controls(&self, palette: Palette, cx: &Context<Self>) -> Option<Div> {
        if matches!(self.source, Source::Saved(_)) {
            return None;
        }
        let local = |en, zh, ja| localized(self.language, en, zh, ja);
        let save = local(
            "Save as subscription",
            "保存为订阅",
            "サブスクリプションとして保存",
        );
        if !self.library.is_open() {
            return Some(row().child(self.action(
                "relay-save-open-library",
                local(
                    "Open library to save",
                    "打开订阅库以保存",
                    "ライブラリを開いて保存",
                ),
                palette,
                false,
                cx,
                |this, cx| {
                    this.resume_relay_save = true;
                    cx.emit(OpenSubscriptions);
                },
            )));
        }
        if !self.reviewing_relay_save {
            return Some(row().child(self.action(
                "relay-save-review",
                save,
                palette,
                false,
                cx,
                |this, cx| {
                    if this.current_preview(cx).is_some() {
                        this.reviewing_relay_save = true;
                        cx.notify();
                    }
                },
            )));
        }
        let location = match self.library {
            Library::Temporary => local(
                "Save to the temporary library for this session.",
                "保存到本次会话的临时订阅库。",
                "このセッションの一時ライブラリに保存します。",
            ),
            _ => local(
                "Save to the encrypted library on this device.",
                "保存到本机的加密订阅库。",
                "この端末の暗号化ライブラリに保存します。",
            ),
        };
        let uses_script = self
            .current_preview(cx)
            .is_some_and(|preview| has_script(&preview.config.0.processing.pipeline));
        Some(
            panel(palette)
                .w_full()
                .p_4()
                .gap(px(9.))
                .child(section(save, palette))
                .child(description(location, palette))
                .child(description(
                    local(
                        "Keeps the source, reviewed nodes and processing steps. Choose automatic naming, sorting, connectivity tests and output again when needed.",
                        "保留来源、已审阅的节点和处理步骤。需要时可重新选择自动命名、排序、连通性测试和输出格式。",
                        "ソース、確認済みのノード、処理手順を保存します。自動命名、並べ替え、接続確認、出力形式は必要に応じて再設定できます。",
                    ),
                    palette,
                ))
                .when(uses_script, |panel| {
                    panel.child(description(
                        local(
                            "Scripts can read the saved subscription name as the source label.",
                            "脚本可以读取保存后的订阅名称作为来源名称。",
                            "スクリプトでは保存後のサブスクリプション名をソース名として参照できます。",
                        ),
                        palette,
                    ))
                })
                .child(field(
                    local("Subscription name", "订阅名称", "サブスクリプション名"),
                    &self.relay_save_name,
                    palette,
                ))
                .child(
                    row()
                        .child(self.action(
                            "relay-save-confirm",
                            local("Save to library", "保存到订阅库", "ライブラリに保存"),
                            palette,
                            true,
                            cx,
                            Self::save_relay_preview,
                        ))
                        .child(self.action(
                            "relay-save-cancel",
                            local("Cancel", "取消", "キャンセル"),
                            palette,
                            false,
                            cx,
                            |this, cx| {
                                this.clear_relay_save(cx);
                                this.notice = None;
                                cx.notify();
                            },
                        )),
                ),
        )
    }

    fn save_relay_preview(&mut self, cx: &mut Context<Self>) {
        if !self.reviewing_relay_save || matches!(self.source, Source::Saved(_)) {
            return;
        }
        let (Some(library), Some(preview)) = (self.library.writer(), self.current_preview(cx))
        else {
            return;
        };
        let name = self.relay_save_name.read(cx).value().trim();
        let local = |en, zh, ja| localized(self.language, en, zh, ja);
        if name.is_empty()
            || self
                .workspace
                .subscriptions
                .iter()
                .any(|item| item.name == name)
        {
            self.feedback(
                false,
                local(
                    "Enter a new subscription name.",
                    "请输入一个未使用的订阅名称。",
                    "未使用のサブスクリプション名を入力してください。",
                ),
                cx,
            );
            return;
        }
        let Some(entry) = reviewed_entry(name.to_owned(), &preview.config.0, &preview.result)
        else {
            return;
        };
        let notice = match library {
            Library::Temporary => local(
                "Saved to the temporary library for this session",
                "已保存到本次会话的临时订阅库",
                "このセッションの一時ライブラリに保存しました",
            ),
            _ => local(
                "Saved to the encrypted subscription library",
                "已保存到加密订阅库",
                "暗号化サブスクリプションライブラリに保存しました",
            ),
        };
        let mut workspace = self.workspace.clone();
        self.start(
            move || {
                workspace.add_entry(entry).map_err(message)?;
                library.save(&workspace)?;
                Ok(Completed::SavedRelay(workspace, notice))
            },
            cx,
        );
    }
}

fn reviewed_entry(name: String, config: &RelayConfig, result: &RelayResult) -> Option<Entry> {
    let (source, source_file, content) = match &result.source {
        Some(source) => (
            config.source.clone(),
            source
                .gist_file()
                .map_or_else(|| config.source_file.clone(), |file| file.filename.clone()),
            source.content().to_owned(),
        ),
        None => (
            String::new(),
            String::new(),
            config.local_content.as_deref()?.clone(),
        ),
    };
    Some(Entry {
        id: 0,
        name,
        source,
        source_file,
        content,
        processing: config.processing.clone(),
        publish: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::{self, RunMode, model::editor::NodeEdit};
    use std::sync::atomic::AtomicBool;
    use zeroize::Zeroizing;

    const RAW: &str = "A = socks5, a.example.invalid, 1080\n";

    fn edited_preview() -> (RelayConfig, RelayResult) {
        let raw = Zeroizing::new(RAW.to_owned());
        let mut config = RelayConfig {
            expected_source: Some(raw.clone()),
            local_content: Some(raw),
            rename: false,
            sort: false,
            ..Default::default()
        };
        config.processing.nodes.push(NodeEdit {
            index: 0,
            included: true,
            name: Some("Edited".into()),
        });
        let result = relay::run(
            &config,
            "",
            RunMode::Preview,
            &AtomicBool::new(false),
            &|_| {},
        )
        .unwrap();
        (config, result)
    }

    #[test]
    fn saves_unmodified_cache_and_positional_edits_together() {
        let (config, preview) = edited_preview();
        let nodes = relay::model::Subscription::parse(&preview.content).unwrap();
        assert_eq!(nodes.nodes()[0].name(), "Edited");
        let entry = reviewed_entry("Local".into(), &config, &preview).unwrap();
        assert_eq!(entry.content, RAW);
        assert!(entry.source.is_empty());
        assert_eq!(entry.processing.nodes[0].name.as_deref(), Some("Edited"));
    }

    #[test]
    fn saves_resolved_gist_filename_and_omits_publication_settings() {
        let (mut config, mut preview) = edited_preview();
        config.local_content = None;
        let id = "1234567890abcdef1234567890abcdef";
        config.source = format!("https://gist.github.com/{id}");
        config.output = "https://gist.github.com/abcdef1234567890abcdef1234567890".into();
        preview.source = Some(relay::input::Source::Gist(relay::gist::Source {
            file: relay::gist::GistFile {
                id: id.into(),
                filename: "nodes.list".into(),
            },
            content: Zeroizing::new(RAW.to_owned()),
        }));
        let entry = reviewed_entry("Remote".into(), &config, &preview).unwrap();
        assert_eq!(entry.source, config.source);
        assert_eq!(entry.source_file, "nodes.list");
        assert_eq!(entry.content, RAW);
        assert!(entry.publish.output.is_empty());
    }
}
