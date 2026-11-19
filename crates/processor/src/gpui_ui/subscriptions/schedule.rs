use super::super::{Palette, button, input::TextInput, panel};
use super::view::{column, description, field, row, section};
use super::{Item, Source, Subscriptions, clear, set};
use crate::gui::{
    i18n::{Text, text},
    operation_failure_message,
};
use crate::relay::{
    RelayError, RelayStage,
    library::Workspace,
    model::pipeline,
    schedule::{DEFAULT_INTERVAL, MAX_INTERVAL, MIN_INTERVAL, RetryDeadline},
    session_schedule::{State, Status},
};
use gpui::{Context, Div, Entity, prelude::*, px};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

pub(super) struct Controls {
    state: State,
    interval: Entity<TextInput>,
    polling: bool,
    project_busy: bool,
}

impl Controls {
    pub(super) fn new(cx: &mut Context<Subscriptions>) -> Self {
        let state = State::default();
        let interval = cx.new(|cx| {
            let mut input = TextInput::new("60", false, cx);
            input.set_value(state.interval_minutes.to_string(), cx);
            input
        });
        Self {
            state,
            interval,
            polling: false,
            project_busy: false,
        }
    }
}

impl Subscriptions {
    pub(in crate::gpui_ui) fn schedule_active(&self) -> bool {
        self.schedule.state.armed() || self.schedule_running()
    }

    pub(in crate::gpui_ui) fn set_schedule_project_busy(&mut self, busy: bool) {
        self.schedule.project_busy = busy;
    }

    pub(super) fn schedule_running(&self) -> bool {
        self.schedule.state.running()
    }

    pub(super) fn reset_schedule(&mut self, cx: &mut Context<Self>) {
        self.schedule.state.stop();
        self.set_schedule_interval(DEFAULT_INTERVAL.as_secs() / 60, cx);
    }

    pub(super) fn set_schedule_interval(&mut self, minutes: u64, cx: &mut Context<Self>) {
        set(&self.schedule.interval, &minutes.to_string(), cx);
    }

    pub(super) fn check_schedule_cooldown(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(deadline) = self.schedule.state.server_retry_until() else {
            return false;
        };
        let seconds = deadline.remaining_seconds(Instant::now());
        if seconds == 0 {
            return false;
        }
        let message = if deadline == RetryDeadline::Session {
            text(self.language, Text::RelayRetrySession).to_owned()
        } else {
            format!(
                "{} {seconds} s",
                text(self.language, Text::RelayRateLimited)
            )
        };
        self.feedback(false, message, cx);
        true
    }

    pub(super) fn relay_failure(&mut self, error: RelayError, cx: &mut Context<Self>) {
        self.schedule.state.record_server_retry(&error);
        self.feedback(false, operation_failure_message(self.language, &error), cx);
    }

    fn arm_schedule(&mut self, cx: &mut Context<Self>) {
        if !self.reviewing_publish || self.schedule_active() {
            return;
        }
        if !self.schedule_admitted() {
            self.feedback(false, text(self.language, Text::ScheduleReadyWhenIdle), cx);
            return;
        }
        if self.check_schedule_cooldown(cx) {
            return;
        }
        let Some(preview) = self.current_preview(cx) else {
            self.feedback(false, text(self.language, Text::ScheduleNeedsPreview), cx);
            return;
        };
        if preview.result.tested && preview.result.healthy == 0 {
            self.feedback(false, text(self.language, Text::RelayNoHealthyHelp), cx);
            return;
        }
        let mut config = self.publication_config(preview, cx);
        if config.0.output.is_empty() {
            self.feedback(false, text(self.language, Text::ScheduleNeedsOutput), cx);
            return;
        }
        config.0.allow_public_output = false;
        config.0.test_nodes = preview.result.tested;
        let min = MIN_INTERVAL.as_secs() / 60;
        let max = MAX_INTERVAL.as_secs() / 60;
        let Some(minutes) = self
            .schedule
            .interval
            .read(cx)
            .value()
            .trim()
            .parse::<u64>()
            .ok()
            .filter(|minutes| (min..=max).contains(minutes))
        else {
            self.feedback(
                false,
                format!(
                    "{} · {min}–{max}",
                    text(self.language, Text::ScheduleInterval)
                ),
                cx,
            );
            return;
        };
        let token = Zeroizing::new(self.token.read(cx).value().trim().to_owned());
        if token.is_empty() {
            self.feedback(
                false,
                self.tr("Enter a GitHub token", "请输入 GitHub 令牌"),
                cx,
            );
            return;
        }
        self.schedule.state.interval_minutes = minutes;
        let result = match self.source {
            Source::Saved(Item::Collection(id)) => {
                let Some(collection) = self.collection(id) else {
                    return;
                };
                let mut collection = collection.clone();
                collection.processing = config.0.processing.clone();
                // Retain only the selected collection and its sources for recurring work.
                let workspace = Workspace {
                    version: self.workspace.version,
                    next_id: self.workspace.next_id,
                    subscriptions: self
                        .workspace
                        .subscriptions
                        .iter()
                        .filter(|entry| collection.members.contains(&entry.id))
                        .cloned()
                        .collect(),
                    collections: vec![collection],
                };
                self.schedule.state.arm_collection(
                    std::mem::take(&mut config.0),
                    &token,
                    workspace,
                    id,
                )
            }
            _ => self
                .schedule
                .state
                .arm(std::mem::take(&mut config.0), &token),
        };
        match result {
            Ok(()) => {
                self.conceal_preview(cx);
                clear(&self.source_token, cx);
                self.last_publication = None;
                self.notice = None;
                self.poll_schedule(cx);
                cx.notify();
            }
            Err(error) => self.relay_failure(error, cx),
        }
    }

    pub(super) fn stop_schedule(&mut self, cx: &mut Context<Self>) {
        self.schedule.state.stop();
        self.preview = None;
        self.conceal_preview(cx);
        cx.notify();
    }

    fn schedule_admitted(&self) -> bool {
        !self.pending && !self.schedule.project_busy
    }

    fn run_schedule_now(&mut self, cx: &mut Context<Self>) {
        if self.schedule_admitted() && self.schedule.state.can_run_now() {
            self.schedule.state.request_run_now();
            self.tick_schedule(cx);
        }
    }

    fn tick_schedule(&mut self, cx: &mut Context<Self>) -> Option<Duration> {
        let before = self.schedule.state.status();
        let progress_key = |progress: Option<crate::relay::Progress>| {
            progress.map(|value| (value.stage, value.checked, value.total))
        };
        let previous_progress = progress_key(self.schedule.state.progress);
        let admitted = self.schedule_admitted();
        let completed = self.schedule.state.tick(admitted).is_some();
        let after = self.schedule.state.status();
        if before != Status::Running && after == Status::Running {
            // A scheduled refresh may supersede the plan from the reviewed snapshot.
            self.preview = None;
            self.reviewed_source = None;
        }
        if completed
            || before != after
            || previous_progress != progress_key(self.schedule.state.progress)
            || matches!(after, Status::Waiting { .. })
        {
            cx.notify();
        }
        self.schedule.state.poll_delay()
    }

    fn poll_schedule(&mut self, cx: &mut Context<Self>) {
        if self.schedule.polling {
            return;
        }
        self.schedule.polling = true;
        let executor = cx.background_executor().clone();
        cx.spawn(async move |weak, cx| {
            while let Ok(Some(delay)) = weak.update(cx, |this, cx| {
                let delay = this.tick_schedule(cx);
                this.schedule.polling = delay.is_some();
                delay
            }) {
                executor.timer(delay).await;
            }
        })
        .detach();
    }

    pub(super) fn schedule_setup(&self, palette: Palette, cx: &Context<Self>) -> Div {
        let tr = |key| text(self.language, key);
        let requires_resolution = self
            .current_preview(cx)
            .is_some_and(|preview| pipeline::has_resolution(&preview.config.0.processing.pipeline));
        column()
            .mt_2()
            .pt_4()
            .border_t_1()
            .border_color(palette.border)
            .child(section(tr(Text::ScheduleTitle), palette))
            .child(description(tr(Text::ScheduleCredentialHelp), palette))
            .child(description(tr(Text::ScheduleSessionOnly), palette))
            .when(requires_resolution, |group| {
                group.child(description(tr(Text::ScheduleResolutionHelp), palette))
            })
            .child(
                field(tr(Text::ScheduleInterval), &self.schedule.interval, palette).max_w(px(200.)),
            )
            .child(self.action(
                "relay-enable-schedule",
                tr(Text::ScheduleEnable),
                palette,
                false,
                cx,
                Self::arm_schedule,
            ))
    }

    pub(super) fn schedule_panel(&self, palette: Palette, cx: &Context<Self>) -> Option<Div> {
        let state = &self.schedule.state;
        let status = state.status();
        let tr = |key| text(self.language, key);
        let status_text = match status {
            Status::Idle => return None,
            Status::Running => tr(Text::ScheduleRunning).to_owned(),
            Status::Stopping => tr(Text::RelayCancelling).to_owned(),
            Status::Paused => tr(Text::SchedulePaused).to_owned(),
            Status::Waiting { seconds: 0, .. } => tr(Text::ScheduleReadyWhenIdle).to_owned(),
            Status::Waiting { seconds, retry } => {
                let label = tr(if retry {
                    Text::ScheduleRetryIn
                } else {
                    Text::ScheduleNext
                });
                format!(
                    "{label} {:02}:{:02}:{:02}",
                    seconds / 3600,
                    seconds / 60 % 60,
                    seconds % 60
                )
            }
        };
        let mut content = column()
            .gap_2()
            .child(section(tr(Text::ScheduleTitle), palette))
            .child(description(status_text, palette));
        if let Some(progress) = state.progress.filter(|_| status == Status::Running) {
            let label = self.progress_text(progress.stage);
            content = content.child(description(
                if progress.total == 0 {
                    label.to_owned()
                } else {
                    format!("{label} · {}/{}", progress.checked, progress.total)
                },
                palette,
            ));
            if progress.stage == RelayStage::Publish {
                content = content.child(description(tr(Text::RelayCancelWriteWarning), palette));
            }
        }
        if status == Status::Stopping {
            content = content.child(description(tr(Text::RelayCancelWriteWarning), palette));
        }
        if matches!(status, Status::Paused | Status::Waiting { retry: true, .. })
            && let Some(error) = &state.last_error
        {
            content = content.child(
                description(operation_failure_message(self.language, error), palette)
                    .text_color(gpui::rgb(0xe88378)),
            );
        }
        if let Some(result) = state.result {
            let label = tr(if result.unchanged {
                Text::RelayUnchanged
            } else {
                Text::RelayPublished
            });
            content = content.child(description(
                format!(
                    "{} · {label} · {} {} · {} {}",
                    tr(Text::ScheduleLastConfirmed),
                    tr(Text::RelayTotal),
                    result.total,
                    tr(Text::RelayRemoved),
                    result.removed,
                ),
                palette,
            ));
        }
        if state.armed() {
            let mut actions = row().child(
                button(
                    "relay-stop-schedule",
                    tr(Text::ScheduleStop),
                    palette,
                    false,
                )
                .on_click(cx.listener(|this, _, _, cx| this.stop_schedule(cx))),
            );
            if state.can_run_now() && self.schedule_admitted() {
                actions = actions.child(
                    button(
                        "relay-run-schedule",
                        tr(Text::ScheduleRunNow),
                        palette,
                        true,
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.run_schedule_now(cx))),
                );
            }
            content = content.child(actions);
        } else if status == Status::Paused {
            content = content
                .child(description(tr(Text::ScheduleRecovery), palette))
                .child(self.action(
                    "relay-dismiss-schedule",
                    tr(Text::Close),
                    palette,
                    false,
                    cx,
                    |this, cx| {
                        this.schedule.state.clear_feedback();
                        cx.notify();
                    },
                ));
        }
        Some(panel(palette).w_full().p_5().child(content))
    }
}
