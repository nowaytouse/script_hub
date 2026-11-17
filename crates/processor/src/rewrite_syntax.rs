//! Shared, lossless syntax for whitespace-delimited rewrite fields.
//!
//! Upstream sources sometimes use bare JSON quotes inside regex fields. Those
//! quotes are data, whereas a complete quoted field can also contain spaces.
//! Published ordinary body rewrites use an unambiguous client-safe encoding.

use std::fmt;
pub mod http;
pub mod map_local;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RewriteError {
    LineBreak,
    UnterminatedQuote,
    WrongArity { expected: usize, actual: usize },
    InvalidPhase,
    UnsupportedSyntax,
    UnrepresentableField,
}

impl fmt::Display for RewriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LineBreak => "rewrite must be a single line without NUL",
            Self::UnterminatedQuote => "rewrite has an unterminated quoted field",
            Self::WrongArity { .. } => "rewrite has an incorrect number of fields",
            Self::InvalidPhase => "rewrite has an unsupported HTTP phase",
            Self::UnsupportedSyntax => {
                "conditional or structured body rewrite needs its own grammar"
            }
            Self::UnrepresentableField => {
                "rewrite field cannot be encoded losslessly for the client"
            }
        })
    }
}
impl std::error::Error for RewriteError {}

fn single_line(line: &str) -> Result<(), RewriteError> {
    if line.contains(['\r', '\n', '\0']) {
        Err(RewriteError::LineBreak)
    } else {
        Ok(())
    }
}
fn separator(ch: char) -> bool {
    matches!(ch, ' ' | '\t')
}

/// Read source fields. Only a complete field's enclosing quotes are syntax;
/// JSON quotes within an otherwise bare field remain literal regex data.
pub fn parse_source(line: &str) -> Result<Vec<String>, RewriteError> {
    Ok(source_tokens(line, usize::MAX)?
        .into_iter()
        .map(|(value, _)| value)
        .collect())
}

fn source_tokens(line: &str, limit: usize) -> Result<Vec<(String, usize)>, RewriteError> {
    single_line(line)?;
    let chars = line.chars().collect::<Vec<_>>();
    let mut fields = Vec::new();
    let mut i = 0;
    while i < chars.len() && fields.len() < limit {
        if separator(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        if matches!(chars[i], '"' | '\'') {
            let quote = chars[i];
            let mut end = i + 1;
            let mut value = String::new();
            while end < chars.len() {
                if quote == '"' && chars[end] == '\\' && chars.get(end + 1) == Some(&'"') {
                    value.push('"');
                    end += 2;
                    continue;
                }
                if chars[end] == quote {
                    if end + 1 == chars.len() || separator(chars[end + 1]) {
                        fields.push((value, end + 1));
                        i = end + 1;
                    }
                    break;
                }
                value.push(chars[end]);
                end += 1;
            }
            if end == chars.len() {
                return Err(RewriteError::UnterminatedQuote);
            }
            if i != start {
                continue;
            }
        }
        while i < chars.len() && !separator(chars[i]) {
            i += 1;
        }
        fields.push((chars[start..i].iter().collect(), i));
    }
    Ok(fields)
}

/// Target token contract: double quotes open a run anywhere, single quotes at
/// the start of a field or after `=`, and only `\"` is decoded inside doubles.
/// This models the pinned FlClash preview importer's token helper and detects
/// lossy encoding; it is not a claim that the native Core accepted a line.
pub fn parse_strict(line: &str) -> Result<Vec<String>, RewriteError> {
    single_line(line)?;
    let chars = line.chars().collect::<Vec<_>>();
    let mut fields = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if separator(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        let mut quote = None;
        let mut value = String::new();
        while i < chars.len() {
            let c = chars[i];
            if let Some(q) = quote {
                if q == '"' && c == '\\' && chars.get(i + 1) == Some(&'"') {
                    value.push('"');
                    i += 2;
                    continue;
                }
                if c == q {
                    quote = None;
                } else {
                    value.push(c);
                }
            } else if separator(c) {
                break;
            } else if c == '"' || c == '\'' && (i == start || chars[i - 1] == '=') {
                quote = Some(c);
            } else {
                value.push(c);
            }
            i += 1;
        }
        if quote.is_some() {
            return Err(RewriteError::UnterminatedQuote);
        }
        fields.push(value);
    }
    Ok(fields)
}

/// Quote without changing regex escapes, replacement capture references or
/// literal JSON quotes. Verify the actual target's token value before use.
pub fn quote_field(value: &str) -> Result<String, RewriteError> {
    single_line(value)?;
    let encoded = if !value.is_empty()
        && !value
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '"' | '\''))
    {
        value.to_owned()
    } else {
        format!("\"{}\"", value.replace('"', "\\\""))
    };
    if parse_strict(&encoded).is_ok_and(|fields| fields == [value]) {
        Ok(encoded)
    } else {
        Err(RewriteError::UnrepresentableField)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BodyRewrite {
    Regex {
        phase: String,
        pattern: String,
        search: String,
        replacement: String,
        extra_pairs: Vec<(String, String)>,
    },
    Jq {
        phase: String,
        pattern: String,
        expression: String,
    },
}

impl BodyRewrite {
    pub fn parse_source(line: &str) -> Result<Self, RewriteError> {
        let prefix = source_tokens(line, 2)?;
        if prefix.first().is_some_and(|(phase, _)| {
            matches!(phase.as_str(), "http-request-jq" | "http-response-jq")
        }) {
            let Some((pattern, end)) = prefix.get(1) else {
                return Err(RewriteError::WrongArity {
                    expected: 3,
                    actual: prefix.len(),
                });
            };
            let raw_expression = line.chars().skip(*end).collect::<String>();
            let raw_expression = raw_expression.trim();
            if pattern.is_empty() || raw_expression.is_empty() {
                return Err(RewriteError::UnsupportedSyntax);
            }
            let expression_prefix = source_tokens(raw_expression, 1)?;
            let expression = if expression_prefix[0].1 == raw_expression.chars().count() {
                expression_prefix[0].0.clone()
            } else {
                raw_expression.to_owned()
            };
            if expression.trim().is_empty() {
                return Err(RewriteError::UnsupportedSyntax);
            }
            return Ok(Self::Jq {
                phase: prefix[0].0.clone(),
                pattern: pattern.clone(),
                expression,
            });
        }
        Self::from_fields(parse_source(line)?)
    }
    pub fn parse_strict(line: &str) -> Result<Self, RewriteError> {
        Self::from_fields(parse_strict(line)?)
    }
    fn from_fields(fields: Vec<String>) -> Result<Self, RewriteError> {
        let phase = fields.first().ok_or(RewriteError::InvalidPhase)?;
        let jq = matches!(phase.as_str(), "http-request-jq" | "http-response-jq");
        if !jq && !matches!(phase.as_str(), "http-request" | "http-response") {
            return Err(RewriteError::InvalidPhase);
        }
        if fields.get(1).is_some_and(|field| field == "if")
            || fields.get(2).is_some_and(|field| field == "json")
        {
            return Err(RewriteError::UnsupportedSyntax);
        }
        let expected = if jq { 3 } else { 4 };
        if jq && fields.len() != 3
            || !jq && (fields.len() < 4 || !(fields.len() - 2).is_multiple_of(2))
        {
            return Err(RewriteError::WrongArity {
                expected,
                actual: fields.len(),
            });
        }
        if fields[1].is_empty()
            || fields[2].is_empty()
            || !jq && fields.iter().skip(4).step_by(2).any(String::is_empty)
        {
            return Err(RewriteError::UnsupportedSyntax);
        }
        Ok(if jq {
            Self::Jq {
                phase: fields[0].clone(),
                pattern: fields[1].clone(),
                expression: fields[2].clone(),
            }
        } else {
            Self::Regex {
                phase: fields[0].clone(),
                pattern: fields[1].clone(),
                search: fields[2].clone(),
                replacement: fields[3].clone(),
                extra_pairs: fields[4..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| (p[0].clone(), p[1].clone()))
                    .collect(),
            }
        })
    }
    pub fn pattern(&self) -> &str {
        match self {
            Self::Regex { pattern, .. } | Self::Jq { pattern, .. } => pattern,
        }
    }
    pub fn regexes(&self) -> impl Iterator<Item = &str> {
        let (first, rest): (Option<&str>, &[(String, String)]) = match self {
            Self::Regex {
                search,
                extra_pairs,
                ..
            } => (Some(search.as_str()), extra_pairs),
            Self::Jq { .. } => (None, &[]),
        };
        first
            .into_iter()
            .chain(rest.iter().map(|(search, _)| search.as_str()))
    }
    pub fn render(&self) -> Result<String, RewriteError> {
        let fields = match self {
            Self::Regex {
                phase,
                pattern,
                search,
                replacement,
                extra_pairs,
            } => {
                let mut fields = vec![phase, pattern, search, replacement];
                for (search, replacement) in extra_pairs {
                    fields.extend([search, replacement]);
                }
                fields
            }
            Self::Jq {
                phase,
                pattern,
                expression,
            } => vec![phase, pattern, expression],
        };
        let rendered = fields
            .into_iter()
            .map(|field| quote_field(field))
            .collect::<Result<Vec<_>, _>>()?
            .join(" ");
        if Self::parse_strict(&rendered)? != *self {
            return Err(RewriteError::UnrepresentableField);
        }
        Ok(rendered)
    }
}

/// Normalize the shared ordinary Body Rewrite grammar. jq is validated but its
/// source quoting is kept; URL and header entries use their own shared grammar.
pub fn normalize_section_line(section: &str, line: &str) -> Result<String, RewriteError> {
    if section == "Map Local" {
        return map_local::normalize(line);
    }
    if matches!(section, "URL Rewrite" | "Header Rewrite") {
        return http::normalize(section, line).map_err(|_| RewriteError::UnsupportedSyntax);
    }
    if section != "Body Rewrite" {
        return Ok(line.to_owned());
    }
    let parsed = BodyRewrite::parse_source(line)?;
    match parsed {
        BodyRewrite::Regex { .. } => parsed.render(),
        BodyRewrite::Jq { .. } => Ok(line.to_owned()),
    }
}

/// Normalize HTTP rewrite entries in any module publisher, retaining every
/// other section, header, blank line, comment and original line ending.
/// A malformed active body row fails the module instead of being dropped.
pub fn normalize_module(content: &str) -> Result<String, RewriteError> {
    let mut output = String::with_capacity(content.len());
    let mut section = String::new();
    for raw in content.split_inclusive('\n') {
        let (line, ending) = raw
            .strip_suffix("\r\n")
            .map(|s| (s, "\r\n"))
            .or_else(|| raw.strip_suffix('\n').map(|s| (s, "\n")))
            .unwrap_or((raw, ""));
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = name.to_owned();
            output.push_str(raw);
        } else if !matches!(
            section.as_str(),
            "Body Rewrite" | "Header Rewrite" | "URL Rewrite" | "Map Local"
        ) || trimmed.is_empty()
            || trimmed.starts_with(['#', ';'])
            || trimmed.starts_with("//")
        {
            output.push_str(raw);
        } else {
            output.push_str(&normalize_section_line(&section, trimmed)?);
            output.push_str(ending);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_json_quotes_are_literal_fields_and_publish_as_four_target_fields() {
        let line = r#"http-response ^https?:\/\/api\.example\.test\/cms offset_ms":\d+ offset_ms":99999999999999"#;
        let parsed = BodyRewrite::parse_source(line).unwrap();
        assert_eq!(parse_source(line).unwrap().len(), 4);
        assert_eq!(parse_strict(line).unwrap().len(), 3);
        let output = normalize_section_line("Body Rewrite", line).unwrap();
        assert!(output.contains(r#""offset_ms\":\d+""#));
        assert_eq!(BodyRewrite::parse_strict(&output).unwrap(), parsed);
        assert_eq!(
            normalize_section_line("Body Rewrite", &output).unwrap(),
            output
        );
    }

    #[test]
    fn wrapped_spaces_empty_replacement_and_capture_references_survive() {
        for line in [
            r#"http-response "^https://example\.test/a b" "\"ad\":\s*true" "\"ad\":false""#,
            r#"http-request ^https://example\.test (ad_)(\d+) "$1$2\\tail""#,
            r#"http-response ^https://example\.test "ad with space" """#,
            r#"http-response ^https://example\.test "ad":true "ad":false"#,
            r#"http-response ^https://example\.test "popTitle":".+?" {"status":"SUCCESS"}"#,
            r#"http-response ^https://example\.test "list":\[.+\] "list":[]"#,
            r#"http-response ^https://example\.test "imgUrl" "fmz200""#,
            r#"http-response ^https://example\.test ads 'say "hello"'"#,
        ] {
            let parsed = BodyRewrite::parse_source(line).unwrap();
            let rendered = parsed.render().unwrap();
            assert_eq!(BodyRewrite::parse_strict(&rendered).unwrap(), parsed);
            assert_eq!(BodyRewrite::parse_source(&rendered).unwrap(), parsed);
        }
        assert_eq!(
            parse_source(r#""popTitle":".+?" {"status":"SUCCESS"}"#).unwrap(),
            [r#""popTitle":".+?""#, r#"{"status":"SUCCESS"}"#]
        );
        assert_eq!(
            parse_source(r#""imgUrl" "fmz200""#).unwrap(),
            ["imgUrl", "fmz200"]
        );
    }

    #[test]
    fn incomplete_conditional_and_json_forms_are_not_claimed_as_regular_rewrites() {
        for line in [
            "http-response pattern search",
            "http-response pattern search replacement extra",
            "http-response if pattern then replace",
            "http-response pattern json del(.ad)",
            "http-response-jq pattern ''",
            "http-request pattern search replacement\nhttp-response second x y",
        ] {
            assert!(BodyRewrite::parse_source(line).is_err());
        }
        assert!(parse_strict("a \"unclosed").is_err());
        assert_eq!(
            parse_source("http-response pattern \"unclosed replacement"),
            Err(RewriteError::UnterminatedQuote)
        );
        assert!(quote_field("space ending in a slash\\").is_err());
    }

    #[test]
    fn jq_has_its_own_shape_and_original_quoting_is_retained() {
        let line = r#"http-response-jq ^https://example\.test 'del(.ad) | .data'"#;
        assert!(matches!(
            BodyRewrite::parse_source(line).unwrap(),
            BodyRewrite::Jq { .. }
        ));
        assert_eq!(normalize_section_line("Body Rewrite", line).unwrap(), line);
        assert_eq!(
            normalize_section_line("General", "unchanged input").unwrap(),
            "unchanged input"
        );
        let unwrapped = "http-response-jq ^https://example.test del(.ad) | .data";
        assert!(
            matches!(BodyRewrite::parse_source(unwrapped).unwrap(), BodyRewrite::Jq {expression,..} if expression == "del(.ad) | .data")
        );
        assert_eq!(
            normalize_section_line("Body Rewrite", unwrapped).unwrap(),
            unwrapped
        );
    }

    #[test]
    fn whole_module_keeps_context_and_refuses_invalid_active_body_rows() {
        let original = "#!name=Public fixture\r\n[General]\r\nkey = value\r\n[Body Rewrite]\r\n# keep comment\r\n\r\nhttp-response ^https://example.test offset_ms\":\\d+ offset_ms\":99\r\n[MITM]\r\nhostname = example.test\r\n";
        let output = normalize_module(original).unwrap();
        assert!(output.starts_with("#!name=Public fixture\r\n[General]\r\nkey = value\r\n[Body Rewrite]\r\n# keep comment\r\n\r\n"));
        assert!(output.ends_with("[MITM]\r\nhostname = example.test\r\n"));
        assert_eq!(normalize_module(&output).unwrap(), output);
        assert!(normalize_module("[Body Rewrite]\nhttp-response pattern search\n").is_err());
        let marker = "synthetic-secret-marker";
        let invalid = format!("[Body Rewrite]\nhttp-response pattern \"{marker}\n");
        assert!(
            !normalize_module(&invalid)
                .unwrap_err()
                .to_string()
                .contains(marker)
        );
    }

    #[test]
    fn all_pairs_keep_order_and_quoted_values_without_hiding_incomplete_pairs() {
        for line in [
            r#"http-response ^https://example.test a b c d"#,
            r#"http-request ^https://example.test "first value" "" offset_ms":\d+ offset_ms":9 "list":\[.+\] "list":[]"#,
        ] {
            let parsed = BodyRewrite::parse_source(line).unwrap();
            let output = parsed.render().unwrap();
            assert_eq!(BodyRewrite::parse_strict(&output).unwrap(), parsed);
            assert_eq!(BodyRewrite::parse_source(&output).unwrap(), parsed);
            assert!(parsed.regexes().count() >= 2);
        }
        for line in [
            "http-response pattern a b c",
            "http-response pattern a b c d e",
        ] {
            assert!(matches!(
                BodyRewrite::parse_source(line),
                Err(RewriteError::WrongArity { .. })
            ));
        }
    }

    /// Source regex semantics are checked independently of field-count checks.
    /// This is a synthetic Rust-regex oracle, not a native-client execution test.
    fn apply_synthetic_body(line: &str, input: &str) -> serde_json::Value {
        let source = BodyRewrite::parse_source(line).unwrap();
        let canonical = source.render().unwrap();
        let decoded = BodyRewrite::parse_strict(&canonical).unwrap();
        assert_eq!(decoded, source);
        let apply = |rewrite: &BodyRewrite| {
            let BodyRewrite::Regex {
                pattern,
                search,
                replacement,
                extra_pairs,
                ..
            } = rewrite
            else {
                panic!("Synthetic fixture must be an ordinary regex rewrite");
            };
            assert!(
                regex::Regex::new(pattern)
                    .unwrap()
                    .is_match("https://example.test/body")
            );
            let mut output = input.to_owned();
            for (search, replacement) in std::iter::once((search, replacement))
                .chain(extra_pairs.iter().map(|pair| (&pair.0, &pair.1)))
            {
                output = regex::Regex::new(search)
                    .unwrap()
                    .replace_all(&output, replacement.as_str())
                    .into_owned();
            }
            // A field quoting error must not silently produce malformed JSON.
            let json: serde_json::Value = serde_json::from_str(&output).unwrap();
            (output, json)
        };
        let (source_bytes, source_json) = apply(&source);
        let (canonical_bytes, canonical_json) = apply(&decoded);
        assert_eq!(source_bytes, canonical_bytes);
        assert_eq!(source_json, canonical_json);
        source_json
    }

    #[test]
    fn canonical_body_fields_preserve_synthetic_json_rewrite_results() {
        let cases = [
            (
                r#"http-response ^https://example\.test/body offset_ms":\d+ offset_ms":99999999999999"#,
                r#"{"offset_ms":125,"safe":"keep"}"#,
                serde_json::json!({"offset_ms":99_999_999_999_999_u64,"safe":"keep"}),
            ),
            (
                r#"http-response ^https://example\.test/body "list":\[.+\] "list":[]"#,
                r#"{"list":[1,2],"safe":true}"#,
                serde_json::json!({"list":[],"safe":true}),
            ),
            (
                r#"http-response ^https://example\.test/body \{"status":"FAILED"\} {"status":"SUCCESS"}"#,
                r#"{"status":"FAILED"}"#,
                serde_json::json!({"status":"SUCCESS"}),
            ),
            (
                r#"http-response ^https://example\.test/body "popTitle":".+?" "popTitle":"""#,
                r#"{"popTitle":"Sponsored","safe":1}"#,
                serde_json::json!({"popTitle":"","safe":1}),
            ),
        ];
        for (line, input, expected) in cases {
            assert_eq!(apply_synthetic_body(line, input), expected);
        }
    }

    #[test]
    fn canonical_multi_pair_rewrite_keeps_order_and_unrelated_json_fields() {
        let line = r#"http-response ^https://example\.test/body offset_ms":\d+ offset_ms":99 offset_ms":99 offset_ms":123 "list":\[.+\] "list":[] "popTitle":".+?" "popTitle":"" \{"status":"FAILED"\} {"status":"SUCCESS"}"#;
        let input = r#"{"offset_ms":7,"list":[1,2],"popTitle":"Sponsored","nested":{"status":"FAILED"},"safe":"keep"}"#;
        assert_eq!(
            apply_synthetic_body(line, input),
            serde_json::json!({"offset_ms":123,"list":[],"popTitle":"","nested":{"status":"SUCCESS"},"safe":"keep"})
        );
    }
}
