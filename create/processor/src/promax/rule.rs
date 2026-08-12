use fancy_regex::Regex;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::net::IpAddr;

const POLICIES: &[&str] = &[
    "DIRECT",
    "PROXY",
    "REJECT",
    "REJECT-DROP",
    "REJECT-NO-DROP",
    "REJECT-TINYGIF",
    "REJECT-IMG",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleKind {
    Domain,
    DomainSuffix,
    DomainKeyword,
    DomainWildcard,
    DomainRegex,
    IpCidr,
    IpCidr6,
    IpAsn,
    GeoIp,
    UserAgent,
    UrlRegex,
    ProcessName,
    DestPort,
    SrcPort,
    InPort,
}

impl RuleKind {
    fn parse(value: &str) -> Result<Self, RuleError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "DOMAIN" => Ok(Self::Domain),
            "DOMAIN-SUFFIX" => Ok(Self::DomainSuffix),
            "DOMAIN-KEYWORD" => Ok(Self::DomainKeyword),
            "DOMAIN-WILDCARD" => Ok(Self::DomainWildcard),
            "DOMAIN-REGEX" => Ok(Self::DomainRegex),
            "IP-CIDR" => Ok(Self::IpCidr),
            "IP-CIDR6" => Ok(Self::IpCidr6),
            "IP-ASN" => Ok(Self::IpAsn),
            "GEOIP" => Ok(Self::GeoIp),
            "USER-AGENT" => Ok(Self::UserAgent),
            "URL-REGEX" => Ok(Self::UrlRegex),
            "PROCESS-NAME" => Ok(Self::ProcessName),
            "DEST-PORT" => Ok(Self::DestPort),
            "SRC-PORT" => Ok(Self::SrcPort),
            "IN-PORT" => Ok(Self::InPort),
            other => Err(RuleError(format!("unsupported rule type {other}"))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Domain => "DOMAIN",
            Self::DomainSuffix => "DOMAIN-SUFFIX",
            Self::DomainKeyword => "DOMAIN-KEYWORD",
            Self::DomainWildcard => "DOMAIN-WILDCARD",
            Self::DomainRegex => "DOMAIN-REGEX",
            Self::IpCidr => "IP-CIDR",
            Self::IpCidr6 => "IP-CIDR6",
            Self::IpAsn => "IP-ASN",
            Self::GeoIp => "GEOIP",
            Self::UserAgent => "USER-AGENT",
            Self::UrlRegex => "URL-REGEX",
            Self::ProcessName => "PROCESS-NAME",
            Self::DestPort => "DEST-PORT",
            Self::SrcPort => "SRC-PORT",
            Self::InPort => "IN-PORT",
        }
    }

    fn payload_may_contain_commas(self) -> bool {
        matches!(self, Self::DomainRegex | Self::UrlRegex | Self::UserAgent)
    }

    fn requires_regex_validation(self) -> bool {
        matches!(self, Self::DomainRegex | Self::UrlRegex)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurgeRule {
    pub kind: RuleKind,
    pub payload: String,
    pub options: Vec<String>,
    pub policy: Option<String>,
}

impl SurgeRule {
    pub fn parse(line: &str) -> Result<Self, RuleError> {
        let fields = split_fields(line.trim())?;
        if fields.len() < 2 {
            return Err(RuleError("missing rule payload".to_string()));
        }

        let kind = RuleKind::parse(&fields[0])?;
        let mut end = fields.len();
        let mut policy = None;
        let mut options = Vec::new();

        while end > 2 {
            let tail = fields[end - 1].trim();
            let upper = tail.to_ascii_uppercase();
            if POLICIES.contains(&upper.as_str()) {
                if policy.is_some() {
                    return Err(RuleError("multiple rule policies".to_string()));
                }
                policy = Some(upper);
                end -= 1;
                continue;
            }
            if is_option(&upper) {
                options.push(upper);
                end -= 1;
                continue;
            }
            break;
        }
        options.reverse();
        let mut seen_options = HashSet::new();
        options.retain(|option| seen_options.insert(option.clone()));

        if !kind.payload_may_contain_commas() && end != 2 {
            return Err(RuleError("unexpected extra rule fields".to_string()));
        }

        let payload = fields[1..end].join(",").trim().to_string();
        if payload.is_empty() {
            return Err(RuleError("empty rule payload".to_string()));
        }
        validate_payload(kind, &payload)?;

        Ok(Self {
            kind,
            payload,
            options,
            policy,
        })
    }

    pub fn render_external(&self) -> String {
        self.render(false, "", true)
    }

    pub fn render_module(&self, default_policy: &str) -> String {
        self.render(true, default_policy, false)
    }

    fn render(&self, include_policy: bool, default_policy: &str, external: bool) -> String {
        let payload = if self.payload.contains(',') {
            format!("\"{}\"", self.payload)
        } else {
            self.payload.clone()
        };
        let mut fields = vec![self.kind.as_str().to_string(), payload];
        if include_policy {
            fields.push(
                self.policy
                    .as_deref()
                    .unwrap_or(default_policy)
                    .to_ascii_uppercase(),
            );
        }
        fields.extend(
            self.options
                .iter()
                .filter(|option| option_allowed(self.kind, option, external))
                .cloned(),
        );
        fields.join(",")
    }
}

fn option_allowed(kind: RuleKind, option: &str, external: bool) -> bool {
    match option {
        "NO-RESOLVE" => matches!(kind, RuleKind::IpCidr | RuleKind::IpCidr6 | RuleKind::GeoIp),
        "EXTENDED-MATCHING" => true,
        "FORCE-CELLULAR" => !external,
        _ => false,
    }
}

fn validate_payload(kind: RuleKind, payload: &str) -> Result<(), RuleError> {
    if kind.requires_regex_validation() {
        return Regex::new(payload)
            .map(|_| ())
            .map_err(|error| RuleError(format!("invalid regular expression: {error}")));
    }

    match kind {
        RuleKind::Domain | RuleKind::DomainSuffix => validate_domain(payload, false),
        RuleKind::DomainWildcard => validate_domain(payload, true),
        RuleKind::DomainKeyword => {
            if payload.contains("://")
                || payload.contains('/')
                || payload.chars().any(char::is_whitespace)
            {
                Err(RuleError("invalid domain keyword".to_string()))
            } else {
                Ok(())
            }
        }
        RuleKind::IpCidr | RuleKind::IpCidr6 => validate_ip_network(kind, payload),
        RuleKind::IpAsn => payload
            .trim_start_matches(|character: char| character.eq_ignore_ascii_case(&'a'))
            .trim_start_matches(|character: char| character.eq_ignore_ascii_case(&'s'))
            .parse::<u32>()
            .ok()
            .filter(|asn| *asn > 0)
            .map(|_| ())
            .ok_or_else(|| RuleError("invalid IP-ASN payload".to_string())),
        RuleKind::GeoIp => {
            if payload.len() == 2
                && payload
                    .chars()
                    .all(|character| character.is_ascii_alphabetic())
            {
                Ok(())
            } else {
                Err(RuleError("invalid GEOIP country code".to_string()))
            }
        }
        RuleKind::DestPort | RuleKind::SrcPort | RuleKind::InPort => validate_port(payload),
        _ => Ok(()),
    }
}

fn validate_domain(payload: &str, wildcard: bool) -> Result<(), RuleError> {
    let valid = !payload.contains("://")
        && !payload.contains('/')
        && !payload.chars().any(char::is_whitespace)
        && payload.split('.').all(|label| {
            !label.is_empty()
                && label.chars().all(|character| {
                    character.is_ascii_alphanumeric()
                        || character == '-'
                        || character == '_'
                        || wildcard && matches!(character, '*' | '?')
                })
        });
    valid
        .then_some(())
        .ok_or_else(|| RuleError("invalid domain payload".to_string()))
}

fn validate_ip_network(kind: RuleKind, payload: &str) -> Result<(), RuleError> {
    let (address, prefix) = payload
        .split_once('/')
        .ok_or_else(|| RuleError("IP rule requires a CIDR prefix".to_string()))?;
    let address: IpAddr = address
        .parse()
        .map_err(|_| RuleError("invalid IP address".to_string()))?;
    let prefix: u8 = prefix
        .parse()
        .map_err(|_| RuleError("invalid CIDR prefix".to_string()))?;
    let correct_family = matches!(
        (kind, address),
        (RuleKind::IpCidr, IpAddr::V4(_)) | (RuleKind::IpCidr6, IpAddr::V6(_))
    );
    let max_prefix = if address.is_ipv4() { 32 } else { 128 };
    if correct_family && prefix <= max_prefix {
        Ok(())
    } else {
        Err(RuleError("invalid CIDR network".to_string()))
    }
}

fn validate_port(payload: &str) -> Result<(), RuleError> {
    let valid_port = |value: &str| value.parse::<u16>().is_ok();
    let valid = payload.split_once('-').map_or_else(
        || valid_port(payload),
        |(start, end)| {
            valid_port(start)
                && valid_port(end)
                && start.parse::<u16>().ok() <= end.parse::<u16>().ok()
        },
    );
    valid
        .then_some(())
        .ok_or_else(|| RuleError("invalid port payload".to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleError(String);

impl fmt::Display for RuleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for RuleError {}

pub(crate) fn is_option(value: &str) -> bool {
    matches!(
        value,
        "NO-RESOLVE" | "EXTENDED-MATCHING" | "PRE-MATCHING" | "FORCE-CELLULAR"
    ) || value.starts_with("UPDATE-INTERVAL=")
}

pub(crate) fn split_fields(line: &str) -> Result<Vec<String>, RuleError> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut closed_quote = false;
    let mut escaped = false;

    for character in line.chars() {
        if in_quotes {
            if character == '"' && !escaped {
                in_quotes = false;
                closed_quote = true;
                continue;
            }
            escaped = character == '\\' && !escaped;
            if character != '\\' {
                escaped = false;
            }
            current.push(character);
            continue;
        }

        match character {
            '"' if current.trim().is_empty() && !closed_quote => {
                in_quotes = true;
                escaped = false;
            }
            ',' => {
                fields.push(current.trim().to_string());
                current.clear();
                closed_quote = false;
            }
            '"' => {
                return Err(RuleError("quote must begin a field".to_string()));
            }
            value if closed_quote && !value.is_whitespace() => {
                return Err(RuleError(
                    "unexpected content after closing quote".to_string(),
                ));
            }
            value => current.push(value),
        }
    }

    if in_quotes {
        return Err(RuleError("unbalanced double quote".to_string()));
    }
    fields.push(current.trim().to_string());
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::SurgeRule;

    #[test]
    fn quoted_url_regex_keeps_commas_and_peels_policy() {
        let parsed = SurgeRule::parse(r#"URL-REGEX,"^https?://host/(a{1,3}|b)",REJECT"#).unwrap();

        assert_eq!(parsed.payload, r#"^https?://host/(a{1,3}|b)"#);
        assert_eq!(parsed.policy.as_deref(), Some("REJECT"));
        assert_eq!(
            parsed.render_module("REJECT"),
            r#"URL-REGEX,"^https?://host/(a{1,3}|b)",REJECT"#
        );
        assert_eq!(
            parsed.render_external(),
            r#"URL-REGEX,"^https?://host/(a{1,3}|b)""#
        );
    }

    #[test]
    fn rejects_nested_quote_policy_corruption() {
        assert!(SurgeRule::parse(r#"URL-REGEX,""^http://host/d",REJECT",REJECT"#).is_err());
    }

    #[test]
    fn domain_wildcard_is_supported() {
        let parsed = SurgeRule::parse("DOMAIN-WILDCARD,api-*.example.com").unwrap();
        assert_eq!(
            parsed.render_external(),
            "DOMAIN-WILDCARD,api-*.example.com"
        );
    }

    #[test]
    fn module_policy_precedes_rule_options() {
        let parsed =
            SurgeRule::parse("IP-CIDR,192.0.2.0/24,REJECT,extended-matching,no-resolve").unwrap();

        assert_eq!(
            parsed.render_module("REJECT"),
            "IP-CIDR,192.0.2.0/24,REJECT,EXTENDED-MATCHING,NO-RESOLVE"
        );
    }

    #[test]
    fn accepts_surge_lookaround_regex_dialect() {
        let parsed =
            SurgeRule::parse(r#"URL-REGEX,^https://api\.example/recommend/(?!v2),REJECT"#).unwrap();

        assert_eq!(parsed.policy.as_deref(), Some("REJECT"));
    }

    #[test]
    fn external_renderer_drops_module_only_options_and_deduplicates() {
        let parsed = SurgeRule::parse(
            "IP-CIDR,192.0.2.0/24,NO-RESOLVE,pre-matching,update-interval=86400,no-resolve",
        )
        .unwrap();

        assert_eq!(parsed.render_external(), "IP-CIDR,192.0.2.0/24,NO-RESOLVE");
    }

    #[test]
    fn rejects_unbalanced_mid_field_quote_and_invalid_typed_payloads() {
        assert!(SurgeRule::parse(r#"URL-REGEX,^https://host/\"unterminated,REJECT"#).is_err());
        assert!(SurgeRule::parse("IP-CIDR,not-a-cidr").is_err());
        assert!(SurgeRule::parse("DEST-PORT,abc").is_err());
        assert!(SurgeRule::parse("DOMAIN,https://example.com/path").is_err());
    }
}
