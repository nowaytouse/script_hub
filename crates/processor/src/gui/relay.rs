//! Session-only relay state; deliberately separate from preferences and job history.

use super::i18n::Text;
use crate::relay::gist::{GistFile, validate_filename, validate_name};
use crate::relay::{Progress, RelayConfig, RelayError, RelayResult, RunMode};
use eframe::egui;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use zeroize::{Zeroize, Zeroizing};

pub(super) const SECRET_IDS: [&str; 6] = [
    "relay-token",
    "relay-source",
    "relay-source-file",
    "relay-output",
    "relay-output-file",
    "relay-title",
];

pub(super) enum Failure {
    Validation(Text),
    Operation(crate::relay::RelayError),
}

pub(super) fn failure_message(
    language: crate::runtime::settings::Language,
    failure: &Failure,
) -> String {
    use crate::relay::{engine::EngineError, gist::GistError, model::ModelError};
    let key = match failure {
        Failure::Validation(key) => *key,
        Failure::Operation(error) => match error {
            RelayError::InvalidInput => Text::RelayConfigInvalid,
            RelayError::NoHealthyNodes => Text::RelayNoHealthy,
            RelayError::SourceChanged => Text::RelaySourceChanged,
            RelayError::Cancelled => Text::RelayCancelled,
            RelayError::Model(error) => match error {
                ModelError::InvalidInput => Text::RelaySubscriptionInvalid,
                ModelError::Unsupported => Text::RelayUnsupported,
                ModelError::SecurityBlocked => Text::RelaySecurityBlocked,
                ModelError::InvalidDependency => Text::RelayDependencyInvalid,
                ModelError::InvalidHealth | ModelError::Render => Text::RelayRenderFailed,
            },
            RelayError::Engine(error) => match error {
                EngineError::InvalidOptions => Text::RelayOptionsInvalid,
                EngineError::Unavailable => Text::RelayEngineUnavailable,
                EngineError::Cancelled => Text::RelayCancelled,
                EngineError::Infrastructure => Text::RelayEngineFailed,
            },
            RelayError::Gist(error) => match error {
                GistError::InvalidTarget => Text::RelayConfigInvalid,
                GistError::PinnedTarget => Text::RelayPinnedTarget,
                GistError::InvalidName => Text::RelayNameInvalid,
                GistError::InvalidToken => Text::RelayTokenInvalid,
                GistError::Cancelled => Text::RelayCancelled,
                GistError::Transport => Text::RelayConnectionFailed,
                GistError::HttpStatus(status) => {
                    return format!(
                        "{} {status}",
                        super::i18n::text(language, Text::RelayHttpStatus)
                    );
                }
                GistError::InvalidResponse => Text::RelayResponseInvalid,
                GistError::TooLarge => Text::RelayTooLarge,
                GistError::IncompleteFiles => Text::RelayFileListIncomplete,
                GistError::Conflict => Text::RelayOutputChanged,
                GistError::PublicOutput => Text::RelayPublicOutput,
                GistError::UncertainWrite => Text::RelayWriteUncertain,
                GistError::VerificationFailed => Text::RelayReadbackFailed,
                GistError::CreatedUnverified(created) => {
                    return format!(
                        "{} {}",
                        super::i18n::text(language, Text::RelayReadbackFailed),
                        created.url
                    );
                }
            },
        },
    };
    super::i18n::text(language, key).to_owned()
}

pub(super) struct RelayState {
    pub token: Zeroizing<String>,
    pub source: Zeroizing<String>,
    pub source_file: Zeroizing<String>,
    pub output: Zeroizing<String>,
    pub output_file: Zeroizing<String>,
    pub title: Zeroizing<String>,
    pub concurrency: usize,
    pub timeout_seconds: u64,
    pub compatibility: bool,
    pub doh: bool,
    pub rename: bool,
    pub sort: bool,
    pub test_nodes: bool,
    pub allow_public_output: bool,
    pub acknowledge_publish: bool,
    pub allow_plaintext: bool,
    pub result: Option<RelayResult>,
    pub progress: Option<Progress>,
    pub error: Option<Failure>,
    pub notice: Option<Text>,
    pending: Option<Pending>,
    generation: u64,
}

struct Pending {
    worker: JoinHandle<()>,
    receiver: mpsc::Receiver<Result<RelayResult, RelayError>>,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Option<Progress>>>,
    generation: u64,
}

struct PrivateConfig(RelayConfig);

impl Drop for PrivateConfig {
    fn drop(&mut self) {
        self.0.source.zeroize();
        self.0.source_file.zeroize();
        self.0.output.zeroize();
        self.0.output_file.zeroize();
        self.0.title.zeroize();
    }
}

impl Default for RelayState {
    fn default() -> Self {
        let defaults = RelayConfig::default();
        Self {
            token: Zeroizing::new(String::new()),
            source: Zeroizing::new(String::new()),
            source_file: Zeroizing::new(String::new()),
            output: Zeroizing::new(String::new()),
            output_file: Zeroizing::new(String::new()),
            title: Zeroizing::new(String::new()),
            concurrency: defaults.concurrency,
            timeout_seconds: defaults.timeout_seconds,
            compatibility: false,
            doh: false,
            rename: defaults.rename,
            sort: defaults.sort,
            test_nodes: defaults.test_nodes,
            allow_public_output: false,
            acknowledge_publish: false,
            allow_plaintext: false,
            result: None,
            progress: None,
            error: None,
            notice: None,
            pending: None,
            generation: 0,
        }
    }
}

impl RelayState {
    pub(super) fn busy(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn validate(&self, mode: RunMode, engine: &Path) -> Result<(), Text> {
        if self.source.trim().is_empty() {
            return Err(Text::RelayInputInvalid);
        }
        let source =
            GistFile::parse(self.source.trim(), &self.source_file).map_err(
                |error| match error {
                    crate::relay::gist::GistError::PinnedTarget => Text::RelayPinnedTarget,
                    _ => Text::RelayInputInvalid,
                },
            )?;
        if !self.output.trim().is_empty() {
            let output = GistFile::parse(self.output.trim(), &self.output_file).map_err(
                |error| match error {
                    crate::relay::gist::GistError::PinnedTarget => Text::RelayPinnedTarget,
                    _ => Text::RelayOutputInvalid,
                },
            )?;
            if source.id == output.id {
                return Err(Text::RelayOutputInvalid);
            }
        } else if mode == RunMode::Publish && validate_filename(&self.output_file).is_err() {
            return Err(Text::RelayNameInvalid);
        }
        if validate_name(&self.title).is_err() || self.output_file.chars().count() > 64 {
            return Err(Text::RelayNameInvalid);
        }
        if self.test_nodes && engine.as_os_str().is_empty() {
            return Err(Text::SingboxRequired);
        }
        if self.test_nodes
            && (!(1..=8).contains(&self.concurrency) || !(1..=20).contains(&self.timeout_seconds))
        {
            return Err(Text::RelayOptionsInvalid);
        }
        if (!self.token.is_empty()
            && (self.token.len() > 1024
                || !self.token.is_ascii()
                || self
                    .token
                    .bytes()
                    .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())))
            || mode == RunMode::Publish && self.token.is_empty()
        {
            return Err(Text::RelayTokenInvalid);
        }
        if mode == RunMode::Publish && !self.acknowledge_publish {
            return Err(Text::RelayConsentRequired);
        }
        self.private_config(engine)
            .0
            .validate(mode)
            .map_err(|_| Text::RelayConfigInvalid)
    }

    fn private_config(&self, engine: &Path) -> PrivateConfig {
        PrivateConfig(RelayConfig {
            source: self.source.trim().to_owned(),
            source_file: self.source_file.to_string(),
            output: self.output.trim().to_owned(),
            output_file: self.output_file.to_string(),
            title: self.title.to_string(),
            engine: engine.to_path_buf(),
            concurrency: self.concurrency,
            timeout_seconds: self.timeout_seconds,
            compatibility: self.compatibility,
            doh: self.doh,
            rename: self.rename,
            sort: self.sort,
            test_nodes: self.test_nodes,
            allow_public_output: self.allow_public_output,
        })
    }

    pub(super) fn clear(&mut self, ctx: &egui::Context) {
        self.generation = self.generation.wrapping_add(1);
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Release);
        }
        self.token.zeroize();
        self.source.zeroize();
        self.source_file.zeroize();
        self.output.zeroize();
        self.output_file.zeroize();
        self.title.zeroize();
        self.result = None;
        self.progress = None;
        self.error = None;
        self.allow_plaintext = false;
        self.acknowledge_publish = false;
        self.allow_public_output = false;
        self.compatibility = false;
        self.doh = false;
        self.test_nodes = false;
        self.notice = Some(if self.busy() {
            Text::RelayCancelling
        } else {
            Text::RelayCleared
        });
        for id in SECRET_IDS {
            super::templates::clear_secret_widget(ctx, id);
        }
    }

    pub(super) fn request_cancel(&mut self) {
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Release);
            self.notice = Some(Text::RelayCancelling);
        }
        self.allow_plaintext = false;
    }

    pub(super) fn tick(&mut self, ctx: &egui::Context) {
        if !ctx.input(|input| input.viewport().focused.unwrap_or(true)) {
            self.allow_plaintext = false;
        }
        let Some(pending) = &self.pending else { return };
        let current = pending.generation == self.generation;
        if current {
            self.progress = *pending
                .progress
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        if !pending.worker.is_finished() {
            ctx.request_repaint_after(Duration::from_millis(100));
            return;
        }
        let pending = self.pending.take().expect("completed relay worker exists");
        let result = pending.receiver.try_recv();
        let joined = pending.worker.join();
        if !current {
            self.notice = Some(Text::RelayCancelled);
            return;
        }
        self.allow_plaintext = false;
        self.acknowledge_publish = false;
        self.allow_public_output = false;
        self.compatibility = false;
        match (joined, result) {
            (Ok(()), Ok(Ok(result))) => {
                if let Some(created) = &result.created {
                    self.output = Zeroizing::new(created.id.clone());
                }
                self.notice = Some(if result.published {
                    Text::RelayPublished
                } else if result.unchanged {
                    Text::RelayUnchanged
                } else {
                    Text::RelayPreviewReady
                });
                self.result = Some(result);
            }
            (Ok(()), Ok(Err(error))) => {
                if let RelayError::Gist(crate::relay::gist::GistError::CreatedUnverified(created)) =
                    &error
                {
                    self.output = Zeroizing::new(created.id.clone());
                }
                self.error = Some(Failure::Operation(error));
            }
            _ => self.notice = Some(Text::RelayWorkerStopped),
        }
    }

    pub(super) fn copy(&mut self, ctx: &egui::Context) {
        if !self.allow_plaintext || self.busy() {
            return;
        }
        if let Some(result) = &self.result {
            ctx.copy_text(result.content.to_string());
            self.notice = Some(Text::Copied);
        }
    }

    pub(super) fn start_with<F>(&mut self, mode: RunMode, engine: &Path, work: F)
    where
        F: FnOnce(
                &RelayConfig,
                &str,
                RunMode,
                &AtomicBool,
                &(dyn Fn(Progress) + Sync),
            ) -> Result<RelayResult, RelayError>
            + Send
            + 'static,
    {
        if self.busy() {
            return;
        }
        if let Err(error) = self.validate(mode, engine) {
            self.error = Some(Failure::Validation(error));
            return;
        }
        let config = self.private_config(engine);
        let token = self.token.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let progress = Arc::new(Mutex::new(None));
        let worker_progress = Arc::clone(&progress);
        let (sender, receiver) = mpsc::sync_channel(1);
        self.result = None;
        self.progress = None;
        self.error = None;
        self.notice = None;
        self.allow_plaintext = false;
        match thread::Builder::new()
            .name("script-hub-private-relay".to_owned())
            .spawn(move || {
                let report = |value| {
                    *worker_progress
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(value);
                };
                let result = work(&config.0, &token, mode, &worker_cancel, &report);
                // Cancellation cannot erase accepted-write receipts; Clear discards by generation.
                let _ = sender.send(result);
            }) {
            Ok(worker) => {
                self.pending = Some(Pending {
                    worker,
                    receiver,
                    cancel,
                    progress,
                    generation: self.generation,
                })
            }
            Err(_) => self.notice = Some(Text::RelayWorkerStopped),
        }
    }
}

impl Drop for RelayState {
    fn drop(&mut self) {
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::sync::{Arc, mpsc};
    use std::time::{Duration, Instant};

    pub(super) fn fixture() -> RelayState {
        let mut state = RelayState::default();
        state.token.push_str("synthetic-token");
        state.source.push_str("0123456789abcdef0123456789abcdef");
        state.source_file.push_str("input.conf");
        state.output.push_str("fedcba9876543210fedcba9876543210");
        state.output_file.push_str("output.conf");
        state.title.push_str("Synthetic relay");
        state
    }

    fn result() -> RelayResult {
        RelayResult {
            content: Zeroizing::new("synthetic private node".to_owned()),
            tested: true,
            created: None,
            total: 2,
            healthy: 1,
            failed: 0,
            unknown: 1,
            security_blocked: 0,
            removed: 0,
            baseline_valid: 2,
            published: false,
            unchanged: false,
        }
    }

    fn finish(state: &mut RelayState, ctx: &egui::Context) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.busy() && Instant::now() < deadline {
            state.tick(ctx);
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!state.busy(), "synthetic worker must finish");
    }

    #[test]
    fn format_only_and_explicit_secret_creation_do_not_require_an_engine_or_existing_gist() {
        let mut state = fixture();
        state.output.clear();
        assert!(state.validate(RunMode::Preview, Path::new("")).is_ok());
        assert!(state.validate(RunMode::Publish, Path::new("")).is_err());
        state.acknowledge_publish = true;
        assert!(state.validate(RunMode::Publish, Path::new("")).is_ok());
    }

    #[test]
    fn created_target_is_reused_by_the_next_explicit_publish() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.output.clear();
        state.acknowledge_publish = true;
        state.start_with(RunMode::Publish, Path::new(""), |config, _, _, _, _| {
            assert!(config.output.is_empty() && !config.test_nodes);
            let mut result = result();
            result.created = Some(crate::relay::gist::CreatedGist {
                id: "aaaaaaaaaaaaaaaa".to_owned(),
                url: "https://gist.github.com/aaaaaaaaaaaaaaaa".to_owned(),
            });
            result.published = true;
            Ok(result)
        });
        finish(&mut state, &ctx);
        assert_eq!(state.output.as_str(), "aaaaaaaaaaaaaaaa");
        state.acknowledge_publish = true;
        state.start_with(RunMode::Publish, Path::new(""), |config, _, _, _, _| {
            assert_eq!(config.output, "aaaaaaaaaaaaaaaa");
            Ok(result())
        });
        finish(&mut state, &ctx);
        assert!(state.result.is_some());
    }

    #[test]
    fn cancellation_keeps_an_unverified_creation_receipt_for_reuse() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.output.clear();
        state.acknowledge_publish = true;
        let (release, wait) = mpsc::sync_channel(1);
        state.start_with(
            RunMode::Publish,
            Path::new(""),
            move |_, _, _, cancel, _| {
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
                assert!(cancel.load(Ordering::Acquire));
                Err(RelayError::Gist(
                    crate::relay::gist::GistError::CreatedUnverified(
                        crate::relay::gist::CreatedGist {
                            id: "aaaaaaaaaaaaaaaa".to_owned(),
                            url: "https://gist.github.com/aaaaaaaaaaaaaaaa".to_owned(),
                        },
                    ),
                ))
            },
        );
        state.request_cancel();
        release.send(()).unwrap();
        finish(&mut state, &ctx);
        assert_eq!(state.output.as_str(), "aaaaaaaaaaaaaaaa");
        assert!(state.result.is_none());
        assert!(matches!(
            state.error,
            Some(Failure::Operation(RelayError::Gist(
                crate::relay::gist::GistError::CreatedUnverified(_)
            )))
        ));
        state.acknowledge_publish = true;
        assert!(state.validate(RunMode::Publish, Path::new("")).is_ok());
    }

    #[test]
    fn real_testing_requires_an_engine_without_changing_format_only_defaults() {
        let mut state = fixture();
        assert!(!state.test_nodes);
        state.test_nodes = true;
        assert_eq!(
            state.validate(RunMode::Preview, Path::new("")),
            Err(Text::SingboxRequired)
        );
    }

    #[test]
    fn relay_validation_rejects_missing_inputs_same_gist_and_unicode_overflow() {
        let engine = std::env::temp_dir().join("synthetic-sing-box");
        assert!(
            RelayState::default()
                .validate(RunMode::Preview, &engine)
                .is_err()
        );
        let mut state = fixture();
        assert!(state.validate(RunMode::Preview, &engine).is_ok());
        state.output = state.source.clone();
        assert!(state.validate(RunMode::Preview, &engine).is_err());
        state = fixture();
        state.title = Zeroizing::new("界".repeat(64));
        state.output_file = Zeroizing::new(format!("{}.conf", "界".repeat(59)));
        assert!(state.validate(RunMode::Preview, &engine).is_ok());
        state.title.push('界');
        assert!(state.validate(RunMode::Preview, &engine).is_err());
        let _ = state.title.pop();
        state.output_file.push('界');
        assert!(state.validate(RunMode::Preview, &engine).is_err());
    }

    #[test]
    fn relay_dispatch_preserves_meaningful_filename_spaces() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.source_file = Zeroizing::new(" input.conf ".to_owned());
        state.output_file = Zeroizing::new(" output.conf ".to_owned());
        let (sent, received) = mpsc::sync_channel(1);
        state.start_with(
            RunMode::Preview,
            &std::env::temp_dir().join("synthetic-sing-box"),
            move |config, _, _, _, _| {
                sent.send((config.source_file.clone(), config.output_file.clone()))
                    .unwrap();
                Ok(result())
            },
        );
        finish(&mut state, &ctx);
        assert_eq!(
            received.recv_timeout(Duration::from_secs(2)).unwrap(),
            (" input.conf ".to_owned(), " output.conf ".to_owned())
        );
    }

    #[test]
    fn publication_and_compatibility_are_explicit_while_preview_never_implies_upload() {
        let engine = std::env::temp_dir().join("synthetic-sing-box");
        let mut state = fixture();
        assert!(!state.compatibility && !state.doh && !state.allow_public_output);
        assert!(state.validate(RunMode::Publish, &engine).is_err());
        state.acknowledge_publish = true;
        assert!(state.validate(RunMode::Publish, &engine).is_ok());
        state.acknowledge_publish = false;
        state.start_with(RunMode::Publish, &engine, |_, _, _, _, _| {
            panic!("unacknowledged publication must not dispatch")
        });
        assert!(!state.busy() && state.error.is_some());
        state.start_with(
            RunMode::Preview,
            &engine,
            |config, token, mode, _, progress| {
                assert!(matches!(mode, RunMode::Preview));
                assert_eq!(token, "synthetic-token");
                assert!(!config.compatibility && !config.doh && !config.allow_public_output);
                progress(Progress {
                    stage: crate::relay::RelayStage::Check,
                    checked: 1,
                    total: 2,
                });
                Ok(result())
            },
        );
        finish(&mut state, &egui::Context::default());
        assert!(state.result.is_some() && !state.allow_plaintext);
    }

    #[test]
    fn clearing_cancels_and_discards_late_results_but_keeps_admission_until_reaped() {
        let ctx = egui::Context::default();
        let engine = std::env::temp_dir().join("synthetic-sing-box");
        let mut state = fixture();
        let observed = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&observed);
        let (release, wait) = mpsc::sync_channel(1);
        state.start_with(RunMode::Preview, &engine, move |_, _, _, cancel, _| {
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            flag.store(cancel.load(Ordering::Acquire), Ordering::Release);
            Ok(result())
        });
        assert!(state.busy());
        state.clear(&ctx);
        assert!(
            state.busy(),
            "clearing must not admit a second worker before the first exits"
        );
        assert!(
            state.token.is_empty()
                && state.source.is_empty()
                && state.source_file.is_empty()
                && state.output.is_empty()
                && state.output_file.is_empty()
                && state.title.is_empty()
        );
        release.send(()).unwrap();
        finish(&mut state, &ctx);
        assert!(observed.load(Ordering::Acquire));
        assert!(state.result.is_none() && state.progress.is_none() && !state.allow_plaintext);
    }

    #[test]
    fn focus_loss_does_not_cancel_an_authorized_background_relay() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        let (release, wait) = mpsc::sync_channel(1);
        state.start_with(
            RunMode::Preview,
            &std::env::temp_dir().join("synthetic-sing-box"),
            move |_, _, _, cancel, _| {
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
                assert!(!cancel.load(Ordering::Acquire));
                Ok(result())
            },
        );
        assert!(state.busy());
        let mut input = egui::RawInput {
            focused: false,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .focused = Some(false);
        let output = ctx.run_ui(input, |ui| state.tick(ui.ctx()));
        output.drop_without_applying_deltas();
        assert!(state.busy() && !state.token.is_empty());
        release.send(()).unwrap();
        finish(&mut state, &ctx);
        assert!(state.result.is_some());
    }

    #[test]
    fn plaintext_copy_requires_fresh_consent_and_clear_removes_secret_widget_state() {
        let ctx = egui::Context::default();
        let mut state = fixture();
        state.result = Some(result());
        let output = ctx.run_ui(Default::default(), |ui| state.copy(ui.ctx()));
        let copied_without_consent = output
            .platform_output
            .commands
            .iter()
            .any(|command| matches!(command, egui::OutputCommand::CopyText(_)));
        output.drop_without_applying_deltas();
        assert!(!copied_without_consent);
        state.allow_plaintext = true;
        let output = ctx.run_ui(Default::default(), |ui| state.copy(ui.ctx()));
        let copied = output.platform_output.commands.iter().any(|command| matches!(command, egui::OutputCommand::CopyText(value) if value == "synthetic private node"));
        output.drop_without_applying_deltas();
        assert!(copied);
        for id in SECRET_IDS {
            egui::text_edit::TextEditState::default().store(&ctx, egui::Id::unique(id));
        }
        state.clear(&ctx);
        assert!(state.result.is_none() && !state.allow_plaintext);
        for id in SECRET_IDS {
            assert!(egui::text_edit::TextEditState::load(&ctx, egui::Id::unique(id)).is_none());
        }
    }
    #[test]
    fn validation_messages_follow_the_selected_language() {
        let failure = Failure::Validation(Text::RelayConsentRequired);
        assert_eq!(
            failure_message(crate::runtime::settings::Language::ZhCn, &failure),
            super::super::i18n::text(
                crate::runtime::settings::Language::ZhCn,
                Text::RelayConsentRequired
            )
        );
    }

    #[test]
    fn pinned_source_and_output_links_explain_the_latest_target_requirement() {
        let engine = std::env::temp_dir().join("synthetic-sing-box");
        for output in [false, true] {
            let mut state = fixture();
            let url = format!(
                "https://gist.githubusercontent.com/fixture/{}/raw/{}/nodes.conf",
                if output {
                    "fedcba9876543210"
                } else {
                    "0123456789abcdef"
                },
                "a".repeat(40)
            );
            if output {
                state.output = Zeroizing::new(url);
                state.output_file = Zeroizing::new(String::new());
            } else {
                state.source = Zeroizing::new(url);
                state.source_file = Zeroizing::new(String::new());
            }
            assert_eq!(
                state.validate(RunMode::Preview, &engine),
                Err(Text::RelayPinnedTarget)
            );
        }
    }

    #[test]
    fn operation_errors_follow_the_selected_language_without_changing_the_core_error() {
        let error = crate::relay::RelayError::NoHealthyNodes;
        let original = error.to_string();
        let failure = Failure::Operation(error);
        let translated = failure_message(crate::runtime::settings::Language::ZhCn, &failure);
        assert_ne!(translated, original);
        assert!(translated.contains("节点"));
    }

    #[test]
    fn worker_failures_keep_typed_status_for_current_language_display() {
        let mut state = fixture();
        state.start_with(
            RunMode::Preview,
            &std::env::temp_dir().join("synthetic-sing-box"),
            |_, _, _, _, _| {
                Err(RelayError::Gist(crate::relay::gist::GistError::HttpStatus(
                    429,
                )))
            },
        );
        finish(&mut state, &egui::Context::default());
        let failure = state.error.as_ref().unwrap();
        for language in [
            crate::runtime::settings::Language::En,
            crate::runtime::settings::Language::ZhCn,
            crate::runtime::settings::Language::Ja,
        ] {
            let message = failure_message(language, failure);
            assert!(message.contains("429"));
            assert!(message.starts_with(super::super::i18n::text(language, Text::RelayHttpStatus)));
        }
    }
}
