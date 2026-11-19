use super::super::{Palette, button, heading, input::TextInput, panel};
use super::{FileMode, Item, Source, Subscriptions, clear};
use crate::gui::i18n::{Text, text};
use crate::relay::{RelayStage, export::OutputFormat};
use gpui::{
    ClipboardItem, Div, ElementId, Entity, SharedString, Stateful, Window, div, prelude::*, px,
};
use std::sync::atomic::Ordering;
use zeroize::Zeroizing;

impl Render for Subscriptions {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.activation.is_none() {
            self.activation = Some(cx.observe_window_activation(window, |this, window, cx| {
                if !window.is_window_active() {
                    this.conceal_preview(cx);
                    clear(&this.source_token, cx);
                    cx.notify();
                }
            }));
        }
        let palette = Palette::for_window(window, cx);
        let body = if self.showing_relay {
            self.relay(palette, cx)
        } else {
            self.library(palette, cx)
        };
        div()
            .id("subscription-workspace")
            .w_full()
            .min_w_0()
            .max_w(px(1040.))
            .child(body)
    }
}

impl Subscriptions {
    pub(super) fn action(
        &self,
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        palette: Palette,
        primary: bool,
        cx: &Context<Self>,
        f: impl Fn(&mut Self, &mut Context<Self>) + 'static,
    ) -> Stateful<Div> {
        button(id, title, palette, primary)
            .when(
                self.pending || self.schedule_active() || self.selecting_plan_file(cx),
                |button| button.opacity(0.55),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if !this.pending && !this.schedule_active() && !this.selecting_plan_file(cx) {
                    f(this, cx);
                }
            }))
    }

    fn messages(&self, palette: Palette) -> Div {
        let mut display = div().flex().flex_col().gap(px(8.));
        if self.pending {
            let stage = self
                .running
                .as_ref()
                .and_then(|run| run.progress.lock().ok().and_then(|value| *value))
                .map(|progress| self.progress_text(progress.stage))
                .unwrap_or_else(|| self.tr("Working…", "正在处理…"));
            display = display.child(description(stage, palette));
        }
        if let Some(notice) = &self.notice {
            display = display.child(
                div()
                    .border_l_2()
                    .border_color(if self.error {
                        gpui::rgb(0xe88378)
                    } else {
                        palette.accent
                    })
                    .pl(px(10.))
                    .text_color(palette.ink)
                    .text_size(px(13.))
                    .child(notice.clone()),
            );
        }
        display
    }

    pub(super) fn progress_text(&self, stage: RelayStage) -> &'static str {
        match stage {
            RelayStage::Read => self.tr("Reading source…", "正在读取来源…"),
            RelayStage::Check => self.tr("Checking nodes…", "正在检查节点…"),
            RelayStage::Render => self.tr("Formatting output…", "正在整理导出…"),
            RelayStage::Publish => self.tr("Publishing to GitHub…", "正在发布到 GitHub…"),
            RelayStage::Complete => self.tr("Wrapping up…", "即将完成…"),
        }
    }

    fn library(&self, palette: Palette, cx: &Context<Self>) -> Div {
        let body = column()
            .child(heading(
                self.tr("Subscriptions", "订阅资料库"),
                self.tr(
                    "Manage private sources and build reusable collections.",
                    "管理私密订阅来源与可复用的合集。",
                ),
                palette,
            ))
            .child(self.messages(palette))
            .children(self.schedule_panel(palette, cx));
        if self.store.is_none() {
            let title = if self.storage_exists {
                self.tr("Unlock library", "解锁资料库")
            } else {
                self.tr("Create encrypted library", "创建加密资料库")
            };
            let mut form = panel(palette)
                .w_full()
                .max_w(px(460.))
                .p_5()
                .gap_4()
                .child(section(title, palette))
                .child(description(
                    self.tr("The password stays on this device.", "密码仅在本机使用。"),
                    palette,
                ))
                .child(field(self.tr("Password", "密码"), &self.password, palette));
            if !self.storage_exists {
                form = form.child(field(
                    self.tr("Repeat password", "重复密码"),
                    &self.confirmation,
                    palette,
                ));
            }
            return body.child(form.child(row().child(self.action(
                "unlock-subscriptions",
                title,
                palette,
                true,
                cx,
                Self::open_store,
            ))));
        }
        if let Some(item) = self.editor {
            return body.child(self.library_editor(item, palette, cx));
        }

        let actions = row()
            .child(self.action(
                "new-subscription",
                self.tr("New subscription", "新建订阅"),
                palette,
                true,
                cx,
                |this, cx| this.edit(Item::Entry(0), cx),
            ))
            .child(self.action(
                "new-collection",
                self.tr("New collection", "新建合集"),
                palette,
                false,
                cx,
                |this, cx| this.edit(Item::Collection(0), cx),
            ))
            .child(self.action(
                "import-backup",
                self.tr("Import backup", "导入备份"),
                palette,
                false,
                cx,
                |this, cx| this.select_file(FileMode::Backup, cx),
            ))
            .child(self.action(
                "lock-subscriptions",
                self.tr("Lock library", "锁定资料库"),
                palette,
                false,
                cx,
                Self::lock,
            ));
        if self.workspace.subscriptions.is_empty() && self.workspace.collections.is_empty() {
            return body.child(actions).child(
                panel(palette)
                    .w_full()
                    .p_5()
                    .gap_2()
                    .child(section(
                        self.tr("Your library is empty", "资料库为空"),
                        palette,
                    ))
                    .child(description(
                        self.tr(
                            "Add a subscription URL or load nodes from the clipboard.",
                            "添加订阅链接，或从剪贴板载入节点。",
                        ),
                        palette,
                    )),
            );
        }
        let query = Zeroizing::new(self.query.read(cx).value().trim().to_lowercase());
        let matches = |name: &str| {
            query.is_empty() || Zeroizing::new(name.to_lowercase()).contains(query.as_str())
        };
        let matching_entries = self
            .workspace
            .subscriptions
            .iter()
            .filter(|entry| matches(&entry.name));
        let matching_collections = self
            .workspace
            .collections
            .iter()
            .filter(|item| matches(&item.name));
        let mut entries = column();
        if matching_entries.clone().next().is_some() {
            entries = entries.child(section(self.tr("Subscriptions", "订阅"), palette));
        }
        for entry in matching_entries.clone() {
            let subtitle = if entry.source.is_empty() {
                self.tr("Local data", "本地数据")
            } else if entry.content.is_empty() {
                self.tr("HTTPS source", "HTTPS 来源")
            } else {
                self.tr("Cached HTTPS source", "已缓存的 HTTPS 来源")
            };
            entries = entries.child(self.library_row(
                Item::Entry(entry.id),
                &entry.name,
                subtitle.into(),
                palette,
                cx,
            ));
        }
        let mut collections = column();
        if matching_collections.clone().next().is_some() {
            collections = collections.child(section(self.tr("Collections", "合集"), palette));
        }
        for collection in matching_collections.clone() {
            let subtitle = format!(
                "{} {}",
                collection.members.len(),
                self.tr("subscriptions", "个订阅")
            );
            collections = collections.child(self.library_row(
                Item::Collection(collection.id),
                &collection.name,
                subtitle,
                palette,
                cx,
            ));
        }
        let mut body = body.child(actions).child(self.query.clone());
        if matching_entries.count() == 0 && matching_collections.count() == 0 {
            body = body.child(description(
                self.tr(
                    "No matching subscriptions or collections",
                    "没有匹配的订阅或合集",
                ),
                palette,
            ));
        }
        if self.deleting.is_some() {
            body = body.child(
                panel(palette).w_full().p_5().child(
                    column()
                        .child(section(
                            self.tr(
                                "Remove the selected library item?",
                                "移除选中的资料库项目？",
                            ),
                            palette,
                        ))
                        .child(
                            row()
                                .child(self.action(
                                    "delete-subscription-confirm",
                                    self.tr("Remove from library", "从资料库移除"),
                                    palette,
                                    true,
                                    cx,
                                    Self::delete,
                                ))
                                .child(self.action(
                                    "delete-subscription-cancel",
                                    self.tr("Cancel", "取消"),
                                    palette,
                                    false,
                                    cx,
                                    |this, cx| {
                                        this.deleting = None;
                                        cx.notify();
                                    },
                                )),
                        ),
                ),
            );
        }
        body.child(entries).child(collections)
    }

    fn library_row(
        &self,
        item: Item,
        name: &str,
        subtitle: String,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Div {
        let id = match item {
            Item::Entry(id) | Item::Collection(id) => id,
        };
        panel(palette).w_full().child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    column()
                        .gap(px(3.))
                        .child(section(name.to_owned(), palette))
                        .child(description(subtitle, palette)),
                )
                .child(
                    row()
                        .child(self.action(
                            ("open-relay", id),
                            self.tr("Open in Relay", "在中继中打开"),
                            palette,
                            true,
                            cx,
                            move |this, cx| this.open_saved(item, cx),
                        ))
                        .child(self.action(
                            ("edit-subscription", id),
                            self.tr("Edit", "编辑"),
                            palette,
                            false,
                            cx,
                            move |this, cx| this.edit(item, cx),
                        ))
                        .child(self.action(
                            ("delete-subscription", id),
                            self.tr("Remove", "移除"),
                            palette,
                            false,
                            cx,
                            move |this, cx| {
                                this.deleting = Some(item);
                                cx.notify();
                            },
                        )),
                ),
        )
    }

    fn library_editor(&self, item: Item, palette: Palette, cx: &Context<Self>) -> Div {
        let mut form = column().child(field(self.tr("Name", "名称"), &self.name, palette));
        match item {
            Item::Entry(_) => {
                form = form
                    .child(field(
                        self.tr("Subscription URL", "订阅链接"),
                        &self.entry_url,
                        palette,
                    ))
                    .child(field(
                        self.tr("Source filename", "源文件名"),
                        &self.entry_file,
                        palette,
                    ))
                    .child(
                        row()
                            .child(self.action(
                                "library-paste-node-data",
                                self.tr("Load from clipboard", "从剪贴板载入"),
                                palette,
                                false,
                                cx,
                                |this, cx| this.load_clipboard(true, cx),
                            ))
                            .child(self.action(
                                "library-load-node-file",
                                self.tr("Load file", "载入文件"),
                                palette,
                                false,
                                cx,
                                |this, cx| this.select_file(FileMode::LibraryNodes, cx),
                            ))
                            .when(self.added_content.is_some(), |row| {
                                row.child(description(
                                    self.tr("Replacement data ready", "替换内容已就绪"),
                                    palette,
                                ))
                            }),
                    );
            }
            Item::Collection(_) => {
                form = form.child(section(
                    self.tr("Include subscriptions", "加入订阅"),
                    palette,
                ));
                let mut members = row();
                for entry in &self.workspace.subscriptions {
                    let id = entry.id;
                    let selected = self.members.contains(&id);
                    members = members.child(self.action(
                        ("collection-member", id),
                        entry.name.clone(),
                        palette,
                        selected,
                        cx,
                        move |this, cx| {
                            if selected {
                                this.members.retain(|member| *member != id);
                            } else {
                                this.members.push(id);
                            }
                            cx.notify();
                        },
                    ));
                }
                form = form.child(members);
            }
        }
        panel(palette).w_full().p_5().child(
            form.child(self.library_pipeline.clone()).child(
                row()
                    .child(self.action(
                        "save-subscription",
                        self.tr("Save to library", "保存到资料库"),
                        palette,
                        true,
                        cx,
                        Self::save,
                    ))
                    .child(self.action(
                        "cancel-subscription-edit",
                        self.tr("Discard draft", "放弃草稿"),
                        palette,
                        false,
                        cx,
                        |this, cx| {
                            this.reset_editor(cx);
                            this.notice = None;
                            cx.notify();
                        },
                    )),
            ),
        )
    }

    fn relay(&self, palette: Palette, cx: &Context<Self>) -> Div {
        let cancel = self
            .running
            .as_ref()
            .is_some_and(|run| !run.cancel.load(Ordering::Relaxed));
        let mut body = column()
            .child(heading(
                self.tr("Relay", "订阅中继"),
                self.tr(
                    "Prepare a private subscription, review the result, then export or publish.",
                    "准备私密订阅，预览结果再导出或发布。",
                ),
                palette,
            ))
            .child(self.messages(palette))
            .children(self.schedule_panel(palette, cx));
        if cancel {
            body = body.child(
                button(
                    "cancel-relay-run",
                    self.tr("Cancel operation", "取消操作"),
                    palette,
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| this.cancel_run(cx))),
            );
        }
        let source_title = match self.source {
            Source::Saved(Item::Entry(id)) => self.entry(id).map(|entry| entry.name.as_str()),
            Source::Saved(Item::Collection(id)) => {
                self.collection(id).map(|entry| entry.name.as_str())
            }
            Source::Local(_) => Some(self.tr("Local subscription ready", "本地订阅已就绪")),
            Source::Link => None,
        };
        let mut source = column()
            .child(section(self.tr("Source", "来源"), palette))
            .child(
                row()
                    .child(self.action(
                        "relay-source-url",
                        self.tr("HTTPS URL", "HTTPS 链接"),
                        palette,
                        matches!(self.source, Source::Link),
                        cx,
                        |this, cx| {
                            if !matches!(this.source, Source::Link) {
                                this.set_source(Source::Link, cx);
                                cx.notify();
                            }
                        },
                    ))
                    .child(self.action(
                        "relay-source-local",
                        self.tr("Load file", "载入文件"),
                        palette,
                        false,
                        cx,
                        |this, cx| this.select_file(FileMode::Nodes, cx),
                    ))
                    .child(self.action(
                        "relay-source-clipboard",
                        self.tr("Load clipboard", "载入剪贴板"),
                        palette,
                        false,
                        cx,
                        |this, cx| this.load_clipboard(false, cx),
                    )),
            );
        if let Some(name) = source_title {
            source = source.child(description(name.to_owned(), palette));
        } else if !self.schedule_active() {
            source = source
                .child(field(
                    self.tr("Subscription URL", "订阅链接"),
                    &self.relay_url,
                    palette,
                ))
                .child(field(
                    self.tr("Source filename", "源文件名"),
                    &self.relay_file,
                    palette,
                ));
        }
        if self.is_remote_source() && !self.schedule_active() {
            source = source.child(field(
                self.tr("Private Gist token (optional)", "私密 Gist 令牌（可选）"),
                &self.source_token,
                palette,
            ));
        }
        let mut formats = row();
        for (label, format) in [
            (self.tr("Original", "原始格式"), OutputFormat::Original),
            ("Surge", OutputFormat::Surge),
            ("Mihomo", OutputFormat::Mihomo),
            ("sing-box", OutputFormat::SingBox),
        ] {
            formats = formats.child(self.action(
                label,
                label,
                palette,
                self.format == format,
                cx,
                move |this, cx| {
                    if this.format != format {
                        this.format = format;
                        this.preview = None;
                        this.conceal_preview(cx);
                        cx.notify();
                    }
                },
            ));
        }
        let preview_title = if self.needs_source_review(cx) {
            self.tr("Review remote source", "检查远程来源")
        } else {
            self.tr("Run preview", "运行预览")
        };
        source = source
            .child(section(self.tr("Output format", "输出格式"), palette))
            .child(formats)
            .when(!self.schedule_active(), |source| {
                source.child(description(
                    self.tr(
                        "Edits here apply to this relay session. Edit a library item to save its steps.",
                        "此处编辑仅用于当前中继。要保存步骤，请编辑资料库中的项目。",
                    ),
                    palette,
                )).child(self.relay_pipeline.clone())
            })
            .when_some(self.reviewed_source.as_ref().filter(|_| !self.schedule_active()), |source, review| {
                source.child(description(
                    format!("{} {}", self.tr("Nodes in reviewed remote source", "已检查远程来源节点数"), review.nodes),
                    palette,
                ))
            })
            .when(!self.schedule_active() && self.needs_resolution(cx), |source| {
                source.child(description(
                    self.tr(
                        "Domain resolution sends node hostnames to the DNS resolvers configured in the steps.",
                        "域名解析会把节点主机名发送给各步骤中配置的 DNS 解析器。",
                    ),
                    palette,
                )).child(self.action(
                    "relay-resolution-consent",
                    if self.allow_resolution {
                        self.tr("Revoke DNS approval", "撤销 DNS 授权")
                    } else {
                        self.tr("Allow DNS for this preview", "允许本次预览查询 DNS")
                    },
                    palette,
                    self.allow_resolution,
                    cx,
                    |this, cx| {
                        this.allow_resolution = !this.allow_resolution;
                        cx.notify();
                    },
                ))
            })
            .child(self.action(
                "relay-preview",
                preview_title,
                palette,
                true,
                cx,
                Self::preview,
            ));
        body = body.child(panel(palette).w_full().p_5().child(source));
        if !self.schedule_active()
            && let Some(preview) = self.current_preview(cx)
        {
            let mut summary = column()
                .child(section(self.tr("Preview results", "预览结果"), palette))
                .child(description(
                    format!(
                        "{} {}   ·   {} {}   ·   {} {}",
                        self.tr("Input", "输入"),
                        preview.result.total,
                        self.tr("Prepared", "已处理"),
                        preview.result.processed,
                        self.tr("Removed", "已移除"),
                        preview.result.removed,
                    ),
                    palette,
                ))
                .when(!preview.result.stages.is_empty(), |summary| {
                    summary
                        .child(section(self.tr("Step results", "步骤结果"), palette))
                        .children(preview.result.stages.iter().filter_map(|stage| {
                            let step = preview
                                .config
                                .0
                                .processing
                                .pipeline
                                .iter()
                                .find(|step| step.id == stage.id)?;
                            Some(description(
                                format!(
                                    "{} · {} → {} · {} {} · {} {} · {} {}",
                                    crate::gui::i18n::text(
                                        self.language,
                                        crate::gui::operation_label(&step.operation)
                                    ),
                                    stage.before,
                                    stage.after,
                                    self.tr("renamed", "已重命名"),
                                    stage.renamed,
                                    self.tr("modified", "已修改"),
                                    stage.modified,
                                    self.tr("skipped", "已跳过"),
                                    stage.skipped,
                                ),
                                palette,
                            ))
                        }))
                })
                .child(
                    row()
                        .when(self.allow_plaintext, |row| {
                            row.child(self.action(
                                "relay-copy-output",
                                self.tr("Copy nodes", "复制节点"),
                                palette,
                                false,
                                cx,
                                Self::copy_result,
                            ))
                            .child(self.action(
                                "relay-export",
                                self.tr("Export file", "导出文件"),
                                palette,
                                true,
                                cx,
                                Self::export_result,
                            ))
                        })
                        .when(!self.allow_plaintext, |row| {
                            row.child(self.action(
                                "relay-allow-plaintext",
                                self.tr("Allow plaintext output", "允许明文输出"),
                                palette,
                                false,
                                cx,
                                |this, cx| {
                                    this.allow_plaintext = true;
                                    cx.notify();
                                },
                            ))
                        })
                        .child(self.action(
                            "relay-node-names",
                            if self.showing_node_names {
                                self.tr("Hide node names", "隐藏节点名称")
                            } else {
                                self.tr("Show node names", "查看节点名称")
                            },
                            palette,
                            false,
                            cx,
                            |this, cx| {
                                this.showing_node_names = !this.showing_node_names;
                                cx.notify();
                            },
                        ))
                        .child(self.action(
                            "relay-review-gist",
                            self.tr("Review Gist publication", "审核 Gist 发布"),
                            palette,
                            false,
                            cx,
                            |this, cx| {
                                this.reviewing_publish = true;
                                cx.notify();
                            },
                        )),
                );
            if self.showing_node_names {
                let count = preview.names.len();
                let mut names = column().gap(px(5.)).child(description(
                    format!(
                        "{} {count}",
                        self.tr("Prepared node names shown", "显示的节点名称数")
                    ),
                    palette,
                ));
                for name in &preview.names {
                    names = names.child(description(name.to_string(), palette));
                }
                summary = summary.child(names);
            }
            if self.reviewing_publish {
                summary = summary
                    .child(description(self.tr(
                        "This sends the reviewed nodes to GitHub. Anyone with the Gist link can read them. Leave the destination empty to create a secret Gist.",
                        "节点将发送到 GitHub，持有 Gist 链接的人可以读取。留空目标可新建不公开列出的 Gist。",
                    ), palette))
                    .child(field(self.tr("Gist destination", "Gist 目标"), &self.output_url, palette))
                    .child(field(self.tr("Output filename", "输出文件名"), &self.output_name, palette))
                    .child(field(text(self.language, Text::RelayTitle), &self.output_title, palette))
                    .child(field(self.tr("GitHub token", "GitHub 令牌"), &self.token, palette))
                    .child(row()
                        .child(self.action("relay-confirm-gist", self.tr("Publish to secret Gist", "发布到私密 Gist"), palette, true, cx, Self::publish))
                        .child(self.action("relay-cancel-gist", self.tr("Cancel publication", "取消发布"), palette, false, cx, |this, cx| {
                            clear(&this.token, cx);
                            this.reviewing_publish = false;
                            cx.notify();
                        })))
                    .child(self.schedule_setup(palette, cx));
            }
            body = body.child(panel(palette).w_full().p_5().child(summary));
        }
        if let Some(url) = self
            .last_publication
            .as_ref()
            .and_then(|result| result.created.as_ref())
            .map(|gist| &gist.url)
        {
            let url = Zeroizing::new(url.clone());
            body = body.child(row().child(self.action(
                "relay-copy-gist",
                self.tr("Copy secret Gist link", "复制私密 Gist 链接"),
                palette,
                false,
                cx,
                move |this, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(url.to_string()));
                    this.feedback(true, this.tr("Secret link copied", "已复制私密链接"), cx);
                },
            )));
        }
        body
    }
}

pub(super) fn column() -> Div {
    div().flex().flex_col().items_start().gap(px(14.)).w_full()
}

pub(super) fn row() -> Div {
    div().flex().flex_wrap().items_center().gap(px(8.))
}

pub(super) fn section(label: impl Into<SharedString>, palette: Palette) -> Div {
    div()
        .text_color(palette.ink)
        .text_size(px(15.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(label.into())
}

pub(super) fn description(label: impl Into<SharedString>, palette: Palette) -> Div {
    div()
        .text_color(palette.muted)
        .text_size(px(13.))
        .child(label.into())
}

pub(super) fn field(label: &'static str, input: &Entity<TextInput>, palette: Palette) -> Div {
    column()
        .gap(px(5.))
        .child(description(label, palette))
        .child(input.clone())
}
