//! Localized summaries for relay failures.

use super::i18n::{Text, text};
use crate::relay::RelayError;
use crate::relay::schedule::{RetryDeadline, server_retry_after};
use crate::runtime::settings::Language;
use std::time::Instant;

pub(crate) fn operation_failure_message(language: Language, error: &RelayError) -> String {
    use crate::relay::model::pipeline::ResolveError;
    use crate::relay::{engine::EngineError, gist::GistError, model::ModelError};
    if server_retry_after(error)
        .is_some_and(|wait| RetryDeadline::new(Instant::now(), wait) == RetryDeadline::Session)
    {
        return text(language, Text::RelayRetrySession).to_owned();
    }
    let key = match error {
        RelayError::InvalidInput => Text::RelayConfigInvalid,
        RelayError::InputUnavailable => Text::RelayInputUnavailable,
        RelayError::InputInvalid => Text::RelaySubscriptionInvalid,
        RelayError::InputHttpStatus(status) => {
            return format!("{} {status}", text(language, Text::RelayInputStatus));
        }
        RelayError::InputRateLimited {
            retry_after_seconds,
        } => {
            return match retry_after_seconds {
                Some(seconds) => format!("{} {seconds} s", text(language, Text::RelayInputLimited)),
                None => text(language, Text::RelayInputLimited).to_owned(),
            };
        }
        RelayError::NoHealthyNodes => Text::RelayNoHealthy,
        RelayError::SourceChanged => Text::RelaySourceChanged,
        RelayError::Cancelled => Text::RelayCancelled,
        RelayError::ExportFormat => Text::RelayExportFormatMismatch,
        RelayError::ExportNode { index, .. } => {
            return text(language, Text::RelayExportNodeFailed)
                .replace("{node}", &index.to_string());
        }
        RelayError::Model(error) => match error {
            ModelError::InvalidInput => Text::RelaySubscriptionInvalid,
            ModelError::Unsupported => Text::RelayUnsupported,
            ModelError::SecurityBlocked => Text::RelaySecurityBlocked,
            ModelError::InvalidDependency => Text::RelayDependencyInvalid,
            ModelError::InvalidEdit => Text::RelayEditInvalid,
            ModelError::InvalidHealth | ModelError::Render => Text::RelayRenderFailed,
            ModelError::Resolution(error) => match error {
                ResolveError::ConsentRequired => Text::ResolutionConsentRequired,
                ResolveError::Cancelled => Text::ResolutionCancelled,
                ResolveError::Unavailable => Text::ResolutionUnavailable,
                ResolveError::InvalidResponse => Text::ResolutionResponseInvalid,
                ResolveError::Budget => Text::ResolutionBudget,
                ResolveError::Dnssec => Text::ResolutionDnssec,
                ResolveError::InvalidOptions => Text::ResolutionOptionsInvalid,
                ResolveError::RateLimited {
                    retry_after_seconds,
                } => {
                    return format!(
                        "{} {retry_after_seconds} s",
                        text(language, Text::ResolutionRateLimited)
                    );
                }
            },
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
            GistError::RateLimited {
                retry_after_seconds,
            } => {
                return match retry_after_seconds {
                    Some(seconds) => {
                        format!("{} {seconds} s", text(language, Text::RelayRateLimited))
                    }
                    None => text(language, Text::RelayRateLimitedUnknown).to_owned(),
                };
            }
            GistError::HttpStatus(status) => {
                return format!("{} {status}", text(language, Text::RelayHttpStatus));
            }
            GistError::InvalidResponse => Text::RelayResponseInvalid,
            GistError::TooLarge => Text::RelayTooLarge,
            GistError::IncompleteFiles => Text::RelayFileListIncomplete,
            GistError::SelectFile(_) => Text::RelaySelectFile,
            GistError::Conflict => Text::RelayOutputChanged,
            GistError::PublicOutput => Text::RelayPublicOutput,
            GistError::UncertainWrite => Text::RelayWriteUncertain,
            GistError::VerificationFailed => Text::RelayReadbackFailed,
            GistError::CreatedUnverified(created) => {
                return format!(
                    "{} {}",
                    text(language, Text::RelayReadbackFailed),
                    created.url
                );
            }
        },
    };
    text(language, key).to_owned()
}

pub(crate) fn archive_error(error: crate::relay::archive::ArchiveError) -> Text {
    use crate::relay::archive::ArchiveError;
    match error {
        ArchiveError::TooLarge => Text::RelayTooLarge,
        ArchiveError::InvalidKey => Text::ArchiveKeyInvalid,
        ArchiveError::Unsupported => Text::ArchiveUnsupported,
        ArchiveError::InvalidArchive | ArchiveError::EncryptionFailed => Text::ArchiveInvalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_errors_follow_the_selected_language_without_changing_the_core_error() {
        let error = RelayError::NoHealthyNodes;
        let original = error.to_string();
        let translated = operation_failure_message(Language::ZhCn, &error);
        assert_ne!(translated, original);
        assert!(translated.contains("节点"));
    }
}
