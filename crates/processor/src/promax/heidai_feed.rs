//! Verification and protected domain projection for the complete 217heidai DNS feed.

use super::rule::SurgeRule;
use super::safety::{SafetyDecision, SafetyPolicy, classify_domain_payload};
use super::source::{DownloadConfig, Downloader};
use crate::dns_name::{ancestors, is_dotted_ascii_host};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
use std::fmt::Write as _;

pub(crate) const HEIDAI_FULL: &str =
    "https://github.com/217heidai/adblockfilters/releases/download/rule/adblockdns.txt";
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;

pub(crate) struct Feed {
    blocks: BTreeSet<String>,
    pub(crate) block_rows: usize,
    pub(crate) unsupported_blocks: usize,
    exceptions: HashSet<String>,
    pub(crate) exception_rows: usize,
    pub(crate) version: String,
}

pub(crate) fn parse_feed(text: &str) -> Result<Feed, String> {
    let mut blocks = BTreeSet::new();
    let mut exceptions = HashSet::new();
    let mut version = None;
    let mut block_count = None;
    let mut exception_count = None;
    let mut block_rows = 0;
    let mut exception_rows = 0;
    let mut unsupported_blocks = 0;
    for (index, raw) in text.lines().enumerate() {
        let row = raw.trim().trim_start_matches('\u{feff}');
        if let Some(value) = row.strip_prefix("! Version: ") {
            version = Some(value);
        } else if let Some(value) = row.strip_prefix("! Blocked domains: ") {
            block_count = value.parse::<usize>().ok();
        } else if let Some(value) = row.strip_prefix("! unBlocked domains: ") {
            exception_count = value.parse::<usize>().ok();
        } else if row.starts_with("!#") {
            return Err("Complete source contains an unsupported conditional".into());
        } else if !row.is_empty() && !row.starts_with('!') && row != "[Adblock Plus 2.0]" {
            let (exception, pattern) = row
                .strip_prefix("@@")
                .map_or((false, row), |pattern| (true, pattern));
            let anchor = pattern.strip_prefix("||").ok_or_else(|| {
                format!("Unsupported complete-source record on line {}", index + 1)
            })?;
            let host = anchor
                .strip_suffix('^')
                .map(str::to_ascii_lowercase)
                .filter(|host| is_dotted_ascii_host(host));
            if exception {
                exception_rows += 1;
                exceptions.insert(host.ok_or_else(|| {
                    format!(
                        "Unsupported complete-source exception on line {}",
                        index + 1
                    )
                })?);
            } else {
                block_rows += 1;
                if let Some(host) = host {
                    blocks.insert(host);
                } else {
                    unsupported_blocks += 1;
                }
            }
        }
    }
    let version = version
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or("Complete source is missing a valid version")?;
    if blocks.is_empty()
        || block_count != Some(block_rows)
        || exception_count != Some(exception_rows)
    {
        return Err("Complete source counts do not match its release header".into());
    }
    Ok(Feed {
        blocks,
        block_rows,
        unsupported_blocks,
        exceptions,
        exception_rows,
        version: version.into(),
    })
}

pub(crate) fn select_domains<'a, 'host>(
    feed: &'a Feed,
    safety: &SafetyPolicy,
    protected_hosts: impl IntoIterator<Item = &'host str>,
) -> Result<Vec<&'a str>, String> {
    let exception_parents = feed
        .exceptions
        .iter()
        .flat_map(|host| ancestors(host))
        .collect::<HashSet<_>>();
    let infrastructure_parents = protected_hosts
        .into_iter()
        .flat_map(ancestors)
        .collect::<HashSet<_>>();
    let mut selected = BTreeSet::new();
    for host in &feed.blocks {
        if ancestors(host).any(|name| feed.exceptions.contains(name))
            || exception_parents.contains(host.as_str())
            || infrastructure_parents.contains(host.as_str())
            || psl::suffix(host.as_bytes())
                .is_some_and(|suffix| suffix.is_known() && suffix.as_bytes() == host.as_bytes())
        {
            continue;
        }
        let line = format!("DOMAIN-SUFFIX,{host},REJECT");
        let rule = SurgeRule::parse(&line).map_err(|error| error.to_string())?;
        if safety.decision(&rule) == SafetyDecision::Keep
            && classify_domain_payload(host) == SafetyDecision::Keep
        {
            selected.insert(host.as_str());
        }
    }
    Ok(selected
        .iter()
        .copied()
        .filter(|host| {
            !ancestors(host)
                .skip(1)
                .any(|parent| selected.contains(parent))
        })
        .collect())
}

pub(crate) fn download() -> Result<(Feed, String), String> {
    let raw = Downloader::new(DownloadConfig {
        max_bytes: MAX_SOURCE_BYTES as u64,
        ..DownloadConfig::default()
    })
    .get(HEIDAI_FULL)
    .map_err(|error| error.to_string())?;
    let feed = parse_feed(&raw)?;
    let mut digest = String::with_capacity(64);
    for byte in Sha256::digest(raw.as_bytes()) {
        write!(digest, "{byte:02x}").expect("writing to a string cannot fail");
    }
    Ok((feed, digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_declared_counts_and_does_not_widen_abp_conditions() {
        let header = "! Version: 20261118144847\n! Blocked domains: 2\n! unBlocked domains: 1\n";
        let text = format!(
            "{header}||ordinary.test^\n||fb_servpub-a.example.com^\n@@||child.ordinary.test^\n"
        );
        let feed = parse_feed(&text).unwrap();
        assert_eq!(feed.blocks.len(), 2);
        assert_eq!(feed.exceptions.len(), 1);
        assert!(parse_feed(&text.replace("2\n!", "3\n!")).is_err());
        let scoped =
            parse_feed(&text.replace("||ordinary.test^", "||ordinary.test^$third-party")).unwrap();
        assert_eq!(scoped.block_rows, 2);
        assert_eq!(scoped.unsupported_blocks, 1);
        assert!(!scoped.blocks.contains("ordinary.test"));
        assert!(
            parse_feed(&text.replace("child.ordinary.test^", "child.ordinary.test^$third-party"))
                .is_err()
        );
    }
}
