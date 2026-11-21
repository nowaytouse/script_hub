use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};
#[path = "../src/module_header.rs"]
mod module_header;
#[path = "../src/rule_catalog.rs"]
mod rule_catalog;

pub fn rules(root: &Path) -> std::io::Result<Vec<u8>> {
    let mut rules = rule_catalog::load_rules(root).map_err(std::io::Error::other)?;
    rules.retain(|rule| {
        let (directory, name) = match rule.name.strip_prefix("rulesets/AdBlock/") {
            Some(name) => ("rulesets/AdBlock", name),
            None => ("rulesets/RULE-SET", rule.name.as_str()),
        };
        if name.contains(['/', '\\']) || !name.ends_with(".list") {
            return false;
        }
        let relative = format!("{directory}/{name}");
        let mirror = root.join(directory).join("github").join(name);
        // The publication mirrors distinguish available rules from local-only lists.
        fs::symlink_metadata(mirror).is_ok_and(|metadata| metadata.is_file())
            && rule.cdn.replace("%2F", "/") == rule_catalog::subscription_url(&relative, false)
            && rule.github.replace("%2F", "/") == rule_catalog::subscription_url(&relative, true)
    });
    serde_json::to_vec(&rules).map_err(std::io::Error::other)
}

pub fn modules(root: &Path, platform: &str, extension: &str) -> std::io::Result<Value> {
    let mut modules = Vec::new();
    for category in ["amplify_nexus", "head_expanse", "narrow_pierce"] {
        let directory = root.join("modules").join(platform).join(category);
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !directory.exists() => {
                continue;
            }
            Err(error) => return Err(error),
        };
        let mut paths = entries
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        for path in paths {
            if path.extension().is_none_or(|e| e != extension) {
                continue;
            }
            if !fs::symlink_metadata(&path)?.is_file() {
                return Err(std::io::Error::other("Nonregular public module"));
            }
            let content = module_header::read(&path)?;
            let cdn_available = fs::metadata(&path)?.len() <= module_header::MAX_CDN_FILE_BYTES;
            let mut metadata = BTreeMap::new();
            for line in content
                .lines()
                .map(str::trim)
                .take_while(|line| !line.starts_with('['))
            {
                let Some(line) = line.strip_prefix("#!") else {
                    continue;
                };
                let Some((key, value)) = line.split_once(['=', ':']) else {
                    continue;
                };
                let key = key.trim();
                if !key.is_empty()
                    && key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                {
                    metadata.insert(key.to_ascii_lowercase(), value.trim().to_owned());
                }
            }
            let filename = path.file_name().unwrap().to_string_lossy();
            let name = metadata
                .get("name")
                .cloned()
                .unwrap_or_else(|| path.file_stem().unwrap().to_string_lossy().into_owned());
            let retired = name.contains("已退役")
                || metadata.get("tag").is_some_and(|v| {
                    v.split(',')
                        .any(|t| t.trim().eq_ignore_ascii_case("deprecated"))
                });
            let relative = format!("modules/{platform}/{category}/{filename}");
            let mirror = path
                .parent()
                .unwrap()
                .join("github")
                .join(filename.as_ref());
            let raw = if mirror.is_file() {
                format!("modules/{platform}/{category}/github/{filename}")
            } else {
                relative.clone()
            };
            let encode = |p: &str| {
                p.split('/')
                    .map(|s| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string())
                    .collect::<Vec<_>>()
                    .join("/")
            };
            modules.push(json!({"name":name,"desc":metadata.get("desc").cloned().unwrap_or_default(),"category":category,"author":metadata.get("author").cloned().unwrap_or_default(),"date":metadata.get("date").cloned().unwrap_or_default(),"filename":filename,"merged_into":if retired {"PROMAX"}else{""},"install_url":if cdn_available {format!("https://cdn.jsdelivr.net/gh/nowaytouse/script_hub@master/{}",encode(&relative))}else{String::new()},"github_install_url":format!("https://raw.githubusercontent.com/nowaytouse/script_hub/master/{}",encode(&raw))}));
        }
    }
    Ok(json!(modules))
}
