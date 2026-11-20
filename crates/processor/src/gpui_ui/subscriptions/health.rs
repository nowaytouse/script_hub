//! Explicit, private connectivity checks on a reviewed subscription.

use super::super::{Palette, input::TextInput, localized, panel};
use super::view::{column, description, field, row, section};
use super::{Subscriptions, set};
use crate::desktop::i18n::{Text, text};
use crate::relay::{
    RelayResult,
    engine::{self, ProbeDetail, ProbeOptions, ProbeReason},
};
use gpui::{App, AppContext as _, Context, Div, Entity, PathPromptOptions, div, prelude::*, px};
use std::{path::PathBuf, time::Duration};

pub(super) struct Controls {
    open: bool,
    detected: bool,
    engine: Option<PathBuf>,
    parallel: Entity<TextInput>,
    timeout: Entity<TextInput>,
    compatibility: bool,
    doh: bool,
    show_details: bool,
}

impl Controls {
    pub(super) fn new(cx: &mut Context<Subscriptions>) -> Self {
        let defaults = ProbeOptions::default();
        let parallel = cx.new(|cx| TextInput::new("", false, cx));
        let timeout = cx.new(|cx| TextInput::new("", false, cx));
        set(&parallel, &defaults.concurrency.to_string(), cx);
        set(&timeout, &defaults.timeout.as_secs().to_string(), cx);
        Self {
            open: false,
            detected: false,
            engine: None,
            parallel,
            timeout,
            compatibility: false,
            doh: false,
            show_details: false,
        }
    }

    pub(super) fn clear_transient(&mut self) {
        self.compatibility = false;
        self.doh = false;
        self.show_details = false;
    }

    pub(super) fn engine_selected(&self) -> bool {
        self.engine.is_some()
    }

    fn options(&self, cx: &App) -> Result<ProbeOptions, Text> {
        let concurrency = self
            .parallel
            .read(cx)
            .value()
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|value| (1..=8).contains(value))
            .ok_or(Text::RelayOptionsInvalid)?;
        let timeout = self
            .timeout
            .read(cx)
            .value()
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|value| (1..=20).contains(value))
            .ok_or(Text::RelayOptionsInvalid)?;
        let engine = self.engine.clone().ok_or(Text::RelayEngineUnavailable)?;
        Ok(ProbeOptions {
            engine,
            concurrency,
            timeout: Duration::from_secs(timeout),
            compatibility: self.compatibility,
            doh: self.doh,
        })
    }
}

impl Subscriptions {
    pub(in crate::gpui_ui) fn checker_selected(&self) -> bool {
        self.health.engine_selected()
    }

    fn toggle_health_panel(&mut self, cx: &mut Context<Self>) {
        self.health.open = !self.health.open;
        if self.health.open && !self.health.detected {
            self.detect_health_engine(cx);
        } else if !self.health.open {
            self.health.clear_transient();
        }
        cx.notify();
    }

    fn detect_health_engine(&mut self, cx: &mut Context<Self>) {
        self.pending = true;
        self.set_pipeline_busy(true, cx);
        let detection = cx
            .background_executor()
            .spawn(async { engine::discover_engine() });
        cx.spawn(async move |target, cx| {
            let engine = detection.await;
            let _ = target.update(cx, |this, cx| {
                this.health.detected = true;
                this.health.engine = engine;
                this.pending = false;
                this.set_pipeline_busy(false, cx);
                this.notice = None;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn choose_health_engine(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(text(self.language, Text::RelayChooseEngine).into()),
        });
        let executor = cx.background_executor().clone();
        self.pending = true;
        self.set_pipeline_busy(true, cx);
        cx.spawn(async move |target, cx| {
            let selected = match prompt.await {
                Ok(Ok(Some(paths))) => match paths.into_iter().next() {
                    Some(path) => Some(
                        executor
                            .spawn(async move { engine::resolve_selected_engine(&path) })
                            .await
                            .ok(),
                    ),
                    None => None,
                },
                Ok(Ok(None)) => None,
                _ => Some(None),
            };
            let _ = target.update(cx, |this, cx| {
                this.pending = false;
                this.set_pipeline_busy(false, cx);
                match selected {
                    Some(Some(path)) => {
                        this.health.detected = true;
                        this.health.engine = Some(path);
                        this.feedback(true, text(this.language, Text::RelayEngineSelected), cx);
                    }
                    Some(None) => {
                        this.feedback(false, text(this.language, Text::RelayEngineUnavailable), cx)
                    }
                    None => cx.notify(),
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn health_source_reviewed(&self, cx: &App) -> bool {
        let url = self.relay_url.read(cx).value();
        let file = self.relay_file.read(cx).value();
        self.reviewed_source
            .as_ref()
            .is_some_and(|review| review.revision.matches(url, file))
            || self
                .node_editor
                .binding
                .as_ref()
                .is_some_and(|binding| binding.revision.matches(url, file))
    }

    fn check_connectivity(&mut self, cx: &mut Context<Self>) {
        if !self.health_source_reviewed(cx) {
            return;
        }
        match self.health.options(cx) {
            Ok(options) => self.preview_inner(false, Some(options), cx),
            Err(label) => self.feedback(false, text(self.language, label), cx),
        }
    }

    pub(super) fn health_panel(&self, palette: Palette, cx: &Context<Self>) -> Div {
        let tr = |key| text(self.language, key);
        let review_label = localized(
            self.language,
            "Review source for testing",
            "查看待测订阅",
            "検査するサブスクリプションを確認",
        );
        let mut card = column().child(self.action(
            "toggle-relay-health",
            if self.health.open {
                localized(
                    self.language,
                    "Hide testing setup",
                    "收起测试设置",
                    "検査設定を閉じる",
                )
            } else {
                tr(Text::RelayTestSetup)
            },
            palette,
            false,
            cx,
            Self::toggle_health_panel,
        ));
        if !self.health.open {
            return card;
        }
        card = card
            .child(description(tr(Text::RelayEnginePurpose), palette))
            .child(description(
                tr(if self.health.engine_selected() {
                    Text::RelayEngineReady
                } else {
                    Text::RelayEngineMissing
                }),
                palette,
            ))
            .child(
                row()
                    .child(self.action(
                        "detect-relay-checker",
                        tr(Text::RelayDetectEngine),
                        palette,
                        false,
                        cx,
                        Self::detect_health_engine,
                    ))
                    .child(self.action(
                        "choose-relay-checker",
                        tr(Text::RelayChooseEngine),
                        palette,
                        false,
                        cx,
                        Self::choose_health_engine,
                    )),
            );
        if !self.health.engine_selected() {
            return card.child(description(tr(Text::RelayEngineHelp), palette));
        }
        card = card
            .child(description(tr(Text::RelayCleanHelp), palette))
            .child(description(tr(Text::RelayStrictProbeHelp), palette))
            .child(
                row()
                    .child(
                        field(tr(Text::RelayConcurrency), &self.health.parallel, palette)
                            .w(px(190.)),
                    )
                    .child(
                        field(tr(Text::RelayTimeout), &self.health.timeout, palette).w(px(190.)),
                    ),
            )
            .child(
                column()
                    .gap(px(6.))
                    .child(self.action(
                        "relay-probe-compatibility",
                        flag_text(
                            self.health.compatibility,
                            localized(
                                self.language,
                                "Allow requested TLS bypass",
                                "允许来源要求的 TLS 跳过验证",
                                "入力元が要求する TLS 検証の省略を許可",
                            ),
                        ),
                        palette,
                        false,
                        cx,
                        |this, cx| {
                            this.health.compatibility = !this.health.compatibility;
                            cx.notify();
                        },
                    ))
                    .child(description(tr(Text::RelayCompatibility), palette)),
            )
            .child(
                column()
                    .gap(px(6.))
                    .child(self.action(
                        "relay-probe-doh",
                        flag_text(
                            self.health.doh,
                            localized(
                                self.language,
                                "Use encrypted DNS",
                                "使用加密 DNS",
                                "暗号化 DNS を使用",
                            ),
                        ),
                        palette,
                        false,
                        cx,
                        |this, cx| {
                            this.health.doh = !this.health.doh;
                            cx.notify();
                        },
                    ))
                    .child(description(tr(Text::RelayDoh), palette)),
            );
        if self.health_source_reviewed(cx) {
            card.child(description(
                localized(
                    self.language,
                    "Testing connects through the reviewed proxy nodes to Google and Cloudflare’s public 204 endpoints.",
                    "测试会通过已查看的代理节点访问 Google 和 Cloudflare 的公共 204 端点。",
                    "検査では確認したプロキシノードを経由して Google と Cloudflare の公開 204 エンドポイントに接続します。",
                ),
                palette,
            ))
            .child(self.action(
                "run-relay-health",
                tr(Text::RelayPreview),
                palette,
                true,
                cx,
                Self::check_connectivity,
            ))
        } else {
            card.child(self.action(
                "review-source-for-relay-health",
                review_label,
                palette,
                true,
                cx,
                |this, cx| this.preview_inner(true, None, cx),
            ))
        }
    }

    pub(super) fn health_summary(
        &self,
        result: &RelayResult,
        palette: Palette,
        cx: &Context<Self>,
    ) -> Div {
        if !result.tested {
            return div();
        }
        let tr = |key| text(self.language, key);
        let mut summary = panel(palette)
            .w_full()
            .p_3()
            .gap_2()
            .child(section(tr(Text::RelayTested), palette))
            .child(description(
                format!(
                    "{} {} · {} {}",
                    tr(Text::RelayHealthy),
                    result.healthy,
                    tr(Text::RelayRemoved),
                    result.removed
                ),
                palette,
            ))
            .when(result.healthy == 0, |view| {
                view.child(description(tr(Text::RelayNoHealthyHelp), palette))
            })
            .child(self.action(
                "toggle-relay-check-breakdown",
                tr(Text::RelayResultDetails),
                palette,
                self.health.show_details,
                cx,
                |this, cx| {
                    this.health.show_details = !this.health.show_details;
                    cx.notify();
                },
            ));
        if !self.health.show_details {
            return summary;
        }
        for (label, value) in [
            (Text::RelayFailed, result.failed),
            (Text::RelayUnknown, result.unknown),
            (Text::RelaySecurityBlocked, result.security_blocked),
            (Text::RelayBaselines, result.baseline_valid),
        ] {
            summary = summary.child(description(format!("{} {value}", tr(label)), palette));
        }
        let strict_count = result
            .details
            .iter()
            .filter(|detail| detail.strict_tls)
            .count();
        if strict_count > 0 {
            let verified = result
                .details
                .iter()
                .filter(|detail| detail.strict_tls && detail.reason == ProbeReason::Verified)
                .count();
            summary = summary.child(description(
                format!(
                    "{} {strict_count} · {} {verified}",
                    tr(Text::RelayStrictProbed),
                    tr(Text::RelayStrictVerified)
                ),
                palette,
            ));
        }
        let mut reasons = [0_usize; PROBE_REASONS.len()];
        for detail in &result.details {
            if let Some(index) = PROBE_REASONS
                .iter()
                .position(|&(reason, _)| reason == detail.reason)
            {
                reasons[index] += 1;
            }
        }
        for ((_, label), count) in PROBE_REASONS.into_iter().zip(reasons) {
            if count > 0 {
                summary = summary.child(description(format!("{} {count}", tr(label)), palette));
            }
        }
        summary = summary.child(description(tr(Text::RelayLatencyHelp), palette));
        for (target, name) in [(0, "Google 204"), (1, "Cloudflare 204")] {
            if let Some((minimum, median)) = latency_stats(&result.details, target) {
                summary = summary.child(description(
                    format!(
                        "{name} · {} {minimum} ms · {} {median} ms",
                        tr(Text::RelayMinimum),
                        tr(Text::RelayMedian)
                    ),
                    palette,
                ));
            }
        }
        summary
    }
}

const PROBE_REASONS: [(ProbeReason, Text); 18] = [
    (ProbeReason::Verified, Text::RelayReasonVerified),
    (ProbeReason::Unsupported, Text::RelayReasonUnsupported),
    (ProbeReason::UnsafeAddress, Text::RelayReasonUnsafeAddress),
    (
        ProbeReason::AddressUnverified,
        Text::RelayReasonAddressUnverified,
    ),
    (
        ProbeReason::ExternalResource,
        Text::RelayReasonExternalResource,
    ),
    (
        ProbeReason::EngineConfigRejected,
        Text::RelayReasonEngineConfigRejected,
    ),
    (
        ProbeReason::EngineConfigTimeout,
        Text::RelayReasonEngineConfigTimeout,
    ),
    (ProbeReason::EngineNotReady, Text::RelayReasonEngineNotReady),
    (ProbeReason::EngineExited, Text::RelayReasonEngineExited),
    (
        ProbeReason::ConnectionFailed,
        Text::RelayReasonConnectionFailed,
    ),
    (ProbeReason::Timeout, Text::RelayReasonTimeout),
    (ProbeReason::TunnelRejected, Text::RelayReasonTunnelRejected),
    (ProbeReason::HttpResponse, Text::RelayReasonHttpResponse),
    (ProbeReason::TlsFailure, Text::RelayReasonTlsFailure),
    (ProbeReason::Inconclusive, Text::RelayReasonInconclusive),
    (
        ProbeReason::StrictTlsUnverified,
        Text::RelayReasonStrictTlsUnverified,
    ),
    (
        ProbeReason::BaselineUnstable,
        Text::RelayReasonBaselineUnstable,
    ),
    (ProbeReason::NotTested, Text::RelayReasonNotTested),
];

fn flag_text(value: bool, title: &str) -> String {
    format!("{} {title}", if value { "☑" } else { "☐" })
}

fn latency_stats(details: &[ProbeDetail], target: usize) -> Option<(u64, u64)> {
    let mut values = details
        .iter()
        .filter(|detail| detail.reason == ProbeReason::Verified && detail.target == Some(target))
        .filter_map(|detail| detail.latency_ms)
        .collect::<Vec<_>>();
    values.sort_unstable();
    let minimum = *values.first()?;
    let middle = values.len() / 2;
    let median = if values.len().is_multiple_of(2) {
        values[middle - 1] + (values[middle] - values[middle - 1]) / 2
    } else {
        values[middle]
    };
    Some((minimum, median))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_reports_only_verified_measurements_for_the_selected_target() {
        let sample = |reason, target, latency_ms| ProbeDetail {
            reason,
            target: Some(target),
            latency_ms,
            strict_tls: false,
        };
        let measurements = [
            sample(ProbeReason::Verified, 0, Some(20)),
            sample(ProbeReason::Verified, 0, Some(40)),
            sample(ProbeReason::Verified, 1, Some(100)),
            sample(ProbeReason::Timeout, 0, Some(1)),
            sample(ProbeReason::Verified, 0, None),
        ];
        assert_eq!(latency_stats(&measurements, 0), Some((20, 30)));
        assert_eq!(latency_stats(&measurements, 1), Some((100, 100)));
        assert_eq!(latency_stats(&measurements, 2), None);
    }
}
