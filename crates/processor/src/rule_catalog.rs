use std::path::Path;

use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct RuleItem {
    pub name: String,
    pub category: String,
    pub cdn: String,
    pub github: String,
    pub count: Option<u64>,
}

pub fn subscription_url(path: &str, github: bool) -> String {
    let path = path
        .split('/')
        .map(|part| utf8_percent_encode(part, NON_ALPHANUMERIC).to_string())
        .collect::<Vec<_>>()
        .join("/");
    let prefix = if github {
        "https://raw.githubusercontent.com/nowaytouse/script_hub/master/"
    } else {
        "https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/"
    };
    format!("{prefix}{path}")
}

pub fn load_rules(root: &Path) -> Result<Vec<RuleItem>, String> {
    let path = root.join("rulesets/AdBlock/catalog.json");
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
    let directory = root.join("rulesets/RULE-SET");
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
