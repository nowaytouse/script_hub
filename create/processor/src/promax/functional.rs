use once_cell::sync::Lazy;
use regex::Regex;
use std::net::IpAddr;

static DOMAIN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^[a-z0-9_*?](?:[a-z0-9._*?-]*[a-z0-9_*?])?$")
        .expect("domain source regex must compile")
});

pub(crate) fn prepare_functional_source<'a>(
    source: &str,
    content: &'a str,
) -> std::borrow::Cow<'a, str> {
    // PROMAX does not expose the child module's template arguments; keep ordinary video posts.
    if source == "https://raw.githubusercontent.com/app2smile/rules/master/module/tieba.sgmodule" {
        content
            .replace("{{{per_filter_video}}}", "0")
            // Keep full alternative hosts visible to safety checks, not a spurious baidu.com parent.
            .replace(
                r"(tiebac|c\.tieba)\.baidu\.com",
                r"(tiebac\.baidu\.com|c\.tieba\.baidu\.com)",
            )
            .into()
    } else {
        content.into()
    }
}

pub(crate) fn narrow_http_authority_pattern(pattern: &str) -> String {
    let Some(prefix) = [
        r"^https?:\/\/",
        r"^https:\/\/",
        r"^http:\/\/",
        "^https?://",
        "^https://",
        "^http://",
    ]
    .into_iter()
    .find(|prefix| pattern.starts_with(prefix)) else {
        return pattern.to_string();
    };
    let rest = &pattern[prefix.len()..];
    let end = super::safety::regex_authority_end(rest);
    if end == rest.len() {
        return pattern.to_string();
    }
    let authority = &rest[..end];
    // Complex host assertions may intentionally inspect the path; leave them unchanged.
    if authority.contains(['(', ')', '[', ']', '|']) {
        return pattern.to_string();
    }
    let mut narrowed = String::new();
    let mut chars = authority.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\\' {
            narrowed.push(character);
            if let Some(escaped) = chars.next() {
                narrowed.push(escaped);
            }
        } else if character == '.' && chars.peek().is_some_and(|next| matches!(next, '*' | '+')) {
            // Hex delimiters preserve the existing source-admission tokenizer's decisions.
            narrowed.push_str(r"[^\x2f\x3f\x23]");
            narrowed.push(chars.next().unwrap_or('*'));
        } else {
            narrowed.push(character);
        }
    }
    format!("{prefix}{narrowed}{}", &rest[end..])
}

pub(crate) fn narrow_functional_authority(section: &str, line: &str) -> String {
    if !matches!(
        section,
        "Script" | "Rewrite" | "URL Rewrite" | "Map Local" | "Body Rewrite" | "Header Rewrite"
    ) {
        return line.to_string();
    }
    let pattern = if section == "Script" {
        let trimmed = line.trim_start();
        if trimmed.starts_with("http-request ") || trimmed.starts_with("http-response ") {
            trimmed.split_whitespace().nth(1)
        } else {
            line.split_once('=').and_then(|(_, parameters)| {
                crate::smart_cleanup::split_script_parameters(parameters)
                    .into_iter()
                    .find_map(|field| field.strip_prefix("pattern=").map(str::trim))
            })
        }
    } else {
        line.split_whitespace().next()
    };
    let Some(pattern) = pattern else {
        return line.to_string();
    };
    let narrowed = narrow_http_authority_pattern(pattern);
    if narrowed == pattern {
        return line.to_string();
    }
    // Parameter slices borrow this line, including when argument text resembles a pattern.
    let start = pattern.as_ptr().addr() - line.as_ptr().addr();
    format!(
        "{}{narrowed}{}",
        &line[..start],
        &line[start + pattern.len()..]
    )
}

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
    fn tieba_merge_keeps_video_posts_and_does_not_change_unrelated_templates() {
        let template = "argument=per_filter_video_thread={{{per_filter_video}}}";
        assert_eq!(
            super::prepare_functional_source(
                "https://raw.githubusercontent.com/app2smile/rules/master/module/tieba.sgmodule",
                template
            ),
            "argument=per_filter_video_thread=0"
        );
        assert_eq!(
            super::prepare_functional_source("https://example.test/other.sgmodule", template),
            template
        );
    }

    #[test]
    fn tieba_host_expansion_preserves_matching_without_touching_image_hosts() {
        let pattern = r"^https?:\/\/(tiebac|c\.tieba)\.baidu\.com/c/f/ad/getFeedAd";
        let prepared = super::prepare_functional_source(
            "https://raw.githubusercontent.com/app2smile/rules/master/module/tieba.sgmodule",
            pattern,
        );
        let original = regex::Regex::new(pattern).unwrap();
        let expanded = regex::Regex::new(&prepared).unwrap();
        for (url, expected) in [
            ("https://tiebac.baidu.com/c/f/ad/getFeedAd", true),
            ("http://c.tieba.baidu.com/c/f/ad/getFeedAd?x=1", true),
            ("https://t10.baidu.com/c/f/ad/getFeedAd", false),
            (
                "https://unrelated.test/c.tieba.baidu.com/c/f/ad/getFeedAd",
                false,
            ),
        ] {
            assert_eq!(original.is_match(url), expected, "{url}");
            assert_eq!(expanded.is_match(url), expected, "{url}");
        }
    }

    #[test]
    fn host_wildcards_do_not_trigger_body_scripts_for_domains_in_url_paths() {
        for (pattern, target, unrelated) in [
            (
                r"^https?:\/\/.*cupid\.iqiyi\.com\/mixer\?",
                "https://api.cupid.iqiyi.com/mixer?ad=1",
                "https://unrelated.example/path/cupid.iqiyi.com/mixer?ad=1",
            ),
            (
                r"^https?:\/\/.*\.xima.*\.com\/mobile-user\/v\d\/homePage",
                "https://mobile.ximalaya.com/mobile-user/v1/homePage",
                "https://unrelated.example/path/.ximaevil.com/mobile-user/v1/homePage",
            ),
            (
                r"^https?://.+\.example\.test/feed/.*",
                "https://api.example.test/feed/a/b",
                "https://unrelated.test/path/api.example.test/feed/a/b",
            ),
        ] {
            let old = regex::Regex::new(&pattern.replace(r"\/", "/")).unwrap();
            assert!(old.is_match(target));
            assert!(old.is_match(unrelated));
            let optimized = super::narrow_http_authority_pattern(pattern);
            let new = regex::Regex::new(&optimized.replace(r"\/", "/")).unwrap();
            assert!(new.is_match(target), "lost target: {optimized}");
            assert!(
                !new.is_match(unrelated),
                "cross-authority match: {optimized}"
            );
        }
    }

    #[test]
    fn authority_optimization_preserves_payloads_and_complex_patterns() {
        let line = r#"fixture = type=http-response,pattern=^https://.*\.example\.test/feed/.*,requires-body=1,script-path=https://scripts.test/a.js,argument="https://.*example.test""#;
        let narrowed = super::narrow_functional_authority("Script", line);
        assert_eq!(
            narrowed,
            line.replacen("https://.*", r"https://[^\x2f\x3f\x23]*", 1)
        );
        for pattern in [
            r"^https://(?!.*forbidden)api\.example\.test/.*",
            r"^https://[a-z.]*\.example\.test/.*",
            r"^https://api\.*example\.test/feed/.*",
            r"https://.*\.example\.test/feed",
        ] {
            assert_eq!(super::narrow_http_authority_pattern(pattern), pattern);
        }
    }

    #[test]
    fn authority_optimization_also_bounds_query_only_urls() {
        let pattern = r"^https://.*\.example\.test\?api=1";
        let narrowed = super::narrow_http_authority_pattern(pattern);
        let matcher = regex::Regex::new(&narrowed).unwrap();
        assert!(matcher.is_match("https://api.example.test?api=1"));
        assert!(!matcher.is_match("https://unrelated.test/path/api.example.test?api=1"));
    }

    #[test]
    fn authority_optimization_never_edits_argument_text_that_looks_like_a_pattern() {
        let line = r#"fixture = type=http-response,argument="pattern=^https://.*\.payload\.test/feed",pattern=^https://.*\.api\.test/ads,requires-body=1,script-path=https://scripts.test/a.js"#;
        let narrowed = super::narrow_functional_authority("Script", line);
        assert!(narrowed.contains(r#"argument="pattern=^https://.*\.payload\.test/feed""#));
        assert!(narrowed.contains(r"pattern=^https://[^\x2f\x3f\x23]*\.api\.test/ads"));
    }

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
