//! Read-only catalog models shared by the desktop views and their tests.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize)]
pub struct ModuleItem {
    pub name: String,
    pub desc: String,
    pub category: String,
    pub install_url: String,
    pub github_install_url: String,
    pub merged_into: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub filename: String,
}

impl ModuleItem {
    pub fn matches(&self, query: &str, category: &str) -> bool {
        (category.is_empty() || self.category == category)
            && [&self.name, &self.desc, &self.category, &self.author]
                .iter()
                .any(|value| value.to_lowercase().contains(query))
    }
}

#[derive(Clone, Debug)]
pub struct RuleItem {
    pub name: String,
    pub category: String,
    pub cdn: String,
    pub github: String,
    pub count: Option<u64>,
}

#[derive(Default)]
pub struct Catalog {
    pub modules: Vec<ModuleItem>,
    pub counterparts: Vec<ModuleItem>,
    pub rules: Vec<RuleItem>,
}

pub(super) fn safe_https_single_field_url(url: &str) -> bool {
    if url.chars().any(|character| {
        character.is_whitespace()
            || character.is_control()
            || matches!(character, ',' | '#' | '\'' | '"')
    }) {
        return false;
    }
    let Some((authority, path)) = url
        .strip_prefix("https://")
        .and_then(|remainder| remainder.split_once('/'))
    else {
        return false;
    };
    !authority.is_empty()
        && !authority.starts_with(':')
        && !authority.contains('@')
        && !path.is_empty()
}

pub fn selected_module_urls(
    modules: &[ModuleItem],
    selected: &BTreeSet<usize>,
    github: bool,
) -> String {
    selected
        .iter()
        .filter_map(|index| modules.get(*index))
        .filter(|module| module.merged_into.is_empty())
        .map(|module| {
            if github {
                module.github_install_url.as_str()
            } else {
                module.install_url.as_str()
            }
        })
        .filter(|url| safe_https_single_field_url(url))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn selected_rule_urls(rules: &[RuleItem], selected: &BTreeSet<usize>, github: bool) -> String {
    selected
        .iter()
        .filter_map(|index| rules.get(*index))
        .map(|rule| {
            if github {
                rule.github.as_str()
            } else {
                rule.cdn.as_str()
            }
        })
        .filter(|url| safe_https_single_field_url(url))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn valid_rule_set_policy(policy: &str) -> bool {
    !policy.is_empty()
        && policy.trim() == policy
        && !policy.chars().any(|character| {
            character.is_control()
                || (character.is_whitespace() && character != ' ')
                || matches!(character, ',' | '#' | ';' | '\'' | '"')
        })
        && !policy.contains("//")
}

pub fn selected_surge_rule_set_snippets(
    rules: &[RuleItem],
    selected: &BTreeSet<usize>,
    github: bool,
    policy: &str,
) -> Option<String> {
    if !valid_rule_set_policy(policy) {
        return None;
    }
    let snippets: Vec<_> = selected
        .iter()
        .filter_map(|index| rules.get(*index))
        .filter(|rule| {
            let url = if github { &rule.github } else { &rule.cdn };
            safe_https_single_field_url(url)
        })
        .map(|rule| {
            let url = if github { &rule.github } else { &rule.cdn };
            format!("RULE-SET,{url},{policy},no-resolve")
        })
        .collect();
    (!snippets.is_empty()).then(|| snippets.join("\n"))
}

impl Catalog {
    pub fn load(root: &Path, shadowrocket: bool) -> Result<Self, String> {
        let modules =
            serde_json::from_value(crate::module_catalog::catalog_snapshot(root, shadowrocket)?)
                .map_err(|error| format!("Invalid module catalog: {error}"))?;
        let counterparts = serde_json::from_value(crate::module_catalog::catalog_snapshot(
            root,
            !shadowrocket,
        )?)
        .map_err(|error| format!("Invalid counterpart catalog: {error}"))?;
        Ok(Self {
            modules,
            counterparts,
            rules: load_rules(root)?,
        })
    }

    pub fn counterpart_exists(&self, module: &ModuleItem) -> bool {
        let stem = |name: &str| {
            name.trim_end_matches(".sgmodule")
                .trim_end_matches(".module")
                .to_string()
        };
        self.counterparts
            .iter()
            .any(|other| stem(&other.filename) == stem(&module.filename))
    }
}

pub fn subscription_url(path: &str, github: bool) -> String {
    let path = path
        .split('/')
        .map(|part| {
            percent_encoding::utf8_percent_encode(part, percent_encoding::NON_ALPHANUMERIC)
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("/");
    let prefix = if github {
        "https://raw.githubusercontent.com/nowaytouse/script_hub/master/"
    } else {
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/"
    };
    format!("{prefix}{path}")
}

fn load_rules(root: &Path) -> Result<Vec<RuleItem>, String> {
    let path = root.join("rulesets").join("AdBlock").join("catalog.json");
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
    let catalog: serde_json::Value =
        serde_json::from_str(&content).map_err(|error| format!("Invalid rule catalog: {error}"))?;
    let shards = catalog["shards"]
        .as_array()
        .ok_or("Rule catalog has no shards array")?;
    let mut rules = Vec::new();
    for shard in shards {
        rules.push(RuleItem {
            name: shard["file"]
                .as_str()
                .or_else(|| shard["filename"].as_str())
                .ok_or("Shard has no filename")?
                .to_string(),
            category: shard["category"].as_str().unwrap_or("AdBlock").to_string(),
            cdn: shard["cdn_url"]
                .as_str()
                .ok_or("Shard has no CDN URL")?
                .to_string(),
            github: shard["github_url"]
                .as_str()
                .ok_or("Shard has no GitHub URL")?
                .to_string(),
            count: shard["rule_count"].as_u64(),
        });
    }
    let directory = root.join("rulesets").join("RULE-SET");
    for entry in std::fs::read_dir(&directory)
        .map_err(|error| format!("Cannot read {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| format!("Cannot read ruleset entry: {error}"))?;
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "list")
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = format!("rulesets/RULE-SET/{name}");
            rules.push(RuleItem {
                name,
                category: "Routing".into(),
                cdn: subscription_url(&relative, false),
                github: subscription_url(&relative, true),
                count: None,
            });
        }
    }
    rules.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(rules)
}

pub fn discover_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    let executable = std::env::current_exe().ok();
    for start in [
        Some(cwd.as_path()),
        executable.as_deref().and_then(Path::parent),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(root) = start.ancestors().find(|path| {
            path.join("rulesets/Sources/adblock_whitelist.list")
                .is_file()
        }) {
            return root.to_path_buf();
        }
    }
    cwd
}

pub fn select_root(explicit: Option<PathBuf>, saved: &str, discovered: PathBuf) -> PathBuf {
    explicit
        .or_else(|| (!saved.trim().is_empty()).then(|| PathBuf::from(saved)))
        .unwrap_or(discovered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_paths_encode_components_without_losing_separators() {
        assert!(
            subscription_url("rulesets/RULE-SET/Apple Media.list", true)
                .ends_with("rulesets/RULE%2DSET/Apple%20Media%2Elist")
        );
    }

    #[test]
    fn missing_catalog_is_an_error_instead_of_a_silent_empty_view() {
        assert!(Catalog::load(Path::new("nonexistent-script-hub-root"), false).is_err());
    }

    #[test]
    fn both_platforms_load_without_network_and_have_counterparts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        for platform in [false, true] {
            let catalog = Catalog::load(root, platform).unwrap();
            assert!(!catalog.modules.is_empty());
            assert!(catalog.modules.iter().all(|module| {
                safe_https_single_field_url(&module.install_url)
                    && safe_https_single_field_url(&module.github_install_url)
            }));
            assert!(catalog.rules.iter().any(|rule| rule.count.is_some()));
            assert!(
                catalog
                    .modules
                    .iter()
                    .any(|module| catalog.counterpart_exists(module))
            );
        }
    }

    #[test]
    fn explicit_workspace_overrides_saved_and_discovery() {
        assert_eq!(
            select_root(Some("explicit".into()), "saved", "discovered".into()),
            PathBuf::from("explicit")
        );
        assert_eq!(
            select_root(None, "saved", "discovered".into()),
            PathBuf::from("saved")
        );
        assert_eq!(
            select_root(None, "", "discovered".into()),
            PathBuf::from("discovered")
        );
    }

    #[test]
    fn batch_copy_uses_only_selected_urls_and_never_includes_retired_modules() {
        let modules = vec![
            ModuleItem {
                name: "one".into(),
                desc: String::new(),
                category: String::new(),
                install_url: "https://cdn.example.test/modules/one.sgmodule".into(),
                github_install_url: "https://raw.example.test/modules/one.sgmodule".into(),
                merged_into: String::new(),
                author: String::new(),
                date: String::new(),
                filename: String::new(),
            },
            ModuleItem {
                name: "retired".into(),
                desc: String::new(),
                category: String::new(),
                install_url: "https://cdn.example.test/modules/retired.sgmodule".into(),
                github_install_url: "https://raw.example.test/modules/retired.sgmodule".into(),
                merged_into: "PROMAX".into(),
                author: String::new(),
                date: String::new(),
                filename: String::new(),
            },
            ModuleItem {
                name: "injected".into(),
                desc: String::new(),
                category: String::new(),
                install_url: "https://cdn.example.test/rules.list,\nRULE-SET,evil,DIRECT".into(),
                github_install_url: "https://raw.example.test/rules.list#comment".into(),
                merged_into: String::new(),
                author: String::new(),
                date: String::new(),
                filename: String::new(),
            },
        ];
        let selected = BTreeSet::from([0, 1, 2, 99]);
        assert_eq!(
            selected_module_urls(&modules, &selected, false),
            "https://cdn.example.test/modules/one.sgmodule"
        );
        assert_eq!(
            selected_module_urls(&modules, &selected, true),
            "https://raw.example.test/modules/one.sgmodule"
        );
    }

    #[test]
    fn ruleset_copy_requires_an_explicit_single_field_policy() {
        let rules = vec![
            RuleItem {
                name: "ads.list".into(),
                category: "AdBlock".into(),
                cdn: "https://cdn.example.test/rules.list".into(),
                github: "https://raw.example.test/rules.list".into(),
                count: None,
            },
            RuleItem {
                name: "tracking.list".into(),
                category: "AdBlock".into(),
                cdn: "https://cdn.example.test/tracking.list".into(),
                github: "https://raw.example.test/tracking.list".into(),
                count: None,
            },
            RuleItem {
                name: "injected.list".into(),
                category: "AdBlock".into(),
                cdn: "https://cdn.example.test/rules.list,\nRULE-SET,evil,DIRECT".into(),
                github: "https://raw.example.test/rules.list#comment".into(),
                count: None,
            },
        ];
        let selected = BTreeSet::from([0]);
        for malformed in [
            "http://cdn.example.test/rules.list",
            "https:///rules.list",
            "https://cdn.example.test",
            "https://cdn.example.test/rules.list,\nRULE-SET,evil,DIRECT",
            "https://cdn.example.test/rules.list#comment",
        ] {
            assert!(!safe_https_single_field_url(malformed));
        }
        assert!(safe_https_single_field_url(
            "https://cdn.example.test/rules%2Clist"
        ));
        assert_eq!(
            selected_rule_urls(&rules, &selected, false),
            "https://cdn.example.test/rules.list"
        );
        assert_eq!(
            selected_rule_urls(&rules, &selected, true),
            "https://raw.example.test/rules.list"
        );
        assert_eq!(
            selected_surge_rule_set_snippets(&rules, &selected, false, "Reject Ads"),
            Some("RULE-SET,https://cdn.example.test/rules.list,Reject Ads,no-resolve".into())
        );
        assert_eq!(
            selected_surge_rule_set_snippets(&rules, &selected, false, "REJECT"),
            Some("RULE-SET,https://cdn.example.test/rules.list,REJECT,no-resolve".into())
        );
        assert_eq!(
            selected_surge_rule_set_snippets(&rules, &selected, true, "MyProxy"),
            Some("RULE-SET,https://raw.example.test/rules.list,MyProxy,no-resolve".into())
        );
        assert_eq!(
            selected_surge_rule_set_snippets(&rules, &selected, false, ""),
            None
        );
        for invalid in [
            "REJECT,PROXY",
            "REJECT\nRULE-SET,host,PROXY",
            "US\u{2028}Proxy",
            "US\tProxy",
            " REJECT",
            "REJECT ",
            "REJECT#comment",
            "REJECT;comment",
            "REJECT//comment",
            "REJECT'comment",
            "REJECT\"comment",
        ] {
            assert!(
                !valid_rule_set_policy(invalid),
                "accepted unsafe policy {invalid:?}"
            );
        }
        assert!(valid_rule_set_policy("US Proxy"));
        assert!(valid_rule_set_policy("🇯🇵 日本"));
        assert_eq!(
            selected_surge_rule_set_snippets(&rules, &selected, false, "🇯🇵 日本"),
            Some("RULE-SET,https://cdn.example.test/rules.list,🇯🇵 日本,no-resolve".into())
        );
        assert_eq!(
            selected_surge_rule_set_snippets(&rules, &BTreeSet::new(), false, "REJECT"),
            None
        );

        let selected = BTreeSet::from([2, 1, 0, 1, 99]);
        assert_eq!(
            selected_surge_rule_set_snippets(&rules, &selected, false, "  US Proxy  "),
            None
        );
        assert_eq!(
            selected_surge_rule_set_snippets(&rules, &selected, false, "US Proxy"),
            Some(
                "RULE-SET,https://cdn.example.test/rules.list,US Proxy,no-resolve\nRULE-SET,https://cdn.example.test/tracking.list,US Proxy,no-resolve"
                    .into()
            )
        );
        let invalid_url_only = BTreeSet::from([2]);
        assert!(selected_rule_urls(&rules, &invalid_url_only, false).is_empty());
        assert_eq!(
            selected_surge_rule_set_snippets(&rules, &invalid_url_only, false, "US Proxy"),
            None
        );
    }
}
