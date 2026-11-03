//! In-process scheduling; the caller owns the worker and session credentials.

use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use super::{
    Progress, RelayConfig, RelayError, RelayResult, RunMode,
    gist::GistError,
    model::{ModelError, pipeline::ResolveError},
};

pub const MIN_INTERVAL: Duration = Duration::from_secs(60);
pub const MAX_INTERVAL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// One in-flight job at most. The owner must call `finish` after each `begin`.
pub struct Schedule {
    interval: Duration,
    next: Option<Instant>,
    running: bool,
    paused: bool,
    failures: u32,
}

impl Schedule {
    pub fn new(interval: Duration, now: Instant) -> Result<Self, RelayError> {
        if !(MIN_INTERVAL..=MAX_INTERVAL).contains(&interval) {
            return Err(RelayError::InvalidInput);
        }
        let next = now.checked_add(interval).ok_or(RelayError::InvalidInput)?;
        Ok(Self {
            interval,
            next: Some(next),
            running: false,
            paused: false,
            failures: 0,
        })
    }

    pub fn next_due(&self) -> Option<Instant> {
        self.next
    }
    pub fn is_running(&self) -> bool {
        self.running
    }
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Whether the next deadline is a retry after a transient failure.
    pub fn is_waiting_to_retry(&self) -> bool {
        self.next.is_some() && self.failures != 0
    }

    pub fn due(&self, now: Instant) -> bool {
        !self.paused && !self.running && self.next.is_some_and(|next| now >= next)
    }

    pub fn begin(&mut self, now: Instant) -> bool {
        if !self.due(now) {
            return false;
        }
        self.running = true;
        self.next = None;
        true
    }

    pub fn pause(&mut self) {
        self.paused = true;
        self.next = None;
    }

    pub fn resume(&mut self, now: Instant) -> Result<(), RelayError> {
        if self.running {
            return Err(RelayError::InvalidInput);
        }
        let next = now
            .checked_add(self.interval)
            .ok_or(RelayError::InvalidInput)?;
        self.paused = false;
        self.failures = 0;
        self.next = Some(next);
        Ok(())
    }

    pub fn finish(&mut self, now: Instant, result: &Result<RelayResult, RelayError>) {
        self.finish_status(now, result.as_ref().map(|_| ()));
    }

    pub fn finish_status(&mut self, now: Instant, result: Result<(), &RelayError>) {
        if !self.running {
            return;
        }
        self.running = false;
        if self.paused {
            return;
        }
        let delay = match result {
            Ok(_) => {
                self.failures = 0;
                self.interval
            }
            Err(error) => match classify(error) {
                Outcome::Stop => {
                    self.pause();
                    return;
                }
                Outcome::Retry(minimum) => {
                    self.failures = self.failures.saturating_add(1);
                    let exponent = self.failures.saturating_sub(1).min(20);
                    let backoff = MIN_INTERVAL.saturating_mul(1 << exponent).min(MAX_INTERVAL);
                    backoff.max(minimum)
                }
            },
        };
        self.next = now.checked_add(delay);
        if self.next.is_none() {
            self.paused = true;
        }
    }
}

enum Outcome {
    Retry(Duration),
    Stop,
}

fn classify(error: &RelayError) -> Outcome {
    if let Some(delay) = server_retry_after(error) {
        return Outcome::Retry(delay);
    }
    match error {
        RelayError::InputUnavailable
        | RelayError::SourceChanged
        | RelayError::Gist(GistError::Transport) => Outcome::Retry(Duration::ZERO),
        RelayError::InputHttpStatus(status) | RelayError::Gist(GistError::HttpStatus(status))
            if (500..=599).contains(status) =>
        {
            Outcome::Retry(Duration::ZERO)
        }
        _ => Outcome::Stop,
    }
}

/// Server-directed waits only; local retry backoff does not apply to other workflows.
pub(crate) fn server_retry_after(error: &RelayError) -> Option<Duration> {
    let seconds = match error {
        RelayError::InputRateLimited {
            retry_after_seconds,
        }
        | RelayError::Gist(GistError::RateLimited {
            retry_after_seconds,
        }) => retry_after_seconds.unwrap_or(MIN_INTERVAL.as_secs()),
        RelayError::Model(ModelError::Resolution(ResolveError::RateLimited {
            retry_after_seconds,
        })) => *retry_after_seconds,
        _ => return None,
    };
    Some(Duration::from_secs(seconds))
}

/// Reject index bindings to changing upstream data before other validation.
pub fn validate(config: &RelayConfig) -> Result<(), RelayError> {
    if config.output.trim().is_empty()
        || (config.local_content.is_none() && !config.processing.nodes.is_empty())
    {
        return Err(RelayError::InvalidInput);
    }
    config.validate(RunMode::Publish)
}

/// Import fresh input and publish through the normal validated relay pipeline.
pub fn run(
    config: &RelayConfig,
    token: &str,
    cancel: &AtomicBool,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<RelayResult, RelayError> {
    validate(config)?;
    let imported = super::run(config, token, RunMode::Import, cancel, progress)?;
    let mut fresh = config.clone();
    fresh.expected_source = Some(imported.content);
    super::run(&fresh, token, RunMode::Publish, cancel, progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_bounds_single_flight_and_no_catch_up() {
        let now = Instant::now();
        assert!(Schedule::new(MIN_INTERVAL - Duration::from_secs(1), now).is_err());
        assert!(Schedule::new(MAX_INTERVAL + Duration::from_secs(1), now).is_err());
        let mut schedule = Schedule::new(MIN_INTERVAL, now).unwrap();
        let late = now + Duration::from_secs(600);
        assert!(schedule.begin(late));
        assert!(!schedule.begin(late));
        schedule.finish(late, &Err(RelayError::SourceChanged));
        assert!(!schedule.due(late));
        assert!(schedule.due(late + MIN_INTERVAL));
    }

    #[test]
    fn backoff_rate_limit_and_unsafe_write_stop() {
        let now = Instant::now();
        let mut schedule = Schedule::new(MIN_INTERVAL, now).unwrap();
        let mut at = now + MIN_INTERVAL;
        for seconds in [60, 120, 240] {
            assert!(schedule.begin(at));
            schedule.finish(at, &Err(RelayError::Gist(GistError::Transport)));
            assert_eq!(schedule.next_due(), Some(at + Duration::from_secs(seconds)));
            at += Duration::from_secs(seconds);
        }
        assert!(schedule.begin(at));
        schedule.finish(
            at,
            &Err(RelayError::Gist(GistError::RateLimited {
                retry_after_seconds: Some(1000),
            })),
        );
        assert_eq!(schedule.next_due(), Some(at + Duration::from_secs(1000)));
        at += Duration::from_secs(1000);
        assert!(schedule.begin(at));
        schedule.finish(at, &Err(RelayError::Gist(GistError::UncertainWrite)));
        assert!(schedule.is_paused());
        assert_eq!(schedule.next_due(), None);
        schedule.resume(at).unwrap();
        assert_eq!(schedule.next_due(), Some(at + MIN_INTERVAL));
    }

    #[test]
    fn pause_during_job_stays_paused() {
        let now = Instant::now();
        let mut schedule = Schedule::new(MIN_INTERVAL, now).unwrap();
        assert!(schedule.begin(now + MIN_INTERVAL));
        schedule.pause();
        assert!(schedule.resume(now).is_err());
        schedule.finish(now, &Err(RelayError::SourceChanged));
        assert!(schedule.is_paused());
        assert_eq!(schedule.next_due(), None);
    }

    #[test]
    fn empty_output_rejected_before_network() {
        let config = RelayConfig::default();
        assert!(matches!(
            run(&config, "", &AtomicBool::new(false), &|_| {}),
            Err(RelayError::InvalidInput)
        ));
    }

    #[test]
    fn source_errors_apply_retry_boundaries() {
        for status in [300, 301, 401, 403, 499, 600] {
            assert!(matches!(
                classify(&RelayError::InputHttpStatus(status)),
                Outcome::Stop
            ));
        }
        for status in [500, 503, 599] {
            assert!(matches!(
                classify(&RelayError::InputHttpStatus(status)),
                Outcome::Retry(_)
            ));
        }
        assert!(matches!(classify(&RelayError::InputInvalid), Outcome::Stop));
        let now = Instant::now();
        let mut schedule = Schedule::new(MIN_INTERVAL, now).unwrap();
        let at = now + MIN_INTERVAL;
        assert!(schedule.begin(at));
        schedule.finish(
            at,
            &Err(RelayError::InputRateLimited {
                retry_after_seconds: None,
            }),
        );
        assert_eq!(schedule.next_due(), Some(at + MIN_INTERVAL));
        let at = at + MIN_INTERVAL;
        assert!(schedule.begin(at));
        schedule.finish(
            at,
            &Err(RelayError::InputRateLimited {
                retry_after_seconds: Some(900),
            }),
        );
        assert_eq!(schedule.next_due(), Some(at + Duration::from_secs(900)));
    }

    #[test]
    fn dns_rate_limit_waits_for_retry_after_instead_of_pausing() {
        let now = Instant::now();
        let mut schedule = Schedule::new(MIN_INTERVAL, now).unwrap();
        let at = now + MIN_INTERVAL;
        assert!(!schedule.is_waiting_to_retry());
        assert!(schedule.begin(at));
        schedule.finish_status(
            at,
            Err(&RelayError::Model(ModelError::Resolution(
                ResolveError::RateLimited {
                    retry_after_seconds: 900,
                },
            ))),
        );
        assert!(schedule.is_waiting_to_retry());
        assert!(!schedule.due(at + Duration::from_secs(899)));
        let retry = at + Duration::from_secs(900);
        assert!(schedule.begin(retry));
        assert!(!schedule.is_waiting_to_retry());
        schedule.finish_status(retry, Ok(()));
        assert!(!schedule.is_waiting_to_retry());
        assert_eq!(schedule.next_due(), Some(retry + MIN_INTERVAL));
    }

    #[test]
    fn dns_safety_failures_still_pause() {
        for error in [ResolveError::ConsentRequired, ResolveError::Dnssec] {
            assert!(matches!(
                classify(&RelayError::Model(ModelError::Resolution(error))),
                Outcome::Stop
            ));
        }
    }

    #[test]
    fn server_wait_does_not_include_generic_retry_backoff() {
        assert_eq!(server_retry_after(&RelayError::InputUnavailable), None);
        assert_eq!(
            server_retry_after(&RelayError::InputRateLimited {
                retry_after_seconds: None,
            }),
            Some(MIN_INTERVAL)
        );
    }

    #[test]
    fn indexed_edits_require_static_local_content() {
        use super::super::model::editor::NodeEdit;
        let mut config = RelayConfig {
            source: "https://example.com/subscription".into(),
            output: "0123456789abcdef0123456789abcdef".into(),
            ..RelayConfig::default()
        };
        config.processing.nodes.push(NodeEdit {
            index: 0,
            included: true,
            name: None,
        });
        assert!(matches!(validate(&config), Err(RelayError::InvalidInput)));
        config.source.clear();
        config.local_content = Some(zeroize::Zeroizing::new("static".into()));
        assert!(validate(&config).is_ok());
    }
}
