use std::collections::BTreeSet;

const MLXX_RESOURCE: &str =
    "https://raw.githubusercontent.com/fmz200/wool_scripts/main/Scripts/mlxx/mlxx.js";
const GOOFISH_RESOURCE: &str =
    "https://raw.githubusercontent.com/ishowshu/qx/main/script/goofish.js";

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

/// Rewrite only known script resource parameter values, preserving all other text.
pub fn normalize_script_line(line: &str) -> String {
    let Some((start, end)) = script_path_range(line) else {
        return line.to_string();
    };
    let Some(replacement) = replacement_for(&line[start..end]) else {
        return line.to_string();
    };
    format!("{}{}{}", &line[..start], replacement, &line[end..])
}

/// List only the supported resources referenced from script-path parameters.
pub fn required_resources(content: &str) -> BTreeSet<&'static str> {
    let mut resources = BTreeSet::new();
    for line in content.lines() {
        if let Some(url) = script_resource_url(line) {
            if let Some(replacement) = replacement_for(url) {
                resources.insert(replacement);
            } else if url.eq_ignore_ascii_case(MLXX_RESOURCE) {
                resources.insert(MLXX_RESOURCE);
            } else if url.eq_ignore_ascii_case(GOOFISH_RESOURCE) {
                resources.insert(GOOFISH_RESOURCE);
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
        GOOFISH_RESOURCE, MLXX_RESOURCE, normalize_script_line, replacement_for,
        required_resources, script_resource_url, validate_script_body,
    };
    use std::collections::BTreeSet;

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
        let line = r#"mlxx = type=http-response,pattern="^https://example.test/(foo,bar)",script-path=https://raw.githubusercontent.com/Yu9191/Rewrite/refs/heads/main/mlxx.js?old=secret,argument="https://raw.githubusercontent.com/Yu9191/Rewrite/main/mlxx.js""#;
        let normalized = normalize_script_line(line);
        assert!(normalized.contains(MLXX_RESOURCE));
        assert!(normalized.contains(r#"pattern="^https://example.test/(foo,bar)""#));
        assert!(normalized.contains(
            r#"argument="https://raw.githubusercontent.com/Yu9191/Rewrite/main/mlxx.js""#
        ));
        assert!(!normalized.contains("old=secret"));
        assert_eq!(
            normalize_script_line(
                "# script-path=https://klraw.pages.dev/kelv1n1n/script/goofish.js?token=secret"
            ),
            "# script-path=https://klraw.pages.dev/kelv1n1n/script/goofish.js?token=secret"
        );
    }

    #[test]
    fn resource_discovery_reads_only_script_path_values_and_normalized_families() {
        let content = r#"
one = type=http-response,script-path=https://raw.githubusercontent.com/Yu9191/Rewrite/main/mlxx.js
two = type=http-response,script-path=https://klraw.pages.dev/kelv1n1n/script/goofish.js?token=secret
three = type=generic,argument="https://raw.githubusercontent.com/Yu9191/Rewrite/main/mlxx.js"
"#;
        assert_eq!(
            required_resources(content),
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
