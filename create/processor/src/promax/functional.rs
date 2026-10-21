use once_cell::sync::Lazy;
use regex::Regex;
use std::net::IpAddr;

static DOMAIN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^[a-z0-9_*?](?:[a-z0-9._*?-]*[a-z0-9_*?])?$")
        .expect("domain source regex must compile")
});

pub fn normalize_domain_source_line(line: &str) -> Option<String> {
    let mut candidate = line.trim();
    if candidate.is_empty()
        || candidate.starts_with('#')
        || candidate.starts_with(';')
        || candidate.starts_with('!')
    {
        return None;
    }
    candidate = candidate.strip_prefix("- ").unwrap_or(candidate).trim();
    if let Some(stripped) = candidate.strip_prefix("||") {
        candidate = stripped
            .strip_suffix("^|")
            .or_else(|| stripped.strip_suffix('^'))
            .unwrap_or(stripped);
    } else if candidate.split_whitespace().count() >= 2 {
        let mut fields = candidate.split_whitespace();
        let address = fields.next()?;
        if address.parse::<IpAddr>().is_err() {
            return None;
        }
        candidate = fields.next()?;
    }

    candidate = candidate
        .trim_matches(['\'', '"'])
        .trim_start_matches("+.")
        .trim_start_matches('.')
        .trim_end_matches('^');
    let subdomain_wildcard = candidate.starts_with("*.");
    if subdomain_wildcard {
        candidate = &candidate[2..];
    }
    if !candidate.contains('.')
        || candidate.contains('/')
        || candidate.contains(':')
        || candidate.parse::<IpAddr>().is_ok()
        || !DOMAIN.is_match(candidate)
    {
        return None;
    }

    let kind = if subdomain_wildcard || candidate.contains('*') || candidate.contains('?') {
        "DOMAIN-WILDCARD"
    } else {
        "DOMAIN-SUFFIX"
    };
    let domain = candidate.to_ascii_lowercase();
    let domain = if subdomain_wildcard {
        format!("*.{domain}")
    } else {
        domain
    };
    Some(format!("{kind},{domain}"))
}

#[cfg(test)]
mod tests {
    use super::normalize_domain_source_line;

    #[test]
    fn preserves_subdomain_only_wildcards() {
        assert_eq!(
            normalize_domain_source_line("*.example.test").as_deref(),
            Some("DOMAIN-WILDCARD,*.example.test")
        );
        assert_eq!(
            normalize_domain_source_line("||*.example.test^").as_deref(),
            Some("DOMAIN-WILDCARD,*.example.test")
        );
        assert_eq!(
            normalize_domain_source_line("+.example.test").as_deref(),
            Some("DOMAIN-SUFFIX,example.test")
        );
        assert_eq!(
            normalize_domain_source_line(".example.test").as_deref(),
            Some("DOMAIN-SUFFIX,example.test")
        );
    }

    #[test]
    fn recognizes_plain_hosts_and_adblock_domain_lines() {
        assert_eq!(
            normalize_domain_source_line("ads.example.com").as_deref(),
            Some("DOMAIN-SUFFIX,ads.example.com")
        );
        assert_eq!(
            normalize_domain_source_line("0.0.0.0 tracker.example.net").as_deref(),
            Some("DOMAIN-SUFFIX,tracker.example.net")
        );
        assert_eq!(
            normalize_domain_source_line("||telemetry.example.org^").as_deref(),
            Some("DOMAIN-SUFFIX,telemetry.example.org")
        );
        assert_eq!(
            normalize_domain_source_line("- '+.sponsor.example.dev'").as_deref(),
            Some("DOMAIN-SUFFIX,sponsor.example.dev")
        );
    }
}
