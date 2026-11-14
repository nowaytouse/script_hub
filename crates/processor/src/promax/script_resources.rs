use std::collections::BTreeSet;

const MLXX_RESOURCE: &str =
    "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Scripts/mlxx/mlxx.js";
const MLXX_CDN: &str = "https://cdn.jsdelivr.net/gh/fmz200/wool_scripts@main/Scripts/mlxx/mlxx.js";
const MLXX_GITHUB: &str = "https://github.com/fmz200/wool_scripts/raw/main/Scripts/mlxx/mlxx.js";
const MLXX_PATTERN: &str =
    r"^https://api\.bevol\.com/(?:appmain/app/home/launch|trialbox/shop/app/homePage)$";
const MLXX_MIXED_PATTERN: &str = r"^https://api\.bevol\.com/(?:appmain/app/home/page|usercenter/account/info|personal/page|appmain/app/home/launch|trialbox/shop/app/homePage)$";
const MLXX_LAUNCH_PATTERN: &str = r"^https://api\.bevol\.com/appmain/app/home/launch$";
const MLXX_TRIAL_PATTERN: &str = r"^https://api\.bevol\.com/trialbox/shop/app/homePage$";
const GOOFISH_RESOURCE: &str =
    "https://raw.githubusercontent.com/ishowshu/qx/main/script/goofish.js";

const APP2SMILE_RESOURCES: &[&str] = &[
    "https://raw.githubusercontent.com/app2smile/rules/master/js/qidian.js",
    "https://raw.githubusercontent.com/app2smile/rules/master/js/tieba-json.js",
    "https://raw.githubusercontent.com/app2smile/rules/master/js/tieba-proto.js",
    "https://raw.githubusercontent.com/app2smile/rules/master/js/qq-news.js",
];

pub(crate) fn official_app_resource(url: &str) -> Option<&'static str> {
    const RAW: &str = "https://raw.githubusercontent.com/app2smile/rules/master/";
    const CDN: &str = "https://cdn.jsdelivr.net/gh/app2smile/rules@master/";
    let path = url.strip_prefix(RAW).or_else(|| url.strip_prefix(CDN))?;
    APP2SMILE_RESOURCES
        .iter()
        .copied()
        .find(|resource| resource.strip_prefix(RAW) == Some(path))
}

fn script_path_range(line: &str) -> Option<(usize, usize)> {
    if line.trim_start().starts_with('#') {
        return None;
    }
    let bytes = line.as_bytes();
    let mut cursor = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut depths = [0_u32; 3];

    while cursor <= bytes.len() {
        let end = cursor == bytes.len();
        let byte = bytes.get(cursor).copied();
        if end {
            break;
        }
        let at_top_level = quote.is_none() && depths == [0; 3];
        let is_boundary =
            cursor == 0 || bytes[cursor - 1] == b',' || bytes[cursor - 1].is_ascii_whitespace();
        if at_top_level
            && is_boundary
            && bytes
                .get(cursor..cursor + b"script-path".len())
                .is_some_and(|key| key.eq_ignore_ascii_case(b"script-path"))
        {
            let mut equals = cursor + b"script-path".len();
            while bytes.get(equals).is_some_and(u8::is_ascii_whitespace) {
                equals += 1;
            }
            if bytes.get(equals) == Some(&b'=') {
                let mut value_start = equals + 1;
                while bytes.get(value_start).is_some_and(u8::is_ascii_whitespace) {
                    value_start += 1;
                }
                let first = bytes.get(value_start).copied()?;
                if matches!(first, b'\'' | b'"') {
                    let mut value_end = value_start + 1;
                    let mut value_escaped = false;
                    while let Some(character) = bytes.get(value_end).copied() {
                        if character == first && !value_escaped {
                            return Some((value_start + 1, value_end));
                        }
                        value_escaped = character == b'\\' && !value_escaped;
                        if character != b'\\' {
                            value_escaped = false;
                        }
                        value_end += 1;
                    }
                    return None;
                }
                let mut value_end = value_start;
                while bytes
                    .get(value_end)
                    .is_some_and(|character| *character != b',' && !character.is_ascii_whitespace())
                {
                    value_end += 1;
                }
                return (value_start < value_end).then_some((value_start, value_end));
            }
        }
        if let Some(byte) = byte {
            if let Some(delimiter) = quote {
                if byte == delimiter && !escaped {
                    quote = None;
                }
                escaped = byte == b'\\' && !escaped;
                if byte != b'\\' {
                    escaped = false;
                }
            } else {
                match byte {
                    b'\'' | b'"' => quote = Some(byte),
                    b'{' => depths[0] += 1,
                    b'}' => depths[0] = depths[0].saturating_sub(1),
                    b'[' => depths[1] += 1,
                    b']' => depths[1] = depths[1].saturating_sub(1),
                    b'(' => depths[2] += 1,
                    b')' => depths[2] = depths[2].saturating_sub(1),
                    _ => {}
                }
            }
        }
        cursor += 1;
    }
    None
}

/// Return the URL or path belonging to a top-level script-path parameter.
pub fn script_resource_url(line: &str) -> Option<&str> {
    let (start, end) = script_path_range(line)?;
    line.get(start..end)
}

/// Match only the two known broken upstream resource families.
pub fn replacement_for(url: &str) -> Option<&'static str> {
    let trimmed = url.trim().trim_matches(['\'', '"']);
    let scheme_end = trimmed.find("://")?;
    let host_start = scheme_end + 3;
    let path_start = trimmed[host_start..].find('/')? + host_start;
    let host = &trimmed[host_start..path_start];
    let rest = &trimmed[path_start..];
    let path_end = rest.find(['?', '#']).unwrap_or(rest.len());
    let path = &rest[..path_end];
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let host = host.to_ascii_lowercase();

    let is_yu9191_mlxx = match host.as_str() {
        "raw.githubusercontent.com" => {
            segments.len() >= 4
                && segments[0].eq_ignore_ascii_case("Yu9191")
                && segments[1].eq_ignore_ascii_case("Rewrite")
                && segments
                    .last()
                    .is_some_and(|part| part.eq_ignore_ascii_case("mlxx.js"))
        }
        "cdn.jsdelivr.net" => {
            segments.len() >= 4
                && segments[0].eq_ignore_ascii_case("gh")
                && segments[1].eq_ignore_ascii_case("Yu9191")
                && segments[2]
                    .split('@')
                    .next()
                    .is_some_and(|repo| repo.eq_ignore_ascii_case("Rewrite"))
                && segments
                    .last()
                    .is_some_and(|part| part.eq_ignore_ascii_case("mlxx.js"))
        }
        _ => false,
    };
    if is_yu9191_mlxx {
        return Some(MLXX_RESOURCE);
    }

    if host == "klraw.pages.dev"
        && path.contains("/kelv1n1n/script/")
        && segments
            .last()
            .is_some_and(|part| part.eq_ignore_ascii_case("goofish.js"))
    {
        return Some(GOOFISH_RESOURCE);
    }
    None
}

fn is_current_mlxx_resource(url: &str) -> bool {
    [MLXX_RESOURCE, MLXX_CDN, MLXX_GITHUB]
        .iter()
        .any(|resource| {
            url.split(['?', '#'])
                .next()
                .is_some_and(|path| path.eq_ignore_ascii_case(resource))
        })
}

fn mlxx_pattern(line: &str) -> Option<&str> {
    let (_, fields) = line.split_once('=')?;
    let mut pattern = None;
    let mut kind = false;
    let mut body = false;
    let mut path = false;
    for field in crate::smart_cleanup::split_script_parameters(fields) {
        match field {
            "type=http-response" if !kind => kind = true,
            "requires-body=true" | "requires-body=1" if !body => body = true,
            _ if field.starts_with("pattern=") && pattern.is_none() => {
                pattern = field.strip_prefix("pattern=");
            }
            _ if field.starts_with("script-path=") && !path => path = true,
            _ => return None,
        }
    }
    (kind && body && path).then_some(pattern).flatten()
}

fn pure_mlxx_pattern(pattern: &str) -> bool {
    matches!(
        pattern,
        MLXX_PATTERN | MLXX_LAUNCH_PATTERN | MLXX_TRIAL_PATTERN
    )
}

pub(crate) fn is_unreviewed_mlxx(line: &str) -> bool {
    script_resource_url(line).is_some_and(|url| {
        (is_current_mlxx_resource(url) || replacement_for(url) == Some(MLXX_RESOURCE))
            && !mlxx_pattern(line)
                .is_some_and(|pattern| pure_mlxx_pattern(&pattern.replace(r"\/", "/")))
    })
}

/// Repair known script references; narrow the mixed Mlxx handler before admission.
pub fn normalize_script_line(line: &str) -> String {
    let Some((start, end)) = script_path_range(line) else {
        return line.to_string();
    };
    let url = &line[start..end];
    let replacement = replacement_for(url);
    if is_current_mlxx_resource(url) || replacement == Some(MLXX_RESOURCE) {
        let Some(pattern) = mlxx_pattern(line) else {
            return line.to_string();
        };
        let normalized = pattern.replace(r"\/", "/");
        let target = if normalized == MLXX_MIXED_PATTERN {
            // Account, profile, and home-page branches modify unrelated application state.
            MLXX_PATTERN
        } else if pure_mlxx_pattern(&normalized) {
            normalized.as_str()
        } else {
            return line.to_string();
        };
        let span = pattern.as_ptr().addr() - line.as_ptr().addr();
        let mut updated = line.to_string();
        let mut changes = [
            (start, end, MLXX_RESOURCE),
            (span, span + pattern.len(), target),
        ];
        changes.sort_unstable_by_key(|change| std::cmp::Reverse(change.0));
        for (start, end, text) in changes {
            updated.replace_range(start..end, text);
        }
        return updated;
    }
    let Some(replacement) = replacement else {
        return line.to_string();
    };
    format!("{}{}{}", &line[..start], replacement, &line[end..])
}

/// List only the supported resources referenced from script-path parameters.
pub fn required_resources(content: &str) -> BTreeSet<&'static str> {
    let mut resources = BTreeSet::new();
    for line in content.lines() {
        if let Some(url) = script_resource_url(line) {
            let replacement = replacement_for(url);
            if is_current_mlxx_resource(url) || replacement == Some(MLXX_RESOURCE) {
                if mlxx_pattern(line).is_some_and(|pattern| {
                    let normalized = pattern.replace(r"\/", "/");
                    normalized == MLXX_MIXED_PATTERN || pure_mlxx_pattern(&normalized)
                }) {
                    resources.insert(MLXX_RESOURCE);
                }
            } else if let Some(replacement) = replacement {
                resources.insert(replacement);
            } else if url.eq_ignore_ascii_case(GOOFISH_RESOURCE) {
                resources.insert(GOOFISH_RESOURCE);
            } else if let Some(resource) = official_app_resource(url) {
                resources.insert(resource);
            }
        }
    }
    resources
}

/// Reject empty/error-page responses and require a minimal Surge/Quantumult script shape.
pub fn validate_script_body(body: &str) -> Result<(), String> {
    let body = body
        .trim_start_matches(|character: char| character.is_whitespace() || character == '\u{feff}');
    if body.is_empty() {
        return Err("script resource is empty".to_string());
    }
    let lower = body
        .get(..body.len().min(32))
        .unwrap_or(body)
        .to_ascii_lowercase();
    if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
        return Err("script resource returned HTML".to_string());
    }
    if body.starts_with('{') || body.starts_with('[') {
        return Err("script resource returned JSON instead of JavaScript".to_string());
    }
    if !body.contains("$done") || !(body.contains("$response") || body.contains("function")) {
        return Err(
            "script resource does not contain the expected script runtime shape".to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        GOOFISH_RESOURCE, MLXX_CDN, MLXX_GITHUB, MLXX_MIXED_PATTERN, MLXX_PATTERN, MLXX_RESOURCE,
        is_unreviewed_mlxx, normalize_script_line, replacement_for, required_resources,
        script_resource_url, validate_script_body,
    };
    use std::collections::BTreeSet;

    #[test]
    fn licensed_app_dependencies_are_checked_without_crawling_other_resources() {
        let content = format!(
            "a = type=http-response,script-path={}\nb = type=http-response,script-path=https://example.test/unknown.js\n",
            super::APP2SMILE_RESOURCES[0]
        );
        assert_eq!(
            required_resources(&content),
            BTreeSet::from([super::APP2SMILE_RESOURCES[0]])
        );
        assert!(required_resources("a = type=http-response,script-path=https://raw.githubusercontent.com/app2smile/rules/master/js/qidian.js.evil").is_empty());
    }

    #[test]
    fn replaces_only_known_mlxx_raw_and_jsdelivr_references() {
        for url in [
            "https://raw.githubusercontent.com/Yu9191/Rewrite/main/mlxx.js",
            "https://raw.githubusercontent.com/Yu9191/Rewrite/refs/heads/main/mlxx.js",
            "https://cdn.jsdelivr.net/gh/Yu9191/Rewrite@main/mlxx.js",
            "https://cdn.jsdelivr.net/gh/Yu9191/Rewrite@refs/heads/main/mlxx.js?cache=1",
        ] {
            assert_eq!(replacement_for(url), Some(MLXX_RESOURCE));
        }
    }

    #[test]
    fn replaces_only_known_tokenized_klraw_goofish_reference() {
        assert_eq!(
            replacement_for("https://klraw.pages.dev/kelv1n1n/script/goofish.js?token=secret"),
            Some(GOOFISH_RESOURCE)
        );
    }

    #[test]
    fn rejects_lookalike_hosts_paths_and_maintainers() {
        for url in [
            "https://raw.githubusercontent.com.evil/Yu9191/Rewrite/main/mlxx.js",
            "https://evil.example/Yu9191/Rewrite/main/mlxx.js",
            "https://raw.githubusercontent.com/Other/Rewrite/main/mlxx.js",
            "https://raw.githubusercontent.com/Yu9191/Other/main/mlxx.js",
            "https://raw.githubusercontent.com/Yu9191/Rewrite/main/other.js",
            "https://klraw.pages.dev/other/script/goofish.js?token=secret",
            "https://evil.pages.dev/kelv1n1n/script/goofish.js?token=secret",
        ] {
            assert_eq!(replacement_for(url), None, "{url}");
        }
    }

    #[test]
    fn normalization_changes_only_script_path_value_and_drops_its_stale_query() {
        let line = r#"fish = type=http-response,pattern="^https://example.test/(foo,bar)",script-path=https://klraw.pages.dev/kelv1n1n/script/goofish.js?old=secret,argument="https://klraw.pages.dev/kelv1n1n/script/goofish.js""#;
        let normalized = normalize_script_line(line);
        assert!(normalized.contains(GOOFISH_RESOURCE));
        assert!(normalized.contains(r#"pattern="^https://example.test/(foo,bar)""#));
        assert!(
            normalized.contains(r#"argument="https://klraw.pages.dev/kelv1n1n/script/goofish.js""#)
        );
        assert!(!normalized.contains("old=secret"));
        assert_eq!(
            normalize_script_line(
                "# script-path=https://klraw.pages.dev/kelv1n1n/script/goofish.js?token=secret"
            ),
            "# script-path=https://klraw.pages.dev/kelv1n1n/script/goofish.js?token=secret"
        );
    }

    #[test]
    fn narrows_known_mlxx_sources_and_rejects_unreviewed_variants() {
        let id = |url, pattern| {
            format!("mlxx = type=http-response,script-path={url},pattern={pattern},requires-body=1")
        };
        for url in [
            MLXX_RESOURCE,
            MLXX_CDN,
            MLXX_GITHUB,
            "https://raw.githubusercontent.com/Yu9191/Rewrite/main/mlxx.js",
        ] {
            let line = id(url, MLXX_MIXED_PATTERN);
            assert!(is_unreviewed_mlxx(&line));
            let normalized = normalize_script_line(&line);
            assert_eq!(normalized, id(MLXX_RESOURCE, MLXX_PATTERN));
            assert!(!is_unreviewed_mlxx(&normalized));
            assert!(!is_unreviewed_mlxx(&id(MLXX_CDN, MLXX_PATTERN)));
            assert_eq!(normalize_script_line(&normalized), normalized);
            let unknown = id(url, r"^https://api\.bevol\.com/anything");
            assert_eq!(normalize_script_line(&unknown), unknown);
            assert!(is_unreviewed_mlxx(&unknown));
            assert!(required_resources(&unknown).is_empty());
        }
        let matcher = regex::Regex::new(MLXX_PATTERN).unwrap();
        for (path, expected) in [
            ("/appmain/app/home/launch", true),
            ("/trialbox/shop/app/homePage", true),
            ("/usercenter/account/info", false),
            ("/personal/page", false),
            ("/appmain/app/home/page", false),
            ("/appmain/app/home/launch/extra", false),
        ] {
            assert_eq!(
                matcher.is_match(&format!("https://api.bevol.com{path}")),
                expected,
                "{path}"
            );
        }
    }

    #[test]
    fn resource_discovery_reads_only_script_path_values_and_normalized_families() {
        let content = format!(
            r#"
one = type=http-response,script-path=https://raw.githubusercontent.com/Yu9191/Rewrite/main/mlxx.js,pattern={MLXX_MIXED_PATTERN},requires-body=true
two = type=http-response,script-path=https://klraw.pages.dev/kelv1n1n/script/goofish.js?token=secret
three = type=generic,argument="https://raw.githubusercontent.com/Yu9191/Rewrite/main/mlxx.js"
"#
        );
        assert_eq!(
            required_resources(&content),
            BTreeSet::from([MLXX_RESOURCE, GOOFISH_RESOURCE])
        );
        assert_eq!(
            script_resource_url(
                r#"x = type=generic,argument="script-path=https://example.test/a.js",script-path=https://example.test/real.js"#
            ),
            Some("https://example.test/real.js")
        );
        assert_eq!(
            script_resource_url("x = type=generic,not-script-path=https://example.test/a.js"),
            None
        );
        assert_eq!(
            script_resource_url(
                r"http-response ^https://api.example.test/item/{1,3} script-path=https://klraw.pages.dev/kelv1n1n/script/goofish.js?token=secret timeout=10"
            ),
            Some("https://klraw.pages.dev/kelv1n1n/script/goofish.js?token=secret")
        );
    }

    #[test]
    fn script_body_validation_rejects_empty_html_and_json_but_accepts_script_shape() {
        assert!(validate_script_body(" \n").is_err());
        assert!(validate_script_body("<!doctype html><title>404</title>").is_err());
        assert!(validate_script_body(r#"{"error":"not found"}"#).is_err());
        assert!(validate_script_body("const x = 1; $done({body: $response.body});").is_ok());
        assert!(validate_script_body("function handler() { $done({}); }").is_ok());
    }
}
