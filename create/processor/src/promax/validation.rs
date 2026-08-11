use super::rule::{SurgeRule, split_fields};
use fancy_regex::Regex;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub line: usize,
    pub code: &'static str,
    pub message: String,
}

const SECTIONS: &[&str] = &[
    "General",
    "Host",
    "Rule",
    "URL Rewrite",
    "Map Local",
    "Script",
    "Body Rewrite",
    "Header Rewrite",
    "MITM",
];

pub fn validate_surge_module(content: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut section = None;
    let mut has_name = false;
    let mut seen_sections = HashSet::new();

    for (index, raw_line) in content.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') && !line.starts_with("#!") {
            continue;
        }
        if line.starts_with("#!") {
            has_name |= line.split_once(['=', ':']).is_some_and(|(key, value)| {
                key.eq_ignore_ascii_case("#!name") && !value.trim().is_empty()
            });
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let name = &line[1..line.len() - 1];
            if !SECTIONS.contains(&name) {
                errors.push(error(
                    line_number,
                    "unsupported-section",
                    format!("unsupported section [{name}]"),
                ));
                section = None;
                continue;
            }
            if !seen_sections.insert(name.to_string()) {
                errors.push(error(
                    line_number,
                    "duplicate-section",
                    format!("duplicate section [{name}]"),
                ));
            }
            section = Some(name);
            continue;
        }

        match section {
            Some("Rule") => validate_module_rule(line_number, line, &mut errors),
            Some("MITM") => validate_mitm(line_number, line, &mut errors),
            Some("Script") => {
                validate_surge_script(line_number, line, &mut errors);
            }
            Some("URL Rewrite" | "Map Local" | "Body Rewrite" | "Header Rewrite") => {
                validate_regex_entry(section.unwrap(), line_number, line, &mut errors)
            }
            Some(_) => {}
            None => errors.push(error(
                line_number,
                "line-outside-section",
                "content appears outside a module section",
            )),
        }
    }

    if !has_name {
        errors.insert(
            0,
            error(1, "missing-name", "module metadata is missing #!name"),
        );
    }
    errors
}

pub fn validate_external_ruleset(content: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for (index, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        match SurgeRule::parse(line) {
            Ok(rule) if rule.policy.is_some() => errors.push(error(
                index + 1,
                "external-policy",
                "external rulesets must not contain a policy",
            )),
            Ok(rule) => {
                for option in &rule.options {
                    if !matches!(option.as_str(), "NO-RESOLVE" | "EXTENDED-MATCHING") {
                        errors.push(error(
                            index + 1,
                            "invalid-rule-option",
                            format!("option {option} is not valid inside an external ruleset"),
                        ));
                    }
                }
            }
            Err(parse_error) => {
                errors.push(error(index + 1, "invalid-rule", parse_error.to_string()))
            }
        }
    }
    errors
}

pub fn validate_loon_plugin(content: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut section = None;
    let mut has_name = false;
    for (index, raw_line) in content.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') && !line.starts_with("#!") {
            continue;
        }
        if line.starts_with("#!") {
            has_name |= line.split_once(['=', ':']).is_some_and(|(key, value)| {
                key.eq_ignore_ascii_case("#!name") && !value.trim().is_empty()
            });
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let name = &line[1..line.len() - 1];
            if !["Rule", "Rewrite", "Script", "MitM"].contains(&name) {
                errors.push(error(
                    line_number,
                    "unsupported-section",
                    format!("unsupported Loon section [{name}]"),
                ));
                section = None;
            } else {
                section = Some(name);
            }
            continue;
        }
        match section {
            Some("Rule") => validate_module_rule(line_number, line, &mut errors),
            Some("MitM") => validate_mitm(line_number, line, &mut errors),
            Some("Rewrite") => validate_regex_entry("Rewrite", line_number, line, &mut errors),
            Some("Script") => validate_loon_script(line_number, line, &mut errors),
            Some(_) => {}
            None => errors.push(error(
                line_number,
                "line-outside-section",
                "content appears outside a Loon section",
            )),
        }
    }
    if !has_name {
        errors.insert(
            0,
            error(1, "missing-name", "plugin metadata is missing #!name"),
        );
    }
    errors
}

fn validate_module_rule(line_number: usize, line: &str, errors: &mut Vec<ValidationError>) {
    if line.to_ascii_uppercase().starts_with("RULE-SET,") {
        validate_rule_set(line_number, line, errors);
        return;
    }
    match SurgeRule::parse(line) {
        Ok(rule) => {
            match rule.policy.as_deref() {
                Some("PROXY") => errors.push(error(
                    line_number,
                    "unsupported-policy",
                    "PROMAX rules may not route traffic through PROXY",
                )),
                Some(_) => {}
                None => errors.push(error(
                    line_number,
                    "missing-policy",
                    "module rule is missing a policy",
                )),
            }
            for option in &rule.options {
                if matches!(option.as_str(), "PRE-MATCHING")
                    || option.starts_with("UPDATE-INTERVAL=")
                {
                    errors.push(error(
                        line_number,
                        "invalid-rule-option",
                        format!("option {option} belongs on RULE-SET, not an inline rule"),
                    ));
                }
            }
        }
        Err(parse_error) => {
            errors.push(error(line_number, "invalid-rule", parse_error.to_string()))
        }
    }
}

fn validate_rule_set(line_number: usize, line: &str, errors: &mut Vec<ValidationError>) {
    let fields = match split_fields(line) {
        Ok(fields) => fields,
        Err(parse_error) => {
            errors.push(error(line_number, "invalid-rule", parse_error.to_string()));
            return;
        }
    };
    if fields.len() < 3 || fields[1].trim().is_empty() {
        errors.push(error(
            line_number,
            "invalid-rule-set",
            "RULE-SET requires a URL and policy",
        ));
        return;
    }
    let policy = fields[2].to_ascii_uppercase();
    if ![
        "DIRECT",
        "REJECT",
        "REJECT-DROP",
        "REJECT-NO-DROP",
        "REJECT-TINYGIF",
        "REJECT-IMG",
    ]
    .contains(&policy.as_str())
    {
        errors.push(error(
            line_number,
            "unsupported-policy",
            format!("unsupported PROMAX RULE-SET policy: {}", fields[2]),
        ));
    }
    for option in &fields[3..] {
        let option = option.to_ascii_uppercase();
        let valid = matches!(
            option.as_str(),
            "NO-RESOLVE" | "EXTENDED-MATCHING" | "PRE-MATCHING"
        ) || option
            .strip_prefix("UPDATE-INTERVAL=")
            .and_then(|value| value.parse::<u32>().ok())
            .is_some_and(|value| value > 0);
        if !valid {
            errors.push(error(
                line_number,
                "invalid-rule-option",
                format!("unsupported RULE-SET option: {option}"),
            ));
        }
    }
}

fn validate_mitm(line_number: usize, line: &str, errors: &mut Vec<ValidationError>) {
    let Some((key, value)) = line.split_once('=') else {
        errors.push(error(
            line_number,
            "invalid-mitm",
            "MITM entry must use key = value syntax",
        ));
        return;
    };
    if !key.trim().eq_ignore_ascii_case("hostname") {
        errors.push(error(line_number, "invalid-mitm", "unsupported MITM key"));
        return;
    }
    for candidate in value.split(',').map(str::trim) {
        let candidate = candidate
            .strip_prefix("%APPEND%")
            .or_else(|| candidate.strip_prefix("%INSERT%"))
            .unwrap_or(candidate)
            .trim();
        if candidate.is_empty() {
            continue;
        }
        if candidate.contains("://")
            || candidate.contains('/')
            || candidate.chars().any(char::is_whitespace)
        {
            errors.push(error(
                line_number,
                "invalid-mitm-host",
                format!("invalid MITM hostname: {candidate}"),
            ));
        }
    }
}

fn validate_regex_entry(
    section: &str,
    line_number: usize,
    line: &str,
    errors: &mut Vec<ValidationError>,
) {
    let mut fields = line.split_whitespace();
    let first = fields.next().unwrap_or("");
    let pattern = if section == "Body Rewrite" && first.starts_with("http-") {
        fields.next()
    } else {
        Some(first)
    };
    let Some(pattern) = pattern.filter(|pattern| !pattern.is_empty()) else {
        errors.push(error(
            line_number,
            "invalid-section-regex",
            "missing request pattern",
        ));
        return;
    };
    if let Err(regex_error) = Regex::new(pattern) {
        errors.push(error(
            line_number,
            "invalid-section-regex",
            regex_error.to_string(),
        ));
    }
}

fn validate_surge_script(line_number: usize, line: &str, errors: &mut Vec<ValidationError>) {
    let Some((label, content)) = line.split_once('=') else {
        errors.push(error(
            line_number,
            "invalid-script",
            "script entry is missing a label",
        ));
        return;
    };
    if label.trim().is_empty() {
        errors.push(error(
            line_number,
            "invalid-script",
            "script label is empty",
        ));
        return;
    }
    let params = parse_parameters(content);
    let script_type = parameter(&params, "type");
    let script_path = parameter(&params, "script-path");
    let valid = match script_type {
        Some("http-request" | "http-response") => {
            let pattern = parameter(&params, "pattern");
            if let Some(pattern) = pattern {
                if let Err(regex_error) = Regex::new(pattern) {
                    errors.push(error(
                        line_number,
                        "invalid-script-regex",
                        regex_error.to_string(),
                    ));
                }
            }
            pattern.is_some() && script_path.is_some()
        }
        Some("cron") => {
            script_path.is_some()
                && ["cronexp", "cron-expression", "cron"]
                    .iter()
                    .any(|key| parameter(&params, key).is_some())
        }
        Some("event" | "generic" | "network-changed") => script_path.is_some(),
        _ => false,
    };
    if !valid {
        errors.push(error(
            line_number,
            "invalid-script",
            "script entry is missing a supported type, pattern/cron, or script-path",
        ));
    }
}

fn validate_loon_script(line_number: usize, line: &str, errors: &mut Vec<ValidationError>) {
    let mut fields = line.split_whitespace();
    let script_type = fields.next().unwrap_or("");
    let valid_type = matches!(
        script_type,
        "http-request" | "http-response" | "cron" | "network-changed" | "generic"
    );
    let has_path = line.to_ascii_lowercase().contains("script-path=");
    if !valid_type || !has_path {
        errors.push(error(
            line_number,
            "invalid-script",
            "invalid Loon script entry",
        ));
        return;
    }
    if matches!(script_type, "http-request" | "http-response") {
        let Some(pattern) = fields.next() else {
            errors.push(error(
                line_number,
                "invalid-script",
                "Loon HTTP script is missing a pattern",
            ));
            return;
        };
        if let Err(regex_error) = Regex::new(pattern) {
            errors.push(error(
                line_number,
                "invalid-script-regex",
                regex_error.to_string(),
            ));
        }
    }
}

fn parse_parameters(content: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut depths = [0_u32; 3];
    for character in content.chars() {
        if let Some(delimiter) = quote {
            current.push(character);
            if character == delimiter && !escaped {
                quote = None;
            }
            escaped = character == '\\' && !escaped;
            if character != '\\' {
                escaped = false;
            }
            continue;
        }
        match character {
            '\'' | '"' => {
                quote = Some(character);
                current.push(character);
            }
            '{' => {
                depths[0] += 1;
                current.push(character);
            }
            '}' => {
                depths[0] = depths[0].saturating_sub(1);
                current.push(character);
            }
            '[' => {
                depths[1] += 1;
                current.push(character);
            }
            ']' => {
                depths[1] = depths[1].saturating_sub(1);
                current.push(character);
            }
            '(' => {
                depths[2] += 1;
                current.push(character);
            }
            ')' => {
                depths[2] = depths[2].saturating_sub(1);
                current.push(character);
            }
            ',' if depths == [0, 0, 0] => {
                fields.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(character),
        }
    }
    fields.push(current.trim().to_string());
    fields
}

fn parameter<'a>(params: &'a [String], name: &str) -> Option<&'a str> {
    params
        .iter()
        .find_map(|field| {
            let (key, value) = field.split_once('=')?;
            key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
        })
        .filter(|value| !value.is_empty())
}

fn error(line: usize, code: &'static str, message: impl Into<String>) -> ValidationError {
    ValidationError {
        line,
        code,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_surge_module;

    #[test]
    fn reports_structural_errors_with_exact_source_lines() {
        let fixture = r#"#!name=PROMAX fixture
[Rule]
URL-REGEX,"^https://example.com/(a{1,3}|b)",REJECT
URL-REGEX,""^https://bad.example/d",REJECT",REJECT
DOMAIN-SUFFIX,ads.example,PROXY
[MITM]
hostname = good.example, https://bad.example
"#;

        let actual: Vec<(usize, &str)> = validate_surge_module(fixture)
            .iter()
            .map(|error| (error.line, error.code))
            .collect();

        assert_eq!(
            actual,
            vec![
                (4, "invalid-rule"),
                (5, "unsupported-policy"),
                (7, "invalid-mitm-host"),
            ]
        );
    }

    #[test]
    fn rejects_unknown_rule_set_options() {
        let fixture = "#!name=PROMAX fixture\n[Rule]\nRULE-SET,https://example.test/ad.list,REJECT,magic-option\n";
        let errors = validate_surge_module(fixture);

        assert_eq!(errors.len(), 1);
        assert_eq!((errors[0].line, errors[0].code), (3, "invalid-rule-option"));
    }

    #[test]
    fn validates_real_body_rewrite_pattern_and_surge_regex_dialect() {
        let fixture = r#"#!name=PROMAX fixture
[Body Rewrite]
http-response-jq ^https:\/\/(?<=api\.)example\.com\/recommend\/(?!v2) 'del(.data.ad)'
http-response-jq [unterminated 'del(.data.ad)'
"#;
        let errors = validate_surge_module(fixture);

        assert_eq!(errors.len(), 1);
        assert_eq!(
            (errors[0].line, errors[0].code),
            (4, "invalid-section-regex")
        );
    }

    #[test]
    fn rejects_incomplete_http_script_entries() {
        let fixture = r#"#!name=PROMAX fixture
[Script]
bad = type=http-response,pattern=^https://example.com/ad
"#;
        let errors = validate_surge_module(fixture);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "invalid-script");
    }

    #[test]
    fn accepts_representative_surge_regex_dialect_fixture() {
        let fixture = include_str!("../../tests/fixtures/promax_regex_dialect.sgmodule");
        assert_eq!(validate_surge_module(fixture), Vec::new());
    }
}
