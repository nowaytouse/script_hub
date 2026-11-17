//! Shared Surge-style URL and header rewrite grammar.
//!
//! Official shape: manual.nssurge.com/http/{url-rewrite,header-rewrite}.html.
//! The known reject response variants also occur in current public inputs and
//! the pinned Preview importer. This is syntax validation, not live Core acceptance.

use super::{RewriteError, parse_source, parse_strict, quote_field};
use fancy_regex::Regex;

fn lexical(error: RewriteError) -> String {
    error.to_string()
}

fn arity(fields: &[String], expected: usize) -> Result<(), String> {
    if fields.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "rewrite requires {expected} fields, got {}",
            fields.len()
        ))
    }
}

fn regex(value: &str, label: &str) -> Result<(), String> {
    Regex::new(value)
        .map(|_| ())
        .map_err(|_| format!("invalid {label} regular expression"))
}

fn reject(action: &str) -> bool {
    matches!(
        action,
        "reject" | "reject-img" | "reject-dict" | "reject-array" | "reject-200"
    )
}

fn request_pattern(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err("rewrite requires a nonempty request pattern".to_owned());
    }
    regex(value, "request URL")
}

fn header_fields(mut fields: Vec<String>) -> Result<Vec<String>, String> {
    let first = fields.first().ok_or("header rewrite is empty")?;
    if !matches!(first.as_str(), "http-request" | "http-response") {
        if first.starts_with("http-") {
            return Err(RewriteError::InvalidPhase.to_string());
        }
        // Surge's documented legacy form omits the HTTP direction.
        fields.insert(0, "http-request".to_owned());
    }
    let action = fields
        .get(2)
        .ok_or("header rewrite requires a pattern and action")?;
    let count = match action.as_str() {
        "header-del" => 4,
        "header-add" | "header-replace" => 5,
        "header-replace-regex" => 6,
        _ => return Err("unsupported header rewrite action".to_owned()),
    };
    arity(&fields, count)?;
    request_pattern(&fields[1])?;
    let name = &fields[3];
    // This is a header name, not a regex over names. Prevent separators and
    // controls from turning a quoted value into an invalid header field.
    if name.is_empty()
        || name
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == ':')
    {
        return Err("invalid header field name".to_owned());
    }
    if action == "header-replace-regex" {
        regex(&fields[4], "header search")?;
    }
    Ok(fields)
}

fn url_fields(mut fields: Vec<String>, explicit_default: bool) -> Result<Vec<String>, String> {
    if !matches!(fields.len(), 2 | 3) {
        return Err(
            "URL rewrite requires pattern and replacement/type, optionally an explicit type"
                .to_owned(),
        );
    }
    request_pattern(&fields[0])?;
    if fields.len() == 2 {
        if reject(&fields[1]) {
            return Ok(fields);
        }
        // A two-field non-rejection is the documented default header mode;
        // it must not be rejected as an unknown action or missing status.
        if fields[1].is_empty() {
            return Err("URL rewrite replacement is empty".to_owned());
        }
        if explicit_default {
            fields.push("header".to_owned());
        }
    } else {
        let action = fields[2].as_str();
        if !reject(action) && !matches!(action, "header" | "302" | "307") {
            return Err("unsupported URL rewrite type (shared grammar: header, 302, 307 or known reject type)".to_owned());
        }
        // Reject ignores its placeholder. Retain it for legacy Surge output.
        if !reject(action) && fields[1].is_empty() {
            return Err("URL rewrite replacement is empty".to_owned());
        }
    }
    Ok(fields)
}

fn checked_fields(
    section: &str,
    fields: Vec<String>,
    explicit_default: bool,
) -> Result<Vec<String>, String> {
    match section {
        "Header Rewrite" => header_fields(fields),
        "URL Rewrite" => url_fields(fields, explicit_default),
        _ => Err("unsupported HTTP rewrite section".to_owned()),
    }
}

/// Canonicalize supported public source syntax without altering field values.
/// Implicit request direction and default URL mode become explicit.
pub fn normalize(section: &str, line: &str) -> Result<String, String> {
    let fields = checked_fields(section, parse_source(line).map_err(lexical)?, true)?;
    let output = fields
        .iter()
        .map(|field| quote_field(field).map_err(lexical))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    if parse_strict(&output).map_err(lexical)? != fields {
        return Err(RewriteError::UnrepresentableField.to_string());
    }
    validate(section, &output)?;
    Ok(output)
}

/// Check published syntax independently from serialization. Ambiguous quote
/// handling is rejected rather than accepted with changed regex/value text.
pub fn validate(section: &str, line: &str) -> Result<(), String> {
    let source = parse_source(line).map_err(lexical)?;
    let target = parse_strict(line).map_err(lexical)?;
    if source != target {
        return Err("rewrite quoting changes a field's literal value".to_owned());
    }
    checked_fields(section, target, false).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_redirect_captures_spaces_and_default_mode_round_trip() {
        for line in [
            r#"^https://example\.test/(.*) https://next.example.test/$1 302"#,
            r#""^https://example\.test/a b/(.*)" "https://next.example.test/$1?q=a b" 307"#,
            r#"^https://example\.test/(.*) https://next.example.test/$1"#,
            r#"^https://example\.test/(.*) "$1?$2" header"#,
        ] {
            let output = normalize("URL Rewrite", line).unwrap();
            validate("URL Rewrite", &output).unwrap();
            assert_eq!(normalize("URL Rewrite", &output).unwrap(), output);
            assert!(output.contains("$1"));
        }
        assert!(
            normalize(
                "URL Rewrite",
                "^https://example.test https://next.example.test"
            )
            .unwrap()
            .ends_with(" header")
        );
        for line in [
            "^https://example.test reject",
            "^https://example.test _ reject",
            "^https://example.test - reject-dict",
            "^https://example.test reject-img",
        ] {
            assert_eq!(normalize("URL Rewrite", line).unwrap(), line);
        }
    }

    #[test]
    fn header_actions_default_phase_and_literal_quote_values_round_trip() {
        for line in [
            r#"^https://example\.test header-del Cookie"#,
            r#"http-response ^https://example\.test header-add X-Value {"ad":false}"#,
            r#"http-request ^https://example\.test header-replace User-Agent "A Browser/1.0""#,
            r#"http-response ^https://example\.test header-replace-regex X-Value "(ad) with space" "$1 \"quoted\"""#,
            r#"http-request ^https://example\.test header-add X-Empty """#,
        ] {
            let output = normalize("Header Rewrite", line).unwrap();
            validate("Header Rewrite", &output).unwrap();
            assert_eq!(normalize("Header Rewrite", &output).unwrap(), output);
            let mut source = parse_source(line).unwrap();
            if !source[0].starts_with("http-") {
                source.insert(0, "http-request".into());
            }
            assert_eq!(parse_strict(&output).unwrap(), source);
        }
    }

    #[test]
    fn malformed_http_rewrites_fail_instead_of_being_passed_through() {
        for line in [
            "",
            "^https://example.test",
            "^https://example.test target 999",
            "^https://example.test target 302 extra",
            "[unterminated target 302",
            "^https://example.test \"\" 307",
        ] {
            assert!(normalize("URL Rewrite", line).is_err(), "{line}");
        }
        for line in [
            "http-other ^https://example.test header-del X",
            "http-response ^https://example.test header-del X extra",
            "http-request ^https://example.test header-add X",
            "http-request ^https://example.test unknown X value",
            "http-request [bad header-del X",
            "http-response ^https://example.test header-replace-regex X [bad value",
            "http-request ^https://example.test header-add \"Bad Header\" value",
        ] {
            assert!(normalize("Header Rewrite", line).is_err(), "{line}");
        }
        assert!(
            validate(
                "Header Rewrite",
                r#"http-request ^https://example.test header-add X {"ad":false}"#
            )
            .is_err()
        );
        assert!(
            validate(
                "URL Rewrite",
                r#"^https://example.test https://next.test/?q="ad" header"#
            )
            .is_err()
        );
    }
}
