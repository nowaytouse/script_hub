use super::super::super::{Palette, input::TextInput, localized};
use super::super::view::{column, description, field, row, section};
use super::super::{Completed, Subscriptions, clear, documents_directory, message};
use crate::desktop::{
    i18n::{Text, text},
    relay_feedback::archive_error,
    template_paths::safe_export_destination,
};
use crate::relay::{RelayConfig, archive, backup};
use crate::runtime::settings::Language;
use gpui::{AppContext as _, Context, Div, prelude::*};
use zeroize::Zeroizing;

enum Encoding {
    Plain,
    Encrypted(Zeroizing<String>),
}

impl Encoding {
    fn filename(&self) -> &'static str {
        match self {
            Self::Plain => "scripthub-relay.json",
            Self::Encrypted(_) => "scripthub-relay.age",
        }
    }
}

fn encode(
    config: &RelayConfig,
    content: &str,
    encoding: &Encoding,
    language: Language,
) -> Result<Zeroizing<Vec<u8>>, String> {
    let bytes = backup::encode(config, content).map_err(message)?;
    match encoding {
        Encoding::Plain => Ok(bytes),
        Encoding::Encrypted(recipient) => archive::encrypt(&bytes, recipient)
            .map_err(|error| text(language, archive_error(error)).into()),
    }
}

impl Subscriptions {
    pub(in crate::gpui_ui::subscriptions) fn clear_backup_recipient(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if let Some(recipient) = self.migration.recipient.take() {
            clear(&recipient, cx);
        }
    }

    fn review_encrypted_backup_export(&mut self, cx: &mut Context<Self>) {
        if self.current_preview(cx).is_none() || self.migration.recipient.is_some() {
            return;
        }
        self.migration.recipient = Some(
            cx.new(|cx| TextInput::new(text(self.language, Text::ArchiveRecipient), false, cx)),
        );
        cx.notify();
    }

    fn export_encrypted_backup(&mut self, cx: &mut Context<Self>) {
        let Some(recipient) = self.migration.recipient.as_ref() else {
            return;
        };
        let recipient = Zeroizing::new(recipient.read(cx).value().trim().to_owned());
        if recipient.parse::<age::x25519::Recipient>().is_err() {
            self.feedback(false, text(self.language, Text::ArchiveKeyInvalid), cx);
            return;
        }
        self.export_backup_file(Encoding::Encrypted(recipient), cx);
    }

    fn export_backup_file(&mut self, encoding: Encoding, cx: &mut Context<Self>) {
        if matches!(encoding, Encoding::Plain) && !self.allow_plaintext {
            return;
        }
        let Some(preview) = self.current_preview(cx) else {
            return;
        };
        let mut config = self.publication_config(preview, cx);
        config.0.output_format = self.format;
        let content = preview.result.content.clone();
        self.allow_plaintext = false;
        self.clear_backup_recipient(cx);
        let language = self.language;
        let success = match encoding {
            Encoding::Plain => localized(
                language,
                "Subscription backup exported.",
                "订阅备份已导出。",
                "サブスクリプションのバックアップを書き出しました。",
            ),
            Encoding::Encrypted(_) => localized(
                language,
                "Encrypted subscription backup exported.",
                "加密订阅备份已导出。",
                "暗号化したサブスクリプションのバックアップを書き出しました。",
            ),
        };
        let unsafe_destination = localized(
            language,
            "Choose a backup destination outside Git repositories and vault folders.",
            "请选择 Git 仓库和保险库文件夹以外的备份位置。",
            "Git リポジトリと保管庫フォルダーの外にあるバックアップ先を選んでください。",
        );
        let picker_error = localized(
            language,
            "Could not choose a backup destination.",
            "无法选择备份保存位置。",
            "バックアップ先を選択できませんでした。",
        );
        let prompt = cx.prompt_for_new_path(&documents_directory(), Some(encoding.filename()));
        let executor = cx.background_executor().clone();
        self.begin_migration_work(cx);
        cx.spawn(async move |weak, cx| {
            let result = match prompt.await {
                Ok(Ok(Some(path))) => {
                    executor
                        .spawn(async move {
                            if !safe_export_destination(&path, None) {
                                return Err(unsafe_destination.into());
                            }
                            let bytes = encode(&config.0, &content, &encoding, language)?;
                            crate::vault::write_private(&path, &bytes, false).map_err(message)?;
                            Ok(Completed::Notice(success))
                        })
                        .await
                }
                Ok(Ok(None)) => Ok(Completed::Cancelled),
                _ => Err(picker_error.into()),
            };
            let _ = weak.update(cx, |this, cx| this.complete(result, cx));
        })
        .detach();
    }

    pub(in crate::gpui_ui::subscriptions) fn portable_backup_controls(
        &self,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Div {
        let tr = |key| text(self.language, key);
        let body = column().child(section(tr(Text::RelayOutputBackupOptions), palette));
        if let Some(recipient) = &self.migration.recipient {
            return body
                .child(description(tr(Text::ArchiveExportHelp), palette))
                .child(field(tr(Text::ArchiveRecipient), recipient, palette))
                .child(
                    row()
                        .child(self.action(
                            "export-encrypted-relay-backup",
                            tr(Text::ArchiveExport),
                            palette,
                            true,
                            cx,
                            Self::export_encrypted_backup,
                        ))
                        .child(self.action(
                            "cancel-encrypted-relay-backup",
                            tr(Text::Cancel),
                            palette,
                            false,
                            cx,
                            |this, cx| {
                                this.clear_backup_recipient(cx);
                                cx.notify();
                            },
                        )),
                );
        }
        body.when(self.allow_plaintext, |body| {
            body.child(description(tr(Text::RelayBackupWarning), palette))
        })
        .child(
            row()
                .child(self.action(
                    "review-encrypted-relay-backup",
                    tr(Text::ArchiveExport),
                    palette,
                    false,
                    cx,
                    Self::review_encrypted_backup_export,
                ))
                .when(self.allow_plaintext, |row| {
                    row.child(self.action(
                        "export-plaintext-relay-backup",
                        tr(Text::RelayExportBackup),
                        palette,
                        false,
                        cx,
                        |this, cx| this.export_backup_file(Encoding::Plain, cx),
                    ))
                }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use age::secrecy::ExposeSecret as _;

    #[test]
    fn exported_age_backup_retains_the_reviewed_source_for_import() {
        let identity = age::x25519::Identity::generate();
        let key = Encoding::Encrypted(Zeroizing::new(identity.to_public().to_string()));
        let original = "Sample = socks5, sample.example.invalid, 1080\n";
        let config = RelayConfig {
            expected_source: Some(Zeroizing::new(original.to_owned())),
            ..Default::default()
        };
        let ciphertext = encode(&config, "filtered", &key, Language::En).unwrap();
        assert!(archive::detect(&ciphertext));
        let decrypted =
            archive::decrypt(&ciphertext, identity.to_string().expose_secret()).unwrap();
        let stored = backup::decode(&decrypted).unwrap();
        assert_eq!(stored.content.as_str(), original);
        assert!(!stored.config.allow_resolution);
    }
}
