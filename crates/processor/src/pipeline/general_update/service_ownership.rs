use super::{RuleBoundary, apple_ownership_category_paths, read_optional_rules, read_rules};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Exact global Microsoft account and Entra sign-in endpoints, ordered before shared suffixes.
pub const MICROSOFT_SIGN_IN_HOSTS: &[&str] = &[
    "account.live.com",
    "login.live.com",
    "login.microsoft.com",
    "login.microsoftonline.com",
    "login.windows.net",
];

const MICROSOFT_MANIFEST: &str = "rulesets/Sources/Links/Microsoft_proxy_sources.list";
const GITHUB_OWNERS: &str = "rulesets/Sources/custom/GitHub_manual.list";
const MICROSOFT_OWNERS: &str = "rulesets/Sources/custom/Microsoft_manual.list";

pub(super) struct Change {
    pub(super) name: String,
    pub(super) path: PathBuf,
    pub(super) rules: BTreeSet<String>,
}

fn sign_in_rule(rule: &str) -> bool {
    let mut fields = rule.split(',');
    let kind = fields.next().unwrap_or_default();
    let host = fields.next().unwrap_or_default();
    (kind.eq_ignore_ascii_case("DOMAIN") || kind.eq_ignore_ascii_case("DOMAIN-SUFFIX"))
        && MICROSOFT_SIGN_IN_HOSTS
            .iter()
            .any(|name| host.eq_ignore_ascii_case(name))
}

fn reviewed_domains(root: &Path, source: &str, required: &str) -> Result<BTreeSet<String>, String> {
    let rules = read_rules(&root.join(source))?;
    if !rules.contains(required)
        || rules.iter().any(|rule| {
            !(rule.starts_with("DOMAIN,") || rule.starts_with("DOMAIN-SUFFIX,"))
                || rule.split(',').count() != 2
                || crate::normalize_rule(rule).as_ref() != Some(rule)
        })
    {
        return Err(format!("invalid curated ownership rules in {source}"));
    }
    Ok(rules)
}

pub(super) fn plan(root: &Path) -> Result<Vec<Change>, String> {
    if !root
        .join(MICROSOFT_MANIFEST)
        .try_exists()
        .map_err(|e| e.to_string())?
    {
        return Ok(Vec::new());
    }
    let github = reviewed_domains(root, GITHUB_OWNERS, "DOMAIN-SUFFIX,github.com")?;
    let microsoft = reviewed_domains(root, MICROSOFT_OWNERS, "DOMAIN-SUFFIX,cloud.microsoft")?;

    let directory = root.join("rulesets/RULE-SET");
    let github_boundary = RuleBoundary::from_rules(&github);
    let microsoft_boundary = RuleBoundary::from_rules(&microsoft);
    if microsoft
        .iter()
        .any(|rule| github_boundary.intersects(rule))
    {
        return Err("curated GitHub and Microsoft service scopes overlap".to_string());
    }
    let direct_sign_in: BTreeSet<_> = MICROSOFT_SIGN_IN_HOSTS
        .iter()
        .map(|host| format!("DOMAIN,{host}"))
        .collect();
    let is_github = |rule: &str| {
        github_boundary.covers(rule)
            || rule
                .strip_prefix("DOMAIN-SUFFIX,")
                .is_some_and(|host| github_boundary.domains.contains(&host.to_ascii_lowercase()))
            || rule.eq_ignore_ascii_case("DOMAIN-KEYWORD,github")
    };
    let proxy_path = directory.join("Microsoft_proxy.list");
    let mut proxy = read_optional_rules(&proxy_path)?;
    proxy.retain(|rule| !is_github(rule) && !sign_in_rule(rule));
    proxy.extend(github);
    proxy.extend(microsoft);

    let mut changes = Vec::new();
    for (name, rules) in [
        ("Microsoft_direct", direct_sign_in),
        ("Microsoft_proxy", proxy),
    ] {
        let path = directory.join(format!("{name}.list"));
        if read_optional_rules(&path)? != rules {
            changes.push(Change {
                name: name.into(),
                path,
                rules,
            });
        }
    }

    let categories = apple_ownership_category_paths(&directory)?;
    for required in ["Direct", "GlobalProxy"] {
        if !categories.iter().any(|(name, _)| name == required) {
            return Err(format!("missing {required}.list for service ownership"));
        }
    }
    for (name, path) in categories {
        if matches!(
            name.as_str(),
            "GitHub"
                | "GitHub_ip"
                | "GitHub_extra"
                | "Microsoft"
                | "Microsoft_ip"
                | "Microsoft_extra"
        ) {
            changes.push(Change {
                name,
                path,
                rules: BTreeSet::new(),
            });
            continue;
        }
        if matches!(
            name.as_str(),
            "Microsoft_direct" | "Microsoft_proxy" | "Profile_Exceptions"
        ) || name.ends_with("_ip")
            || name.ends_with("_extra")
        {
            continue;
        }
        let mut rules = read_optional_rules(&path)?;
        let initial_count = rules.len();
        // Keep shared parent suffixes. Templates match the exact sign-in hosts first.
        rules.retain(|rule| {
            !is_github(rule) && !microsoft_boundary.covers(rule) && !sign_in_rule(rule)
        });
        if initial_count != rules.len() {
            changes.push(Change { name, path, rules });
        }
    }
    Ok(changes)
}
