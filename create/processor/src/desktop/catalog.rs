//! Read-only catalog models shared by the desktop views and their tests.

use serde::Deserialize;
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
}
