use super::rule::{RuleKind, SurgeRule, split_fields};
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
    "Panel",
    "SSID Setting",
    "Port Forwarding",
    "MITM",
];

pub fn validate_surge_module(content: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut section = None;
    let mut has_name = false;
    let mut seen_sections = HashSet::new();
    let mut seen_general_keys = HashSet::new();
    let mut seen_script_matchers = HashSet::new();
    let strict_promax = content.lines().any(|line| {
        line.trim().to_ascii_lowercase().starts_with("#!name")
            && line.to_ascii_lowercase().contains("promax")
    });

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
            if !SECTIONS.contains(&name)
                && !name.starts_with("Ruleset ")
                && !name.starts_with("WireGuard ")
            {
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
            Some("General") => {
                validate_general(line_number, line, &mut seen_general_keys, &mut errors)
            }
            Some("Rule") => validate_module_rule(line_number, line, strict_promax, &mut errors),
            Some("MITM") => validate_mitm(line_number, line, &mut errors),
            Some("Script") => {
                validate_surge_script(line_number, line, &mut errors);
                if let Some(key) = crate::smart_cleanup::script_matcher_key_pub(line)
                    && !seen_script_matchers.insert(key)
                {
                    errors.push(error(
                        line_number,
                        "shadowed-script-matcher",
                        "only the first HTTP script for a request phase and pattern can run",
                    ));
                }
            }
            Some("URL Rewrite" | "Body Rewrite" | "Header Rewrite") => {
                validate_regex_entry(section.unwrap(), line_number, line, &mut errors)
            }
            Some("Map Local") => validate_map_local(line_number, line, &mut errors),
            Some("Host") => validate_host(line_number, line, &mut errors),
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
    errors.extend(validate_transport_security(content));
    errors
}

/// Validate one functional line in the same context used by a Surge module.
///
/// The compiler calls this before accepting third-party functional entries so
/// malformed regexes and incomplete scripts can be quarantined without
/// invalidating the whole generated product.
pub fn validate_surge_section_line(section: &str, line: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    match section {
        "General" => {
            let mut seen = HashSet::new();
            validate_general(1, line, &mut seen, &mut errors);
        }
        "Rule" => validate_module_rule(1, line, false, &mut errors),
        "MITM" => validate_mitm(1, line, &mut errors),
        "Script" => validate_surge_script(1, line, &mut errors),
        "URL Rewrite" | "Body Rewrite" | "Header Rewrite" => {
            validate_regex_entry(section, 1, line, &mut errors);
        }
        "Map Local" => validate_map_local(1, line, &mut errors),
        "Host" => validate_host(1, line, &mut errors),
        _ => {}
    }
    errors.extend(validate_transport_security(line));
    errors
}

/// Reject settings and rewrites that weaken transport security by default.
pub fn validate_transport_security(content: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for (index, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') && !line.starts_with("#!") {
            continue;
        }
        let compact: String = line
            .chars()
            .filter(|character| !character.is_whitespace())
            .flat_map(char::to_lowercase)
            .collect();
        if compact.contains("skip-server-cert-verify=true") {
            errors.push(error(
                index + 1,
                "tls-verification-disabled",
                "certificate verification must not be disabled",
            ));
        }
        if compact.contains("isworkaroundsslpinning:true")
            || compact.contains("isworkaroundsslpinning=true")
        {
            errors.push(error(
                index + 1,
                "tls-pinning-workaround-default",
                "SSL-pinning workarounds must be opt-in instead of enabled by default",
            ));
        }

        let normalized = line.replace(r"\/", "/");
        let tokens: Vec<&str> = normalized.split_whitespace().collect();
        let strict_https_source = tokens
            .first()
            .map(|token| token.trim_matches(['\"', '\'']))
            .is_some_and(|token| token.starts_with("^https://"));
        if strict_https_source
            && tokens.iter().skip(1).any(|token| {
                token
                    .trim_matches(['\"', '\'', ','])
                    .to_ascii_lowercase()
                    .starts_with("http://")
            })
        {
            errors.push(error(
                index + 1,
                "tls-downgrade-rewrite",
                "HTTPS requests must not be redirected to plaintext HTTP",
            ));
        }
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

/// Reject project artifacts that can interfere with Apple Account, iCloud, or APNs.
/// `implicit_reject` is used for files published inside the AdBlock tree, where
/// rules intentionally omit their policy.
pub fn validate_apple_compatibility(content: &str, implicit_reject: bool) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut section = None;
    for (index, raw_line) in content.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = Some(&line[1..line.len() - 1]);
            continue;
        }

        if section == Some("MITM") {
            if let Some((key, value)) = line.split_once('=')
                && key.trim().eq_ignore_ascii_case("hostname")
            {
                for candidate in value.split(',').map(str::trim) {
                    let candidate = candidate
                        .strip_prefix("%APPEND%")
                        .or_else(|| candidate.strip_prefix("%INSERT%"))
                        .unwrap_or(candidate)
                        .trim();
                    if !candidate.starts_with('-')
                        && crate::promax::safety::apple_host_pattern_is_critical(candidate)
                    {
                        errors.push(error(
                            line_number,
                            "apple-critical-mitm",
                            format!("Apple critical host must be excluded from MITM: {candidate}"),
                        ));
                    }
                }
            }
            continue;
        }

        if section == Some("Host") {
            if let Some((host, resolver)) = line.split_once('=')
                && crate::promax::safety::apple_host_pattern_is_critical(host)
                && !resolver.trim().eq_ignore_ascii_case("server:system")
            {
                errors.push(error(
                    line_number,
                    "apple-critical-host-mapping",
                    "Apple critical hosts must use server:system instead of fixed or third-party DNS",
                ));
            }
            continue;
        }

        if section != Some("Rule") && !implicit_reject {
            continue;
        }
        let cleaned = crate::strip_inline_comment(line);
        let cleaned = cleaned.trim();
        let upper = cleaned.to_ascii_uppercase();
        if upper.starts_with("DEST-PORT,5223,") && upper.contains("REJECT") {
            errors.push(error(
                line_number,
                "apple-apns-port-block",
                "TCP 5223 is required by Apple Push Notification Service",
            ));
            continue;
        }

        let Ok(rule) = SurgeRule::parse(cleaned) else {
            continue;
        };
        let targets_critical = match rule.kind {
            RuleKind::Domain
            | RuleKind::DomainSuffix
            | RuleKind::DomainWildcard
            | RuleKind::DomainRegex => {
                crate::promax::safety::apple_host_pattern_is_critical(&rule.payload)
            }
            RuleKind::DomainKeyword => crate::promax::safety::APPLE_CRITICAL_EXACT
                .iter()
                .chain(crate::promax::safety::APPLE_CRITICAL_SUFFIXES.iter())
                .any(|host| host.contains(&rule.payload.to_ascii_lowercase())),
            _ => false,
        };
        let is_direct = rule.policy.as_deref() == Some("DIRECT");
        if targets_critical && (implicit_reject || !is_direct) {
            errors.push(error(
                line_number,
                "apple-critical-nondirect",
                format!(
                    "Apple critical rule must be DIRECT and outside ad-block output: {cleaned}"
                ),
            ));
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
            Some("Rule") => validate_loon_rule(line_number, line, &mut errors),
            Some("MitM") => validate_mitm(line_number, line, &mut errors),
            Some("Rewrite") => validate_loon_rewrite(line_number, line, &mut errors),
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
    errors.extend(validate_transport_security(content));
    errors
}

fn validate_loon_rule(line_number: usize, line: &str, errors: &mut Vec<ValidationError>) {
    if line.to_ascii_uppercase().starts_with("RULE-SET,") {
        errors.push(error(
            line_number,
            "unsupported-loon-rule-set",
            "Loon plugins must use inline rules instead of Surge RULE-SET references",
        ));
        return;
    }
    match SurgeRule::parse(line) {
        Ok(rule) => {
            if rule.policy.as_deref() != Some("REJECT") {
                errors.push(error(
                    line_number,
                    "unsupported-policy",
                    "Loon PROMAX rules must use the intrinsic REJECT policy",
                ));
            }
            if rule.options.iter().any(|option| option != "NO-RESOLVE") {
                errors.push(error(
                    line_number,
                    "invalid-rule-option",
                    "unsupported Loon inline rule option",
                ));
            }
        }
        Err(parse_error) => {
            errors.push(error(line_number, "invalid-rule", parse_error.to_string()))
        }
    }
}

fn validate_generic_module_rule(line: &str) -> bool {
    let Ok(fields) = split_fields(line) else {
        return false;
    };
    if fields.len() < 3 || fields[2].trim().is_empty() {
        return false;
    }
    let payload = if fields[1].contains(',') {
        format!("\"{}\"", fields[1].replace('"', "\\\""))
    } else {
        fields[1].clone()
    };
    SurgeRule::parse(&format!("{},{}", fields[0], payload)).is_ok()
}

fn validate_loon_rewrite(line_number: usize, line: &str, errors: &mut Vec<ValidationError>) {
    let mut fields = line.split_whitespace();
    let Some(pattern) = fields.next() else {
        return;
    };
    if let Err(regex_error) = Regex::new(pattern) {
        errors.push(error(
            line_number,
            "invalid-section-regex",
            regex_error.to_string(),
        ));
        return;
    }
    let remaining: Vec<&str> = fields.collect();
    let is_valid_action = |action: &str| {
        matches!(
            action,
            "reject"
                | "reject-200"
                | "reject-img"
                | "reject-dict"
                | "reject-array"
                | "302"
                | "307"
                | "header"
                | "header-add"
                | "header-del"
                | "header-replace"
                | "header-replace-regex"
                | "request-body-replace-regex"
                | "request-body-json-add"
                | "request-body-json-replace"
                | "request-body-json-del"
                | "request-body-json-jq"
                | "response-body-replace-regex"
                | "response-body-json-add"
                | "response-body-json-replace"
                | "response-body-json-del"
                | "response-body-json-jq"
                | "mock-request-body"
                | "mock-response-body"
        )
    };
    let first = remaining
        .first()
        .copied()
        .unwrap_or("")
        .to_ascii_lowercase();
    let second = remaining.get(1).copied().unwrap_or("").to_ascii_lowercase();
    let (action_index, action) = if is_valid_action(&first) {
        (0, first)
    } else if is_valid_action(&second) {
        (1, second)
    } else {
        (0, first)
    };
    if !is_valid_action(&action) {
        errors.push(error(
            line_number,
            "invalid-loon-rewrite-action",
            format!("unsupported Loon rewrite action: {action}"),
        ));
        return;
    }
    if action == "mock-response-body" {
        let parameters = remaining
            .iter()
            .skip(action_index + 1)
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        let has_data = parameters.contains("data=") || parameters.contains("data-path=");
        if !parameters.contains("data-type=") || !has_data {
            errors.push(error(
                line_number,
                "invalid-loon-mock-response",
                "mock-response-body requires data-type and data/data-path",
            ));
        }
    }
}

fn validate_module_rule(
    line_number: usize,
    line: &str,
    strict_promax: bool,
    errors: &mut Vec<ValidationError>,
) {
    let cleaned = crate::strip_inline_comment(line);
    let line = cleaned.trim();
    if line.to_ascii_uppercase().starts_with("RULE-SET,") {
        validate_rule_set(line_number, line, strict_promax, errors);
        return;
    }
    if ["AND,", "OR,", "NOT,"]
        .iter()
        .any(|prefix| line.to_ascii_uppercase().starts_with(prefix))
    {
        let balanced = line.chars().filter(|character| *character == '(').count()
            == line.chars().filter(|character| *character == ')').count();
        let policy = line.rsplit_once(',').map(|(_, policy)| policy.trim());
        if !balanced || policy.is_none_or(str::is_empty) {
            errors.push(error(
                line_number,
                "invalid-logical-rule",
                "logical rule is unbalanced or missing a policy",
            ));
        } else if strict_promax && !is_internal_reject_policy(policy.unwrap()) {
            errors.push(error(
                line_number,
                "unsupported-policy",
                "PROMAX logical rules require an internal policy",
            ));
        }
        return;
    }
    match SurgeRule::parse(line) {
        Ok(rule) => {
            match rule.policy.as_deref() {
                Some(policy) if strict_promax && !is_internal_reject_policy(policy) => {
                    errors.push(error(
                        line_number,
                        "unsupported-policy",
                        "PROMAX rules require an internal policy",
                    ))
                }
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
        Err(_) if !strict_promax && validate_generic_module_rule(line) => {}
        Err(parse_error) => {
            errors.push(error(line_number, "invalid-rule", parse_error.to_string()))
        }
    }
}

fn validate_rule_set(
    line_number: usize,
    line: &str,
    strict_promax: bool,
    errors: &mut Vec<ValidationError>,
) {
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
    if strict_promax && !is_internal_reject_policy(&policy) {
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
            .is_some_and(|value| value > 0 && value <= 31_536_000);
        if !valid {
            errors.push(error(
                line_number,
                "invalid-rule-option",
                format!("unsupported RULE-SET option: {option}"),
            ));
        }
    }
}

fn is_internal_reject_policy(policy: &str) -> bool {
    matches!(
        policy.to_ascii_uppercase().as_str(),
        "REJECT" | "REJECT-DROP" | "REJECT-NO-DROP" | "REJECT-TINYGIF" | "REJECT-IMG"
    )
}

fn validate_general(
    line_number: usize,
    line: &str,
    seen_keys: &mut HashSet<String>,
    errors: &mut Vec<ValidationError>,
) {
    let Some((key, value)) = line.split_once('=') else {
        errors.push(error(
            line_number,
            "invalid-general-entry",
            "General entry must use key = value syntax",
        ));
        return;
    };
    let key = key.trim().to_ascii_lowercase();
    if !seen_keys.insert(key.clone()) {
        errors.push(error(
            line_number,
            "duplicate-general-key",
            format!("duplicate General key: {key}"),
        ));
    }
    if key == "force-http-engine-hosts"
        && value.split(',').map(str::trim).any(|host| {
            host.strip_prefix("%APPEND%")
                .or_else(|| host.strip_prefix("%INSERT%"))
                .unwrap_or(host)
                .trim()
                .starts_with("*:")
        })
    {
        errors.push(error(
            line_number,
            "global-http-engine-port",
            "force-http-engine-hosts must not intercept every host on a port",
        ));
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
    let mut saw_candidate = false;
    let mut saw_inclusion = false;
    for candidate in value.split(',').map(str::trim) {
        let candidate = candidate
            .strip_prefix("%APPEND%")
            .or_else(|| candidate.strip_prefix("%INSERT%"))
            .unwrap_or(candidate)
            .trim();
        if candidate.is_empty() {
            continue;
        }
        saw_candidate = true;
        if candidate.starts_with('-') {
            if saw_inclusion {
                errors.push(error(
                    line_number,
                    "mitm-exclusion-order",
                    "MITM exclusions must precede inclusions because Host List order is significant",
                ));
            }
        } else {
            saw_inclusion = true;
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
    if !saw_candidate {
        errors.push(error(
            line_number,
            "empty-mitm-host-list",
            "MITM hostname entry must contain at least one host or exclusion",
        ));
    }
}

fn validate_host(line_number: usize, line: &str, errors: &mut Vec<ValidationError>) {
    let Some((host, resolver)) = line.split_once('=') else {
        errors.push(error(
            line_number,
            "invalid-host-mapping",
            "Host entry must use host = address syntax",
        ));
        return;
    };
    let host = host.trim();
    if host.is_empty()
        || resolver.trim().is_empty()
        || host.contains("://")
        || host.contains('/')
        || host.chars().any(char::is_whitespace)
    {
        errors.push(error(
            line_number,
            "invalid-host-mapping",
            "Host entry contains an invalid host or empty resolver",
        ));
    }
}

fn validate_map_local(line_number: usize, line: &str, errors: &mut Vec<ValidationError>) {
    validate_regex_entry("Map Local", line_number, line, errors);
    let data_type = line
        .split_whitespace()
        .find_map(|field| field.strip_prefix("data-type="))
        .unwrap_or("file")
        .trim_matches(['\'', '"'])
        .to_ascii_lowercase();
    if !matches!(data_type.as_str(), "file" | "text" | "tiny-gif" | "base64") {
        errors.push(error(
            line_number,
            "invalid-map-local-data-type",
            format!("unsupported Map Local data-type: {data_type}"),
        ));
    }
    if data_type != "tiny-gif"
        && !line
            .split_whitespace()
            .any(|field| field.starts_with("data="))
    {
        errors.push(error(
            line_number,
            "missing-map-local-data",
            "Map Local requires data unless data-type=tiny-gif",
        ));
    }
    if let Some(status) = line
        .split_whitespace()
        .find_map(|field| field.strip_prefix("status-code="))
        && !status
            .trim_matches(['\'', '"'])
            .parse::<u16>()
            .is_ok_and(|status| (200..=999).contains(&status))
    {
        errors.push(error(
            line_number,
            "invalid-map-local-status",
            "Map Local status-code must be between 200 and 999",
        ));
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
            if let Some(pattern) = pattern
                && let Err(regex_error) = Regex::new(pattern)
            {
                errors.push(error(
                    line_number,
                    "invalid-script-regex",
                    regex_error.to_string(),
                ));
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
    use super::{
        validate_apple_compatibility, validate_loon_plugin, validate_surge_module,
        validate_surge_section_line, validate_transport_security,
    };

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
    fn rejects_empty_mitm_hostname_entries() {
        let fixture = "#!name=empty MITM\n[MITM]\nhostname = %APPEND%\n";
        let codes: Vec<&str> = validate_surge_module(fixture)
            .iter()
            .map(|error| error.code)
            .collect();
        assert_eq!(codes, vec!["empty-mitm-host-list"]);
    }

    #[test]
    fn rejects_duplicate_general_keys_and_global_http_engine_ports() {
        let fixture = "#!name=noisy\n[General]\nforce-http-engine-hosts = %APPEND% example.com\nforce-http-engine-hosts = %APPEND% *:8082\n";
        let codes: Vec<&str> = validate_surge_module(fixture)
            .iter()
            .map(|error| error.code)
            .collect();
        assert_eq!(
            codes,
            vec!["duplicate-general-key", "global-http-engine-port"]
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
    fn rejects_shadowed_scripts_and_late_mitm_exclusions() {
        let fixture = r#"#!name=PROMAX fixture
[Script]
first = type=http-response,pattern=^https://ads.example,script-path=a.js
second = type=http-response,pattern=^https://ads.example,script-path=b.js
[MITM]
hostname = %APPEND% *.example.com, -account.apple.com
"#;
        let codes: Vec<&str> = validate_surge_module(fixture)
            .iter()
            .map(|error| error.code)
            .collect();

        assert_eq!(
            codes,
            vec!["shadowed-script-matcher", "mitm-exclusion-order"]
        );
    }

    #[test]
    fn validates_map_local_payload_and_status() {
        let fixture = r#"#!name=PROMAX fixture
[Map Local]
^https://ads\.example data-type=text status-code=199
^https://pixel\.example data-type=tiny-gif status-code=200
"#;
        let codes: Vec<&str> = validate_surge_module(fixture)
            .iter()
            .map(|error| error.code)
            .collect();

        assert_eq!(
            codes,
            vec!["missing-map-local-data", "invalid-map-local-status"]
        );
    }

    #[test]
    fn validates_individual_functional_lines_before_merge() {
        assert_eq!(
            validate_surge_section_line(
                "URL Rewrite",
                r"^https?:\/\/res\.kfc\.com.\cn\/advertisement\/ _ reject",
            )[0]
            .code,
            "invalid-section-regex"
        );
        assert_eq!(
            validate_surge_section_line(
                "Script",
                "bad=type=http-response,pattern=^https://example.com/ad",
            )[0]
            .code,
            "invalid-script"
        );
        assert!(
            validate_surge_section_line("URL Rewrite", r"^https?:\/\/ads\.example\/ - reject",)
                .is_empty()
        );
    }

    #[test]
    fn accepts_representative_surge_regex_dialect_fixture() {
        let fixture = include_str!("../../tests/fixtures/promax_regex_dialect.sgmodule");
        assert_eq!(validate_surge_module(fixture), Vec::new());
    }

    #[test]
    fn loon_validator_rejects_surge_rule_sets_and_untyped_map_local() {
        let fixture = r#"#!name=PROMAX fixture
[Rule]
RULE-SET,https://example.test/ad.list,REJECT,no-resolve
[Rewrite]
^https://ads\.example data-type=text data="{}"
"#;
        let errors = validate_loon_plugin(fixture);
        let codes: Vec<&str> = errors.iter().map(|error| error.code).collect();
        assert_eq!(
            codes,
            vec!["unsupported-loon-rule-set", "invalid-loon-rewrite-action"]
        );
    }

    #[test]
    fn loon_validator_accepts_inline_rules_and_typed_mock_response() {
        let fixture = r#"#!name=PROMAX fixture
[Rule]
DOMAIN-SUFFIX,ads.example,REJECT
[Rewrite]
^https://ads\.example mock-response-body data-type=text data="{}" status-code=200
"#;
        assert_eq!(validate_loon_plugin(fixture), Vec::new());
    }

    #[test]
    fn loon_validator_accepts_redirect_target_before_status_code() {
        let fixture = r#"#!name=PROMAX fixture
[Rewrite]
^https://ads\.example/ https://example.com/blank 302
^https://tracker\.example/ https://example.com/blank 307
^https://mirror\.example/(.*) https://origin.example/$1 header
"#;
        assert_eq!(validate_loon_plugin(fixture), Vec::new());
    }

    #[test]
    fn apple_compatibility_rejects_auth_mitm_dns_and_apns_blocks() {
        let fixture = r#"#!name=unsafe
[Rule]
DOMAIN,account.apple.com,REJECT-DROP
DEST-PORT,5223,REJECT
[Host]
*.icloud.com = 1.2.3.4
[MITM]
hostname = %APPEND% *.icloud.com, -idmsa.apple.com
"#;
        let codes: Vec<&str> = validate_apple_compatibility(fixture, false)
            .iter()
            .map(|error| error.code)
            .collect();
        assert_eq!(
            codes,
            vec![
                "apple-critical-nondirect",
                "apple-apns-port-block",
                "apple-critical-host-mapping",
                "apple-critical-mitm",
            ]
        );
    }

    #[test]
    fn apple_compatibility_accepts_direct_system_dns_and_mitm_exclusions() {
        let fixture = r#"#!name=safe
[Rule]
DOMAIN,account.apple.com,DIRECT
DOMAIN-SUFFIX,icloud.com,DIRECT
[Host]
account.apple.com = server:system
*.icloud.com = server:system
[MITM]
hostname = %INSERT% -account.apple.com, -*.icloud.com, weatherkit.apple.com
"#;
        assert_eq!(validate_apple_compatibility(fixture, false), Vec::new());
    }

    #[test]
    fn transport_security_rejects_unsafe_defaults_and_https_downgrades() {
        let fixture = r#"#!arguments=isWorkaroundSSLPinning:true
skip-server-cert-verify = true
^https:\/\/link\.example\/\?target=(.*) http://$1 302
^https?:\/\/legacy\.example\/(.*) http://$1 302
"#;
        let actual: Vec<(usize, &str)> = validate_transport_security(fixture)
            .iter()
            .map(|error| (error.line, error.code))
            .collect();

        assert_eq!(
            actual,
            vec![
                (1, "tls-pinning-workaround-default"),
                (2, "tls-verification-disabled"),
                (3, "tls-downgrade-rewrite"),
            ]
        );
    }
}
