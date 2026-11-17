//! Lossless named fields for native Map Local responses, including JSON data.
use super::{RewriteError, parse_strict, quote_field, source_tokens};

pub fn fields(line: &str) -> Result<Vec<String>, RewriteError> {
    let prefix = source_tokens(line, 1)?;
    let (pattern, end) = prefix.first().ok_or(RewriteError::UnsupportedSyntax)?;
    let mut fields = vec![pattern.clone()];
    let rest = line.chars().skip(*end).collect::<String>();
    let chars: Vec<char> = rest.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && super::separator(chars[i]) {
            i += 1;
        }
        if i == chars.len() {
            break;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '-') {
            i += 1;
        }
        if i == start || chars.get(i) != Some(&'=') {
            return Err(RewriteError::UnsupportedSyntax);
        }
        let key: String = chars[start..i].iter().collect();
        i += 1;
        let mut value = String::new();
        if matches!(chars.get(i), Some('"' | '\'')) {
            let quote = chars[i];
            i += 1;
            let mut closed = false;
            while i < chars.len() {
                if quote == '"' && chars[i] == '\\' && chars.get(i + 1) == Some(&'"') {
                    value.push('"');
                    i += 2;
                    continue;
                }
                if chars[i] == quote && (i + 1 == chars.len() || super::separator(chars[i + 1])) {
                    i += 1;
                    closed = true;
                    break;
                }
                // Legacy source JSON can contain literal quotes inside its
                // outer parameter quotes. Keep them instead of stripping JSON.
                value.push(chars[i]);
                i += 1;
            }
            if !closed {
                return Err(RewriteError::UnterminatedQuote);
            }
        } else {
            while i < chars.len() && !super::separator(chars[i]) {
                value.push(chars[i]);
                i += 1;
            }
        }
        if fields
            .iter()
            .skip(1)
            .any(|field| field.starts_with(&format!("{key}=")))
        {
            return Err(RewriteError::UnsupportedSyntax);
        }
        fields.push(format!("{key}={value}"));
    }
    Ok(fields)
}

pub fn normalize(line: &str) -> Result<String, RewriteError> {
    let fields = fields(line)?;
    // Preserve already lossless source formatting and minimize generated diffs.
    if parse_strict(line).as_ref() == Ok(&fields) {
        return Ok(line.to_owned());
    }
    let mut output = quote_field(&fields[0])?;
    for field in fields.iter().skip(1) {
        let (key, value) = field
            .split_once('=')
            .ok_or(RewriteError::UnsupportedSyntax)?;
        output.push_str(&format!(" {key}={}", quote_field(value)?));
    }
    if parse_strict(&output).as_ref() != Ok(&fields)
        || self::fields(&output).as_ref() != Ok(&fields)
    {
        return Err(RewriteError::UnrepresentableField);
    }
    Ok(output)
}

pub fn validate(line: &str) -> Result<(), RewriteError> {
    if parse_strict(line)? != fields(line)? {
        return Err(RewriteError::UnrepresentableField);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_responses_preserve_json_quotes_values_and_regex_escapes() {
        for line in [
            r#"^https://example\.test data-type=text data="{"status":"SUCCESS"}" header="Content-Type:application/json""#,
            r#"^https://example\.test data-type=text data='{"name":"with space","safe":true}' status-code=200"#,
            r#"^https://example\.test data-type=text data="{\"safe\":true}""#,
        ] {
            let fixed = normalize(line).unwrap();
            validate(&fixed).unwrap();
            assert_eq!(normalize(&fixed).unwrap(), fixed);
            let parsed = parse_strict(&fixed).unwrap();
            assert_eq!(parsed[0], r"^https://example\.test");
            let data = parsed
                .iter()
                .find_map(|field| field.strip_prefix("data="))
                .unwrap();
            assert!(serde_json::from_str::<serde_json::Value>(data).is_ok());
        }
        assert!(validate(r#"^https://example.test data-type=text data="{"safe":true}""#).is_err());
    }

    #[test]
    fn malformed_or_duplicate_named_fields_fail_closed() {
        for line in [
            "",
            "^https://example.test data=one data=two",
            "^https://example.test data=\"unterminated",
            "^https://example.test missing",
        ] {
            assert!(normalize(line).is_err(), "{line}");
        }
    }
}
