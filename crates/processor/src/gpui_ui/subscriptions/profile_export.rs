use super::super::{Palette, localized, panel};
use super::view::{description, row, section};
use super::{Completed, FileMode, Subscriptions};
use crate::desktop::relay_feedback::operation_failure_message;
use crate::relay::{
    export::OutputFormat,
    profile::{Error, MihomoTemplate},
};
use crate::runtime::settings::Language;
use gpui::{Div, Stateful, div, prelude::*, px};
use std::{path::Path, sync::Arc};
use zeroize::Zeroizing;

#[derive(Default)]
pub(super) enum Form {
    #[default]
    Choose,
    Loaded {
        template: Arc<MihomoTemplate>,
        filename: Zeroizing<String>,
        group: Option<usize>,
    },
}

impl Form {
    pub(super) fn load(path: &Path, language: Language) -> Result<Self, String> {
        let bytes = super::read_limited(path, crate::relay::profile::MAX_TEMPLATE_BYTES)?;
        let yaml = std::str::from_utf8(&bytes).map_err(|_| {
            localized(
                language,
                "Choose a UTF-8 YAML file.",
                "请选择 UTF-8 编码的 YAML 文件。",
                "UTF-8 の YAML ファイルを選んでください。",
            )
        })?;
        let template =
            MihomoTemplate::parse(yaml).map_err(|error| error_message(language, error))?;
        let group = (template.groups().len() == 1).then_some(0);
        Ok(Self::Loaded {
            template: Arc::new(template),
            filename: Zeroizing::new(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            ),
            group,
        })
    }
}

fn error_message(language: Language, error: Error) -> String {
    match error {
        Error::Export(error) => return operation_failure_message(language, &error),
        error if language != Language::ZhCn => return error.to_string(),
        Error::TooLarge => "模板或生成的配置超过 4 MiB。",
        Error::InvalidTemplate => "无法读取该 Mihomo YAML。请检查文件结构和重复键。",
        Error::NoEligibleGroup => "模板没有可接收节点的策略组。请选用一个可添加节点的 select、url-test、fallback 或 load-balance 组。",
        Error::InvalidGroup => "请从当前模板选择策略组。",
        Error::NameConflict => "节点或策略组名称重复，或与 Mihomo 内建策略重名。请调整名称后重试。",
        Error::InvalidNodes => "当前预览无法生成有效的 Mihomo 节点列表。",
        Error::Render => "合并后的配置超出支持的 YAML 结构或大小限制。",
    }
    .to_owned()
}

impl Subscriptions {
    pub(super) fn mihomo_profile_button(
        &self,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        self.action(
            "mihomo-profile-review",
            localized(
                self.language,
                "Build Mihomo profile",
                "生成 Mihomo 完整配置",
                "Mihomo 設定を生成",
            ),
            palette,
            false,
            cx,
            Self::begin_mihomo_profile,
        )
    }

    fn begin_mihomo_profile(&mut self, cx: &mut Context<Self>) {
        let Some(preview) = self.with_preview(cx) else {
            return;
        };
        if preview
            .result
            .script_target
            .is_some_and(|format| format != OutputFormat::Mihomo)
        {
            self.feedback(
                false,
                localized(
                    self.language,
                    "The processing script uses the preview target. Choose Mihomo under Gist output format and run the preview again.",
                    "处理脚本依赖预览目标。请在「Gist 输出格式」选择 Mihomo，再运行预览。",
                    "処理スクリプトはプレビュー形式に依存します。「Gist 出力形式」で Mihomo を選んで再プレビューしてください。",
                ),
                cx,
            );
            return;
        }
        self.conceal_preview(cx);
        self.profile_export = Some(Form::default());
        self.notice = None;
        cx.notify();
    }

    pub(super) fn finish_mihomo_template(
        &mut self,
        result: Result<Form, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = &mut self.profile_export else {
            return;
        };
        match result {
            Ok(selected) => {
                *form = selected;
                cx.notify();
            }
            Err(message) => {
                *form = Form::Choose;
                self.feedback(false, message, cx);
            }
        }
    }

    pub(super) fn mihomo_profile_form(&self, palette: Palette, cx: &Context<Self>) -> Option<Div> {
        let form = self.profile_export.as_ref()?;
        let local = |en, zh, ja| localized(self.language, en, zh, ja);
        let pick = self.action(
            "mihomo-profile-template",
            if matches!(form, Form::Loaded { .. }) {
                local("Change template", "更换模板", "テンプレートを変更")
            } else {
                local("Choose template", "选择模板", "テンプレートを選択")
            },
            palette,
            matches!(form, Form::Choose),
            cx,
            |this, cx| this.select_file(FileMode::MihomoTemplate, cx),
        );
        let mut card = panel(palette)
            .p_5()
            .gap_4()
            .child(section(
                local("Build Mihomo profile", "生成 Mihomo 完整配置", "Mihomo 設定を生成"),
                palette,
            ))
            .child(description(
                local(
                    "Choose a local YAML configuration and a group for these prepared nodes. The result is saved as a new file.",
                    "选择本地 YAML 配置，以及接收本次预览节点的策略组。配置会另存为新文件。",
                    "ローカルの YAML 設定とノードの追加先グループを選びます。設定は別ファイルに保存されます。",
                ),
                palette,
            ));
        if let Form::Loaded {
            template,
            filename,
            group,
        } = form
        {
            card = card
                .child(description(
                    format!("{} · {}", local("Template", "模板", "テンプレート"), filename.as_str()),
                    palette,
                ))
                .child(section(local("Proxy group", "策略组", "プロキシグループ"), palette))
                .child(
                    div()
                        .id("mihomo-template-groups")
                        .max_h(px(160.))
                        .overflow_y_scroll()
                        .child(row().children(template.groups().iter().enumerate().map(
                            |(index, name)| {
                                self.action(
                                    ("mihomo-target-group", index),
                                    name.clone(),
                                    palette,
                                    *group == Some(index),
                                    cx,
                                    move |this, cx| {
                                        if let Some(Form::Loaded { group, .. }) = &mut this.profile_export {
                                            *group = Some(index);
                                            this.notice = None;
                                            cx.notify();
                                        }
                                    },
                                )
                            },
                        ))),
                )
                .child(description(
                    local(
                        "The YAML contains private node details. Save it to a local file you can import into FlClash or Mihomo.",
                        "YAML 包含私密节点信息。请保存到本地，再导入 FlClash 或 Mihomo。",
                        "YAML には非公開のノード情報が含まれます。ローカルに保存し、FlClash または Mihomo にインポートしてください。",
                    ),
                    palette,
                ));
        }
        Some(
            card.child(
                row()
                    .child(pick)
                    .when(matches!(form, Form::Loaded { group: Some(_), .. }), |row| {
                        row.child(self.action(
                            "mihomo-profile-export",
                            local(
                                "Export private YAML",
                                "导出私密 YAML",
                                "非公開 YAML を書き出す",
                            ),
                            palette,
                            true,
                            cx,
                            Self::export_mihomo_profile,
                        ))
                    })
                    .child(self.action(
                        "mihomo-profile-cancel",
                        local("Cancel", "取消", "キャンセル"),
                        palette,
                        false,
                        cx,
                        |this, cx| {
                            this.profile_export = None;
                            this.notice = None;
                            cx.notify();
                        },
                    )),
            ),
        )
    }

    fn export_mihomo_profile(&mut self, cx: &mut Context<Self>) {
        let Some(Form::Loaded {
            template,
            group: Some(index),
            ..
        }) = &self.profile_export
        else {
            return;
        };
        let Some(group) = template.groups().get(*index).cloned() else {
            return;
        };
        let template = Arc::clone(template);
        let Some(preview) = self.with_preview(cx) else {
            return;
        };
        let content = preview.result.content.clone();
        let language = self.language;
        let notice = localized(
            self.language,
            "Private Mihomo profile exported",
            "私密 Mihomo 配置已导出",
            "非公開の Mihomo 設定を書き出しました",
        );
        self.save_text_export(
            "scripthub-mihomo.yaml",
            move || {
                template
                    .build(&content, &group)
                    .map_err(|error| error_message(language, error))
            },
            crate::relay::profile::write_private_yaml,
            Completed::ProfileExported(notice),
            cx,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choosing_another_template_binds_its_filename_and_groups() {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let name = format!("profile-choice-{}-{id}.yaml", std::process::id());
        let path = std::env::temp_dir().canonicalize().unwrap().join(&name);
        let single = "proxy-groups:\n  - {name: First, type: select, proxies: [DIRECT]}\n";
        std::fs::write(&path, single).unwrap();
        let first = Form::load(&path, Language::En).unwrap();
        std::fs::write(
            &path,
            format!("{single}  - {{name: Second, type: select, proxies: [DIRECT]}}\n"),
        )
        .unwrap();
        let second = Form::load(&path, Language::En).unwrap();
        std::fs::write(&path, b"invalid: [").unwrap();
        let invalid = Form::load(&path, Language::En);
        std::fs::remove_file(path).unwrap();

        assert!(
            matches!(first, Form::Loaded { filename, group: Some(0), .. } if filename.as_str() == name)
        );
        assert!(matches!(second, Form::Loaded { group: None, .. }));
        assert!(invalid.is_err());
    }
}
