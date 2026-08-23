use crate::promax::rule::{RuleKind, SurgeRule};
use fancy_regex::Regex as FancyRegex;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::net::IpAddr;

static MEDIA_MARKER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(png|jpe?g|gif|webp|avif|heic|mp4|m3u8|images?|media|videos?|avatars?|thumbnails?|thumb|[0-9]{2,4}x[0-9]{2,4})",
    )
    .unwrap()
});

static AD_INTENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(^|[./_?&=\\-])(ads?|adx|adserver|adnet|adtech|adverts?|advertis(?:e|ing|ement)?|splash(?:_?ad)?|promotion|promote|commercial|launchad|feed_ad|rtb|sponsor)([./_?&=$\\-]|$)",
    )
    .unwrap()
});

static HOST_CANDIDATE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:[a-z0-9*?_-]+\.)+[a-z0-9*?_-]+").unwrap());

/// Apple documents these hosts as required for Apple Account authentication.
pub const APPLE_CRITICAL_EXACT: &[&str] = &[
    "account.apple.com",
    "appleid.cdn-apple.com",
    "idmsa.apple.com",
    "gsa.apple.com",
    "gateway.icloud.com",
    "known-issues.apple.com",
    "mask.icloud.com",
    "mask-h2.icloud.com",
    "mask-api.icloud.com",
    "probe.icloud.com",
    "pong.icloud.com",
    "metrics.icloud.com",
    "apple-native-relay.apple.com",
];

/// Apple documents these domains as required for iCloud and related services.
pub const APPLE_CRITICAL_SUFFIXES: &[&str] = &[
    "apple-cloudkit.com",
    "apple-livephotoskit.com",
    "apzones.com",
    "cdn-apple.com",
    "gc.apple.com",
    "icloud.com",
    "icloud.com.cn",
    "icloud.apple.com",
    "icloud-content.com",
    "iwork.apple.com",
    "apple-dns.net",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyDecision {
    Keep,
    DropDuplicate,
    Quarantine(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RiskClass {
    Invalid,
    ProtectedService,
    BroadMedia,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuarantineRecord {
    pub source: String,
    #[serde(skip_serializing)]
    pub line: usize,
    pub candidate: String,
    pub reason: String,
    pub risk: RiskClass,
}

#[derive(Debug, Default)]
pub struct SafetyPolicy {
    protected_domains: HashSet<String>,
    protected_suffixes: HashSet<String>,
    protected_keywords: HashSet<String>,
    protected_ips: Vec<IpNetwork>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtectionStats {
    pub domain_patterns: usize,
    pub ip_networks: usize,
    pub invalid_entries: usize,
}

pub fn apple_host_pattern_is_critical(pattern: &str) -> bool {
    let pattern = pattern
        .trim()
        .trim_start_matches('-')
        .trim_start_matches("*.")
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    APPLE_CRITICAL_EXACT.iter().any(|host| {
        pattern == *host
            || host.ends_with(&format!(".{pattern}"))
            || pattern.ends_with(&format!(".{host}"))
    }) || APPLE_CRITICAL_SUFFIXES.iter().any(|suffix| {
        pattern == *suffix
            || suffix.ends_with(&format!(".{pattern}"))
            || pattern.ends_with(&format!(".{suffix}"))
    })
}

impl ProtectionStats {
    pub fn is_publish_ready(self) -> bool {
        self.domain_patterns >= 300 && self.ip_networks >= 200 && self.invalid_entries == 0
    }
}

#[derive(Debug, Clone, Copy)]
struct IpNetwork {
    address: IpAddr,
    prefix: u8,
}

impl IpNetwork {
    fn parse(value: &str) -> Option<Self> {
        let (address, prefix) = value.split_once('/').unwrap_or((value, ""));
        let address: IpAddr = address.parse().ok()?;
        let max_prefix = if address.is_ipv4() { 32 } else { 128 };
        let prefix = if prefix.is_empty() {
            max_prefix
        } else {
            prefix.parse().ok()?
        };
        (prefix <= max_prefix).then_some(Self { address, prefix })
    }

    fn overlaps(self, other: Self) -> bool {
        let prefix = self.prefix.min(other.prefix);
        match (self.address, other.address) {
            (IpAddr::V4(left), IpAddr::V4(right)) => {
                let mask = if prefix == 0 {
                    0
                } else {
                    u32::MAX << (32 - prefix)
                };
                u32::from(left) & mask == u32::from(right) & mask
            }
            (IpAddr::V6(left), IpAddr::V6(right)) => {
                let mask = if prefix == 0 {
                    0
                } else {
                    u128::MAX << (128 - prefix)
                };
                u128::from(left) & mask == u128::from(right) & mask
            }
            _ => false,
        }
    }
}

impl SafetyPolicy {
    pub fn from_lines<I, S>(lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::from_lines_with_stats(lines).0
    }

    pub fn from_lines_with_stats<I, S>(lines: I) -> (Self, ProtectionStats)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut policy = Self::default();
        let mut invalid_entries = 0;
        for line in lines {
            let raw = line.as_ref().trim();
            if raw.is_empty() || raw.starts_with('#') {
                continue;
            }
            let Ok(rule) = SurgeRule::parse(raw) else {
                invalid_entries += 1;
                continue;
            };
            let payload = rule.payload.to_ascii_lowercase();
            match rule.kind {
                RuleKind::Domain => {
                    policy.protected_domains.insert(payload);
                }
                RuleKind::DomainSuffix => {
                    policy.protected_suffixes.insert(payload);
                }
                RuleKind::DomainKeyword => {
                    policy.protected_keywords.insert(payload);
                }
                RuleKind::IpCidr | RuleKind::IpCidr6 => {
                    if let Some(network) = IpNetwork::parse(&payload) {
                        policy.protected_ips.push(network);
                    }
                }
                _ => invalid_entries += 1,
            }
        }
        let stats = ProtectionStats {
            domain_patterns: policy.protected_domains.len()
                + policy.protected_suffixes.len()
                + policy.protected_keywords.len(),
            ip_networks: policy.protected_ips.len(),
            invalid_entries,
        };
        (policy, stats)
    }

    pub fn decision(&self, rule: &SurgeRule) -> SafetyDecision {
        let payload = rule.payload.to_ascii_lowercase();
        let intersects = match rule.kind {
            RuleKind::Domain => self.domain_is_protected(&payload),
            RuleKind::DomainSuffix => self.suffix_intersects(&payload),
            RuleKind::DomainKeyword => self.keyword_intersects(&payload),
            RuleKind::DomainWildcard => self.pattern_intersects(&wildcard_regex(&payload)),
            RuleKind::DomainRegex => FancyRegex::new(&payload)
                .ok()
                .is_some_and(|pattern| self.fancy_pattern_intersects(&pattern)),
            RuleKind::UrlRegex => {
                request_host_patterns(&payload)
                    .iter()
                    .any(|candidate| self.candidate_intersects_protected(candidate))
                    || FancyRegex::new(&payload).ok().is_some_and(|pattern| {
                        self.protected_hosts().any(|host| {
                            [
                                format!("https://{host}/"),
                                format!("https://probe.{host}/"),
                                format!("https://r1---sn-probe.{host}/videoplayback"),
                            ]
                            .iter()
                            .any(|probe| pattern.is_match(probe).unwrap_or(false))
                        })
                    })
            }
            RuleKind::IpCidr | RuleKind::IpCidr6 => {
                IpNetwork::parse(&payload).is_some_and(|blocked| {
                    self.protected_ips
                        .iter()
                        .any(|protected| protected.overlaps(blocked))
                })
            }
            _ => false,
        };

        if intersects {
            SafetyDecision::Quarantine("protected-domain-intersection")
        } else if rule.kind == RuleKind::UrlRegex {
            classify_functional_url(&rule.payload)
        } else {
            SafetyDecision::Keep
        }
    }

    pub fn functional_decision(&self, line: &str) -> SafetyDecision {
        let request = functional_request_pattern(line);
        let hosts = request_host_patterns(request);
        let protected = hosts.iter().any(|candidate| {
            self.candidate_intersects_protected(candidate)
                || self
                    .protected_keywords
                    .iter()
                    .any(|keyword| candidate.contains(keyword))
        });
        if protected {
            SafetyDecision::Quarantine("protected-functional-intersection")
        } else if let SafetyDecision::Quarantine(reason) = classify_functional_url(request) {
            SafetyDecision::Quarantine(reason)
        } else {
            SafetyDecision::Keep
        }
    }

    pub fn hostname_is_protected(&self, hostname: &str) -> bool {
        let hostname = hostname
            .trim()
            .trim_start_matches('-')
            .trim_start_matches("*.")
            .split(':')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        self.domain_is_protected(&hostname) || self.suffix_intersects(&hostname)
    }

    pub fn hostname_is_referenced(&self, hostname: &str, functional_lines: &[String]) -> bool {
        let patterns = Self::functional_host_patterns(functional_lines);
        self.hostname_is_referenced_by_patterns(hostname, &patterns)
    }

    pub fn functional_host_patterns(functional_lines: &[String]) -> Vec<String> {
        let mut patterns: Vec<String> = functional_lines
            .iter()
            .flat_map(|line| request_host_patterns(functional_request_pattern(line)))
            .map(|hostname| hostname_reference_token(&hostname))
            .filter(|token| token.len() >= 4)
            .collect();
        patterns.sort();
        patterns.dedup();
        patterns
    }

    pub fn hostname_is_referenced_by_patterns(&self, hostname: &str, patterns: &[String]) -> bool {
        if self.hostname_is_protected(hostname) {
            return false;
        }
        let token = hostname_reference_token(hostname);
        if token.len() < 4 {
            return false;
        }
        patterns.binary_search(&token).is_ok()
    }

    fn protected_hosts(&self) -> impl Iterator<Item = &str> {
        self.protected_domains
            .iter()
            .chain(self.protected_suffixes.iter())
            .map(String::as_str)
    }

    fn candidate_intersects_protected(&self, candidate: &str) -> bool {
        let candidate = candidate.trim_matches('.');
        if candidate.contains(['*', '?']) {
            let pattern = wildcard_regex(candidate);
            return self
                .protected_hosts()
                .any(|host| pattern.is_match(host) || pattern.is_match(&format!("probe.{host}")));
        }
        self.protected_hosts().any(|host| {
            candidate == host
                || candidate.ends_with(&format!(".{host}"))
                || host.ends_with(&format!(".{candidate}"))
        })
    }

    fn domain_is_protected(&self, domain: &str) -> bool {
        self.protected_domains.contains(domain)
            || self
                .protected_suffixes
                .iter()
                .any(|suffix| domain == suffix || domain.ends_with(&format!(".{suffix}")))
            || self
                .protected_keywords
                .iter()
                .any(|keyword| domain.contains(keyword))
    }

    fn suffix_intersects(&self, blocked: &str) -> bool {
        self.protected_hosts().any(|protected| {
            protected == blocked
                || protected.ends_with(&format!(".{blocked}"))
                || blocked.ends_with(&format!(".{protected}"))
        }) || self
            .protected_keywords
            .iter()
            .any(|keyword| blocked.contains(keyword))
    }

    fn keyword_intersects(&self, blocked: &str) -> bool {
        self.protected_hosts()
            .any(|protected| protected.contains(blocked) || blocked.contains(protected))
            || self
                .protected_keywords
                .iter()
                .any(|protected| protected.contains(blocked) || blocked.contains(protected))
    }

    fn pattern_intersects(&self, pattern: &Regex) -> bool {
        self.protected_hosts().any(|host| {
            [
                host.to_string(),
                format!("probe.{host}"),
                format!("r.{host}"),
                format!("r1.{host}"),
                format!("r-probe.{host}"),
            ]
            .iter()
            .any(|probe| pattern.is_match(probe))
        })
    }

    fn fancy_pattern_intersects(&self, pattern: &FancyRegex) -> bool {
        self.protected_hosts().any(|host| {
            pattern.is_match(host).unwrap_or(false)
                || pattern.is_match(&format!("probe.{host}")).unwrap_or(false)
        })
    }
}

fn functional_request_pattern(line: &str) -> &str {
    if let Some((_, pattern)) = line.split_once("pattern=") {
        return top_level_parameter_value(pattern);
    }
    let mut fields = line.split_whitespace();
    let first = fields.next().unwrap_or("");
    if first.starts_with("http-request") || first.starts_with("http-response") {
        fields.next().unwrap_or(first)
    } else {
        first
    }
}

fn top_level_parameter_value(value: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    let mut depths = [0_u32; 3];
    for (index, character) in value.char_indices() {
        if let Some(delimiter) = quote {
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
            '\'' | '"' => quote = Some(character),
            '{' => depths[0] += 1,
            '}' => depths[0] = depths[0].saturating_sub(1),
            '[' => depths[1] += 1,
            ']' => depths[1] = depths[1].saturating_sub(1),
            '(' => depths[2] += 1,
            ')' => depths[2] = depths[2].saturating_sub(1),
            ',' if depths == [0, 0, 0] => return value[..index].trim(),
            _ => {}
        }
    }
    value.trim()
}

fn request_host_patterns(request: &str) -> Vec<String> {
    let normalized = request
        .replace(r"\.", ".")
        .replace(r"\/", "/")
        .to_ascii_lowercase();
    HOST_CANDIDATE
        .find_iter(&normalized)
        .map(|candidate| candidate.as_str().trim_matches('.').to_string())
        .collect()
}

fn hostname_reference_token(hostname: &str) -> String {
    let hostname = hostname
        .trim()
        .trim_start_matches('-')
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let cleaned = hostname
        .trim_matches(['*', '.', '?'])
        .replace('*', "")
        .replace('?', "");
    let labels: Vec<&str> = cleaned
        .split('.')
        .filter(|label| !label.is_empty())
        .collect();
    if labels.len() > 2 {
        labels[labels.len() - 2..].join(".")
    } else {
        labels.join(".")
    }
}

pub fn classify_functional_url(line: &str) -> SafetyDecision {
    let normalized = line
        .trim_matches(['"', '\''])
        .replace(r"\/", "/")
        .to_ascii_lowercase();
    let has_url_scheme = normalized.starts_with("^http://")
        || normalized.starts_with("^https://")
        || normalized.starts_with("^https?://")
        || normalized.starts_with("^(http://")
        || normalized.starts_with("^(https://")
        || normalized.starts_with("^(https?://");
    if has_url_scheme && request_host_patterns(line).is_empty() {
        SafetyDecision::Quarantine("broad-cross-site-rewrite")
    } else if MEDIA_MARKER.is_match(line) && !AD_INTENT.is_match(line) {
        SafetyDecision::Quarantine("broad-media-match")
    } else {
        SafetyDecision::Keep
    }
}

pub fn classify_domain_payload(payload: &str) -> SafetyDecision {
    if MEDIA_MARKER.is_match(payload) && !AD_INTENT.is_match(payload) {
        SafetyDecision::Quarantine("broad-media-domain")
    } else {
        SafetyDecision::Keep
    }
}

fn wildcard_regex(value: &str) -> Regex {
    let escaped = regex::escape(value)
        .replace(r"\*", ".*")
        .replace(r"\?", ".");
    Regex::new(&format!("^{escaped}$")).expect("escaped wildcard must compile")
}

#[cfg(test)]
mod tests {
    use super::{
        QuarantineRecord, RiskClass, SafetyDecision, SafetyPolicy, apple_host_pattern_is_critical,
        classify_domain_payload, classify_functional_url,
    };
    use crate::promax::rule::SurgeRule;

    #[test]
    fn keyword_rule_intersecting_protected_media_is_quarantined() {
        let safety = SafetyPolicy::from_lines(["DOMAIN-SUFFIX,googlevideo.com"]);
        let rule = SurgeRule::parse("DOMAIN-KEYWORD,googlevideo").unwrap();

        assert_eq!(
            safety.decision(&rule),
            SafetyDecision::Quarantine("protected-domain-intersection")
        );
    }

    #[test]
    fn broad_shared_cdn_image_rewrite_is_quarantined() {
        let decision =
            classify_functional_url(r#"^https?://cdn.example.com/.+\.(png|jpe?g|webp)$ _ reject"#);

        assert_eq!(decision, SafetyDecision::Quarantine("broad-media-match"));
    }

    #[test]
    fn explicit_ad_asset_path_is_kept() {
        let decision =
            classify_functional_url(r#"^https?://cdn.example.com/advert/splash.webp$ _ reject"#);

        assert_eq!(decision, SafetyDecision::Keep);
    }

    #[test]
    fn generic_media_domain_is_quarantined_but_ad_cdn_is_kept() {
        assert_eq!(
            classify_domain_payload("feed-image.example.com"),
            SafetyDecision::Quarantine("broad-media-domain")
        );
        assert_eq!(
            classify_domain_payload("cdn.adserver.example.com"),
            SafetyDecision::Keep
        );
    }

    #[test]
    fn quarantine_report_omits_volatile_upstream_line_numbers() {
        let record = QuarantineRecord {
            source: "source.list".to_string(),
            line: 42,
            candidate: "DOMAIN,media.example".to_string(),
            reason: "broad-media-domain".to_string(),
            risk: RiskClass::BroadMedia,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(!json.contains("line"));
        assert!(json.contains("broad-media-domain"));
    }

    #[test]
    fn overlapping_protected_ip_network_is_quarantined() {
        let safety = SafetyPolicy::from_lines(["IP-CIDR,192.0.2.0/24"]);
        let rule = SurgeRule::parse("IP-CIDR,192.0.2.128/25").unwrap();

        assert_eq!(
            safety.decision(&rule),
            SafetyDecision::Quarantine("protected-domain-intersection")
        );
    }

    #[test]
    fn functional_matching_uses_request_pattern_not_script_download_url() {
        let safety = SafetyPolicy::from_lines(["DOMAIN-SUFFIX,githubusercontent.com"]);

        assert_eq!(
            safety.functional_decision(
                r#"x = type=http-response,pattern=^https://raw\.githubusercontent\.com/media,script-path=https://example.test/a.js"#
            ),
            SafetyDecision::Quarantine("protected-functional-intersection")
        );
        assert_eq!(
            safety.functional_decision(
                r#"x = type=http-response,pattern=^https://ads.example.test/feed,script-path=https://raw.githubusercontent.com/a.js"#
            ),
            SafetyDecision::Keep
        );
    }

    #[test]
    fn protected_media_subdomain_url_regex_is_quarantined() {
        let safety = SafetyPolicy::from_lines(["DOMAIN-SUFFIX,googlevideo.com"]);
        let rule = SurgeRule::parse(
            r#"URL-REGEX,^https://r[0-9]+---sn.*\.googlevideo\.com/videoplayback,REJECT"#,
        )
        .unwrap();

        assert_eq!(
            safety.decision(&rule),
            SafetyDecision::Quarantine("protected-domain-intersection")
        );
    }

    #[test]
    fn mitm_host_must_be_referenced_by_a_kept_request_pattern() {
        let safety = SafetyPolicy::from_lines(["DOMAIN-SUFFIX,github.com"]);
        let patterns = vec![
            r#"x = type=http-response,pattern=^https://api\.ads\.example/feed,script-path=https://github.com/script.js"#.to_string(),
        ];

        assert!(safety.hostname_is_referenced("*.ads.example", &patterns));
        assert!(!safety.hostname_is_referenced("baidu.com", &patterns));
        assert!(!safety.hostname_is_referenced("github.com", &patterns));
    }

    #[test]
    fn protected_host_checks_use_dns_label_boundaries_and_wildcards() {
        let safety = SafetyPolicy::from_lines(["DOMAIN-SUFFIX,googlevideo.com"]);

        let unrelated =
            SurgeRule::parse(r#"URL-REGEX,^https://notgooglevideo\.com/ads,REJECT"#).unwrap();
        assert_eq!(safety.decision(&unrelated), SafetyDecision::Keep);

        let protected = SurgeRule::parse(r#"DOMAIN-WILDCARD,r*.googlevideo.com,REJECT"#).unwrap();
        assert_eq!(
            safety.decision(&protected),
            SafetyDecision::Quarantine("protected-domain-intersection")
        );
    }

    #[test]
    fn script_url_cannot_hide_a_broad_media_request() {
        let safety = SafetyPolicy::default();
        assert_eq!(
            safety.functional_decision(
                r#"x = type=http-response,pattern=^https://cdn.example.test/a{1,4}\.(png|webp)$,script-path=https://scripts.example/ads/remove.js"#
            ),
            SafetyDecision::Quarantine("broad-media-match")
        );
    }

    #[test]
    fn mitm_reference_uses_hostname_boundaries() {
        let safety = SafetyPolicy::default();
        let functions = vec![
            r#"x = type=http-response,pattern=^https://notads\.example/path,script-path=https://example.test/x.js"#.to_string(),
        ];

        assert!(!safety.hostname_is_referenced("*.ads.example", &functions));
    }

    #[test]
    fn hostless_and_malformed_cross_site_rewrites_are_quarantined() {
        for pattern in [
            r"^https?:\/\/[^(apple|10010)]+\.(com|cn)\/(a|A)d(s|v)?",
            r"^(https?://)?([a-z0-9-]+\.)+[a-z]{2,}/wp-json/.+",
        ] {
            assert_eq!(
                classify_functional_url(pattern),
                SafetyDecision::Quarantine("broad-cross-site-rewrite")
            );
        }
        assert_eq!(
            classify_functional_url(r"^https://ads\.example\.com/banner"),
            SafetyDecision::Keep
        );
    }

    #[test]
    fn apple_critical_host_matching_catches_broad_patterns() {
        assert!(apple_host_pattern_is_critical("account.apple.com"));
        assert!(apple_host_pattern_is_critical("*.icloud.com"));
        assert!(apple_host_pattern_is_critical("*.apple.com"));
        assert!(!apple_host_pattern_is_critical("weatherkit.apple.com"));
    }
}
