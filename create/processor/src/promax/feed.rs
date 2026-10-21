use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct VerifiedFeed {
    pub(crate) source: String,
    pub(crate) version: String,
    pub(crate) declared_entries: usize,
}

pub(crate) fn verify_rolling_feed(
    source: &str,
    category: &str,
    text: &str,
) -> Result<Option<VerifiedFeed>, String> {
    const OFFICIAL_SOURCES: &[&str] = &[
        "https://raw.githubusercontent.com/hagezi/dns-blocklists/main/adblock/tif.medium.txt",
        "https://gitlab.com/hagezi/mirror/-/raw/main/dns-blocklists/adblock/tif.medium.txt",
    ];
    if category != "ThreatIntel_TIF" || !OFFICIAL_SOURCES.contains(&source) {
        return Ok(None);
    }

    let mut adblock_plus = false;
    let mut version = None;
    let mut last_modified = None;
    let mut declared_entries = None;
    let mut entries = 0usize;
    for (index, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "[Adblock Plus]" {
            if adblock_plus {
                return Err("duplicate Adblock Plus header".to_string());
            }
            adblock_plus = true;
            continue;
        }
        if let Some(metadata) = line.strip_prefix('!') {
            if let Some((key, value)) = metadata.split_once(':') {
                match key.trim() {
                    "Version" if version.is_none() => version = Some(value.trim()),
                    "Version" => return Err("duplicate !Version header".to_string()),
                    "Last modified" if last_modified.is_none() => {
                        last_modified = Some(value.trim());
                    }
                    "Last modified" => return Err("duplicate !Last modified header".to_string()),
                    "Number of entries" if declared_entries.is_none() => {
                        let count = value.trim().parse::<usize>().map_err(|_| {
                            format!("invalid !Number of entries header at line {}", index + 1)
                        })?;
                        if count == 0 {
                            return Err("!Number of entries must be positive".to_string());
                        }
                        declared_entries = Some(count);
                    }
                    "Number of entries" => {
                        return Err("duplicate !Number of entries header".to_string());
                    }
                    _ => {}
                }
            }
            continue;
        }

        if !line.starts_with("||")
            || !line.ends_with('^')
            || line[2..line.len() - 1].contains(['|', '^'])
        {
            return Err(format!("invalid TIF domain rule at line {}", index + 1));
        }
        let normalized = crate::promax::functional::normalize_domain_source_line(line)
            .ok_or_else(|| format!("invalid TIF domain at line {}", index + 1))?;
        let parsed = crate::promax::rule::SurgeRule::parse(&normalized)
            .map_err(|error| format!("invalid TIF domain at line {}: {error}", index + 1))?;
        if parsed.kind != crate::promax::rule::RuleKind::DomainSuffix
            || parsed.policy.is_some()
            || !parsed.options.is_empty()
        {
            return Err(format!("invalid TIF domain rule at line {}", index + 1));
        }
        entries += 1;
    }

    if !adblock_plus {
        return Err("missing Adblock Plus header".to_string());
    }
    let version = version
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "missing or empty !Version header".to_string())?;
    if last_modified.is_none_or(str::is_empty) {
        return Err("missing or empty !Last modified header".to_string());
    }
    let declared_entries =
        declared_entries.ok_or_else(|| "missing !Number of entries header".to_string())?;
    if entries != declared_entries {
        return Err(format!(
            "TIF entry count mismatch: declared {declared_entries}, found {entries}"
        ));
    }
    Ok(Some(VerifiedFeed {
        source: source.to_string(),
        version: version.to_string(),
        declared_entries,
    }))
}

#[cfg(test)]
mod tests {
    use super::verify_rolling_feed;

    const SOURCE: &str =
        "https://raw.githubusercontent.com/hagezi/dns-blocklists/main/adblock/tif.medium.txt";
    const HEADER: &str =
        "[Adblock Plus]\n! Version: 2026.10.21\n! Last modified: 2026-10-21T00:00:00Z\n";

    #[test]
    fn valid_feed_allows_entry_count_to_shrink() {
        let old = format!("{HEADER}! Number of entries: 2\n||one.example^\n||two.example^");
        let new = format!("{HEADER}! Number of entries: 1\n||one.example^");

        assert!(
            verify_rolling_feed(SOURCE, "ThreatIntel_TIF", &old)
                .unwrap()
                .is_some()
        );
        assert!(
            verify_rolling_feed(SOURCE, "ThreatIntel_TIF", &new)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn truncated_feed_does_not_match_declared_count() {
        let text = format!("{HEADER}! Number of entries: 2\n||one.example^");
        assert!(verify_rolling_feed(SOURCE, "ThreatIntel_TIF", &text).is_err());
    }

    #[test]
    fn malformed_domain_is_rejected() {
        let text = format!("{HEADER}! Number of entries: 1\n||not a domain^");
        assert!(verify_rolling_feed(SOURCE, "ThreatIntel_TIF", &text).is_err());
    }

    #[test]
    fn duplicate_zero_or_missing_headers_are_rejected() {
        for text in [
            format!("{HEADER}! Number of entries: 1\n! Number of entries: 1\n||one.example^"),
            format!("{HEADER}! Version: duplicate\n! Number of entries: 1\n||one.example^"),
            format!("{HEADER}! Number of entries: 0\n"),
            format!("{HEADER}! Number of entries: nope\n"),
            format!("{HEADER}||one.example^"),
            "[Adblock Plus]\n! Number of entries: 1\n! Last modified: now\n||one.example^"
                .to_string(),
            format!("{HEADER}! Last modified: \n! Number of entries: 1\n||one.example^"),
            format!(
                "[Adblock Plus]\n[Adblock Plus]\n{HEADER}! Number of entries: 1\n||one.example^"
            ),
        ] {
            assert!(verify_rolling_feed(SOURCE, "ThreatIntel_TIF", &text).is_err());
        }
    }

    #[test]
    fn untrusted_sources_are_not_verified_by_spoofed_headers() {
        let text = format!("{HEADER}! Number of entries: 1\n||one.example^");
        assert!(
            verify_rolling_feed(
                "https://evil.example/tif.medium.txt",
                "ThreatIntel_TIF",
                &text
            )
            .unwrap()
            .is_none()
        );
        assert!(
            verify_rolling_feed(SOURCE, "Advertising", &text)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn verified_metadata_preserves_source_and_version() {
        let text = format!("{HEADER}! Number of entries: 1\n||one.example^");
        let verified = verify_rolling_feed(SOURCE, "ThreatIntel_TIF", &text)
            .unwrap()
            .unwrap();
        assert_eq!(verified.source, SOURCE);
        assert_eq!(verified.version, "2026.10.21");
        assert_eq!(verified.declared_entries, 1);
    }
}
