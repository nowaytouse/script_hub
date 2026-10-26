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

/// Authentication, certificate validation and Apple media transport endpoints.
/// Keep these outside interception without disabling WeatherKit or TV scripts.
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
    "certs.apple.com",
    "crl.apple.com",
    "ocsp.apple.com",
    "ocsp2.apple.com",
    "valid.apple.com",
    "audio-ssl.itunes.apple.com",
    "audio.itunes.apple.com",
    "aod-ssl.itunes.apple.com",
    "aod.itunes.apple.com",
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
    "music.apple.com",
];

pub fn apple_mitm_exclusions() -> Vec<String> {
    let mut exclusions: Vec<String> = APPLE_CRITICAL_EXACT
        .iter()
        .map(|host| format!("-{host}"))
        .collect();
    for suffix in APPLE_CRITICAL_SUFFIXES {
        exclusions.push(format!("-{suffix}"));
        exclusions.push(format!("-*.{suffix}"));
    }
    exclusions.sort();
    exclusions.dedup();
    exclusions
}

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
    reversed_hosts: Vec<String>,
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
pub(crate) struct IpNetwork {
    address: IpAddr,
    prefix: u8,
}

impl IpNetwork {
    pub(crate) fn parse(value: &str) -> Option<Self> {
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

    pub(crate) fn overlaps(self, other: Self) -> bool {
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

    pub(crate) fn covers(self, other: Self) -> bool {
        self.prefix <= other.prefix && self.overlaps(other)
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
        policy.reversed_hosts = policy
            .protected_domains
            .iter()
            .chain(&policy.protected_suffixes)
            .map(|host| host.split('.').rev().collect::<Vec<_>>().join("."))
            .collect();
        policy.reversed_hosts.sort();
        policy.reversed_hosts.dedup();
        (policy, stats)
    }

    pub fn decision(&self, rule: &SurgeRule) -> SafetyDecision {
        let payload = rule.payload.to_ascii_lowercase();
        if rule.kind == RuleKind::UrlRegex && is_bilibili_playurl_endpoint(&payload) {
            return SafetyDecision::Quarantine("broad-media-match");
        }
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
        } else if is_bilibili_playurl_endpoint(request)
            && (is_empty_map_local(line) || is_reject_action(line))
        {
            SafetyDecision::Quarantine("broad-media-match")
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
            .flat_map(|line| {
                let request = functional_request_pattern(line);
                if let Some(hostname) = literal_request_hostname(request) {
                    vec![
                        format!("observed:{}", hostname_reference_token(&hostname)),
                        format!("host:{hostname}"),
                    ]
                } else {
                    request_host_patterns(request)
                        .into_iter()
                        .flat_map(|hostname| {
                            let token = hostname_reference_token(&hostname);
                            [format!("observed:{token}"), format!("fallback:{token}")]
                        })
                        .collect()
                }
            })
            .filter(|pattern| {
                pattern
                    .rsplit(':')
                    .next()
                    .is_some_and(|token| token.len() >= 4)
            })
            .collect();
        patterns.sort();
        patterns.dedup();
        patterns
    }

    pub fn hostname_is_referenced_by_patterns(&self, hostname: &str, patterns: &[String]) -> bool {
        let hostname = hostname.trim();
        if hostname.starts_with('-') {
            return true;
        }
        let hostname = hostname.split_once(':').map_or(hostname, |(host, _)| host);
        if self.hostname_is_protected(hostname) {
            return false;
        }
        let token = hostname_reference_token(hostname);
        // Refinement may reduce the old scope, never activate a previously unreferenced wildcard.
        if patterns
            .binary_search(&format!("observed:{token}"))
            .is_err()
        {
            return false;
        }
        let normalized = hostname.to_ascii_lowercase();
        if normalized.is_empty() {
            return false;
        }
        let has_glob = normalized.contains(['*', '?']);
        let glob = has_glob.then(|| wildcard_regex(&normalized));
        if patterns
            .iter()
            .filter_map(|pattern| pattern.strip_prefix("host:"))
            .any(|host| {
                glob.as_ref()
                    .map_or_else(|| normalized == host, |glob| glob.is_match(host))
            })
        {
            return true;
        }
        if token.len() < 4 {
            return false;
        }
        patterns.binary_search(&format!("fallback:{token}")).is_ok()
    }

    pub fn refine_mitm_hosts(&self, hosts: &[String], patterns: &[String]) -> Vec<String> {
        let mut refined = Vec::new();
        for host in hosts {
            if host.starts_with('-') {
                refined.push(host.clone());
                continue;
            }
            let (pattern_host, port) =
                host.split_once(':')
                    .map_or((host.as_str(), None), |(value, port)| {
                        if !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) {
                            (value, Some(port))
                        } else {
                            (host.as_str(), None)
                        }
                    });
            let Some(suffix) = pattern_host.strip_prefix("*.") else {
                refined.push(host.clone());
                continue;
            };
            let token = hostname_reference_token(suffix);
            if patterns.binary_search(&format!("fallback:{token}")).is_ok() {
                refined.push(host.clone());
                continue;
            }
            let glob = wildcard_regex(pattern_host);
            let mut matches: Vec<String> = patterns
                .iter()
                .filter_map(|pattern| pattern.strip_prefix("host:"))
                .filter(|hostname| glob.is_match(hostname))
                .map(|hostname| match port {
                    Some(port) => format!("{hostname}:{port}"),
                    None => hostname.to_string(),
                })
                .collect();
            matches.sort();
            matches.dedup();
            if matches.is_empty() {
                refined.push(host.clone());
            } else {
                refined.extend(matches);
            }
        }
        refined.sort();
        refined.dedup();
        refined
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
        self.hierarchy_intersects(candidate)
    }

    fn domain_is_protected(&self, domain: &str) -> bool {
        self.protected_domains.contains(domain)
            || self.has_ancestor(domain, &self.protected_suffixes)
            || self
                .protected_keywords
                .iter()
                .any(|keyword| domain.contains(keyword))
    }

    fn suffix_intersects(&self, blocked: &str) -> bool {
        self.hierarchy_intersects(blocked)
            || self
                .protected_keywords
                .iter()
                .any(|keyword| blocked.contains(keyword))
    }

    fn hierarchy_intersects(&self, host: &str) -> bool {
        self.has_ancestor(host, &self.protected_domains)
            || self.has_ancestor(host, &self.protected_suffixes)
            || self.has_descendant(host)
    }

    fn has_ancestor(&self, host: &str, protected: &HashSet<String>) -> bool {
        let mut candidate = host;
        loop {
            if protected.contains(candidate) {
                return true;
            }
            let Some((_, parent)) = candidate.split_once('.') else {
                return false;
            };
            candidate = parent;
        }
    }

    fn has_descendant(&self, host: &str) -> bool {
        let prefix = format!("{}.", host.split('.').rev().collect::<Vec<_>>().join("."));
        let index = self
            .reversed_hosts
            .partition_point(|candidate| candidate < &prefix);
        self.reversed_hosts
            .get(index)
            .is_some_and(|candidate| candidate.starts_with(&prefix))
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

pub(crate) fn functional_request_pattern(line: &str) -> &str {
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

fn literal_request_hostname(request: &str) -> Option<String> {
    let normalized = request.replace(r"\/", "/");
    let rest = normalized
        .strip_prefix("^https://")
        .or_else(|| normalized.strip_prefix("^https?://"))?;
    let authority = rest.split_once('/')?.0;
    if authority.is_empty()
        || authority.contains(':')
        || authority.contains(['*', '?', '+', '|', '[', ']', '(', ')', '{', '}', '^', '$'])
    {
        return None;
    }
    let hostname = authority.replace(r"\.", ".");
    if hostname.contains('.')
        && hostname.split('.').all(|label| {
            !label.is_empty()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
        && authority.matches(r"\.").count() == hostname.matches('.').count()
    {
        Some(hostname.to_ascii_lowercase())
    } else {
        None
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
    // Keep source admission unchanged; runtime refinements must not enable extra handlers.
    let normalized = request
        .replace(r"\.", ".")
        .replace(r"\/", "/")
        .to_ascii_lowercase();
    let authority = normalized
        .find("://")
        .map_or(normalized.as_str(), |scheme| {
            normalized[scheme + 3..]
                .split(['/', '?', '#'])
                .next()
                .unwrap_or("")
        });
    HOST_CANDIDATE
        .find_iter(authority)
        .map(|candidate| candidate.as_str().trim_matches('.').to_string())
        .collect()
}

pub(crate) fn regex_authority_end(pattern: &str) -> usize {
    let mut in_class = false;
    let mut escaped = false;
    for (index, character) in pattern.char_indices() {
        if escaped {
            escaped = false;
            if !in_class && (character == '/' || character == '?') {
                return index.saturating_sub(1);
            }
            continue;
        }
        match character {
            '\\' => escaped = true,
            '[' if !in_class => in_class = true,
            ']' if in_class => in_class = false,
            '/' | '#' if !in_class => return index,
            _ => {}
        }
    }
    pattern.len()
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
        .replace(['*', '?'], "");
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

fn is_bilibili_playurl_endpoint(pattern: &str) -> bool {
    let normalized = pattern
        .replace(r"\\/", "/")
        .replace(r"\/", "/")
        .replace(r"\\.", ".")
        .replace(r"\.", ".");
    let normalized = normalized.to_ascii_lowercase();
    let Some((_, authority_and_path)) = normalized.split_once("://") else {
        return false;
    };
    let Some((authority, path)) = authority_and_path.split_once('/') else {
        return false;
    };
    let bili_api_host = matches!(authority, "api.bilibili.com" | "api.biliapi.net")
        || authority == "api.bili(bili.com|api.net)";
    if !bili_api_host {
        return false;
    }
    [
        "pgc/player/api/playurl",
        "pgc/player/web/playurl",
        "pgc/player/(api|web)/playurl",
        "x/player/playurl",
    ]
    .iter()
    .any(|endpoint| {
        path.strip_prefix(endpoint).is_some_and(|tail| {
            tail.is_empty()
                || tail.starts_with(r"\?")
                || tail.starts_with(r"\\?")
                || tail.starts_with('?')
                || tail.starts_with('$')
        })
    })
}

fn is_empty_map_local(line: &str) -> bool {
    let normalized = line.to_ascii_lowercase();
    normalized.contains("data=\"\"") || normalized.contains("data=''")
}

fn is_reject_action(line: &str) -> bool {
    let action = line
        .split_whitespace()
        .last()
        .unwrap_or("")
        .to_ascii_lowercase();
    action == "reject" || action.starts_with("reject-")
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
    fn literal_request_hosts_do_not_cross_retain_sibling_or_public_suffix_hosts() {
        let safety = SafetyPolicy::default();
        let functions = vec![
            r#"x = type=http-response,pattern=^https://api\.example\.com/feed,script-path=https://scripts.example/x.js"#.to_string(),
            r#"^https://api\.service\.com\.cn/path _ reject"#.to_string(),
        ];

        assert!(safety.hostname_is_referenced("api.example.com", &functions));
        assert!(!safety.hostname_is_referenced("telemetry.example.com", &functions));
        assert!(safety.hostname_is_referenced("api.service.com.cn", &functions));
        assert!(!safety.hostname_is_referenced("other.service.com.cn", &functions));
        assert!(!safety.hostname_is_referenced("scripts.example", &functions));
    }

    #[test]
    fn wildcard_mitm_hosts_refine_only_for_proven_literal_requests() {
        let safety = SafetyPolicy::default();
        let wildcard = vec!["*.example.com".to_string()];
        let literal = vec![r#"^https://api\.example\.com/feed _ reject"#.to_string()];
        let complex = vec![r#"^https://api[0-9]+\.example\.com/feed _ reject"#.to_string()];

        assert_eq!(
            safety.refine_mitm_hosts(&wildcard, &SafetyPolicy::functional_host_patterns(&literal)),
            vec!["api.example.com"]
        );
        assert_eq!(
            safety.refine_mitm_hosts(&wildcard, &SafetyPolicy::functional_host_patterns(&complex)),
            wildcard
        );
        assert!(safety.hostname_is_referenced("api.example.com", &literal));
    }

    #[test]
    fn literal_escaped_slashes_still_filter_sibling_hosts() {
        let safety = SafetyPolicy::default();
        let functions = vec![r#"^https?:\/\/api\.example\.test/path _ reject"#.to_string()];

        assert!(safety.hostname_is_referenced("api.example.test", &functions));
        assert!(!safety.hostname_is_referenced("telemetry.example.test", &functions));
    }

    #[test]
    fn narrowed_authority_character_class_preserves_host_fallback_and_safety() {
        let broad = vec![r#"^https?://.*\.example\.test/ads _ reject"#.to_string()];
        let narrowed =
            vec![r#"^https?://[^\x2f\x3f\x23]*\.example\.test/ads _ reject"#.to_string()];

        assert_eq!(
            SafetyPolicy::functional_host_patterns(&broad),
            SafetyPolicy::functional_host_patterns(&narrowed)
        );
        assert_eq!(
            SafetyPolicy::default().functional_decision(&narrowed[0]),
            SafetyDecision::Keep
        );
    }

    #[test]
    fn runtime_refinement_does_not_activate_previously_unclassified_sources() {
        let source = r"^https?://[^/]+\.example\.test/ads _ reject";
        assert!(matches!(
            SafetyPolicy::default().functional_decision(source),
            SafetyDecision::Quarantine(_)
        ));
    }

    #[test]
    fn precise_references_do_not_activate_a_new_broad_mitm_glob() {
        let patterns = SafetyPolicy::functional_host_patterns(&[
            r"^https://api\.ximalaya\.com/ads _ reject".to_string(),
        ]);
        assert!(
            !SafetyPolicy::default().hostname_is_referenced_by_patterns("*.xima*.*", &patterns)
        );
        assert!(
            SafetyPolicy::default().hostname_is_referenced_by_patterns("*.ximalaya.com", &patterns)
        );
    }

    #[test]
    fn wildcard_reference_matching_handles_internal_stars_and_question_marks() {
        let safety = SafetyPolicy::default();
        let patterns = SafetyPolicy::functional_host_patterns(&[
            r"^https://api\.example\.test/ads _ reject".to_string(),
            r"^https://api1\.example\.test/ads _ reject".to_string(),
        ]);

        assert!(safety.hostname_is_referenced_by_patterns("api*.example.test", &patterns));
        assert!(safety.hostname_is_referenced_by_patterns("api?.example.test", &patterns));
    }

    #[test]
    fn mitm_ports_and_exclusions_are_preserved_for_reference_filtering() {
        let safety = SafetyPolicy::from_lines(["DOMAIN-SUFFIX,protected.test"]);
        let patterns = SafetyPolicy::functional_host_patterns(&[
            r"^https://api\.example\.test/ads _ reject".to_string(),
        ]);

        assert!(safety.hostname_is_referenced_by_patterns("*.example.test:443", &patterns));
        assert_eq!(
            safety.refine_mitm_hosts(&["*.example.test:443".to_string()], &patterns),
            vec!["api.example.test:443"]
        );
        assert!(safety.hostname_is_referenced_by_patterns("-protected.test", &[]));
        assert_eq!(
            safety.refine_mitm_hosts(&["-protected.test".to_string()], &[]),
            vec!["-protected.test"]
        );
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
        assert_eq!(
            classify_functional_url(r"^https?://.*/yyting/advertclient/ClientAdvertList.action"),
            SafetyDecision::Quarantine("broad-cross-site-rewrite")
        );
        assert_eq!(
            classify_functional_url(
                r"^https?://dapis\.mting\.info/yyting/advertclient/ClientAdvertList.action"
            ),
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

    #[test]
    fn whole_bilibili_playurl_reject_is_quarantined() {
        let rule = SurgeRule::parse(
            r"URL-REGEX,^https?://api\.bilibili\.com/pgc/player/(api|web)/playurl\?,REJECT",
        )
        .unwrap();

        assert_eq!(
            SafetyPolicy::default().decision(&rule),
            SafetyDecision::Quarantine("broad-media-match")
        );
    }

    #[test]
    fn empty_maplocal_for_bilibili_playurl_is_quarantined() {
        let map_local = r#"^https?://api\.bili(bili\.com|api\.net)/pgc/player/(api|web)/playurl\? data-type=text data="" status-code=200"#;

        assert_eq!(
            SafetyPolicy::default().functional_decision(map_local),
            SafetyDecision::Quarantine("broad-media-match")
        );
    }

    #[test]
    fn whole_bilibili_playurl_rewrite_reject_is_quarantined() {
        let rewrite = r#"^https?://api\.bilibili\.com/pgc/player/api/playurl\? - ReJeCt-DROP"#;

        assert_eq!(
            SafetyPolicy::default().functional_decision(rewrite),
            SafetyDecision::Quarantine("broad-media-match")
        );
    }

    #[test]
    fn non_reject_bilibili_playurl_redirect_is_kept() {
        let redirect = r#"^https?://api\.bilibili\.com/pgc/player/api/playurl\? https://api.bilibili.com/pgc/player/api/playurl 302"#;

        assert_eq!(
            SafetyPolicy::default().functional_decision(redirect),
            SafetyDecision::Keep
        );
    }

    #[test]
    fn non_destructive_bilibili_playurl_response_script_is_kept() {
        let script = r#"BiliBili.PlayURL.response = type=http-response, pattern=^https?://api\.bilibili\.com/pgc/player/api/playurl\?, script-path=https://example.test/response.js"#;

        assert_eq!(
            SafetyPolicy::default().functional_decision(script),
            SafetyDecision::Keep
        );
    }

    #[test]
    fn official_bilibili_adblock_playurl_response_script_is_kept() {
        let script = r#"📺 BiliBili.ADBlock.playurl.response.grpc = type=http-response, pattern=^https?:\\/\\/(grpc|app)\\.bili(bili\\.com|api\\.net)\\/bilibili\\.app\\.playurl\\.v1\\.PlayURL\\/PlayView$, requires-body=1, binary-body-mode=1, engine=webview, script-path=https://github.com/Biliverse/ADBlock/releases/download/v0.7.5/response.bundle.js"#;

        assert_eq!(
            SafetyPolicy::default().functional_decision(script),
            SafetyDecision::Keep
        );
    }
}
