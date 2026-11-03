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

use super::relay::PrivateConfig;
use crate::relay::{
    Progress, RelayConfig, RelayError, RelayResult, collection_schedule,
    library::Workspace,
    schedule::{self, Schedule},
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

pub struct State {
    pub interval_minutes: u64,
    pub progress: Option<Progress>,
    pub result: Option<Summary>,
    pub last_error: Option<RelayError>,
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
        workspace: &Workspace,
        id: u64,
    ) -> Result<(), RelayError> {
        self.arm_with(config, token, Some((workspace, id)))
    }

    fn arm_with(
        &mut self,
        config: RelayConfig,
        token: &str,
        collection: Option<(&Workspace, u64)>,
    ) -> Result<(), RelayError> {
        let config = PrivateConfig(config);
        if self.running() || self.armed() || token.is_empty() {
            return Err(RelayError::InvalidInput);
        }
        if let Some((workspace, id)) = collection {
            collection_schedule::validate(workspace, id, &config.0)?;
        } else {
            schedule::validate(&config.0)?;
        }
        crate::relay::gist::GistClient::new(token).map_err(RelayError::Gist)?;
        let seconds = self
            .interval_minutes
            .checked_mul(60)
            .ok_or(RelayError::InvalidInput)?;
        let schedule = Schedule::new(Duration::from_secs(seconds), Instant::now())?;
        self.generation = self.generation.wrapping_add(1);
        self.schedule = Some(schedule);
        self.config = Some(config);
        self.collection = collection.map(|(workspace, id)| (Arc::new(workspace.clone()), id));
        self.token = Some(Zeroizing::new(token.to_owned()));
        self.progress = None;
        self.result = None;
        self.last_error = None;
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
    }

    pub fn running(&self) -> bool {
        self.pending.is_some()
    }
    pub fn armed(&self) -> bool {
        self.config.is_some() && self.schedule.as_ref().is_some_and(|s| !s.is_paused())
    }
    pub fn seconds_until_due(&self) -> Option<u64> {
        self.schedule
            .as_ref()?
            .next_due()
            .map(|due| due.saturating_duration_since(Instant::now()).as_secs())
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
            if let Some(schedule) = &mut self.schedule {
                schedule.finish_status(Instant::now(), outcome.as_ref().map(|_| ()));
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
                        self.result = None;
                        self.last_error = Some(error);
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
                    self.last_error = Some(error);
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

    #[test]
    fn stop_cancels_pending_and_discards_late_result() {
        let mut state = State::default();
        state.arm(config(), "test").unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&cancel);
        let (release, wait) = mpsc::channel();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            wait.recv().unwrap();
            assert!(observed.load(Ordering::Acquire));
            sender.send(Err(RelayError::SourceChanged)).unwrap();
        });
        state.pending = Some(Pending {
            worker,
            receiver,
            cancel,
            progress: Arc::new(Mutex::new(None)),
            generation: state.generation,
        });
        state.stop();
        assert!(state.running());
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while state.running() && Instant::now() < deadline {
            assert!(state.tick(&egui::Context::default(), false).is_none());
            thread::yield_now();
        }
        assert!(!state.running());
        assert!(state.last_error.is_none() && state.result.is_none());
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
