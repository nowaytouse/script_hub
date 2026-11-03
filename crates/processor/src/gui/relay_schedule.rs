//! Session-only scheduling state; no credentials are persisted.
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use super::relay::{Failure, PrivateConfig};
use crate::relay::{
    Progress, RelayConfig, RelayError, RelayResult, collection_schedule,
    library::Workspace,
    schedule::{self, RetryDeadline, Schedule},
};
use eframe::egui;
use zeroize::Zeroizing;

#[derive(Clone, Copy)]
pub struct Summary {
    pub total: usize,
    pub removed: usize,
    pub unchanged: bool,
}

impl From<&RelayResult> for Summary {
    fn from(value: &RelayResult) -> Self {
        Self {
            total: value.total,
            removed: value.removed,
            unchanged: value.unchanged,
        }
    }
}

struct Pending {
    worker: JoinHandle<()>,
    receiver: mpsc::Receiver<Result<Completed, RelayError>>,
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Option<Progress>>>,
    generation: u64,
}

pub enum Completed {
    Subscription(Box<RelayResult>),
    Collection(Box<collection_schedule::Completed>),
}

impl Completed {
    fn result(&self) -> &RelayResult {
        match self {
            Self::Subscription(result) => result,
            Self::Collection(completed) => &completed.result,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Status {
    Idle,
    Waiting { seconds: u64, retry: bool },
    Running,
    Stopping,
    Paused,
}

pub struct State {
    pub interval_minutes: u64,
    pub progress: Option<Progress>,
    pub result: Option<Summary>,
    pub(super) last_error: Option<Failure>,
    server_retry_until: Option<RetryDeadline>,
    schedule: Option<Schedule>,
    config: Option<PrivateConfig>,
    collection: Option<(Arc<Workspace>, u64)>,
    token: Option<Zeroizing<String>>,
    pending: Option<Pending>,
    generation: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            interval_minutes: schedule::DEFAULT_INTERVAL.as_secs() / 60,
            progress: None,
            result: None,
            last_error: None,
            server_retry_until: None,
            schedule: None,
            config: None,
            collection: None,
            token: None,
            pending: None,
            generation: 0,
        }
    }
}

impl State {
    pub fn arm(&mut self, config: RelayConfig, token: &str) -> Result<(), RelayError> {
        self.arm_with(config, token, None)
    }

    pub fn arm_collection(
        &mut self,
        config: RelayConfig,
        token: &str,
        workspace: Workspace,
        id: u64,
    ) -> Result<(), RelayError> {
        self.arm_with(config, token, Some((workspace, id)))
    }

    fn arm_with(
        &mut self,
        config: RelayConfig,
        token: &str,
        collection: Option<(Workspace, u64)>,
    ) -> Result<(), RelayError> {
        let config = PrivateConfig(config);
        let now = Instant::now();
        if self.running()
            || self.armed()
            || token.is_empty()
            || self
                .server_retry_until
                .is_some_and(|deadline| deadline.remaining_seconds(now) > 0)
        {
            return Err(RelayError::InvalidInput);
        }
        if let Some((workspace, id)) = &collection {
            collection_schedule::validate(workspace, *id, &config.0)?;
        } else {
            schedule::validate(&config.0)?;
        }
        crate::relay::gist::GistClient::new(token).map_err(RelayError::Gist)?;
        let seconds = self
            .interval_minutes
            .checked_mul(60)
            .ok_or(RelayError::InvalidInput)?;
        let schedule = Schedule::new(Duration::from_secs(seconds), now)?;
        self.generation = self.generation.wrapping_add(1);
        self.schedule = Some(schedule);
        self.config = Some(config);
        self.collection = collection.map(|(workspace, id)| (Arc::new(workspace), id));
        self.token = Some(Zeroizing::new(token.to_owned()));
        self.progress = None;
        self.clear_feedback();
        Ok(())
    }

    pub fn stop(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if let Some(schedule) = &mut self.schedule {
            schedule.pause();
        }
        if let Some(pending) = &self.pending {
            pending.cancel.store(true, Ordering::Release);
        }
        self.config = None;
        self.collection = None;
        self.token = None;
        self.progress = None;
        self.clear_feedback();
    }

    pub fn running(&self) -> bool {
        self.pending.is_some()
    }
    pub fn armed(&self) -> bool {
        self.config.is_some() && self.schedule.as_ref().is_some_and(|s| !s.is_paused())
    }

    pub(super) fn can_run_now(&self) -> bool {
        let now = Instant::now();
        self.armed()
            && !self.running()
            && self
                .server_retry_until
                .is_none_or(|deadline| deadline.remaining_seconds(now) == 0)
            && self
                .schedule
                .as_ref()
                .is_some_and(|schedule| schedule.can_request_now(now))
    }

    pub(super) fn request_run_now(&mut self) {
        if self.can_run_now()
            && let Some(schedule) = &mut self.schedule
        {
            schedule.request_now(Instant::now());
        }
    }

    pub fn clear_feedback(&mut self) {
        self.last_error = None;
        self.result = None;
    }

    pub fn server_retry_until(&self) -> Option<RetryDeadline> {
        self.server_retry_until
    }

    pub(super) fn status(&self) -> Status {
        self.status_at(Instant::now())
    }

    fn status_at(&self, now: Instant) -> Status {
        if self.running() {
            return if self.armed() {
                Status::Running
            } else {
                Status::Stopping
            };
        }
        if let Some(schedule) = self.schedule.as_ref().filter(|_| self.armed())
            && let Some(due) = schedule.next_due()
        {
            let remaining = due.saturating_duration_since(now);
            return Status::Waiting {
                seconds: remaining.as_secs() + u64::from(remaining.subsec_nanos() != 0),
                retry: schedule.is_waiting_to_retry(),
            };
        }
        if self.last_error.is_some() {
            Status::Paused
        } else {
            Status::Idle
        }
    }

    pub fn tick(&mut self, ctx: &egui::Context, admitted: bool) -> Option<Completed> {
        let mut completed = None;
        if let Some(pending) = &self.pending
            && pending.generation == self.generation
            && let Ok(progress) = pending.progress.lock()
        {
            self.progress = *progress;
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.worker.is_finished())
        {
            let pending = self.pending.take().unwrap();
            let joined = pending.worker.join();
            let outcome = if joined.is_ok() {
                pending
                    .receiver
                    .try_recv()
                    .unwrap_or(Err(RelayError::InvalidInput))
            } else {
                Err(RelayError::InvalidInput)
            };
            let now = Instant::now();
            if let Err(error) = &outcome
                && let Some(wait) = schedule::server_retry_after(error)
            {
                self.server_retry_until = self
                    .server_retry_until
                    .max(Some(RetryDeadline::new(now, wait)));
            }
            if let Some(schedule) = &mut self.schedule {
                schedule.finish_status(now, outcome.as_ref().map(|_| ()));
            }
            if pending.generation == self.generation {
                self.progress = None;
                match outcome {
                    Ok(result) => {
                        self.result = Some(Summary::from(result.result()));
                        self.last_error = None;
                        completed = Some(result);
                    }
                    Err(error) => {
                        self.last_error = Some(Failure::Operation(error));
                    }
                }
                if self.schedule.as_ref().is_some_and(Schedule::is_paused) {
                    self.config = None;
                    self.collection = None;
                    self.token = None;
                }
            }
        }
        let now = Instant::now();
        if admitted
            && self.pending.is_none()
            && self.armed()
            && self.schedule.as_mut().is_some_and(|s| s.begin(now))
        {
            let config = PrivateConfig(self.config.as_ref().unwrap().0.clone());
            let token = self.token.as_ref().unwrap().clone();
            let collection = self.collection.clone();
            let cancel = Arc::new(AtomicBool::new(false));
            let progress = Arc::new(Mutex::new(None));
            let (sender, receiver) = mpsc::channel();
            let worker_cancel = Arc::clone(&cancel);
            let worker_progress = Arc::clone(&progress);
            let worker = thread::Builder::new()
                .name("relay-schedule".into())
                .spawn(move || {
                    let report = |value| {
                        if let Ok(mut progress) = worker_progress.lock() {
                            *progress = Some(value);
                        }
                    };
                    let result = if let Some((workspace, id)) = collection {
                        collection_schedule::run(
                            &workspace,
                            id,
                            &config.0,
                            &token,
                            &worker_cancel,
                            &report,
                        )
                        .map(|completed| Completed::Collection(Box::new(completed)))
                    } else {
                        schedule::run(&config.0, &token, &worker_cancel, &report)
                            .map(|result| Completed::Subscription(Box::new(result)))
                    };
                    let _ = sender.send(result);
                });
            match worker {
                Ok(worker) => {
                    self.pending = Some(Pending {
                        worker,
                        receiver,
                        cancel,
                        progress,
                        generation: self.generation,
                    })
                }
                Err(_) => {
                    let error = RelayError::InvalidInput;
                    self.schedule
                        .as_mut()
                        .unwrap()
                        .finish_status(now, Err(&error));
                    self.last_error = Some(Failure::Operation(error));
                    self.config = None;
                    self.collection = None;
                    self.token = None;
                }
            }
        }
        if self.running() {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if self.armed() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        completed
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> RelayConfig {
        RelayConfig {
            local_content: Some(Zeroizing::new("local".into())),
            output: "0123456789abcdef0123456789abcdef".into(),
            ..RelayConfig::default()
        }
    }

    #[test]
    fn arm_is_offline_validated_and_stop_forgets_session() {
        let mut state = State::default();
        assert!(state.arm(RelayConfig::default(), "test").is_err());
        assert!(state.arm(config(), "").is_err());
        assert!(state.arm(config(), "test").is_ok());
        assert!(state.armed());
        assert!(state.tick(&egui::Context::default(), true).is_none());
        assert!(!state.running());
        state.stop();
        assert!(!state.armed());
        assert!(state.config.is_none() && state.token.is_none());
    }

    #[test]
    fn interval_bounds_are_checked_before_arming() {
        let mut state = State::default();
        state.interval_minutes = 0;
        assert!(state.arm(config(), "test").is_err());
        state.interval_minutes = u64::MAX;
        assert!(state.arm(config(), "test").is_err());
    }

    fn finish_failed_run(state: &mut State, error: RelayError) {
        let schedule = state.schedule.as_mut().unwrap();
        assert!(schedule.begin(schedule.next_due().unwrap()));
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || sender.send(Err(error)).unwrap());
        state.pending = Some(Pending {
            worker,
            receiver,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(Mutex::new(None)),
            generation: state.generation,
        });
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(2);
        while state.running() && Instant::now() < deadline {
            assert!(state.tick(&ctx, false).is_none());
            thread::yield_now();
        }
        assert!(!state.running());
    }

    #[test]
    fn transient_and_fatal_errors_keep_last_confirmed_sync_but_have_distinct_states() {
        let mut state = State::default();
        state.arm(config(), "test").unwrap();
        let initial_due = state.schedule.as_ref().unwrap().next_due().unwrap();
        assert_eq!(
            state.status_at(initial_due - Duration::from_millis(500)),
            Status::Waiting {
                seconds: 1,
                retry: false
            }
        );
        assert_eq!(
            state.status_at(initial_due),
            Status::Waiting {
                seconds: 0,
                retry: false
            }
        );
        state.result = Some(Summary {
            total: 4,
            removed: 1,
            unchanged: false,
        });
        finish_failed_run(&mut state, RelayError::SourceChanged);
        assert!(matches!(
            state.status(),
            Status::Waiting { retry: true, .. }
        ));
        assert!(state.last_error.is_some());
        assert_eq!(state.result.unwrap().total, 4);
        finish_failed_run(&mut state, RelayError::NoHealthyNodes);
        assert_eq!(state.status(), Status::Paused);
        assert!(state.last_error.is_some());
        assert_eq!(state.result.unwrap().total, 4);
        assert!(!state.armed() && state.config.is_none() && state.token.is_none());
        state.clear_feedback();
        assert_eq!(state.status(), Status::Idle);
    }

    #[test]
    fn stop_discards_late_feedback_but_preserves_real_server_retry_deadline() {
        for seconds in [120, u64::MAX] {
            check_late_server_retry(seconds);
        }
    }

    fn check_late_server_retry(seconds: u64) {
        let mut state = State::default();
        state.arm(config(), "test").unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&cancel);
        let (release, wait) = mpsc::channel();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            wait.recv().unwrap();
            assert!(observed.load(Ordering::Acquire));
            sender
                .send(Err(RelayError::InputRateLimited {
                    retry_after_seconds: Some(seconds),
                }))
                .unwrap();
        });
        state.pending = Some(Pending {
            worker,
            receiver,
            cancel,
            progress: Arc::new(Mutex::new(None)),
            generation: state.generation,
        });
        state.stop();
        assert_eq!(state.status(), Status::Stopping);
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while state.running() && Instant::now() < deadline {
            assert!(state.tick(&egui::Context::default(), false).is_none());
            thread::yield_now();
        }
        assert_eq!(state.status(), Status::Idle);
        assert!(state.last_error.is_none() && state.result.is_none());
        let deadline = state.server_retry_until().unwrap();
        assert!(deadline.remaining_seconds(Instant::now()) > 0);
        assert_eq!(deadline == RetryDeadline::Session, seconds == u64::MAX);
        state.clear_feedback();
        assert_eq!(state.server_retry_until(), Some(deadline));
        assert!(state.arm(config(), "test").is_err());
    }

    #[test]
    fn completed_result_is_returned_once_with_summary() {
        let mut state = State::default();
        state.arm(config(), "test").unwrap();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            assert!(
                sender
                    .send(Ok(Completed::Subscription(Box::new(RelayResult {
                        source: None,
                        content: Zeroizing::new("fresh".into()),
                        tested: false,
                        created: None,
                        surge_policy: None,
                        script_target: None,
                        total: 1,
                        healthy: 0,
                        processed: 1,
                        excluded: 0,
                        duplicates: 0,
                        duplicate_skipped: 0,
                        stages: Vec::new(),
                        failed: 0,
                        unknown: 1,
                        security_blocked: 0,
                        removed: 0,
                        baseline_valid: 0,
                        details: Vec::new(),
                        published: true,
                        unchanged: false,
                    }))))
                    .is_ok()
            );
        });
        state.pending = Some(Pending {
            worker,
            receiver,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(Mutex::new(None)),
            generation: state.generation,
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut completed = None;
        while state.running() && Instant::now() < deadline {
            completed = state.tick(&egui::Context::default(), false);
            thread::yield_now();
        }
        assert_eq!(completed.unwrap().result().content.as_str(), "fresh");
        assert_eq!(state.result.unwrap().total, 1);
        assert!(state.tick(&egui::Context::default(), false).is_none());
    }
}
