//! Private subscription processing and explicit Gist relay operations.

pub mod backup;
pub mod engine;
pub mod gist;
pub mod model;

use serde::Deserialize;
use std::{
    fmt,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RelayConfig {
    #[serde(skip)]
    pub local_content: Option<Zeroizing<String>>,
    pub source: String,
    pub source_file: String,
    pub output: String,
    pub output_file: String,
    pub title: String,
    pub engine: PathBuf,
    pub test_nodes: bool,
    pub concurrency: usize,
    pub timeout_seconds: u64,
    pub compatibility: bool,
    pub doh: bool,
    pub rename: bool,
    pub sort: bool,
    pub allow_public_output: bool,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            local_content: None,
            source: String::new(),
            source_file: String::new(),
            output: String::new(),
            output_file: "nodes.list".to_owned(),
            title: "Script Hub Relay".to_owned(),
            engine: PathBuf::from("sing-box"),
            test_nodes: false,
            concurrency: 4,
            timeout_seconds: 8,
            compatibility: false,
            doh: false,
            rename: true,
            sort: true,
            allow_public_output: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunMode {
    Preview,
    Publish,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelayStage {
    Read,
    Check,
    Render,
    Publish,
    Complete,
}
#[derive(Clone, Copy, Debug)]
pub struct Progress {
    pub stage: RelayStage,
    pub checked: usize,
    pub total: usize,
}

pub struct RelayResult {
    pub content: Zeroizing<String>,
    pub tested: bool,
    pub created: Option<gist::CreatedGist>,
    pub total: usize,
    pub healthy: usize,
    pub failed: usize,
    pub unknown: usize,
    pub security_blocked: usize,
    pub removed: usize,
    pub baseline_valid: usize,
    pub published: bool,
    pub unchanged: bool,
}

#[derive(Debug)]
pub enum RelayError {
    InvalidInput,
    NoHealthyNodes,
    SourceChanged,
    Cancelled,
    Model(model::ModelError),
    Gist(gist::GistError),
    Engine(engine::EngineError),
}
impl fmt::Display for RelayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => f.write_str(
                "Check the input, distinct output target, name length and probe settings.",
            ),
            Self::NoHealthyNodes => {
                f.write_str("No node has verified success. Existing output was not changed.")
            }
            Self::SourceChanged => {
                f.write_str("The source changed during checking. Run again before publishing.")
            }
            Self::Cancelled => f.write_str("Relay cancelled. No new publication was started."),
            Self::Model(error) => error.fmt(f),
            Self::Gist(error) => error.fmt(f),
            Self::Engine(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for RelayError {}

impl RelayConfig {
    pub fn validate(&self, mode: RunMode) -> Result<(), RelayError> {
        let input = if self.source.trim().is_empty() {
            if self.local_content.is_none() {
                return Err(RelayError::InvalidInput);
            }
            None
        } else {
            Some(
                gist::GistFile::parse_source(self.source.trim(), &self.source_file)
                    .map_err(RelayError::Gist)?,
            )
        };
        if self
            .local_content
            .as_ref()
            .is_some_and(|content| content.len() > 4 * 1024 * 1024)
        {
            return Err(RelayError::InvalidInput);
        }
        if !self.output.trim().is_empty() {
            let output = gist::GistFile::parse(self.output.trim(), &self.output_file)
                .map_err(RelayError::Gist)?;
            if input.as_ref().is_some_and(|input| output.id == input.id) {
                return Err(RelayError::InvalidInput);
            }
        } else if mode == RunMode::Publish {
            gist::validate_filename(&self.output_file).map_err(RelayError::Gist)?;
        }
        gist::validate_name(&self.title).map_err(RelayError::Gist)?;
        if self.test_nodes
            && (!(1..=8).contains(&self.concurrency)
                || !(1..=20).contains(&self.timeout_seconds)
                || self.engine.as_os_str().is_empty())
        {
            return Err(RelayError::InvalidInput);
        }
        Ok(())
    }
}

/// Formats nodes, optionally checks them, and explicitly publishes to a new secret or distinct Gist.
///
/// Cancellation prevents new checks and writes, but cannot undo a write already accepted by GitHub.
/// Errors never contain subscription contents or credentials. Preview does not publish.
pub fn run(
    config: &RelayConfig,
    token: &str,
    mode: RunMode,
    cancel: &AtomicBool,
    progress: &(dyn Fn(Progress) + Sync),
) -> Result<RelayResult, RelayError> {
    config.validate(mode)?;
    active(cancel)?;
    let client = if token.is_empty() && mode == RunMode::Preview {
        gist::GistClient::anonymous()
    } else {
        gist::GistClient::new(token).map_err(RelayError::Gist)?
    };
    run_with(
        config,
        &client,
        mode,
        cancel,
        progress,
        &engine::check_nodes,
    )
}

type Checker<'a> = dyn Fn(
        &model::Subscription,
        &engine::ProbeOptions,
        &AtomicBool,
        &(dyn Fn(usize, usize) + Sync),
    ) -> Result<engine::ProbeBatch, engine::EngineError>
    + 'a;
fn run_with(
    config: &RelayConfig,
    client: &gist::GistClient,
    mode: RunMode,
    cancel: &AtomicBool,
    progress: &(dyn Fn(Progress) + Sync),
    check: &Checker<'_>,
) -> Result<RelayResult, RelayError> {
    config.validate(mode)?;
    active(cancel)?;
    progress(Progress {
        stage: RelayStage::Read,
        checked: 0,
        total: 0,
    });
    let input = if config.local_content.is_none() {
        Some(
            client
                .resolve_source(config.source.trim(), &config.source_file, cancel)
                .map_err(RelayError::Gist)?,
        )
    } else {
        None
    };
    let source = input
        .as_ref()
        .map(|input| client.read(input, cancel))
        .transpose()
        .map_err(RelayError::Gist)?;
    let source_content = config
        .local_content
        .as_ref()
        .or_else(|| source.as_ref().and_then(|source| source.content.as_ref()))
        .ok_or(RelayError::InvalidInput)?;
    let subscription = model::Subscription::parse(source_content).map_err(RelayError::Model)?;
    let destination = if mode == RunMode::Publish && !config.output.trim().is_empty() {
        let output = gist::GistFile::parse(config.output.trim(), &config.output_file)
            .map_err(RelayError::Gist)?;
        let expected = client.read(&output, cancel).map_err(RelayError::Gist)?;
        if expected.public && !config.allow_public_output {
            return Err(RelayError::Gist(gist::GistError::PublicOutput));
        }
        Some((output, expected))
    } else {
        None
    };
    active(cancel)?;
    let total = subscription.nodes().len();
    let batch = if config.test_nodes {
        progress(Progress {
            stage: RelayStage::Check,
            checked: 0,
            total,
        });
        let options = engine::ProbeOptions {
            engine: config.engine.clone(),
            concurrency: config.concurrency,
            timeout: Duration::from_secs(config.timeout_seconds),
            compatibility: config.compatibility,
            doh: config.doh,
        };
        check(&subscription, &options, cancel, &|checked, total| {
            progress(Progress {
                stage: RelayStage::Check,
                checked,
                total,
            })
        })
        .map_err(RelayError::Engine)?
    } else {
        engine::ProbeBatch {
            health: vec![model::Health::Unknown; total],
            baseline_valid: 0,
        }
    };
    active(cancel)?;
    if mode == RunMode::Publish && config.test_nodes && !publication_ready(&batch.health) {
        return Err(RelayError::NoHealthyNodes);
    }
    progress(Progress {
        stage: RelayStage::Render,
        checked: total,
        total,
    });
    let content = subscription
        .render(
            &batch.health,
            &model::EnhanceOptions {
                rename: config.rename,
                sort: config.sort,
            },
        )
        .map_err(RelayError::Model)?;
    let retained = model::Subscription::parse(&content)
        .map_err(RelayError::Model)?
        .nodes()
        .len();
    if retained == 0 {
        return Err(RelayError::NoHealthyNodes);
    }
    let mut result = RelayResult {
        content,
        tested: config.test_nodes,
        created: None,
        total,
        healthy: 0,
        failed: 0,
        unknown: 0,
        security_blocked: 0,
        removed: total.saturating_sub(retained),
        baseline_valid: batch.baseline_valid,
        published: false,
        unchanged: false,
    };
    for health in batch.health {
        match health {
            model::Health::Healthy { .. } => result.healthy += 1,
            model::Health::Failed => result.failed += 1,
            model::Health::Unknown => result.unknown += 1,
            model::Health::SecurityBlocked => result.security_blocked += 1,
        }
    }
    if mode == RunMode::Publish {
        active(cancel)?;
        if let Some(input) = &input {
            let current = client.read(input, cancel).map_err(RelayError::Gist)?;
            if !current
                .content
                .as_ref()
                .is_some_and(|content| model::semantic_equal(source_content, content))
            {
                return Err(RelayError::SourceChanged);
            }
        }
        active(cancel)?;
        progress(Progress {
            stage: RelayStage::Publish,
            checked: total,
            total,
        });
        if let Some((output, expected)) = destination {
            let content = expected
                .content
                .as_ref()
                .filter(|old| model::semantic_equal(old, &result.content))
                .map_or(result.content.as_str(), |old| old.as_str());
            match client
                .publish(
                    input.as_ref(),
                    &output,
                    &expected,
                    content,
                    &config.title,
                    config.allow_public_output,
                    cancel,
                )
                .map_err(RelayError::Gist)?
            {
                gist::PublishOutcome::Unchanged => result.unchanged = true,
                gist::PublishOutcome::Verified => result.published = true,
            }
        } else {
            result.created = Some(
                client
                    .create(&config.output_file, &result.content, &config.title, cancel)
                    .map_err(RelayError::Gist)?,
            );
            result.published = true;
        }
    }
    progress(Progress {
        stage: RelayStage::Complete,
        checked: total,
        total,
    });
    Ok(result)
}

fn active(cancel: &AtomicBool) -> Result<(), RelayError> {
    if cancel.load(Ordering::Acquire) {
        Err(RelayError::Cancelled)
    } else {
        Ok(())
    }
}

fn publication_ready(health: &[model::Health]) -> bool {
    health
        .iter()
        .any(|health| matches!(health, model::Health::Healthy { .. }))
}

#[cfg(test)]
mod workflow_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_accepts_an_explicit_raw_source_without_output_or_token() {
        let config = RelayConfig {
            source: "https://gist.githubusercontent.com/fixture/0123456789abcdef/raw/nodes"
                .to_owned(),
            ..RelayConfig::default()
        };
        assert!(config.validate(RunMode::Preview).is_ok());
    }
    #[test]
    fn publication_requires_a_distinct_output_and_valid_name_before_any_io() {
        let mut config = RelayConfig {
            source: "0123456789abcdef".to_owned(),
            source_file: "nodes".to_owned(),
            output: "0123456789abcdef".to_owned(),
            ..RelayConfig::default()
        };
        assert!(config.validate(RunMode::Publish).is_err());
        config.output = "fedcba9876543210".to_owned();
        assert!(config.validate(RunMode::Publish).is_ok());
        config.title = "x".repeat(65);
        assert!(config.validate(RunMode::Publish).is_err());
    }
    #[test]
    fn untested_or_entirely_failed_candidates_cannot_publish() {
        assert!(!publication_ready(&[
            model::Health::Failed,
            model::Health::Unknown
        ]));
        assert!(!publication_ready(&[model::Health::SecurityBlocked]));
        assert!(publication_ready(&[
            model::Health::Healthy { latency_ms: 5 },
            model::Health::Unknown
        ]));
    }
    #[test]
    fn explicit_raw_filename_cannot_be_silently_trimmed_to_another_file() {
        let config = RelayConfig {
            source: "https://gist.githubusercontent.com/fixture/0123456789abcdef/raw/nodes.txt"
                .to_owned(),
            source_file: " nodes.txt ".to_owned(),
            ..RelayConfig::default()
        };
        assert!(config.validate(RunMode::Preview).is_err());
    }
}
