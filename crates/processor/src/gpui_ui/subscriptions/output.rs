//! Offline output compatibility checks and actions for a verified publication.

use super::super::{Palette, input::TextInput, localized, panel};
use super::view::{column, description, field, row, section};
use super::{Completed, Item, Preview, Source, Subscriptions, clear, gist_filename, set};
use crate::desktop::{
    i18n::{Text, text},
    relay_feedback::operation_failure_message,
};
use crate::relay::{
    RelayConfig, RelayError, RelayResult,
    export::{OutputFormat, export_subscription},
    gist::{GistFile, PolicyLink, default_policy_group_name, validate_policy_group_name},
    model::Subscription,
};
use gpui::{App, AppContext as _, ClipboardItem, Context, Div, Entity, prelude::*};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Destination {
    Local,
    Gist,
}

pub(super) struct FormatCheck {
    destination: Destination,
    format: OutputFormat,
    result: Result<(), RelayError>,
}

#[derive(Clone, Copy)]
struct GistTarget<'a> {
    url: &'a str,
    filename: &'a str,
    format: OutputFormat,
}

impl GistTarget<'_> {
    fn is_valid_existing_target(self) -> bool {
        let filename = gist_filename(self.filename, self.format);
        GistFile::parse(self.url.trim(), &filename).is_ok_and(|mut file| {
            file.id.zeroize();
            file.filename.zeroize();
            true
        })
    }
}

struct ApprovedGistTarget {
    url: Zeroizing<String>,
    filename: Zeroizing<String>,
    format: OutputFormat,
}

#[derive(Default)]
pub(super) struct PublicGistConsent(Option<ApprovedGistTarget>);

impl PublicGistConsent {
    fn allows(&self, target: GistTarget<'_>) -> bool {
        self.0.as_ref().is_some_and(|approved| {
            approved.url.as_str() == target.url
                && approved.filename.as_str() == target.filename
                && approved.format == target.format
        })
    }

    fn toggle(&mut self, target: GistTarget<'_>) {
        self.0 = (!self.allows(target) && target.is_valid_existing_target()).then(|| {
            ApprovedGistTarget {
                url: Zeroizing::new(target.url.to_owned()),
                filename: Zeroizing::new(target.filename.to_owned()),
                format: target.format,
            }
        });
    }

    fn revoke_if_changed(&mut self, target: GistTarget<'_>) {
        if !self.allows(target) {
            self.clear();
        }
    }

    pub(super) fn clear(&mut self) {
        self.0 = None;
    }
}

pub(super) struct Receipt {
    file: GistFile,
    format: OutputFormat,
    node_names: Vec<Zeroizing<String>>,
}

impl Receipt {
    pub(super) fn from_result(config: &RelayConfig, result: &RelayResult) -> Option<Self> {
        if !result.published && !result.unchanged {
            return None;
        }
        let target = result
            .created
            .as_ref()
            .map_or(config.output.as_str(), |gist| gist.url.as_str());
        let file = GistFile::parse(target.trim(), &config.output_file).ok()?;
        let node_names = if result.surge_policy.is_some() {
            let nodes = Subscription::parse(&result.content).ok()?;
            nodes
                .nodes()
                .iter()
                .map(|node| Zeroizing::new(node.name().to_owned()))
                .collect()
        } else {
            Vec::new()
        };
        Some(Self {
            file,
            format: config.output_format,
            node_names,
        })
    }

    fn url(&self) -> Zeroizing<String> {
        Zeroizing::new(format!("https://gist.github.com/{}", self.file.id))
    }

    fn matches(&self, target: &str, filename: &str, format: OutputFormat) -> bool {
        self.format == format
            && GistFile::parse(target.trim(), filename).is_ok_and(|file| {
                self.file.id.eq_ignore_ascii_case(&file.id) && self.file.filename == file.filename
            })
    }

    fn name_issue(&self, name: &str) -> Option<Text> {
        if !validate_policy_group_name(name) {
            Some(Text::RelaySurgeGroupNameInvalid)
        } else if self
            .node_names
            .iter()
            .any(|node| node.eq_ignore_ascii_case(name))
        {
            Some(Text::RelaySurgeGroupNameCollision)
        } else {
            None
        }
    }
}

impl Drop for Receipt {
    fn drop(&mut self) {
        self.file.id.zeroize();
        self.file.filename.zeroize();
    }
}

pub(super) struct Controls {
    local_format: OutputFormat,
    local_check: Option<FormatCheck>,
    gist_check: Option<FormatCheck>,
    receipt: Option<Receipt>,
    group_name: Entity<TextInput>,
}

impl Controls {
    pub(super) fn new(cx: &mut Context<Subscriptions>) -> Self {
        let group_name = cx.new(|cx| TextInput::new("", false, cx));
        cx.observe(&group_name, |_, _, cx| cx.notify()).detach();
        Self {
            local_format: OutputFormat::Original,
            local_check: None,
            gist_check: None,
            receipt: None,
            group_name,
        }
    }

    pub(super) fn local_format(&self) -> OutputFormat {
        self.local_format
    }

    pub(super) fn local_format_valid(&self) -> bool {
        self.local_check
            .as_ref()
            .is_none_or(|check| check.result.is_ok())
    }

    pub(super) fn gist_format_valid(&self) -> bool {
        self.gist_check
            .as_ref()
            .is_none_or(|check| check.result.is_ok())
    }

    pub(super) fn clear_checks(&mut self) {
        self.local_check = None;
        self.gist_check = None;
    }

    pub(super) fn clear_receipt(&mut self, cx: &mut Context<Subscriptions>) {
        self.receipt = None;
        clear(&self.group_name, cx);
    }
}

const FORMATS: [(OutputFormat, &str); 4] = [
    (OutputFormat::Original, "Original"),
    (OutputFormat::Surge, "Surge"),
    (OutputFormat::Mihomo, "Mihomo"),
    (OutputFormat::SingBox, "sing-box"),
];

impl Subscriptions {
    fn gist_target<'a>(&self, cx: &'a App) -> GistTarget<'a> {
        GistTarget {
            url: self.output_url.read(cx).value(),
            filename: self.output_name.read(cx).value(),
            format: self.format,
        }
    }

    pub(super) fn allows_public_gist(&self, preview: &Preview, cx: &App) -> bool {
        preview.public_gist_consent.allows(self.gist_target(cx))
    }

    pub(super) fn revoke_public_gist_if_changed(&mut self, cx: &App) {
        let target = self.gist_target(cx);
        if let Some(preview) = self.preview.as_mut() {
            preview.public_gist_consent.revoke_if_changed(target);
        }
    }

    fn toggle_public_gist(&mut self, cx: &mut Context<Self>) {
        if !self.reviewing_publish || self.current_preview(cx).is_none() {
            return;
        }
        let target = self.gist_target(cx);
        if let Some(preview) = self.preview.as_mut() {
            preview.public_gist_consent.toggle(target);
        }
        cx.notify();
    }

    pub(super) fn public_gist_controls(
        &self,
        preview: &Preview,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let target = self.gist_target(cx);
        if !target.is_valid_existing_target() {
            return None;
        }
        let approved = preview.public_gist_consent.allows(target);
        let label = if approved {
            Text::RelayPublicGistRevoke
        } else {
            Text::RelayPublicGistAllow
        };
        Some(
            column()
                .child(description(
                    text(self.language, Text::RelayPublicGistWarning),
                    palette,
                ))
                .child(self.action(
                    "relay-allow-public-gist",
                    text(self.language, label),
                    palette,
                    false,
                    cx,
                    Self::toggle_public_gist,
                )),
        )
    }

    pub(super) fn publish_button_text(&self, preview: &Preview, cx: &App) -> &'static str {
        let label = if self.allows_public_gist(preview, cx) {
            Text::RelayPublishWithPublicGistApproval
        } else if self.gist_target(cx).url.trim().is_empty() {
            Text::RelayPublishNewSecretGist
        } else {
            Text::RelayPublishGist
        };
        text(self.language, label)
    }

    fn format_for(&self, destination: Destination) -> OutputFormat {
        match destination {
            Destination::Local => self.output.local_format,
            Destination::Gist => self.format,
        }
    }

    fn select_format(
        &mut self,
        destination: Destination,
        format: OutputFormat,
        cx: &mut Context<Self>,
    ) {
        if self.format_for(destination) == format {
            return;
        }
        match destination {
            Destination::Local => {
                self.output.local_format = format;
                self.output.local_check = None;
                self.allow_plaintext = false;
            }
            Destination::Gist => {
                self.format = format;
                self.conceal_preview(cx);
                self.output.clear_receipt(cx);
                self.last_publication = None;
                if self
                    .preview
                    .as_ref()
                    .is_some_and(|preview| preview.result.script_target.is_some())
                {
                    self.preview = None;
                    self.feedback(
                        false,
                        text(self.language, Text::RelayReviewNeedsPreview),
                        cx,
                    );
                }
            }
        }
        cx.notify();
    }

    fn check_format(&mut self, destination: Destination, cx: &mut Context<Self>) {
        let Some(preview) = self.current_preview(cx) else {
            return;
        };
        if preview.result.content.is_empty() {
            return;
        }
        let format = self.format_for(destination);
        let content = preview.result.content.clone();
        self.start(
            move || {
                let result = export_subscription(&content, format).map(drop);
                Ok(Completed::FormatChecked(FormatCheck {
                    destination,
                    format,
                    result,
                }))
            },
            cx,
        );
    }

    pub(super) fn finish_format_check(&mut self, check: FormatCheck, cx: &mut Context<Self>) {
        if self.current_preview(cx).is_some() && self.format_for(check.destination) == check.format
        {
            match check.destination {
                Destination::Local => {
                    if check.result.is_err() {
                        self.allow_plaintext = false;
                    }
                    self.output.local_check = Some(check);
                }
                Destination::Gist => self.output.gist_check = Some(check),
            }
        }
        cx.notify();
    }

    fn format_controls(
        &self,
        destination: Destination,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Div {
        let tr = |key| text(self.language, key);
        let selected = self.format_for(destination);
        let has_contents = self
            .current_preview(cx)
            .is_some_and(|preview| !preview.result.content.is_empty());
        let (scope, title, help, check) = match destination {
            Destination::Local => (
                "relay-local-format",
                Text::RelayExportFormat,
                Text::RelayExportFormatHelp,
                self.output.local_check.as_ref(),
            ),
            Destination::Gist => (
                "relay-gist-format",
                Text::RelayPublishFormat,
                Text::RelayPublishFormatHelp,
                self.output.gist_check.as_ref(),
            ),
        };
        let formats = row().children(FORMATS.into_iter().enumerate().map(
            |(index, (format, label))| {
                self.action(
                    (scope, index),
                    if format == OutputFormat::Original {
                        tr(Text::RelayExportOriginal)
                    } else {
                        label
                    },
                    palette,
                    selected == format,
                    cx,
                    move |this, cx| this.select_format(destination, format, cx),
                )
            },
        ));
        let controls = column()
            .child(section(tr(title), palette))
            .child(formats)
            .child(description(tr(help), palette))
            .when(selected == OutputFormat::Surge, |view| {
                view.child(description(tr(Text::RelayExportSurgeVlessHelp), palette))
            });
        if !has_contents {
            return controls;
        }
        controls
            .child(self.action(
                (scope, FORMATS.len()),
                tr(Text::RelayExportCheck),
                palette,
                false,
                cx,
                move |this, cx| this.check_format(destination, cx),
            ))
            .when_some(
                check.filter(|check| check.format == selected),
                |view, check| {
                    let summary = match &check.result {
                        Ok(()) => tr(Text::RelayExportCheckPassed).to_owned(),
                        Err(error) => operation_failure_message(self.language, error),
                    };
                    view.child(description(summary, palette))
                },
            )
    }

    pub(super) fn local_format_controls(&self, palette: Palette, cx: &Context<Self>) -> Div {
        self.format_controls(Destination::Local, palette, cx)
    }

    pub(super) fn gist_format_controls(&self, palette: Palette, cx: &Context<Self>) -> Div {
        self.format_controls(Destination::Gist, palette, cx)
    }

    pub(super) fn remember_publication(
        &mut self,
        receipt: Option<Receipt>,
        result: &RelayResult,
        cx: &mut Context<Self>,
    ) {
        self.output.clear_receipt(cx);
        let Some(receipt) = receipt else { return };
        if result.created.is_some() {
            set(&self.output_url, &receipt.url(), cx);
            set(&self.output_name, &receipt.file.filename, cx);
        }
        let name = match self.source {
            Source::Saved(Item::Entry(id)) => self.entry(id).map(|item| item.name.as_str()),
            Source::Saved(Item::Collection(id)) => {
                self.collection(id).map(|item| item.name.as_str())
            }
            _ => None,
        };
        let mut suggestion = default_policy_group_name(name.unwrap_or_default());
        if receipt.name_issue(&suggestion).is_some() {
            let alternate = format!("{suggestion} group");
            if receipt.name_issue(&alternate).is_none() {
                suggestion = alternate;
            }
        }
        set(&self.output.group_name, &suggestion, cx);
        self.output.receipt = Some(receipt);
    }

    fn matching_publication(&self, cx: &App) -> Option<(&Receipt, &RelayResult)> {
        let receipt = self.output.receipt.as_ref()?;
        let result = self.last_publication.as_ref()?;
        let target = self.gist_target(cx);
        let filename = gist_filename(target.filename, target.format);
        receipt
            .matches(target.url, &filename, target.format)
            .then_some((receipt, result))
    }

    fn verified_policy(&self, cx: &App) -> Option<(&Receipt, &PolicyLink)> {
        let (receipt, result) = self.matching_publication(cx)?;
        let link = result.surge_policy.as_ref()?;
        (receipt.format == OutputFormat::Surge && link.matches(&receipt.file))
            .then_some((receipt, link))
    }

    pub(super) fn publication_receipt(&self, palette: Palette, cx: &Context<Self>) -> Option<Div> {
        let (_, result) = self.matching_publication(cx)?;
        let tr = |key| text(self.language, key);
        let verification = if result.unchanged {
            Text::RelayUnchanged
        } else {
            Text::RelayPublished
        };
        let card = panel(palette)
            .w_full()
            .p_4()
            .gap_3()
            .child(section(tr(verification), palette))
            .child(
                row()
                    .child(self.action(
                        "relay-open-verified-gist",
                        self.tr("Open verified Gist", "打开已核验的 Gist"),
                        palette,
                        false,
                        cx,
                        |this, cx| {
                            if let Some((receipt, _)) = this.matching_publication(cx) {
                                cx.open_url(&receipt.url());
                            }
                        },
                    ))
                    .child(self.action(
                        "relay-copy-verified-gist",
                        localized(
                            self.language,
                            "Copy verified Gist link",
                            "复制已核验的 Gist 链接",
                            "確認済みの Gist リンクをコピー",
                        ),
                        palette,
                        false,
                        cx,
                        |this, cx| {
                            if let Some((receipt, _)) = this.matching_publication(cx) {
                                let mut url = receipt.url();
                                cx.write_to_clipboard(ClipboardItem::new_string(std::mem::take(
                                    &mut *url,
                                )));
                                this.feedback(true, text(this.language, Text::Copied), cx);
                            }
                        },
                    )),
            );
        let Some((receipt, _)) = self.verified_policy(cx) else {
            return Some(card);
        };
        let name = self.output.group_name.read(cx).value().trim_matches(' ');
        let issue = receipt.name_issue(name);
        let handoff = column()
            .child(section(tr(Text::RelaySurgeHandoffTitle), palette))
            .child(description(tr(Text::RelaySurgeHandoffHelp), palette))
            .child(field(
                tr(Text::RelaySurgeGroupName),
                &self.output.group_name,
                palette,
            ))
            .when_some(issue, |view, issue| {
                view.child(description(tr(issue), palette))
            })
            .child(
                row()
                    .when(issue.is_none(), |row| {
                        row.child(self.action(
                            "relay-copy-surge-policy",
                            tr(Text::RelaySurgeHandoffCopy),
                            palette,
                            true,
                            cx,
                            |this, cx| {
                                if let Some((receipt, link)) = this.verified_policy(cx) {
                                    let name =
                                        this.output.group_name.read(cx).value().trim_matches(' ');
                                    if receipt.name_issue(name).is_none()
                                        && let Some(mut group) = link.named_policy_group(name)
                                    {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            std::mem::take(&mut *group),
                                        ));
                                        this.feedback(true, text(this.language, Text::Copied), cx);
                                    }
                                }
                            },
                        ))
                    })
                    .child(self.action(
                        "relay-copy-surge-subscription",
                        tr(Text::RelaySurgeCopySubscriptionLink),
                        palette,
                        false,
                        cx,
                        |this, cx| {
                            if let Some((_, link)) = this.verified_policy(cx) {
                                let mut url = link.subscription_url();
                                cx.write_to_clipboard(ClipboardItem::new_string(std::mem::take(
                                    &mut *url,
                                )));
                                this.feedback(true, text(this.language, Text::Copied), cx);
                            }
                        },
                    )),
            );
        Some(card.child(handoff))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_gist_consent_is_bound_to_the_preview_and_exact_edited_target() {
        let target = GistTarget {
            url: "0123456789abcdef0123456789abcdef",
            filename: "nodes.list",
            format: OutputFormat::Surge,
        };
        let mut consent = PublicGistConsent::default();
        assert!(!consent.allows(target));

        consent.toggle(target);
        consent.revoke_if_changed(target);
        assert!(consent.allows(target));
        assert!(!PublicGistConsent::default().allows(target));
        consent.clear();
        assert!(!consent.allows(target));

        for edited in [
            GistTarget {
                url: "abcdef0123456789abcdef0123456789",
                ..target
            },
            GistTarget {
                url: " 0123456789abcdef0123456789abcdef",
                ..target
            },
            GistTarget {
                filename: "other.list",
                ..target
            },
            GistTarget {
                format: OutputFormat::Mihomo,
                ..target
            },
        ] {
            consent.toggle(target);
            assert!(!consent.allows(edited));
            consent.revoke_if_changed(edited);
            assert!(!consent.allows(target));
        }
    }

    #[test]
    fn public_gist_consent_cannot_be_granted_without_an_existing_target() {
        let mut consent = PublicGistConsent::default();
        let target = GistTarget {
            url: "",
            filename: "nodes.list",
            format: OutputFormat::Surge,
        };
        consent.toggle(target);
        assert!(!consent.allows(target));
        let existing = GistTarget {
            url: "0123456789abcdef0123456789abcdef",
            filename: "",
            ..target
        };
        consent.toggle(existing);
        assert!(consent.allows(existing));
        consent.toggle(existing);
        assert!(!consent.allows(existing));
    }

    #[test]
    fn surge_receipt_is_bound_to_the_exact_gist_file_and_format() {
        let id = "0123456789abcdef0123456789abcdef";
        let receipt = Receipt {
            file: GistFile::parse(id, "nodes.list").unwrap(),
            format: OutputFormat::Surge,
            node_names: vec![Zeroizing::new("Example".to_owned())],
        };
        assert!(receipt.matches(
            &format!("https://gist.github.com/{id}"),
            "nodes.list",
            OutputFormat::Surge
        ));
        assert!(!receipt.matches(id, "other.list", OutputFormat::Surge));
        assert!(!receipt.matches(id, "nodes.list", OutputFormat::Mihomo));
        assert_eq!(
            receipt.name_issue("EXAMPLE"),
            Some(Text::RelaySurgeGroupNameCollision)
        );
        assert_eq!(
            receipt.name_issue("a,b"),
            Some(Text::RelaySurgeGroupNameInvalid)
        );
        assert_eq!(receipt.name_issue("Example group"), None);
    }
}
