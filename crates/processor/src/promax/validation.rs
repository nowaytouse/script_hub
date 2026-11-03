use super::rule::{RuleKind, SurgeRule, split_fields};
use fancy_regex::Regex;
use std::borrow::Cow;
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
    "IP Rewrite",
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
    validate_module(content, true)
}

/// Validate a Shadowrocket module while retaining shared PROMAX safety checks.
///
/// Both clients support no-resolve; unsupported Surge sections remain dialect-specific.
pub fn validate_shadowrocket_module(content: &str) -> Vec<ValidationError> {
    validate_module(content, false)
}

fn validate_module(content: &str, surge_dialect: bool) -> Vec<ValidationError> {
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
    let ip_rewrite_core = content.lines().any(|line| {
        line.trim()
            .strip_prefix("#!requirement=CORE_VERSION>=")
            .and_then(|version| version.parse::<u32>().ok())
            .is_some_and(|version| version >= 6_010_000)
    });
    let mut ip_fallbacks = HashSet::new();
    if strict_promax {
        let mut in_rule = false;
        for line in content.lines().map(str::trim) {
            if line.starts_with('[') && line.ends_with(']') {
                in_rule = line == "[Rule]";
            }
            if !in_rule || !(line.starts_with("IP-CIDR,") || line.starts_with("IP-CIDR6,")) {
                continue;
            }
            if let Ok(rule) = SurgeRule::parse(line)
                && rule.options.iter().any(|option| option == "NO-RESOLVE")
                && rule
                    .options
                    .iter()
                    .all(|option| matches!(option.as_str(), "NO-RESOLVE" | "PRE-MATCHING"))
                && let Some(target) = rule.ip_rewrite_target()
            {
                let action = match rule.policy.as_deref() {
                    Some("REJECT" | "REJECT-NO-DROP") => "reject",
                    Some("REJECT-DROP") => "drop",
                    _ => continue,
                };
                ip_fallbacks.insert((target, action));
            }
        }
    }

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
            if name == "IP Rewrite" && !surge_dialect
                || !SECTIONS.contains(&name)
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
            Some("Rule") => validate_module_rule(
                line_number,
                line,
                strict_promax,
                true,
                surge_dialect,
                &mut errors,
            ),
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
            Some("IP Rewrite") => validate_ip_rewrite(
                line_number,
                line,
                strict_promax,
                ip_rewrite_core,
                &ip_fallbacks,
                &mut errors,
            ),
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
    errors.extend(validate_apple_compatibility(content, false));
    errors
}

fn validate_ip_rewrite(
    line_number: usize,
    line: &str,
    strict_promax: bool,
    core_supported: bool,
    fallbacks: &HashSet<(String, &'static str)>,
    errors: &mut Vec<ValidationError>,
) {
    let Some((address, action)) = line.split_once('=') else {
        errors.push(error(
            line_number,
            "invalid-ip-rewrite",
            "IP Rewrite requires IP/CIDR = action",
        ));
        return;
    };
    let address = address.trim();
    let action = action.trim();
    let kind = if address.contains(':') {
        "IP-CIDR6"
    } else {
        "IP-CIDR"
    };
    let valid_address = if address.contains('/') {
        SurgeRule::parse(&format!("{kind},{address}")).is_ok()
    } else {
        address.parse::<std::net::IpAddr>().is_ok()
    };
    if !valid_address || !matches!(action, "drop" | "reject" | "reflect") {
        errors.push(error(
            line_number,
            "invalid-ip-rewrite",
            "invalid IP/CIDR or unsupported IP Rewrite action",
        ));
        return;
    }
    if !strict_promax {
        return;
    }
    if !matches!(action, "reject" | "drop") {
        errors.push(error(
            line_number,
            "unsupported-promax-ip-rewrite",
            "PROMAX IP Rewrite only compiles reject/drop actions",
        ));
    }
    if !core_supported {
        errors.push(error(
            line_number,
            "ip-rewrite-core-requirement",
            "IP Rewrite requires CORE_VERSION>=6010000 (iOS 5.23 / macOS 6.10)",
        ));
    }
    let payload = if address.contains('/') {
        address.to_string()
    } else {
        format!("{address}/{}", if kind == "IP-CIDR6" { 128 } else { 32 })
    };
    let target = SurgeRule::parse(&format!("{kind},{payload}"))
        .ok()
        .and_then(|rule| rule.ip_rewrite_target());
    if !target.is_some_and(|target| fallbacks.contains(&(target, action))) {
        errors.push(error(line_number, "ip-rewrite-missing-proxy-fallback", "VIF-only IP Rewrite requires an equivalent no-resolve rule with the same action in [Rule]"));
    }
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
        "Rule" => validate_module_rule(1, line, false, false, true, &mut errors),
        "MITM" => validate_mitm(1, line, &mut errors),
        "Script" => validate_surge_script(1, line, &mut errors),
        "URL Rewrite" | "Body Rewrite" | "Header Rewrite" => {
            validate_regex_entry(section, 1, line, &mut errors);
        }
        "Map Local" => validate_map_local(1, line, &mut errors),
        "Host" => validate_host(1, line, &mut errors),
        "IP Rewrite" => validate_ip_rewrite(1, line, false, false, &HashSet::new(), &mut errors),
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

pub(crate) fn external_rule_line(raw: &str) -> Option<&str> {
    let line = raw.trim();
    (!line.is_empty() && !line.starts_with(['#', ';'])).then_some(line)
}

pub fn validate_external_ruleset(content: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for (index, raw_line) in content.lines().enumerate() {
        let Some(line) = external_rule_line(raw_line) else {
            continue;
        };
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
        let is_direct = rule.policy.as_deref() == Some("DIRECT");
        if (implicit_reject || !is_direct) && targets_apple_critical_domain(&rule) {
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

fn targets_apple_critical_domain(rule: &SurgeRule) -> bool {
    use crate::promax::safety::{
        APPLE_CRITICAL_EXACT, APPLE_CRITICAL_SUFFIXES, apple_host_pattern_is_critical,
    };
    match rule.kind {
        RuleKind::Domain
        | RuleKind::DomainSuffix
        | RuleKind::DomainWildcard
        | RuleKind::DomainRegex => apple_host_pattern_is_critical(&rule.payload),
        RuleKind::DomainKeyword => {
            let keyword = rule.payload.to_ascii_lowercase();
            APPLE_CRITICAL_EXACT
                .iter()
                .chain(APPLE_CRITICAL_SUFFIXES.iter())
                .any(|host| host.contains(&keyword))
        }
        _ => false,
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

type LogicalError = (&'static str, &'static str);
const LOGICAL_SYNTAX: LogicalError = (
    "invalid-logical-rule",
    "logical rules require a parenthesized list of policy-free sub-rules",
);
const MAX_LOGICAL_DEPTH: usize = 10;

fn is_logical_operator(kind: &str) -> bool {
    ["AND", "OR", "NOT"]
        .iter()
        .any(|operator| kind.eq_ignore_ascii_case(operator))
}

fn take_logical_group(input: &str) -> Option<(&str, &str)> {
    let input = input.trim_start().strip_prefix('(')?;
    let mut depth = 1;
    let mut quote = None;
    let mut escaped = false;
    let mut field_start = true;
    for (offset, byte) in input.bytes().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            field_start = false;
            continue;
        }
        if let Some(delimiter) = quote {
            if byte == delimiter {
                quote = None;
            }
            continue;
        }
        match byte {
            b'\'' | b'"' if field_start => {
                quote = Some(byte);
                field_start = false;
            }
            b'(' => {
                depth += 1;
                field_start = true;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&input[..offset], &input[offset + 1..]));
                }
                field_start = false;
            }
            b',' => field_start = true,
            byte if byte.is_ascii_whitespace() => {}
            _ => field_start = false,
        }
    }
    None
}

// Surge accepts single quotes around a matcher; adapt those to the typed parser.
fn logical_leaf_text(line: &str) -> Cow<'_, str> {
    let Some((kind, value)) = line.split_once(',') else {
        return Cow::Borrowed(line);
    };
    let Some(quoted) = value.trim_start().strip_prefix('\'') else {
        return Cow::Borrowed(line);
    };
    let mut normalized = format!("{kind},\"");
    let mut escaped = false;
    for (index, character) in quoted.char_indices() {
        if !escaped {
            match character {
                '\'' => {
                    normalized.push('"');
                    normalized.push_str(&quoted[index + 1..]);
                    return Cow::Owned(normalized);
                }
                '"' => normalized.push('\\'),
                _ => {}
            }
        }
        normalized.push(character);
        escaped = character == '\\' && !escaped;
    }
    Cow::Borrowed(line)
}

fn validate_logical_leaf(
    line: &str,
    promax_dns_guard: bool,
    protect_apple: bool,
) -> Result<bool, LogicalError> {
    let line = logical_leaf_text(line);
    let fields = split_fields(&line).map_err(|_| LOGICAL_SYNTAX)?;
    let [kind, value, options @ ..] = fields.as_slice() else {
        return Err(LOGICAL_SYNTAX);
    };
    if value.is_empty() {
        return Err(LOGICAL_SYNTAX);
    }
    let kind = kind.to_ascii_uppercase();
    let reference = matches!(kind.as_str(), "RULE-SET" | "DOMAIN-SET");
    for option in options {
        let upper = option.to_ascii_uppercase();
        let allowed = match upper.as_str() {
            "NO-RESOLVE" | "EXTENDED-MATCHING" => true,
            "REQUIRES-RESOLVE" => kind == "SCRIPT",
            _ => upper
                .split_once('=')
                .is_some_and(|(option, value)| match option {
                    "UPDATE-INTERVAL" => {
                        reference && value.parse::<i64>().is_ok_and(|interval| interval != 0)
                    }
                    "NOTIFICATION-TEXT" | "NOTIFICATION-INTERVAL" | "ALWAYS-CAPTURE" => {
                        !value.is_empty()
                    }
                    _ => false,
                }),
        };
        if !allowed {
            return Err(LOGICAL_SYNTAX);
        }
    }

    // Validate the matcher with the same typed parser as inline rules. Extra
    // documented Surge matchers stay local to this module validator.
    let matcher = if options.is_empty() {
        line
    } else if value.contains([',', '"']) {
        Cow::Owned(format!("{kind},\"{value}\""))
    } else {
        Cow::Owned(format!("{kind},{value}"))
    };
    let (can_pre_match, needs_no_resolve) = match SurgeRule::parse(&matcher) {
        Ok(rule) if protect_apple && targets_apple_critical_domain(&rule) => {
            return Err((
                "apple-critical-nondirect",
                "Apple critical domains must not be targeted by a non-DIRECT logical rule",
            ));
        }
        Ok(rule) => (
            rule.kind.supports_pre_matching(),
            matches!(
                rule.kind,
                RuleKind::IpCidr | RuleKind::IpCidr6 | RuleKind::IpAsn | RuleKind::GeoIp
            ),
        ),
        Err(_) => match kind.as_str() {
            "RULE-SET" | "DOMAIN-SET" => (true, true),
            "SRC-IP" | "SUBNET" | "CELLULAR-RADIO" | "CELLULAR-CARRIER" => (true, false),
            "DEVICE-NAME" | "MAC-ADDRESS" | "PROTOCOL" | "HOSTNAME-TYPE" | "SCRIPT" => {
                (false, false)
            }
            _ => return Err(LOGICAL_SYNTAX),
        },
    };
    if promax_dns_guard && needs_no_resolve && !options.iter().any(|option| option == "no-resolve")
    {
        return Err((
            "ip-rule-missing-no-resolve",
            "PROMAX logical IP and ruleset sub-rules require canonical no-resolve",
        ));
    }
    if promax_dns_guard && reference && (kind != "RULE-SET" || !is_compiled_adblock_ruleset(value))
    {
        return Err((
            "unfiltered-promax-ruleset",
            "PROMAX logical rulesets must reference safety-filtered AdBlock output",
        ));
    }
    Ok(can_pre_match)
}

fn validate_logical_children(
    operator: &str,
    mut children: &str,
    depth: usize,
    promax_dns_guard: bool,
    non_direct: bool,
    negated: bool,
) -> Result<bool, LogicalError> {
    if depth > MAX_LOGICAL_DEPTH {
        return Err((
            "invalid-logical-rule",
            "logical rule nesting exceeds ten levels",
        ));
    }
    let negated = negated ^ operator.eq_ignore_ascii_case("NOT");
    let mut can_pre_match = true;
    loop {
        let (child, tail) = take_logical_group(children).ok_or(LOGICAL_SYNTAX)?;
        let child = child.trim();
        let (kind, value) = child.split_once(',').ok_or(LOGICAL_SYNTAX)?;
        let kind = kind.trim();
        can_pre_match &= if is_logical_operator(kind) {
            let (nested, tail) = take_logical_group(value).ok_or(LOGICAL_SYNTAX)?;
            if !tail.trim().is_empty() {
                return Err(LOGICAL_SYNTAX);
            }
            validate_logical_children(
                kind,
                nested,
                depth + 1,
                promax_dns_guard,
                non_direct,
                negated,
            )?
        } else {
            validate_logical_leaf(child, promax_dns_guard, non_direct && !negated)?
        };
        let tail = tail.trim();
        if tail.is_empty() {
            return Ok(can_pre_match);
        }
        if operator.eq_ignore_ascii_case("NOT") {
            return Err(("invalid-logical-rule", "NOT requires exactly one sub-rule"));
        }
        children = tail.strip_prefix(',').ok_or(LOGICAL_SYNTAX)?;
    }
}

fn validate_logical_rule(
    operator: &str,
    value: &str,
    strict_promax: bool,
    require_no_resolve: bool,
    surge_dialect: bool,
) -> Result<(), LogicalError> {
    let (children, tail) = take_logical_group(value).ok_or(LOGICAL_SYNTAX)?;
    let tail = crate::strip_inline_comment(tail);
    let tail = tail.strip_prefix(',').ok_or(LOGICAL_SYNTAX)?;
    let fields = split_fields(tail).map_err(|_| LOGICAL_SYNTAX)?;
    let [policy, options @ ..] = fields.as_slice() else {
        return Err(LOGICAL_SYNTAX);
    };
    if policy.is_empty() || super::rule::is_option(&policy.to_ascii_uppercase()) {
        return Err(("invalid-logical-rule", "logical rule is missing a policy"));
    }
    let rejects = is_internal_reject_policy(policy);
    if strict_promax && !rejects {
        return Err((
            "unsupported-policy",
            "PROMAX logical rules require an internal policy",
        ));
    }
    let can_pre_match = validate_logical_children(
        operator,
        children,
        1,
        strict_promax && require_no_resolve,
        !policy.eq_ignore_ascii_case("DIRECT"),
        false,
    )?;
    if surge_dialect
        && options
            .iter()
            .any(|option| option.eq_ignore_ascii_case("pre-matching"))
        && (!rejects || !can_pre_match)
    {
        return Err((
            "invalid-rule-option",
            "pre-matching requires a REJECT policy and supported logical sub-rules",
        ));
    }
    Ok(())
}

fn validate_module_rule(
    line_number: usize,
    line: &str,
    strict_promax: bool,
    require_no_resolve: bool,
    surge_dialect: bool,
    errors: &mut Vec<ValidationError>,
) {
    if let Some((kind, value)) = line.split_once(',')
        && is_logical_operator(kind.trim())
    {
        if let Err((code, message)) = validate_logical_rule(
            kind.trim(),
            value,
            strict_promax,
            require_no_resolve,
            surge_dialect,
        ) {
            errors.push(error(line_number, code, message));
        }
        return;
    }
    let cleaned = crate::strip_inline_comment(line);
    let line = cleaned.trim();
    if line.to_ascii_uppercase().starts_with("RULE-SET,") {
        validate_rule_set(
            line_number,
            line,
            strict_promax,
            require_no_resolve,
            surge_dialect,
            errors,
        );
        return;
    }
    match SurgeRule::parse(line) {
        Ok(rule) => {
            if strict_promax
                && require_no_resolve
                && matches!(
                    rule.kind,
                    RuleKind::IpCidr | RuleKind::IpCidr6 | RuleKind::IpAsn | RuleKind::GeoIp
                )
                && !split_fields(line)
                    .is_ok_and(|fields| fields.iter().skip(2).any(|field| field == "no-resolve"))
            {
                errors.push(error(
                    line_number,
                    "ip-rule-missing-no-resolve",
                    "PROMAX inline IP rules require the canonical lowercase no-resolve option",
                ));
            }
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
                if option == "PRE-MATCHING"
                    && (!rule.kind.supports_pre_matching()
                        || !rule
                            .policy
                            .as_deref()
                            .is_some_and(is_internal_reject_policy))
                {
                    errors.push(error(
                        line_number,
                        "invalid-rule-option",
                        "pre-matching requires a supported inline rule type and a REJECT policy",
                    ));
                }
                if option.starts_with("UPDATE-INTERVAL=") {
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
    require_no_resolve: bool,
    surge_dialect: bool,
    errors: &mut Vec<ValidationError>,
) {
    let fields = match split_fields(line) {
        Ok(fields) => fields,
        Err(parse_error) => {
            errors.push(error(line_number, "invalid-rule", parse_error.to_string()));
            return;
        }
    };
    if fields.len() < 3 || fields[1].trim().is_empty() || fields[2].trim().is_empty() {
        errors.push(error(
            line_number,
            "invalid-rule-set",
            "RULE-SET requires a URL and policy",
        ));
        return;
    }
    if strict_promax
        && require_no_resolve
        && !fields[3..].iter().any(|option| option == "no-resolve")
    {
        errors.push(error(
            line_number,
            "ruleset-missing-no-resolve",
            "PROMAX RULE-SET references require the canonical lowercase no-resolve option",
        ));
    }
    let policy = fields[2].to_ascii_uppercase();
    if strict_promax && !is_compiled_adblock_ruleset(&fields[1]) {
        errors.push(error(
            line_number,
            "unfiltered-promax-ruleset",
            "PROMAX RULE-SET must reference this repository's safety-filtered AdBlock output",
        ));
    }
    if strict_promax && !is_internal_reject_policy(&policy) {
        errors.push(error(
            line_number,
            "unsupported-policy",
            format!("unsupported PROMAX RULE-SET policy: {}", fields[2]),
        ));
    }
    for option in &fields[3..] {
        let option = option.to_ascii_uppercase();
        if surge_dialect && option == "PRE-MATCHING" && !is_internal_reject_policy(&policy) {
            errors.push(error(
                line_number,
                "invalid-rule-option",
                "pre-matching requires a REJECT policy",
            ));
        }
        let valid = matches!(
            option.as_str(),
            "NO-RESOLVE" | "EXTENDED-MATCHING" | "PRE-MATCHING"
        ) || option
            .strip_prefix("UPDATE-INTERVAL=")
            .and_then(|value| value.parse::<i64>().ok())
            .is_some_and(|value| {
                if surge_dialect {
                    value != 0
                } else {
                    value > 0 && value <= 31_536_000
                }
            });
        if !valid {
            errors.push(error(
                line_number,
                "invalid-rule-option",
                format!("unsupported RULE-SET option: {option}"),
            ));
        }
    }
}

fn is_compiled_adblock_ruleset(url: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/",
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/",
    ];
    PREFIXES.iter().any(|prefix| {
        url.strip_prefix(prefix).is_some_and(|path| {
            path.strip_prefix("github/")
                .unwrap_or(path)
                .strip_suffix(".list")
                .is_some_and(|name| {
                    !name.is_empty()
                        && name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                })
        })
    })
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
    if script_path.is_some_and(|url| super::script_resources::replacement_for(url).is_some()) {
        errors.push(error(
            line_number,
            "obsolete-script-resource",
            "script resource has a maintained replacement; refusing the retired 404/TLS proxy reference",
        ));
    }
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
        validate_apple_compatibility, validate_external_ruleset, validate_shadowrocket_module,
        validate_surge_module, validate_surge_section_line, validate_transport_security,
    };

    #[test]
    fn surge_logical_rule_requires_well_formed_policy_free_children() {
        for rule in [
            "AND,garbage,REJECT",
            "OR,(),REJECT",
            "NOT,((DOMAIN,a.example),(DOMAIN,b.example)),REJECT",
            "AND,((FINAL),(DOMAIN,a.example)),REJECT",
            "AND,((DOMAIN,a.example,DIRECT)),REJECT",
            "AND,((DOMAIN,a.example,My Proxy)),REJECT",
            "AND,((DOMAIN,a.example,pre-matching)),REJECT",
            "AND,((NOT,((DOMAIN,a.example)),DIRECT)),REJECT",
            "AND,((DOMAIN,a.example),),REJECT",
            "AND,((DOMAIN,a.example)),pre-matching",
            "AND,)((DOMAIN,a.example)),REJECT",
        ] {
            let content = format!("#!name=fixture\n[Rule]\n{rule}\n");
            assert!(
                validate_surge_module(&content)
                    .iter()
                    .any(|error| error.line == 3 && error.code == "invalid-logical-rule"),
                "{rule}",
            );
        }
    }

    #[test]
    fn surge_logical_rules_accept_valid_matchers_and_bound_nesting() {
        for rule in [
            "AND,((NOT,((SRC-IP,192.168.1.110))),(DOMAIN-SUFFIX,example.com)),My Proxy",
            "AND,((PROTOCOL,UDP),(RULE-SET,https://example.test/streaming.list)),REJECT",
            r#"OR,((USER-AGENT,"Example (, V2"),(USER-AGENT,'Example ), V3')),DIRECT"#,
            "OR,((DOMAIN,a.example),(DOMAIN,b.example)),DIRECT,notification-text=Matched",
        ] {
            let content = format!("#!name=fixture\n[Rule]\n{rule}\n");
            assert!(validate_surge_module(&content).is_empty(), "{rule}");
        }
        let mut leaf = "DOMAIN,a.example".to_string();
        for depth in 1..=11 {
            leaf = format!("NOT,(({leaf}))");
            let content = format!("#!name=fixture\n[Rule]\n{leaf},DIRECT\n");
            let errors = validate_surge_module(&content);
            assert_eq!(errors.is_empty(), depth <= 10, "depth {depth}: {errors:?}");
        }
    }

    #[test]
    fn surge_logical_rules_enforce_pre_matching_and_promax_dns_safety() {
        let rule = "AND,((DOMAIN-SUFFIX,tracker.example.com),(DEST-PORT,443)),REJECT,pre-matching";
        let valid = format!("#!name=PROMAX fixture\n[Rule]\n{rule}\n");
        assert!(validate_surge_module(&valid).is_empty());
        for (rule, code) in [
            (
                "AND,((DOMAIN,a.example),(DEST-PORT,443)),DIRECT,pre-matching",
                "invalid-rule-option",
            ),
            (
                "AND,((DOMAIN,a.example),(PROTOCOL,UDP)),REJECT,pre-matching",
                "invalid-rule-option",
            ),
            (
                "AND,((DOMAIN,a.example),(NOT,((IP-CIDR,192.0.2.0/24)))),REJECT",
                "ip-rule-missing-no-resolve",
            ),
            (
                "AND,((RULE-SET,https://example.test/ads.list,no-resolve)),REJECT",
                "unfiltered-promax-ruleset",
            ),
        ] {
            let name = if code == "invalid-rule-option" {
                "fixture"
            } else {
                "PROMAX fixture"
            };
            let content = format!("#!name={name}\n[Rule]\n{rule}\n");
            assert!(
                validate_surge_module(&content)
                    .iter()
                    .any(|error| error.code == code),
                "{rule}",
            );
        }
        let valid = "#!name=PROMAX fixture\n[Rule]\nAND,((DOMAIN,a.example),(NOT,((IP-CIDR,192.0.2.0/24,no-resolve)))),REJECT\n";
        assert!(validate_surge_module(valid).is_empty());
    }

    #[test]
    fn logical_rules_protect_apple_targets_without_rejecting_exclusions() {
        for (rule, blocked) in [
            (
                "AND,((DOMAIN,account.apple.com),(DEST-PORT,443)),REJECT,pre-matching",
                true,
            ),
            ("NOT,((NOT,((DOMAIN,account.apple.com)))),REJECT", true),
            (
                "AND,((NOT,((DOMAIN,account.apple.com))),(DEST-PORT,443)),REJECT,pre-matching",
                false,
            ),
            (
                "AND,((DOMAIN,account.apple.com),(DEST-PORT,443)),DIRECT",
                false,
            ),
        ] {
            let content = format!("#!name=fixture\n[Rule]\n{rule}\n");
            let errors = validate_surge_module(&content);
            if blocked {
                assert_eq!(errors.len(), 1, "{rule}: {errors:?}");
                assert_eq!(errors[0].code, "apple-critical-nondirect");
            } else {
                assert!(errors.is_empty(), "{rule}: {errors:?}");
            }
        }
    }

    #[test]
    fn shadowrocket_promax_ip_rules_and_rulesets_require_no_resolve() {
        let rules = [
            "IP-CIDR,101.36.166.17/32,REJECT",
            "IP-CIDR6,2001:db8::1/128,REJECT",
            "IP-ASN,13335,REJECT",
            "GEOIP,CN,REJECT",
            "RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/AdBlock_Local_ip.list,REJECT",
        ];
        for rule in rules {
            let unsafe_module = format!("#!name=PROMAX fixture\n[Rule]\n{rule}\n");
            assert!(
                !validate_shadowrocket_module(&unsafe_module).is_empty(),
                "{rule}"
            );
            let safe_module = format!("#!name=PROMAX fixture\n[Rule]\n{rule},no-resolve\n");
            assert!(
                validate_shadowrocket_module(&safe_module).is_empty(),
                "{rule}"
            );
        }
        assert!(
            validate_shadowrocket_module(
                "#!name=Other module\n[Rule]\nIP-CIDR,192.0.2.1/32,REJECT\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn inline_ip_pre_matching_preserves_ip_rewrite_proxy_coverage() {
        let module = "#!name=PROMAX fixture\n#!requirement=CORE_VERSION>=6010000\n[IP Rewrite]\n192.0.2.0/24 = reject\n2001:db8::/32 = reject\n[Rule]\nIP-CIDR,192.0.2.0/24,REJECT,no-resolve,pre-matching\nIP-CIDR6,2001:db8::/32,REJECT,pre-matching,no-resolve\nIP-ASN,64496,REJECT,no-resolve,pre-matching\nGEOIP,CN,REJECT,no-resolve,pre-matching\n";
        assert_eq!(validate_surge_module(module), Vec::new());
    }

    #[test]
    fn inline_pre_matching_requires_supported_rule_kind_and_reject_policy() {
        for rule in [
            "DOMAIN,ads.example.test,DIRECT,pre-matching",
            "DOMAIN-REGEX,^ads\\.example\\.test$,REJECT,pre-matching",
            "URL-REGEX,^https://ads.example.test/,REJECT,pre-matching",
            "USER-AGENT,ad-client,REJECT,pre-matching",
            "PROCESS-NAME,ad-client,REJECT,pre-matching",
        ] {
            let module = format!("#!name=fixture\n[Rule]\n{rule}\n");
            assert!(
                validate_surge_module(&module)
                    .iter()
                    .any(|error| error.code == "invalid-rule-option"),
                "{rule}"
            );
        }
    }

    #[test]
    fn ip_rewrite_drops_require_equivalent_proxy_fallbacks() {
        for address in [
            "101.36.166.17",
            "101.36.166.18",
            "101.36.166.19",
            "47.106.88.76",
        ] {
            let module = format!(
                "#!name=PROMAX fixture\n#!requirement=CORE_VERSION>=6010000\n[IP Rewrite]\n{address} = drop\n[Rule]\nIP-CIDR,{address}/32,REJECT-DROP,no-resolve\n"
            );
            assert!(validate_surge_module(&module).is_empty(), "{address}");
            assert!(
                !validate_surge_module(&module.replace(",REJECT-DROP,no-resolve", ",REJECT-DROP"))
                    .is_empty()
            );
        }
        for target in [
            "101.36.166.0/24",
            "101.36.166.16",
            "101.36.166.20",
            "47.106.88.77",
        ] {
            let module = format!(
                "#!name=PROMAX fixture\n#!requirement=CORE_VERSION>=6010000\n[IP Rewrite]\n{target} = drop\n"
            );
            assert!(
                validate_surge_module(&module)
                    .iter()
                    .any(|error| error.code == "ip-rewrite-missing-proxy-fallback"),
                "{target}"
            );
        }
    }

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
        let fixture = "#!name=PROMAX fixture\n[Rule]\nRULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/Ads_Local.list,REJECT,magic-option,no-resolve\n";
        let errors = validate_surge_module(fixture);

        assert_eq!(errors.len(), 1);
        assert_eq!((errors[0].line, errors[0].code), (3, "invalid-rule-option"));
    }

    #[test]
    fn rule_sets_require_policy_and_reject_policy_for_pre_matching() {
        for rule in [
            "RULE-SET,https://example.test/ads.list,",
            "RULE-SET,https://example.test/ads.list,DIRECT,pre-matching",
        ] {
            let module = format!("#!name=fixture\n[Rule]\n{rule}\n");
            assert!(!validate_surge_module(&module).is_empty(), "{rule}");
        }
        for policy in ["REJECT", "REJECT-DROP", "REJECT-NO-DROP"] {
            let module = format!(
                "#!name=fixture\n[Rule]\nRULE-SET,https://example.test/ads.list,{policy},pre-matching\n"
            );
            assert!(validate_surge_module(&module).is_empty(), "{policy}");
        }
        let module = "#!name=fixture\n[Rule]\nRULE-SET,https://example.test/ads.list,My Policy\n";
        assert!(validate_surge_module(module).is_empty());
        let shadowrocket =
            "#!name=fixture\n[Rule]\nRULE-SET,https://example.test/ads.list,DIRECT,pre-matching\n";
        assert!(validate_shadowrocket_module(shadowrocket).is_empty());
    }

    #[test]
    fn rule_set_update_interval_accepts_negative_disable_and_positive_values() {
        for interval in ["-1", "-60", "86400", "31536001"] {
            let module = format!(
                "#!name=fixture\n[Rule]\nRULE-SET,https://example.test/ads.list,DIRECT,update-interval={interval}\n"
            );
            assert!(validate_surge_module(&module).is_empty(), "{interval}");
        }
        for interval in ["0", "invalid"] {
            let module = format!(
                "#!name=fixture\n[Rule]\nRULE-SET,https://example.test/ads.list,DIRECT,update-interval={interval}\n"
            );
            assert!(
                validate_surge_module(&module)
                    .iter()
                    .any(|error| error.code == "invalid-rule-option"),
                "{interval}"
            );
        }
    }

    #[test]
    fn promax_inline_ip_rules_require_lowercase_no_resolve_only() {
        let missing_options = [
            "IP-CIDR,192.0.2.1/32,REJECT-DROP",
            "IP-CIDR6,2001:db8::1/128,REJECT-DROP",
            "IP-ASN,13335,REJECT-DROP",
            "GEOIP,CN,REJECT-DROP",
        ];
        for rule in missing_options {
            let module = format!("#!name=PROMAX fixture\n[Rule]\n{rule}\n");
            assert!(
                validate_surge_module(&module)
                    .iter()
                    .any(|error| error.code == "ip-rule-missing-no-resolve"),
                "expected guard for {rule}"
            );
        }

        let fixture = r#"#!name=PROMAX fixture
[Rule]
IP-CIDR,192.0.2.1/32,REJECT-DROP,no-resolve
IP-CIDR6,2001:db8::1/128,REJECT-DROP,no-resolve
IP-ASN,13335,REJECT-DROP,no-resolve
GEOIP,CN,REJECT-DROP,no-resolve
DOMAIN-SUFFIX,ads.example,REJECT-DROP
"#;
        assert!(validate_surge_module(fixture).is_empty());

        let uppercase_option =
            "#!name=PROMAX fixture\n[Rule]\nIP-CIDR,192.0.2.1/32,REJECT-DROP,NO-RESOLVE\n";
        assert!(
            validate_surge_module(uppercase_option)
                .iter()
                .any(|error| error.code == "ip-rule-missing-no-resolve")
        );

        let non_promax = "#!name=Other module\n[Rule]\nIP-CIDR,192.0.2.1/32,REJECT-DROP\nRULE-SET,https://example.test/any.list,PROXY\n";
        assert!(validate_surge_module(non_promax).is_empty());
        assert!(
            validate_external_ruleset("IP-CIDR,192.0.2.1/32\n").is_empty(),
            "policy-neutral standalone IP rulesets remain unaffected"
        );
    }

    #[test]
    fn promax_surge_rule_sets_require_lowercase_no_resolve() {
        let base = "RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/Ads_Local.list,REJECT-DROP";
        for suffix in ["", ",NO-RESOLVE"] {
            let module = format!("#!name=PROMAX fixture\n[Rule]\n{base}{suffix}\n");
            assert!(
                validate_surge_module(&module)
                    .iter()
                    .any(|error| error.code == "ruleset-missing-no-resolve"),
                "expected canonical option guard for {suffix:?}"
            );
        }
        let module = format!("#!name=PROMAX fixture\n[Rule]\n{base},no-resolve\n");
        assert!(validate_surge_module(&module).is_empty());
    }

    #[test]
    fn shadowrocket_validation_keeps_promax_safety_and_no_resolve() {
        let valid_shadowrocket = r#"#!name=PROMAX fixture
[Rule]
IP-CIDR,192.0.2.1/32,REJECT-DROP,no-resolve
RULE-SET,https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/Ads_Local.list,REJECT-DROP,no-resolve
"#;
        assert!(validate_shadowrocket_module(valid_shadowrocket).is_empty());
        assert!(validate_surge_module(valid_shadowrocket).is_empty());

        let unsafe_shadowrocket = r#"#!name=PROMAX fixture
[Rule]
RULE-SET,https://example.test/unfiltered.list,PROXY
"#;
        let codes: Vec<_> = validate_shadowrocket_module(unsafe_shadowrocket)
            .iter()
            .map(|error| error.code)
            .collect();
        assert!(codes.contains(&"unfiltered-promax-ruleset"));
        assert!(codes.contains(&"unsupported-policy"));
    }

    #[test]
    fn ip_rewrite_is_surge_only_and_requires_safe_action_with_proxy_fallback() {
        assert!(validate_surge_module("#!name=fixture\n[IP Rewrite]\n2001:db8::/32 = drop\n203.0.113.0/24 = reject\n10.7.0.1 = reflect\n").is_empty());
        assert!(
            !validate_surge_module("#!name=fixture\n[IP Rewrite]\n8.8.8.8/ = drop\n").is_empty()
        );
        let module = "#!name=PROMAX fixture\n#!requirement=CORE_VERSION>=6010000\n[IP Rewrite]\n180.76.76.200 = drop\n[Rule]\nIP-CIDR,180.76.76.200/32,REJECT-DROP,no-resolve\n";
        assert!(validate_surge_module(module).is_empty());
        assert!(!validate_shadowrocket_module(module).is_empty());
        for unsafe_module in [
            module.replace("#!requirement=CORE_VERSION>=6010000\n", ""),
            module.replace("IP-CIDR,180.76.76.200/32,REJECT-DROP,no-resolve\n", ""),
            module.replace("180.76.76.200 = drop", "180.76.76.0/24 = drop"),
            module.replace("180.76.76.200 = drop", "8.8.8.8 = drop"),
            module.replace("180.76.76.200 = drop", "180.76.76.200 = reflect"),
        ] {
            assert!(
                !validate_surge_module(&unsafe_module).is_empty(),
                "{unsafe_module}"
            );
        }
    }

    #[test]
    fn ip_rewrite_fallback_matches_canonical_network_family_and_action() {
        for (target, action, fallback) in [
            (
                "203.0.113.42/24",
                "reject",
                "IP-CIDR,203.0.113.0/24,REJECT,no-resolve",
            ),
            (
                "2001:db8::1",
                "drop",
                "IP-CIDR6,2001:db8::1/128,REJECT-DROP,no-resolve",
            ),
            (
                "2001:db8::1234/64",
                "reject",
                "IP-CIDR6,2001:db8::/64,REJECT,no-resolve",
            ),
        ] {
            let module = format!(
                "#!name=PROMAX fixture\n#!requirement=CORE_VERSION>=6010000\n[IP Rewrite]\n{target} = {action}\n[Rule]\n{fallback}\n"
            );
            assert!(validate_surge_module(&module).is_empty(), "{module}");
        }

        for fallback in [
            "IP-CIDR,192.0.2.1/32,REJECT-DROP,no-resolve",
            "IP-CIDR6,2001:db8::1/128,REJECT,no-resolve",
            "IP-CIDR6,2001:db8::1/128,REJECT-DROP",
            "IP-CIDR6,2001:db8::1/128,REJECT-DROP,no-resolve,extended-matching",
        ] {
            let module = format!(
                "#!name=PROMAX fixture\n#!requirement=CORE_VERSION>=6010000\n[IP Rewrite]\n2001:db8::1 = drop\n[Rule]\n{fallback}\n"
            );
            assert!(
                validate_surge_module(&module)
                    .iter()
                    .any(|error| error.code == "ip-rewrite-missing-proxy-fallback"),
                "{module}"
            );
        }
        let wrong_section = "#!name=PROMAX fixture\n#!requirement=CORE_VERSION>=6010000\n[IP Rewrite]\n2001:db8::1 = drop\n[Other]\nIP-CIDR6,2001:db8::1/128,REJECT-DROP,no-resolve\n";
        assert!(
            validate_surge_module(wrong_section)
                .iter()
                .any(|error| error.code == "ip-rewrite-missing-proxy-fallback")
        );
    }

    #[test]
    fn obsolete_embedded_script_resources_cannot_be_published() {
        for url in [
            "https://cdn.jsdelivr.net/gh/Yu9191/Rewrite@main/mlxx.js",
            "https://klraw.pages.dev/kelv1n1n/script/refs/heads/main/js/goofish.js?token=fixture",
        ] {
            let module = format!(
                "#!name=fixture\n[Script]\nfixture = type=http-response,pattern=^https://ads.example.test/,script-path={url}\n"
            );
            assert!(
                validate_surge_module(&module)
                    .iter()
                    .any(|error| error.code == "obsolete-script-resource")
            );
        }
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
    fn apple_music_playback_rejects_and_mitm_are_rejected() {
        let fixture = "#!name=unsafe\n[Rule]\nDOMAIN,audio-ssl.itunes.apple.com,REJECT\n[MITM]\nhostname = %APPEND% audio-ssl.itunes.apple.com, *.music.apple.com\n";
        let codes: Vec<_> = validate_apple_compatibility(fixture, false)
            .into_iter()
            .map(|error| error.code)
            .collect();
        assert_eq!(
            codes,
            [
                "apple-critical-nondirect",
                "apple-critical-mitm",
                "apple-critical-mitm"
            ]
        );
    }

    #[test]
    fn apple_certificate_validation_cannot_be_remapped() {
        let fixture = "#!name=unsafe\n[Host]\nocsp.apple.com = 0.0.0.0\n[MITM]\nhostname = %APPEND% certs.apple.com\n";
        let codes: Vec<_> = validate_apple_compatibility(fixture, false)
            .into_iter()
            .map(|error| error.code)
            .collect();
        assert_eq!(
            codes,
            ["apple-critical-host-mapping", "apple-critical-mitm"]
        );
    }

    #[test]
    fn apple_music_protection_keeps_weather_and_tv_enhancements_available() {
        assert!(validate_apple_compatibility("[MITM]\nhostname = %APPEND% weatherkit.apple.com, uts-api.itunes.apple.com, play-cdn.itunes.apple.com, hls.itunes.apple.com, hls-svod.itunes.apple.com\n", false).is_empty());
    }

    #[test]
    fn promax_rejects_unfiltered_third_party_ruleset_references() {
        let errors = validate_surge_module(
            "#!name=PROMAX\n[Rule]\nRULE-SET,https://ruleset.skk.moe/List/non_ip/reject-no-drop.conf,REJECT-NO-DROP\n",
        );
        assert!(
            errors
                .iter()
                .any(|error| error.code == "unfiltered-promax-ruleset")
        );
    }

    #[test]
    fn promax_ruleset_boundary_accepts_only_published_list_paths() {
        for base in [
            "https://raw.githubusercontent.com/nowaytouse/script_hub/master/rulesets/AdBlock/",
            "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/rulesets/AdBlock/",
        ] {
            for path in [
                "Ads_Local.list",
                "github/Ads_Local.list",
                "Ads_Security_01.list",
            ] {
                assert!(super::is_compiled_adblock_ruleset(&format!("{base}{path}")));
            }
            for path in [
                "../Apple.list",
                "%2e%2e/Apple.list",
                "Ads.list?raw=1",
                "github/../Ads.list",
                "Ads.list#fragment",
                "sub/Ads.list",
            ] {
                assert!(!super::is_compiled_adblock_ruleset(&format!(
                    "{base}{path}"
                )));
            }
        }
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
