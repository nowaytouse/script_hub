//! Serializable DNS operation settings and non-sensitive execution reports.

use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use zeroize::Zeroize;

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    #[default]
    Cloudflare,
    Google,
}

impl Provider {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cloudflare => "Cloudflare",
            Self::Google => "Google",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    #[default]
    Post,
    Get,
    Json,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Zeroize)]
#[serde(rename_all = "snake_case")]
pub enum Selection {
    #[default]
    Lowest,
    First,
    Stable,
}

#[derive(Clone, Serialize, Deserialize, Zeroize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub provider: Provider,
    pub format: Format,
    pub selection: Selection,
    pub seed: u64,
    pub cache: bool,
    pub require_ad: bool,
    pub include: String,
    pub exclude: String,
    pub timeout_seconds: u64,
    pub concurrency: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            provider: Provider::Cloudflare,
            format: Format::Post,
            selection: Selection::Lowest,
            seed: 0,
            cache: true,
            require_ad: false,
            include: String::new(),
            exclude: String::new(),
            timeout_seconds: 8,
            concurrency: 4,
        }
    }
}

pub fn validate(options: &Options, enabled: bool) -> Result<(), super::ModelError> {
    if [&options.include, &options.exclude]
        .iter()
        .any(|s| s.len() > 512 || s.chars().any(char::is_control))
        || !(1..=20).contains(&options.timeout_seconds)
        || !(1..=4).contains(&options.concurrency)
    {
        return Err(super::ModelError::InvalidEdit);
    }
    if enabled {
        for value in [&options.include, &options.exclude]
            .into_iter()
            .filter(|s| !s.is_empty())
        {
            regex::RegexBuilder::new(value)
                .size_limit(1024 * 1024)
                .build()
                .map_err(|_| super::ModelError::InvalidEdit)?;
        }
    }
    Ok(())
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Report {
    pub unique_hosts: usize,
    pub queries: usize,
    pub cache_hits: usize,
    pub negative_hits: usize,
    pub resolved: usize,
    pub nxdomain: usize,
    pub nodata: usize,
    pub unsafe_answers: usize,
    pub authenticated: usize,
    pub unverified: usize,
    pub filtered: usize,
    pub candidates: usize,
}

pub struct Batch {
    pub addresses: Vec<Option<IpAddr>>,
    pub report: Report,
}

impl Batch {
    pub fn addresses(addresses: Vec<Option<IpAddr>>) -> Self {
        Self {
            addresses,
            report: Report::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_defaults_roundtrip_and_reject_unknown_fields() {
        let options: Options = serde_json::from_str("{}").unwrap();
        assert!(options.provider == Provider::Cloudflare);
        assert!(options.format == Format::Post);
        assert!(options.selection == Selection::Lowest);
        assert_eq!(options.seed, 0);
        assert!(options.cache && !options.require_ad);
        assert!(options.include.is_empty() && options.exclude.is_empty());
        assert_eq!((options.timeout_seconds, options.concurrency), (8, 4));
        let serialized = serde_json::to_string(&options).unwrap();
        let decoded: Options = serde_json::from_str(&serialized).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            serde_json::to_value(options).unwrap()
        );
        assert!(serde_json::from_str::<Options>(r#"{"unknown":true}"#).is_err());
        assert!(serde_json::from_str::<Options>(r#"{"provider":"other"}"#).is_err());
    }

    #[test]
    fn timeout_and_concurrency_require_inclusive_bounds_even_when_disabled() {
        for value in [1, 20] {
            let options = Options {
                timeout_seconds: value,
                ..Default::default()
            };
            assert!(validate(&options, true).is_ok());
        }
        for value in [0, 21] {
            let options = Options {
                timeout_seconds: value,
                ..Default::default()
            };
            assert!(validate(&options, true).is_err() && validate(&options, false).is_err());
        }
        for value in [1, 4] {
            let options = Options {
                concurrency: value,
                ..Default::default()
            };
            assert!(validate(&options, true).is_ok());
        }
        for value in [0, 5] {
            let options = Options {
                concurrency: value,
                ..Default::default()
            };
            assert!(validate(&options, true).is_err() && validate(&options, false).is_err());
        }
    }

    #[test]
    fn regex_drafts_have_bounded_text_and_compile_when_enabled() {
        for include_field in [true, false] {
            let mut options = Options::default();
            let value = if include_field {
                &mut options.include
            } else {
                &mut options.exclude
            };
            *value = "a".repeat(512);
            assert!(validate(&options, true).is_ok());
            let value = if include_field {
                &mut options.include
            } else {
                &mut options.exclude
            };
            *value = "a".repeat(513);
            assert!(validate(&options, true).is_err() && validate(&options, false).is_err());
            let value = if include_field {
                &mut options.include
            } else {
                &mut options.exclude
            };
            *value = "a\nb".into();
            assert!(validate(&options, true).is_err() && validate(&options, false).is_err());
            let value = if include_field {
                &mut options.include
            } else {
                &mut options.exclude
            };
            *value = "[".into();
            assert!(validate(&options, false).is_ok());
            assert!(validate(&options, true).is_err());
        }
    }
}
