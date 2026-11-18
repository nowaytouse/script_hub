//! Parse MetaCubeX classical GeoSite entries without changing their matcher types.
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::pipeline) enum Kind {
    Domain,
    DomainSuffix,
    DomainKeyword,
    DomainRegex,
}

impl Kind {
    pub(in crate::pipeline) fn as_str(self) -> &'static str {
        match self {
            Self::Domain => "DOMAIN",
            Self::DomainSuffix => "DOMAIN-SUFFIX",
            Self::DomainKeyword => "DOMAIN-KEYWORD",
            Self::DomainRegex => "DOMAIN-REGEX",
        }
    }
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::pipeline) struct GeoRule<'a> {
    pub(in crate::pipeline) kind: Kind,
    pub(in crate::pipeline) value: &'a str,
}

pub(in crate::pipeline) fn parse_geosite(text: &str) -> Result<Vec<GeoRule<'_>>, String> {
    let mut rules = BTreeSet::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with(['#', ';']) || line.starts_with("//") {
            continue;
        }
        let number = index + 1;
        let (kind, value) = line
            .split_once(',')
            .ok_or_else(|| format!("GeoSite line {number} is missing its matcher type"))?;
        let kind = match kind {
            "DOMAIN" => Kind::Domain,
            "DOMAIN-SUFFIX" => Kind::DomainSuffix,
            "DOMAIN-KEYWORD" => Kind::DomainKeyword,
            "DOMAIN-REGEX" => Kind::DomainRegex,
            _ => return Err(format!("unsupported GeoSite matcher on line {number}")),
        };
        if value.is_empty()
            || value.trim() != value
            || value.chars().any(char::is_control)
            || kind != Kind::DomainRegex && value.contains(',')
        {
            return Err(format!("invalid GeoSite payload on line {number}"));
        }
        rules.insert(GeoRule { kind, value });
    }
    if rules.is_empty() {
        return Err("GeoSite source has no active rules".into());
    }
    Ok(rules.into_iter().collect())
}
