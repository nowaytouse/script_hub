use crate::promax::rule::{RuleKind, SurgeRule};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;

static MEDIA_MARKER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(png|jpe?g|gif|webp|avif|heic|mp4|m3u8|images?|media|videos?|avatars?|thumbnails?|thumb|[0-9]{2,4}x[0-9]{2,4})",
    )
    .unwrap()
});

static AD_INTENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(^|[./_?&=\\-])(ads?|adverts?|advertis(?:e|ing|ement)?|splash(?:_?ad)?|promotion|promote|commercial|launchad|feed_ad)([./_?&=$\\-]|$)",
    )
    .unwrap()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyDecision {
    Keep,
    DropDuplicate,
    Quarantine(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RiskClass {
    Invalid,
    ProtectedService,
    BroadMedia,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuarantineRecord {
    pub source: String,
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
    protected_ips: HashSet<String>,
}

impl SafetyPolicy {
    pub fn from_lines<I, S>(lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut policy = Self::default();
        for line in lines {
            let raw = line.as_ref().trim();
            if raw.is_empty() || raw.starts_with('#') {
                continue;
            }
            let Ok(rule) = SurgeRule::parse(raw) else {
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
                    policy.protected_ips.insert(payload);
                }
                _ => {}
            }
        }
        policy
    }

    pub fn decision(&self, rule: &SurgeRule) -> SafetyDecision {
        let payload = rule.payload.to_ascii_lowercase();
        let intersects = match rule.kind {
            RuleKind::Domain => self.domain_is_protected(&payload),
            RuleKind::DomainSuffix => self.suffix_intersects(&payload),
            RuleKind::DomainKeyword => self.keyword_intersects(&payload),
            RuleKind::DomainWildcard => self.pattern_intersects(&wildcard_regex(&payload)),
            RuleKind::DomainRegex => Regex::new(&payload)
                .ok()
                .is_some_and(|pattern| self.pattern_intersects(&pattern)),
            RuleKind::UrlRegex => Regex::new(&payload).ok().is_some_and(|pattern| {
                self.protected_hosts().any(|host| {
                    pattern.is_match(&format!("https://{host}/"))
                        || pattern.is_match(&format!("http://{host}/"))
                })
            }),
            RuleKind::IpCidr | RuleKind::IpCidr6 => self.protected_ips.contains(&payload),
            _ => false,
        };

        if intersects {
            SafetyDecision::Quarantine("protected-domain-intersection")
        } else {
            SafetyDecision::Keep
        }
    }

    fn protected_hosts(&self) -> impl Iterator<Item = &str> {
        self.protected_domains
            .iter()
            .chain(self.protected_suffixes.iter())
            .map(String::as_str)
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
        self.protected_hosts()
            .any(|host| pattern.is_match(host) || pattern.is_match(&format!("probe.{host}")))
    }
}

pub fn classify_functional_url(line: &str) -> SafetyDecision {
    if MEDIA_MARKER.is_match(line) && !AD_INTENT.is_match(line) {
        SafetyDecision::Quarantine("broad-media-match")
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
    use super::{SafetyDecision, SafetyPolicy, classify_functional_url};
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
}
