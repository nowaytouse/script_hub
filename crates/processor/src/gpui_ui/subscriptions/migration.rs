//! Review portable backups and use the existing vault and age formats.

mod export;

use super::super::{Palette, input::TextInput, localized, panel};
use super::view::{column, description, field, row, section};
use super::{Completed, MAX_BACKUP, OpenRelay, Source, Subscriptions, clear, read_limited, set};
use crate::desktop::{
    i18n::{Text, text},
    relay_feedback::archive_error,
};
use crate::relay::{
    archive,
    backup::{self, RelayBackup},
    compat,
    library::{Entry, PublishSettings, Workspace},
    library_import,
    session_schedule::PrivateConfig,
};
use gpui::{
    AppContext as _, Context, Div, Entity, PathPromptOptions, Stateful, div, prelude::*, px,
};
use std::{path::Path, sync::Arc};
use zeroize::Zeroizing;

#[derive(Default)]
pub(super) struct Migration {
    review: Option<Review>,
    recipient: Option<Entity<TextInput>>,
}

impl Migration {
    pub(super) fn clear(&mut self) {
        self.review = None;
        self.recipient = None;
    }
}

enum Review {
    Archive {
        ciphertext: Arc<Zeroizing<Vec<u8>>>,
        key: Entity<TextInput>,
    },
    Portable(Arc<Prepared>),
}

enum Loaded {
    Archive(Zeroizing<Vec<u8>>),
    Portable(Prepared),
}

enum Backup {
    Hub(Box<RelayBackup>),
    SubStore(compat::Report),
}

struct Prepared {
    backup: Backup,
    summary: library_import::Summary,
}

fn prepare_review(bytes: &[u8], workspace: &Workspace) -> Result<Prepared, Text> {
    let backup = if let Ok(hub) = backup::decode(bytes) {
        Backup::Hub(Box::new(hub))
    } else {
        Backup::SubStore(compat::import(bytes).map_err(|error| match error {
            compat::ImportError::TooLarge => Text::RelayTooLarge,
            _ => Text::RelayBackupInvalid,
        })?)
    };
    let (_, summary) = merge_backup(workspace, &backup)?;
    Ok(Prepared { backup, summary })
}

fn merge_backup(
    workspace: &Workspace,
    backup: &Backup,
) -> Result<(Workspace, library_import::Summary), Text> {
    let hub = match backup {
        Backup::SubStore(report) => {
            return library_import::import(workspace, report, true)
                .map_err(|_| Text::WorkspaceStorageFailed);
        }
        Backup::Hub(backup) => backup,
    };
    let config = &hub.config;
    let title = config.title.trim();
    let name = library_import::unique_name(
        if title.is_empty() {
            "Imported subscription"
        } else {
            title
        },
        |name| {
            workspace
                .subscriptions
                .iter()
                .any(|entry| entry.name == name)
        },
        workspace.next_id,
    )
    .map_err(|_| Text::WorkspaceDraftInvalid)?;
    let mut candidate = workspace.clone();
    candidate
        .add_entry(Entry {
            id: 0,
            name,
            source: config.source.clone(),
            source_file: config.source_file.clone(),
            content: hub.content.to_string(),
            processing: config.processing.clone(),
            publish: PublishSettings {
                output: config.output.clone(),
                output_file: config.output_file.clone(),
                format: config.output_format,
                title: config.title.clone(),
                ..Default::default()
            },
        })
        .map_err(|_| Text::WorkspaceStorageFailed)?;
    Ok((
        candidate,
        library_import::Summary {
            subscriptions: 1,
            ..Default::default()
        },
    ))
}

fn read_backup(path: &Path, workspace: &Workspace) -> Result<Loaded, Text> {
    let bytes = read_limited(path, MAX_BACKUP).map_err(|_| Text::WorkspaceImportReadFailed)?;
    if archive::detect(&bytes) {
        Ok(Loaded::Archive(bytes))
    } else {
        prepare_review(&bytes, workspace).map(Loaded::Portable)
    }
}

fn decrypt_backup(ciphertext: &[u8], key: &str, workspace: &Workspace) -> Result<Prepared, Text> {
    let plaintext = archive::decrypt(ciphertext, key).map_err(archive_error)?;
    prepare_review(&plaintext, workspace)
}

impl Subscriptions {
    fn migration_available(&self) -> bool {
        self.library.is_open()
            && self.editor.is_none()
            && self.deleting.is_none()
            && !self.pending
            && !self.schedule_active()
    }

    fn migration_title(&self) -> &'static str {
        localized(
            self.language,
            "Review backup import",
            "审阅并导入备份",
            "バックアップを確認して読み込む",
        )
    }

    fn begin_migration_work(&mut self, cx: &mut Context<Self>) {
        self.pending = true;
        self.notice = None;
        self.set_pipeline_busy(true, cx);
        cx.notify();
    }

    fn finish_migration_load(
        &mut self,
        result: Result<Option<Loaded>, Text>,
        cx: &mut Context<Self>,
    ) {
        self.pending = false;
        self.set_pipeline_busy(false, cx);
        match result {
            Ok(Some(Loaded::Archive(ciphertext))) => {
                let key = cx
                    .new(|cx| TextInput::new(text(self.language, Text::ArchiveIdentity), true, cx));
                self.migration.review = Some(Review::Archive {
                    ciphertext: Arc::new(ciphertext),
                    key,
                });
            }
            Ok(Some(Loaded::Portable(prepared))) => {
                self.migration.review = Some(Review::Portable(Arc::new(prepared)));
            }
            Ok(None) => {}
            Err(error) => self.feedback(false, text(self.language, error), cx),
        }
        cx.notify();
    }

    fn choose_migration_backup(&mut self, cx: &mut Context<Self>) {
        if !self.migration_available() || self.migration.review.is_some() {
            return;
        }
        self.begin_migration_work(cx);
        let workspace = self.workspace.clone();
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(self.migration_title().into()),
        });
        let executor = cx.background_executor().clone();
        cx.spawn(async move |weak, cx| {
            let result = match picker.await {
                Ok(Ok(Some(files))) => {
                    executor
                        .spawn(async move {
                            let path = files.first().ok_or(Text::WorkspaceImportReadFailed)?;
                            read_backup(path, &workspace).map(Some)
                        })
                        .await
                }
                Ok(Ok(None)) => Ok(None),
                _ => Err(Text::WorkspaceImportReadFailed),
            };
            let _ = weak.update(cx, |this, cx| this.finish_migration_load(result, cx));
        })
        .detach();
    }

    fn unlock_migration_backup(&mut self, cx: &mut Context<Self>) {
        if !self.migration_available() {
            return;
        }
        let Some(Review::Archive { ciphertext, key }) = &self.migration.review else {
            return;
        };
        let ciphertext = ciphertext.clone();
        let key_value = Zeroizing::new(key.read(cx).value().trim().to_owned());
        clear(key, cx);
        let workspace = self.workspace.clone();
        self.begin_migration_work(cx);
        let task = cx.background_executor().spawn(async move {
            decrypt_backup(&ciphertext, &key_value, &workspace)
                .map(Loaded::Portable)
                .map(Some)
        });
        cx.spawn(async move |weak, cx| {
            let result = task.await;
            let _ = weak.update(cx, |this, cx| this.finish_migration_load(result, cx));
        })
        .detach();
    }

    fn save_migration(&mut self, cx: &mut Context<Self>) {
        if !self.migration_available() {
            return;
        }
        let (Some(Review::Portable(prepared)), Some(library)) =
            (&self.migration.review, self.library.writer())
        else {
            return;
        };
        let summary = prepared.summary;
        if summary.subscriptions + summary.collections == 0 {
            return;
        }
        let prepared = prepared.clone();
        let workspace = self.workspace.clone();
        let language = self.language;
        let review_again = localized(
            language,
            "The library changed. Review the backup again before importing.",
            "资料库已更改，请重新审阅备份后再导入。",
            "ライブラリが変更されました。バックアップを確認し直してください。",
        );
        self.begin_migration_work(cx);
        let task = cx.background_executor().spawn(async move {
            let (candidate, current_summary) = merge_backup(&workspace, &prepared.backup)
                .map_err(|error| text(language, error).to_owned())?;
            if current_summary != summary {
                return Err(review_again.into());
            }
            library.save(&candidate)?;
            Ok::<Workspace, String>(candidate)
        });
        cx.spawn(async move |weak, cx| {
            let result = task.await;
            let _ = weak.update(cx, |this, cx| match result {
                Ok(candidate) => {
                    this.migration.clear();
                    this.complete(Ok(Completed::Saved(candidate, "")), cx);
                    let notice = text(this.language, Text::WorkspaceImportSummary)
                        .replace("{subscriptions}", &summary.subscriptions.to_string())
                        .replace("{collections}", &summary.collections.to_string())
                        .replace("{skipped}", &summary.skipped.to_string());
                    this.feedback(true, notice, cx);
                }
                Err(error) => this.complete(Err(error), cx),
            });
        })
        .detach();
    }

    fn open_migrated_source(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.migration_available() {
            return;
        }
        let Some(Review::Portable(prepared)) = &self.migration.review else {
            return;
        };
        let Backup::SubStore(report) = &prepared.backup else {
            return;
        };
        let Ok(config) = report.select_item(index, true) else {
            self.feedback(false, text(self.language, Text::RelayBackupInvalid), cx);
            return;
        };
        let mut config = PrivateConfig(config);
        let source = config
            .0
            .local_content
            .take()
            .map_or(Source::Link, Source::Local);
        self.migration.clear();
        self.set_source(source, cx);
        for field in [
            &self.relay_url,
            &self.relay_file,
            &self.source_token,
            &self.output_url,
            &self.output_name,
            &self.output_title,
        ] {
            clear(field, cx);
        }
        if matches!(self.source, Source::Link) {
            set(&self.relay_url, &config.0.source, cx);
        }
        self.format = config.0.output_format;
        self.rename = config.0.rename;
        self.sort = config.0.sort;
        let steps = std::mem::take(&mut config.0.processing.pipeline);
        self.relay_pipeline
            .update(cx, |editor, cx| editor.replace(steps, cx));
        self.showing_relay = true;
        cx.emit(OpenRelay);
        self.feedback(
            true,
            localized(
                self.language,
                "Backup source opened. Run a preview to continue.",
                "备份来源已打开。运行预览后继续。",
                "バックアップのソースを開きました。続行するにはプレビューを実行してください。",
            ),
            cx,
        );
    }

    pub(super) fn migration_button(&self, palette: Palette, cx: &Context<Self>) -> Stateful<Div> {
        self.action(
            "import-portable-backup",
            self.migration_title(),
            palette,
            false,
            cx,
            Self::choose_migration_backup,
        )
    }

    pub(super) fn migration_review(&self, palette: Palette, cx: &Context<Self>) -> Option<Div> {
        let tr = |key| text(self.language, key);
        let body = match self.migration.review.as_ref()? {
            Review::Archive { key, .. } => column()
                .child(section(tr(Text::ArchiveUnlock), palette))
                .child(description(tr(Text::ArchiveHelp), palette))
                .child(field(tr(Text::ArchiveIdentity), key, palette))
                .child(row().child(self.action(
                    "unlock-portable-backup",
                    tr(Text::ArchiveUnlock),
                    palette,
                    true,
                    cx,
                    Self::unlock_migration_backup,
                ))),
            Review::Portable(prepared) => self.portable_review(prepared, palette, cx),
        };
        Some(
            panel(palette)
                .w_full()
                .p_5()
                .child(body.child(row().child(self.action(
                    "cancel-portable-backup",
                    tr(Text::Cancel),
                    palette,
                    false,
                    cx,
                    |this, cx| {
                        if let Some(Review::Archive { key, .. }) = &this.migration.review {
                            clear(key, cx);
                        }
                        this.migration.clear();
                        this.notice = None;
                        cx.notify();
                    },
                )))),
        )
    }

    fn portable_review(&self, prepared: &Prepared, palette: Palette, cx: &Context<Self>) -> Div {
        let detail = match &prepared.backup {
            Backup::SubStore(report) => self.substore_details(report, palette, cx),
            Backup::Hub(backup) => column()
                .child(section(
                    localized(
                        self.language,
                        "Script Hub backup preview",
                        "Script Hub 备份预览",
                        "Script Hub バックアップの確認",
                    ),
                    palette,
                ))
                .when(!backup.config.title.trim().is_empty(), |body| {
                    body.child(description(backup.config.title.clone(), palette))
                })
                .child(description(
                    localized(
                        self.language,
                        "Save one subscription and its processing settings to the encrypted library.",
                        "将一个订阅及其处理设置保存到加密资料库。",
                        "サブスクリプションとその処理設定を暗号化ライブラリへ保存します。",
                    ),
                    palette,
                )),
        };
        let summary = prepared.summary;
        let proposed = localized(
            self.language,
            "Will add {subscriptions} subscriptions and {collections} collections. {skipped} items will be skipped.",
            "将添加 {subscriptions} 个订阅和 {collections} 个合集，跳过 {skipped} 个项目。",
            "サブスクリプション {subscriptions} 件とコレクション {collections} 件を追加し、{skipped} 件をスキップします。",
        )
        .replace("{subscriptions}", &summary.subscriptions.to_string())
        .replace("{collections}", &summary.collections.to_string())
        .replace("{skipped}", &summary.skipped.to_string());
        detail.child(description(proposed, palette)).when(
            summary.subscriptions + summary.collections > 0,
            |review| {
                review.child(row().child(self.action(
                    "save-portable-backup-import",
                    text(self.language, Text::WorkspaceImportAll),
                    palette,
                    true,
                    cx,
                    Self::save_migration,
                )))
            },
        )
    }

    fn substore_details(
        &self,
        report: &compat::Report,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Div {
        let tr = |key| text(self.language, key);
        let switches = report
            .items
            .iter()
            .map(|item| item.operations.len())
            .sum::<usize>()
            + report
                .collections
                .iter()
                .map(|item| item.operations.len())
                .sum::<usize>();
        let collections = localized(self.language, "collections", "个合集", "コレクション");
        let breakdown = format!(
            "{} {} · {} {} · {} {} · {} {}",
            report.items.len(),
            tr(Text::MigrationItems),
            report.collections.len(),
            collections,
            switches,
            tr(Text::WorkspaceOperationsTab),
            report.unsupported_count,
            tr(Text::MigrationUnsupported),
        );
        let names = column()
            .children(report.items.iter().enumerate().map(|(index, item)| {
                row()
                    .child(div().flex_1().min_w_0().child(item.name.to_string()))
                    .when(item.content.is_some() || item.source.is_some(), |entry| {
                        entry.child(self.action(
                            ("use-substore-source", index),
                            tr(Text::MigrationChoose),
                            palette,
                            false,
                            cx,
                            move |this, cx| this.open_migrated_source(index, cx),
                        ))
                    })
            }))
            .children(report.collections.iter().map(|collection| {
                div().child(format!("{} · {collections}", collection.name.as_str()))
            }));
        column()
            .child(section(tr(Text::MigrationPreview), palette))
            .child(description(breakdown, palette))
            .child(description(localized(
                self.language,
                "Open one source in Relay or add compatible items to the encrypted library. Compatible processing steps are converted. Unsupported options, scripts, settings and tokens are skipped.",
                "可以在中转页打开一个来源，或将兼容项目加入加密资料库。兼容的处理步骤会转换，不受支持的选项、脚本、设置和令牌会跳过。",
                "ソースを一件リレーで開くか、対応する項目を暗号化ライブラリに追加します。対応する処理は変換され、非対応の設定、スクリプト、トークンはスキップされます。",
            ), palette))
            .child(
                div()
                    .id("substore-review-names")
                    .max_h(px(230.))
                    .overflow_y_scroll()
                    .p_3()
                    .rounded_md()
                    .bg(palette.subtle)
                    .child(names),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use age::secrecy::ExposeSecret as _;
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    #[test]
    fn review_accepts_plain_base64_and_age_and_proposes_the_same_vault_import() {
        let fixture = crate::relay::demo::backup();
        let original = Workspace::default();
        let plain = prepare_review(fixture, &original).unwrap();
        let encoded = STANDARD.encode(fixture);
        assert_eq!(
            prepare_review(encoded.as_bytes(), &original)
                .unwrap()
                .summary,
            plain.summary
        );

        let identity = age::x25519::Identity::generate();
        let encrypted = archive::encrypt(fixture, &identity.to_public().to_string()).unwrap();
        let wrong = age::x25519::Identity::generate().to_string();
        assert!(matches!(
            decrypt_backup(&encrypted, wrong.expose_secret(), &original),
            Err(Text::ArchiveInvalid)
        ));
        let decrypted =
            decrypt_backup(&encrypted, identity.to_string().expose_secret(), &original).unwrap();
        assert_eq!(decrypted.summary, plain.summary);
        assert_eq!(plain.summary.subscriptions, 2);
        assert_eq!(plain.summary.collections, 1);
        assert_eq!(plain.summary.skipped, 0);
        assert!(original.subscriptions.is_empty());
    }

    #[test]
    fn encrypted_hub_import_keeps_saved_processing_and_avoids_name_collisions() {
        use crate::relay::{
            RelayConfig,
            export::OutputFormat,
            model::pipeline::{Operation, Step},
        };

        let fixture = "Sample = socks5, sample.example.invalid, 1080\n";
        let mut config = RelayConfig {
            title: "Portable sample".into(),
            output_format: OutputFormat::Surge,
            ..Default::default()
        };
        config.processing.pipeline.push(Step {
            id: 1,
            enabled: true,
            operation: Operation::Deduplicate,
        });
        let serialized = backup::encode(&config, fixture).unwrap();
        let first = prepare_review(&serialized, &Workspace::default()).unwrap();
        let (workspace, _) = merge_backup(&Workspace::default(), &first.backup).unwrap();

        let identity = age::x25519::Identity::generate();
        let ciphertext = archive::encrypt(&serialized, &identity.to_public().to_string()).unwrap();
        let prepared = decrypt_backup(
            &ciphertext,
            identity.to_string().expose_secret(),
            &workspace,
        )
        .unwrap();
        let (merged, summary) = merge_backup(&workspace, &prepared.backup).unwrap();
        assert_eq!(summary.subscriptions, 1);
        assert_eq!(workspace.subscriptions.len(), 1);
        assert_eq!(merged.subscriptions.len(), 2);
        assert_ne!(merged.subscriptions[0].name, merged.subscriptions[1].name);
        let added = &merged.subscriptions[1];
        assert_eq!(added.content, fixture);
        assert_eq!(added.publish.format, OutputFormat::Surge);
        let [step] = added.processing.pipeline.as_slice() else {
            panic!("The submitted processing step should survive the import");
        };
        assert_eq!(step.id, 1);
        assert!(step.enabled && matches!(step.operation, Operation::Deduplicate));
    }
}
